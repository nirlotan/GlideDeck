"""Synthesizes a fallback music bed with numpy: an upbeat 118 BPM groove in C,
arranged to the video (sparse opening, a lift at the turn, the full groove
from the reveal, one last hit under the wordmark).

Used only when ElevenLabs music generation is not available:
    python scripts/beat.py <seconds> [lift] [full] [hit]
writes public/audio/music.mp3. The three optional times (seconds) say where
the lift, the full groove and the final hit fall.
"""
import subprocess
import sys
import wave
from pathlib import Path

import numpy as np

SR = 44100
BPM = 118.0
BEAT = 60.0 / BPM
BAR = 4 * BEAT
total = float(sys.argv[1]) if len(sys.argv) > 1 else 70.0
lift = float(sys.argv[2]) if len(sys.argv) > 2 else 11.5
full = float(sys.argv[3]) if len(sys.argv) > 3 else 19.0
hit = float(sys.argv[4]) if len(sys.argv) > 4 else total - 5.0

n = int(SR * (total + 2.0))
left = np.zeros(n)
right = np.zeros(n)
duck = np.ones(n)  # side-chain: everything but the drums dips on each kick
rng = np.random.default_rng(11)


def add(buf_l, buf_r, at, sig, gain=1.0, pan=0.5):
    i = int(at * SR)
    if i >= n or i < 0:
        return
    sig = sig[: n - i]
    buf_l[i : i + len(sig)] += sig * gain * np.sqrt(1 - pan) * 1.414
    buf_r[i : i + len(sig)] += sig * gain * np.sqrt(pan) * 1.414


def lowpass(x, cutoff):
    """One-pole low-pass; `cutoff` in Hz, a number or one value per sample."""
    a = 1 - np.exp(-2 * np.pi * np.broadcast_to(cutoff, x.shape) / SR)
    y = np.empty_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc += a[i] * (x[i] - acc)
        y[i] = acc
    return y


def hz(note):
    return 440.0 * 2 ** ((note - 69) / 12)


def kick():
    t = np.arange(int(SR * 0.32)) / SR
    freq = 46 + 110 * np.exp(-t * 34)
    return np.sin(2 * np.pi * np.cumsum(freq) / SR) * np.exp(-t * 9) + 0.25 * rng.standard_normal(len(t)) * np.exp(-t * 300)


def clap():
    t = np.arange(int(SR * 0.22)) / SR
    noise = rng.standard_normal(len(t))
    noise = noise - lowpass(noise, 900.0)
    env = sum(np.exp(-np.maximum(t - d, 0) * 55) * (t >= d) for d in (0.0, 0.011, 0.023)) / 3
    return noise * env


def hat(length=0.045):
    t = np.arange(int(SR * length)) / SR
    noise = rng.standard_normal(len(t))
    return (noise - lowpass(noise, 6500.0)) * np.exp(-t / (length * 0.3))


def saw(freq, t):
    phase = (freq * t) % 1.0
    return 2 * phase - 1


def bass(note, length):
    t = np.arange(int(SR * length)) / SR
    f = hz(note)
    x = 0.6 * saw(f, t) + 0.4 * np.sign(np.sin(2 * np.pi * f * t)) + 0.5 * np.sin(2 * np.pi * f * t)
    x = lowpass(x, 180 + 900 * np.exp(-t * 16))
    return x * np.minimum(1, t * 400) * np.exp(-t * 5) * np.minimum(1, (length - t) * 200)


def pluck(note, length=0.22, bright=1.0):
    t = np.arange(int(SR * length)) / SR
    f = hz(note)
    x = saw(f, t) + 0.6 * saw(f * 1.005, t) + 0.3 * saw(f * 2, t)
    x = lowpass(x, 500 + 5200 * bright * np.exp(-t * 26))
    return x * np.minimum(1, t * 800) * np.exp(-t * 15)


def pad(notes, length):
    t = np.arange(int(SR * length)) / SR
    x = sum(saw(hz(k), t) + saw(hz(k) * 1.004, t) for k in notes) / len(notes)
    x = lowpass(x, 1400.0)
    return x * np.minimum(1, t * 6) * np.minimum(1, (length - t) * 3)


def riser(length):
    t = np.arange(int(SR * length)) / SR
    noise = rng.standard_normal(len(t))
    x = lowpass(noise, 300 + 7000 * (t / length) ** 2.4)
    return x * (t / length) ** 2


# C, A minor, F, G: one bar each. (root for the bass, chord tones for the plucks)
CHORDS = [(36, [60, 64, 67, 72]), (33, [57, 60, 64, 69]), (29, [53, 57, 60, 65]), (31, [55, 59, 62, 67])]
ARP = [0, 2, 1, 3, 2, 1, 3, 2]

K, C, H, HO = kick(), clap(), hat(), hat(0.16)
bars = int(np.ceil(hit / BAR)) + 1
for b in range(bars):
    t0 = b * BAR
    if t0 >= hit:
        break
    root, tones = CHORDS[b % 4]
    opening = t0 + BAR <= lift + 0.05
    building = not opening and t0 + BAR <= full + 0.05
    groove = not opening and not building
    # A two-bar breather at the half-way mark keeps it from droning.
    breather = groove and (b - int(full / BAR)) % 16 in (14,)
    for step in range(16):
        t = t0 + step * BEAT / 4
        if t >= hit:
            break
        on_beat = step % 4 == 0
        # Drums.
        if (building or groove) and on_beat and not (breather and step >= 8):
            add(left, right, t, K, 0.95)
            i = int(t * SR)
            env = 1 - 0.55 * np.exp(-np.arange(int(SR * 0.3)) / (SR * 0.085))
            duck[i : i + len(env)] = np.minimum(duck[i : i + len(env)], env[: n - i])
        if groove and step in (4, 12):
            add(left, right, t, C, 0.5)
        if groove and step % 2 == 0:
            add(left, right, t, HO if step % 8 == 6 else H, 0.16 if step % 4 else 0.1, 0.62)
        if groove and step % 2 == 1 and (b % 2 == 1):
            add(left, right, t, H, 0.07, 0.38)
        if opening and step in (4, 12):
            add(left, right, t, H, 0.1, 0.6)
        if building and step % 2 == 0:
            add(left, right, t, H, 0.06 + 0.1 * (t - lift) / max(0.1, full - lift), 0.6)
        # Bass: root on the beat, octave on the off-beats.
        if (building or groove) and step % 2 == 0 and not (breather and step >= 8):
            note = root + (12 if step % 4 == 2 else 0)
            add(left, right, t, bass(note, BEAT / 2 * 0.92), 0.5)
        # Plucks: muted in the opening, an arpeggio once things move.
        if opening and step in (0, 3, 6, 10, 13):
            add(left, right, t, pluck(tones[ARP[step % 8] % 4], bright=0.25), 0.2, 0.35 + 0.3 * (step % 2))
        if (building or groove) and step % 2 == 0 or (groove and step in (3, 11)):
            bright = 0.45 if building else 0.9
            add(left, right, t, pluck(tones[ARP[(step // 2 + b) % 8] % 4] + (12 if groove and step % 8 == 6 else 0), bright=bright), 0.17, 0.3 + 0.4 * ((step // 2) % 2))
    if groove:
        add(left, right, t0, pad(tones[:3], min(BAR, max(0.2, hit - t0))), 0.1)

# Into the full groove, and into the last hit.
for target, length in ((full, min(3.2, full - lift)), (hit, 2.0)):
    add(left, right, target - length, riser(length), 0.22)

# The last hit: a kick, a clap and a C chord that rings out.
add(left, right, hit, K, 1.0)
add(left, right, hit, C, 0.45)
tail = max(1.0, total - hit)
t = np.arange(int(SR * tail)) / SR
chord = sum(saw(hz(k), t) + saw(hz(k) * 1.003, t) for k in (48, 60, 64, 67, 72)) / 5
chord = lowpass(chord, 600 + 3500 * np.exp(-t * 2.2)) * np.exp(-t * 1.5)
add(left, right, hit, chord, 0.42)
add(left, right, hit, bass(36, min(1.5, tail)), 0.6)

mix = np.stack([left * duck, right * duck], axis=1)[: int(SR * total)]
# A touch of width and room: a quiet, slightly delayed copy on the other side.
d = int(SR * 0.021)
mix[d:, 0] += 0.18 * mix[:-d, 1]
mix[d:, 1] += 0.18 * mix[:-d, 0]
mix = np.tanh(mix * 1.5 / np.abs(mix).max() * 1.2)
fade = int(SR * 1.2)
mix[-fade:] *= np.linspace(1, 0, fade)[:, None] ** 2
mix = mix / np.abs(mix).max() * 0.9

out = Path(__file__).resolve().parent.parent / "public" / "audio"
out.mkdir(parents=True, exist_ok=True)
path = out / "music-synth.wav"
with wave.open(str(path), "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes((mix * 32767).astype("<i2").tobytes())
subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(path), "-b:a", "192k", str(out / "music.mp3")], check=True)
path.unlink()
print(out / "music.mp3", f"{total:.1f}s, lift {lift:.1f}, full {full:.1f}, hit {hit:.1f}")
