import React, {createContext, useContext} from 'react';
import {Easing, interpolate, spring} from 'remotion';
import {FPS} from './timeline';

export type Fmt = {W: number; H: number; tall: boolean};
const FmtContext = createContext<Fmt>({W: 1920, H: 1080, tall: false});
export const FmtProvider = FmtContext.Provider;
export const useFmt = () => useContext(FmtContext);

export const E = {
  out: Easing.bezier(0.16, 1, 0.3, 1),
  inOut: Easing.bezier(0.65, 0, 0.35, 1),
  in: Easing.bezier(0.5, 0, 0.9, 0.4),
  snap: Easing.bezier(0.2, 0.9, 0.1, 1),
};

/** `a` before `from`, `b` after `to`, eased in between. */
export const tw = (frame: number, from: number, to: number, a: number, b: number, easing: (t: number) => number = E.out) =>
  interpolate(frame, [from, Math.max(to, from + 1)], [a, b], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing});

/** A springy 0 → 1 that starts at `at`. */
export const pop = (frame: number, at: number, bounce = 12) =>
  spring({frame: frame - at, fps: FPS, config: {damping: bounce, stiffness: 170, mass: 0.7}});

export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

const hex = (c: string) => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16));
/** Blends two #rrggbb colours. */
export const mix = (a: string, b: string, t: number) => {
  const [x, y] = [hex(a), hex(b)];
  return `rgb(${x.map((v, i) => Math.round(lerp(v, y[i], t))).join(',')})`;
};

/** Deterministic pseudo-random number in 0..1. */
export const rnd = (seed: number) => {
  const x = Math.sin(seed * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
};

/** Words that rise into place, each at its own frame. */
export const Rise: React.FC<{
  frame: number;
  words: {text: string; at: number; color?: string}[];
  size: number;
  color: string;
  font: string;
  weight?: number;
  align?: 'left' | 'center';
  lineHeight?: number;
  tracking?: number;
  maxWidth?: number;
}> = ({frame, words, size, color, font, weight = 800, align = 'left', lineHeight = 1.02, tracking = -0.035, maxWidth}) => (
  <div
    style={{
      fontFamily: font,
      fontWeight: weight,
      fontSize: size,
      lineHeight,
      letterSpacing: `${tracking}em`,
      color,
      textAlign: align,
      maxWidth,
      display: 'flex',
      flexWrap: 'wrap',
      justifyContent: align === 'center' ? 'center' : 'flex-start',
      columnGap: '0.24em',
    }}
  >
    {words.map((w, i) => {
      const p = tw(frame, w.at, w.at + 12, 0, 1);
      return w.text === '\n' ? (
        <div key={i} style={{flexBasis: '100%', height: 0}} />
      ) : (
        <span key={i} style={{display: 'inline-block', overflow: 'hidden', paddingBottom: '0.12em', marginBottom: '-0.12em'}}>
          <span style={{display: 'inline-block', transform: `translateY(${(1 - p) * 105}%)`, opacity: tw(frame, w.at, w.at + 5, 0, 1), color: w.color}}>{w.text}</span>
        </span>
      );
    })}
  </div>
);
