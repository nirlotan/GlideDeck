// Writes the fallback beat (scripts/beat.py), timed to the current narration.
import {execFileSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const beats = JSON.parse(fs.readFileSync(path.join(root, 'src/beats.json'), 'utf8'));
const vo = JSON.parse(fs.readFileSync(path.join(root, 'src/vo.json'), 'utf8'));
let t = 0.6;
const start = {};
for (const b of beats) {
  start[b.id] = t;
  t += vo.beats[b.id].duration + b.gap;
}
const hit = start.close + vo.beats.close.words.find((w) => /glidedeck/i.test(w[0]))[1];
const args = [(t + 3.4).toFixed(2), start.turn.toFixed(2), start.reveal.toFixed(2), hit.toFixed(2)];
execFileSync('python', [path.join(root, 'scripts/beat.py'), ...args], {stdio: 'inherit'});
const manifestPath = path.join(root, 'src/audio.json');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
manifest.music = 'audio/music.mp3';
manifest.musicSource = 'synthesized by scripts/beat.py';
fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
