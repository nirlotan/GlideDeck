// Records the narration with ElevenLabs, one file per beat, and writes
// src/vo.json with each beat's length and word timings.
//
//   node scripts/voice.mjs              record every beat
//   node scripts/voice.mjs hook close   re-record only these beats
//   node scripts/voice.mjs --retime     no API call: tighten the takes already recorded
//   node scripts/voice.mjs --estimate   no API call: estimated timings, no audio
//
// Reads ELEVENLABS_API_KEY (and optionally ELEVENLABS_VOICE_ID, ELEVENLABS_MODEL)
// from .env or the environment.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync, spawnSync} from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const env = {...process.env};
const envFile = path.join(root, '.env');
if (fs.existsSync(envFile)) {
  for (const line of fs.readFileSync(envFile, 'utf8').split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/);
    if (m && m[2]) env[m[1]] = m[2].replace(/^["']|["']$/g, '');
  }
}
const beats = JSON.parse(fs.readFileSync(path.join(root, 'src/beats.json'), 'utf8'));
const voPath = path.join(root, 'src/vo.json');
const audioDir = path.join(root, 'public/audio');
fs.mkdirSync(audioDir, {recursive: true});
const args = process.argv.slice(2);
const estimate = args.includes('--estimate');
const retime = args.includes('--retime');
const only = args.filter((a) => !a.startsWith('--'));

/** Groups character timings into words: [word, start, end]. */
function toWords(chars, starts, ends) {
  const words = [];
  let cur = null;
  chars.forEach((ch, i) => {
    if (/\s/.test(ch)) {
      if (cur) words.push(cur);
      cur = null;
    } else if (cur) {
      cur[0] += ch;
      cur[2] = ends[i];
    } else {
      cur = [ch, starts[i], ends[i]];
    }
  });
  if (cur) words.push(cur);
  return words.map(([w, s, e]) => [w, +s.toFixed(3), +e.toFixed(3)]);
}

/** A rough guess at the timings, for laying out the video before the voice exists. */
function guess(text) {
  const chars = [...text];
  const starts = [];
  const ends = [];
  let t = 0;
  for (const ch of chars) {
    starts.push(t);
    t += /[.?!]/.test(ch) ? 0.34 : /,/.test(ch) ? 0.2 : 0.058;
    ends.push(t);
  }
  return {words: toWords(chars, starts, ends), duration: +t.toFixed(3)};
}

const vo = fs.existsSync(voPath) ? JSON.parse(fs.readFileSync(voPath, 'utf8')) : {beats: {}};
const rawDir = path.join(audioDir, 'raw');
fs.mkdirSync(rawDir, {recursive: true});

/** Pauses longer than this are shortened to it, so the read stays brisk. */
const MAX_PAUSE = 0.3;

function silences(file, length) {
  const r = spawnSync('ffmpeg', ['-hide_banner', '-i', file, '-af', 'silencedetect=noise=-38dB:d=0.12', '-f', 'null', '-'], {encoding: 'utf8'});
  const out = [];
  let start = null;
  for (const line of r.stderr.split(/\r?\n/)) {
    const s = line.match(/silence_start: ([\d.]+)/);
    const e = line.match(/silence_end: ([\d.]+)/);
    if (s) start = +s[1];
    if (e && start !== null) {
      out.push([Math.max(0, start), Math.min(length, +e[1])]);
      start = null;
    }
  }
  if (start !== null) out.push([start, length]);
  return out;
}

function tightenTake(id) {
  const raw = path.join(rawDir, `${id}.mp3`);
  const a = JSON.parse(fs.readFileSync(path.join(rawDir, `${id}.json`), 'utf8'));
  const length = +execFileSync('ffprobe', ['-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', raw]).toString().trim();
  // What to remove: all but a sliver of the silence at each end, and the excess of each long pause.
  const cuts = [];
  for (const [s, e] of silences(raw, length)) {
    const keep = s <= 0.01 ? 0.03 : e >= length - 0.01 ? 0.1 : MAX_PAUSE;
    if (e - s <= keep + 0.02) continue;
    if (s <= 0.01) cuts.push([0, e - keep]);
    else if (e >= length - 0.01) cuts.push([s + keep, length]);
    else cuts.push([s + keep / 2, e - keep / 2]);
  }
  const kept = [];
  let at = 0;
  for (const [s, e] of cuts) {
    if (s > at) kept.push([at, s]);
    at = e;
  }
  if (at < length) kept.push([at, length]);
  const file = `audio/vo-${id}.mp3`;
  const filter = kept.map(([s, e], i) => `[0:a]atrim=${s.toFixed(4)}:${e.toFixed(4)},asetpts=PTS-STARTPTS[a${i}]`).join(';') + ';' + kept.map((_, i) => `[a${i}]`).join('') + `concat=n=${kept.length}:v=0:a=1[out]`;
  execFileSync('ffmpeg', ['-v', 'error', '-y', '-i', raw, '-filter_complex', filter, '-map', '[out]', '-c:a', 'libmp3lame', '-b:a', '192k', path.join(root, 'public', file)]);
  const shift = (t) => {
    let gone = 0;
    for (const [s, e] of cuts) {
      if (t >= e) gone += e - s;
      else if (t > s) gone += t - s;
    }
    return t - gone;
  };
  const duration = kept.reduce((n, [s, e]) => n + e - s, 0);
  const words = toWords(a.characters, a.character_start_times_seconds.map(shift), a.character_end_times_seconds.map(shift));
  return {file, duration: +duration.toFixed(3), words};
}

if (estimate) {
  vo.estimated = true;
  for (const b of beats) vo.beats[b.id] = {file: null, ...guess(b.text)};
} else {
  if (!retime) {
    const key = env.ELEVENLABS_API_KEY;
    if (!key) throw new Error('Set ELEVENLABS_API_KEY in .env first.');
    const voice = env.ELEVENLABS_VOICE_ID;
    if (!voice) throw new Error('Set ELEVENLABS_VOICE_ID in .env.');
    const model = env.ELEVENLABS_MODEL || 'eleven_multilingual_v2';
    for (const [i, b] of beats.entries()) {
      if (only.length && !only.includes(b.id)) continue;
      const res = await fetch(`https://api.elevenlabs.io/v1/text-to-speech/${voice}/with-timestamps?output_format=mp3_44100_128`, {
        method: 'POST',
        headers: {'xi-api-key': key, 'content-type': 'application/json'},
        body: JSON.stringify({
          text: b.say ?? b.text,
          model_id: model,
          previous_text: beats.slice(0, i).map((x) => x.text).join(' ').slice(-600) || undefined,
          next_text: beats.slice(i + 1).map((x) => x.text).join(' ').slice(0, 400) || undefined,
          voice_settings: {stability: 0.4, similarity_boost: 0.8, style: 0.45, use_speaker_boost: true, speed: 1.08},
        }),
      });
      if (!res.ok) throw new Error(`${b.id}: ${res.status} ${await res.text()}`);
      const json = await res.json();
      fs.writeFileSync(path.join(rawDir, `${b.id}.mp3`), Buffer.from(json.audio_base64, 'base64'));
      fs.writeFileSync(path.join(rawDir, `${b.id}.json`), JSON.stringify(json.alignment));
      console.log(`recorded ${b.id}`);
    }
    vo.voice = voice;
    vo.model = model;
  }
  for (const b of beats) {
    vo.beats[b.id] = tightenTake(b.id);
    console.log(`${b.id}: ${vo.beats[b.id].duration.toFixed(2)}s`);
  }
  vo.estimated = false;
}
fs.writeFileSync(voPath, JSON.stringify(vo, null, 1) + '\n');
const total = beats.reduce((t, b) => t + (vo.beats[b.id]?.duration ?? 0) + b.gap, 0);
console.log(`narration: ${total.toFixed(1)}s including gaps`);
