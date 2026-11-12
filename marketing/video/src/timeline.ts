import beats from './beats.json';
import vo from './vo.json';

export const FPS = 30;
/** Silence before the first line, and the hold after the last one. */
const LEAD = 0.6;
const TAIL = 3.4;
/** A scene starts a little before its line is spoken. */
const SCENE_LEAD = 0.3;

export type Word = {w: string; s: number; e: number};
export type Beat = {
  id: string;
  text: string;
  file: string | null;
  /** Frames, in the whole video. */
  start: number;
  end: number;
  voStart: number;
  voEnd: number;
  /** Word timings in frames from the start of the scene. */
  words: Word[];
};

const f = (seconds: number) => Math.round(seconds * FPS);

type Take = {file: string | null; duration: number; words: [string, number, number][]};
const takes = (vo as unknown as {beats: Record<string, Take>}).beats;

let t = LEAD;
const timed = beats.map((b, i) => {
  const take = takes[b.id];
  const voStart = t;
  t += take.duration + b.gap;
  return {b, take, voStart, start: i === 0 ? 0 : voStart - SCENE_LEAD};
});

export const TOTAL = f(t + TAIL);

export const BEATS: Beat[] = timed.map((x, i) => {
  const start = f(x.start);
  return {
    id: x.b.id,
    text: x.b.text,
    file: x.take.file,
    start,
    end: i + 1 < timed.length ? f(timed[i + 1].start) : TOTAL,
    voStart: f(x.voStart),
    voEnd: f(x.voStart + x.take.duration),
    words: x.take.words.map(([w, s, e]) => ({w, s: f(x.voStart + s) - start, e: f(x.voStart + e) - start})),
  };
});

export const beat = (id: string): Beat => {
  const b = BEATS.find((x) => x.id === id);
  if (!b) throw new Error(`no beat ${id}`);
  return b;
};

const plain = (w: string) => w.toLowerCase().replace(/[^a-z0-9]/g, '');

/** The frame (from the start of the beat's scene) at which the nth `word` starts. */
export const when = (b: Beat, word: string, nth = 1): number => {
  let seen = 0;
  for (const x of b.words) {
    if (plain(x.w) === plain(word) && ++seen === nth) return x.s;
  }
  throw new Error(`"${word}" #${nth} is not in beat ${b.id}`);
};

/** The frame at which the nth `word` ends. */
export const after = (b: Beat, word: string, nth = 1): number => {
  let seen = 0;
  for (const x of b.words) {
    if (plain(x.w) === plain(word) && ++seen === nth) return x.e;
  }
  throw new Error(`"${word}" #${nth} is not in beat ${b.id}`);
};
