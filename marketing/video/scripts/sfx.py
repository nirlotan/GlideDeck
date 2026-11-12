"""Synthesizes the three small sound effects (whoosh, click, pop) with numpy,
for when ElevenLabs sound generation is not available.
    python scripts/sfx.py
"""
import subprocess
import wave
from pathlib import Path

import numpy as np

SR = 44100
out = Path(__file__).resolve().parent.parent / "public" / "audio"
out.mkdir(parents=True, exist_ok=True)
rng = np.random.default_rng(7)


def bandpass(x, freq, q):
    """A state-variable band-pass whose centre frequency can change per sample."""
    low = band = 0.0
    y = np.zeros_like(x)
    f = 2 * np.sin(np.pi * np.minimum(freq, SR / 6.5) / SR)
    for i in range(len(x)):
        low += f[i] * band
        high = x[i] - low - band / q
        band += f[i] * high
        y[i] = band
    return y


def save(name, left, right=None):
    right = left if right is None else right
    stereo = np.stack([left, right], axis=1)
    stereo = stereo / max(1e-9, np.abs(stereo).max()) * 0.85
    path = out / f"{name}.wav"
    with wave.open(str(path), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes((stereo * 32767).astype("<i2").tobytes())
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(path), "-b:a", "192k", str(out / f"{name}.mp3")], check=True)
    path.unlink()
    print(out / f"{name}.mp3")


# Whoosh: noise through a band that sweeps up and back, swelling in the middle, moving left to right.
n = int(SR * 0.75)
t = np.linspace(0, 1, n)
sweep = 250 * (2400 / 250) ** np.sin(np.pi * t ** 0.8)
body = bandpass(rng.standard_normal(n), sweep, 1.6) + 0.35 * bandpass(rng.standard_normal(n), sweep * 2.6, 2.5)
env = np.sin(np.pi * t ** 0.7) ** 2.4
pan = 0.5 + 0.35 * (t - 0.5) * 2
save("whoosh", body * env * np.sqrt(1 - pan), body * env * np.sqrt(pan))

# Click: a tiny burst of noise and a damped tone.
n = int(SR * 0.09)
t = np.arange(n) / SR
click = rng.standard_normal(n) * np.exp(-t * 900) * 0.7 + np.sin(2 * np.pi * 1900 * t) * np.exp(-t * 260) * 0.6 + np.sin(2 * np.pi * 320 * t) * np.exp(-t * 120) * 0.5
save("click", click)

# Pop: a short tone that jumps up in pitch.
n = int(SR * 0.16)
t = np.arange(n) / SR
freq = 380 + 620 * (1 - np.exp(-t * 60))
pop = np.sin(2 * np.pi * np.cumsum(freq) / SR) * np.exp(-t * 34) * (1 - np.exp(-t * 900))
save("pop", pop)
