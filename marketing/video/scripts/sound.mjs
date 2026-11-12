// Generates the music bed and the sound effects with ElevenLabs and records
// which ones exist in src/audio.json.
//
//   node scripts/sound.mjs            everything that is missing
//   node scripts/sound.mjs music      (re)generate just these: music, whoosh, click, pop
//
// Reads ELEVENLABS_API_KEY from .env or the environment.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const env = {...process.env};
const envFile = path.join(root, '.env');
if (fs.existsSync(envFile)) {
  for (const line of fs.readFileSync(envFile, 'utf8').split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/);
    if (m && m[2]) env[m[1]] = m[2].replace(/^["']|["']$/g, '');
  }
}
const key = env.ELEVENLABS_API_KEY;
if (!key) throw new Error('Set ELEVENLABS_API_KEY in .env first.');
const audioDir = path.join(root, 'public/audio');
fs.mkdirSync(audioDir, {recursive: true});
const manifestPath = path.join(root, 'src/audio.json');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const only = process.argv.slice(2);

const effects = {
  whoosh: {text: 'Short, soft, airy whoosh for a fast camera move. Clean and modern, no impact, no reverb tail.', duration_seconds: 0.9},
  click: {text: 'One crisp, dry computer mouse click. Very short. No room sound.', duration_seconds: 0.5},
  pop: {text: 'One soft, bright, bubbly interface pop. Short and friendly.', duration_seconds: 0.5},
};

const music = {
  prompt:
    'Instrumental only, no vocals. Upbeat, modern, youthful music for a tech product launch video, 118 BPM. ' +
    'Bouncy synth bass, tight punchy drums with crisp claps, bright plucky synths and warm chords: confident, playful and optimistic. ' +
    'It opens sparse and curious for about twelve seconds with a muted pluck and light percussion, lifts into the full groove at about eighteen seconds, ' +
    'keeps building, and ends on one clean final hit that rings out. Leave room for a voice-over: no busy lead melody.',
  music_length_ms: 70000,
};

async function call(url, body) {
  const res = await fetch(url, {method: 'POST', headers: {'xi-api-key': key, 'content-type': 'application/json'}, body: JSON.stringify(body)});
  if (!res.ok) throw new Error(`${url}: ${res.status} ${await res.text()}`);
  return Buffer.from(await res.arrayBuffer());
}

for (const [name, body] of Object.entries(effects)) {
  const file = `audio/${name}.mp3`;
  const wanted = only.length ? only.includes(name) : !fs.existsSync(path.join(root, 'public', file));
  if (wanted) {
    try {
      fs.writeFileSync(path.join(root, 'public', file), await call('https://api.elevenlabs.io/v1/sound-generation', {...body, prompt_influence: 0.6}));
      console.log(`made ${file}`);
    } catch (e) {
      console.log(`no ${name}: ${e.message}`);
    }
  }
  manifest[name] = fs.existsSync(path.join(root, 'public', file)) ? file : null;
}

{
  const file = 'audio/music.mp3';
  const wanted = only.length ? only.includes('music') : !fs.existsSync(path.join(root, 'public', file));
  if (wanted) {
    try {
      fs.writeFileSync(path.join(root, 'public', file), await call('https://api.elevenlabs.io/v1/music', music));
      manifest.musicSource = 'ElevenLabs Music';
      console.log(`made ${file}`);
    } catch (e) {
      console.log(`no music: ${e.message}`);
    }
  }
  manifest.music = fs.existsSync(path.join(root, 'public', file)) ? file : null;
}

fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
console.log(manifest);
