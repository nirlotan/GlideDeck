// Turns the rendered masters into the files to publish:
//
//   out/glidedeck-desktop.mp4   16:9, 1080p, H.264 + AAC, under 10 MB: the size GitHub
//                               accepts for a video dragged into a README, issue or release
//                               note on a free account.
//   out/glidedeck-mobile.mp4    9:16, 1080 x 1920, H.264 + AAC, for Reels, Shorts and TikTok.
//
//   node scripts/encode.mjs
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';

const out = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'out');
const run = (args) => execFileSync('ffmpeg', ['-v', 'error', '-y', ...args], {stdio: 'inherit', cwd: out});
const seconds = (file) => +execFileSync('ffprobe', ['-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', path.join(out, file)]).toString().trim();
const mb = (file) => (fs.statSync(path.join(out, file)).size / 1024 / 1024).toFixed(2);
// Voice and music at a steady, platform-friendly loudness.
const loud = 'loudnorm=I=-15:TP=-1.5:LRA=11';
// Remotion renders full-range colour; players expect the usual TV range, tagged BT.709.
const colour = ['-vf', 'scale=in_range=full:out_range=tv:out_color_matrix=bt709,format=yuv420p', '-color_range', 'tv', '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709'];
const common = [...colour, '-pix_fmt', 'yuv420p', '-profile:v', 'high', '-level', '4.2', '-movflags', '+faststart', '-c:a', 'aac', '-ar', '48000'];

// Desktop: two passes at the bitrate that lands just under 10 MB.
{
  const src = 'glidedeck-desktop-master.mp4';
  const audio = 128;
  const video = Math.floor((9.5 * 8 * 1024) / seconds(src) - audio - 6);
  const x264 = ['-c:v', 'libx264', '-preset', 'veryslow', '-b:v', `${video}k`, '-maxrate', `${Math.round(video * 2.2)}k`, '-bufsize', `${video * 4}k`, '-tune', 'animation'];
  run(['-i', src, ...colour, ...x264, '-pass', '1', '-passlogfile', 'pass', '-an', '-f', 'null', process.platform === 'win32' ? 'NUL' : '/dev/null']);
  run(['-i', src, ...x264, '-pass', '2', '-passlogfile', 'pass', '-af', loud, '-b:a', `${audio}k`, ...common, 'glidedeck-desktop.mp4']);
  for (const f of fs.readdirSync(out)) if (f.startsWith('pass')) fs.unlinkSync(path.join(out, f));
  console.log(`glidedeck-desktop.mp4: ${mb('glidedeck-desktop.mp4')} MB at ${video} kbps video`);
}

// Mobile: quality first; the platforms re-encode anyway.
{
  run(['-i', 'glidedeck-mobile-master.mp4', '-c:v', 'libx264', '-preset', 'slow', '-crf', '19', '-af', loud, '-b:a', '192k', ...common, 'glidedeck-mobile.mp4']);
  console.log(`glidedeck-mobile.mp4: ${mb('glidedeck-mobile.mp4')} MB`);
}
