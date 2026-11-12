import React from 'react';
import {Freeze, OffthreadVideo, Sequence, staticFile, useCurrentFrame} from 'remotion';
import {C} from './theme';
import {tw} from './util';

/** What the recorder writes beside each clip. */
export type Clip = {fps: number; frames: number; marks: Record<string, number>; cursor: number[][]};

/** Plays source frames `src[0]..src[1]` of the clip over the scene frames `at[0]..at[1]`. */
export type Cut = {src: [number, number]; at: [number, number]};

/** Makes the output ranges of the cuts follow one another without overlapping. */
export const tidy = (cuts: Cut[]): Cut[] => {
  let floor = -Infinity;
  return cuts.map((c) => {
    const a = Math.max(Math.round(c.at[0]), floor);
    const b = Math.max(Math.round(c.at[1]), a + 1);
    floor = b;
    return {src: c.src, at: [a, b]};
  });
};

/** The source frame showing at scene frame `frame`. */
export const sourceAt = (cuts: Cut[], frame: number): number => {
  let cur = cuts[0];
  for (const c of cuts) {
    if (frame >= c.at[0]) cur = c;
  }
  const t = Math.min(1, Math.max(0, (frame - cur.at[0]) / (cur.at[1] - cur.at[0])));
  return cur.src[0] + (cur.src[1] - cur.src[0]) * t;
};

/** The clip, retimed: each cut plays its stretch of the source at whatever speed fits. */
export const Retimed: React.FC<{file: string; clip: Clip; cuts: Cut[]; style?: React.CSSProperties}> = ({file, clip, cuts, style}) => {
  const last = clip.frames - 1;
  const src = staticFile(file);
  const still = (from: number) => (
    <Freeze frame={0}>
      <OffthreadVideo src={src} trimBefore={Math.min(last, Math.max(0, Math.round(from)))} muted style={style} />
    </Freeze>
  );
  return (
    <>
      {cuts.map((c, i) => {
        const end = i + 1 < cuts.length ? cuts[i + 1].at[0] : c.at[1] + 100000;
        const from = Math.min(last, Math.max(0, Math.round(c.src[0])));
        const len = c.at[1] - c.at[0];
        const rate = (Math.min(last, c.src[1]) - from) / len;
        const hold = end - c.at[1];
        return (
          <React.Fragment key={i}>
            {i === 0 && c.at[0] > 0 ? (
              <Sequence durationInFrames={c.at[0]} layout="none">
                {still(from)}
              </Sequence>
            ) : null}
            <Sequence from={c.at[0]} durationInFrames={len} layout="none">
              {rate < 0.02 ? still(from) : <OffthreadVideo src={src} trimBefore={from} playbackRate={rate} muted style={style} />}
            </Sequence>
            {hold > 0 ? (
              <Sequence from={c.at[1]} durationInFrames={hold} layout="none">
                {still(c.src[1])}
              </Sequence>
            ) : null}
          </React.Fragment>
        );
      })}
    </>
  );
};

/** Where the window looks: the source pixel at its center, and screen pixels per source pixel. */
export type View = {cx: number; cy: number; scale: number};

/**
 * Editor footage in a window: retimed, with a view that can punch in and
 * follow the action, and the pointer redrawn on top from the recorded track.
 */
export const AppWindow: React.FC<{
  file: string;
  clip: Clip;
  cuts: Cut[];
  source: [number, number];
  width: number;
  height: number;
  view: View;
  radius?: number;
  cursor?: boolean;
}> = ({file, clip, cuts, source, width, height, view, radius = 22, cursor = true}) => {
  const frame = useCurrentFrame();
  const [sw, sh] = source;
  const scale = Math.max(view.scale, width / sw, height / sh);
  const cx = Math.min(sw - width / 2 / scale, Math.max(width / 2 / scale, view.cx));
  const cy = Math.min(sh - height / 2 / scale, Math.max(height / 2 / scale, view.cy));
  const at = sourceAt(cuts, frame);
  const i = Math.min(clip.cursor.length - 1, Math.max(0, Math.round(at)));
  const [px, py, down] = clip.cursor[i];
  // A ring spreads from each click.
  let click = -1;
  for (let k = i; k > Math.max(0, i - 14); k--) {
    if (clip.cursor[k][2] && !clip.cursor[k - 1]?.[2]) {
      click = i - k;
      break;
    }
  }
  const size = 30 / Math.min(1, scale * 1.3);
  return (
    <div
      style={{
        width,
        height,
        borderRadius: radius,
        overflow: 'hidden',
        position: 'relative',
        backgroundColor: C.surface,
        boxShadow: `0 0 0 1.5px ${C.line}, 0 50px 120px rgba(0,0,0,0.6)`,
      }}
    >
      <div
        style={{
          position: 'absolute',
          left: 0,
          top: 0,
          width: sw,
          height: sh,
          transformOrigin: '0 0',
          transform: `translate(${width / 2 - cx * scale}px, ${height / 2 - cy * scale}px) scale(${scale})`,
        }}
      >
        <Retimed file={file} clip={clip} cuts={cuts} style={{position: 'absolute', left: 0, top: 0, width: sw, height: sh}} />
        {cursor && px > 0 ? (
          <>
            {click >= 0 ? (
              <div
                style={{
                  position: 'absolute',
                  left: px,
                  top: py,
                  width: 0,
                  height: 0,
                  borderRadius: '50%',
                  boxShadow: `0 0 0 ${tw(click, 0, 14, 6, 70) / scale}px rgba(255,210,63,${tw(click, 0, 14, 0.55, 0)})`,
                }}
              />
            ) : null}
            <svg
              width={size}
              height={size * 1.25}
              viewBox="0 0 24 30"
              style={{position: 'absolute', left: px, top: py, transformOrigin: '0 0', transform: `scale(${down ? 0.88 : 1})`, filter: 'drop-shadow(0 3px 6px rgba(0,0,0,0.5))'}}
            >
              <path d="M2 1.5 L2 23 L7.6 18 L11.4 27 L15.2 25.3 L11.5 16.6 L19 16.2 Z" fill="#fff" stroke="#111" strokeWidth={1.6} strokeLinejoin="round" />
            </svg>
          </>
        ) : null}
      </div>
    </div>
  );
};
