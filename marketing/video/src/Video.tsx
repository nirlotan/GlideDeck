import React from 'react';
import {AbsoluteFill, Audio, interpolate, Sequence, staticFile, useCurrentFrame} from 'remotion';
import {Backdrop} from './Backdrop';
import {Ideas} from './scenes/Ideas';
import {Hero} from './scenes/Hero';
import {Proof} from './scenes/Proof';
import {Animate, Arrange} from './scenes/Flows';
import {Close, Values} from './scenes/Outro';
import {C, F} from './theme';
import {beat, BEATS, TOTAL, when} from './timeline';
import {E, FmtProvider, tw, useFmt} from './util';
import audio from './audio.json';

/** How long one scene takes to fly into the next. */
const FLY = 12;

/** A scene that flies in from a distance and flies out past the camera. */
const Shell: React.FC<{length: number; enter?: boolean; exit?: boolean; children: React.ReactNode}> = ({length, enter = true, exit = true, children}) => {
  const frame = useCurrentFrame();
  const a = enter ? tw(frame, 0, FLY, 0, 1) : 1;
  const z = exit ? tw(frame, length - FLY, length, 0, 1, E.in) : 0;
  return (
    <AbsoluteFill style={{opacity: Math.min(1, a * 1.6) * (1 - z), transform: `scale(${(0.82 + 0.18 * a) * (1 + z * 0.7)})`}}>{children}</AbsoluteFill>
  );
};

type Scene = {from: number; to: number; enter?: boolean; exit?: boolean; node: React.ReactNode};

const scenes = (): Scene[] => {
  const s = (id: string) => beat(id).start;
  return [
    {from: 0, to: s('reveal'), enter: false, exit: false, node: <Ideas />},
    {from: s('reveal'), to: s('proof') + FLY, enter: false, node: <Hero />},
    {from: s('proof'), to: s('flow1a') + FLY, node: <Proof />},
    {from: s('flow1a'), to: s('flow2a') + FLY, node: <Arrange />},
    {from: s('flow2a'), to: s('values') + FLY, node: <Animate />},
    {from: s('values'), to: s('close') + FLY, node: <Values />},
    {from: s('close'), to: TOTAL, exit: false, node: <Close />},
  ];
};

/** The canvas behind the type scenes: it drifts, and zooms on through each scene change. */
const Canvas: React.FC = () => {
  const frame = useCurrentFrame();
  const zoom = scenes()
    .slice(2)
    .reduce((z, sc) => z + tw(frame, sc.from - 4, sc.from + FLY + 6, 0, 1, E.inOut), 0);
  return <Backdrop cam={{x: frame * 1.1, y: frame * 0.45, s: 0.8 * 2 ** (zoom * 0.9), r: -3}} />;
};

/** Burned-in captions for the phone cut: a few words at a time, the spoken one in yellow. */
const Captions: React.FC = () => {
  const frame = useCurrentFrame();
  const {W} = useFmt();
  for (const b of BEATS) {
    const local = frame - b.start;
    // Break the line into short phrases at punctuation, or after three words.
    const groups: (typeof b.words)[] = [];
    let cur: typeof b.words = [];
    for (const w of b.words) {
      cur.push(w);
      const chars = cur.reduce((n, x) => n + x.w.length + 1, 0);
      if (/[.?!,]$/.test(w.w) || cur.length >= 4 || chars > 20) {
        groups.push(cur);
        cur = [];
      }
    }
    if (cur.length) groups.push(cur);
    for (let i = 0; i < groups.length; i++) {
      const g = groups[i];
      const from = g[0].s - 2;
      const to = Math.min(groups[i + 1]?.[0].s ?? Infinity, g[g.length - 1].e + 14) - 2;
      if (local < from || local >= to) continue;
      return (
        <div style={{position: 'absolute', left: 0, width: W, top: 1500, display: 'flex', justifyContent: 'center'}}>
          <div
            style={{
              display: 'flex',
              gap: 16,
              padding: '16px 34px 20px',
              borderRadius: 26,
              backgroundColor: 'rgba(15,17,19,0.9)',
              boxShadow: `0 0 0 2px ${C.line}`,
              fontFamily: F.display,
              fontWeight: 700,
              fontSize: 58,
              letterSpacing: '-0.02em',
              transform: `scale(${tw(local, from, from + 5, 0.92, 1)})`,
            }}
          >
            {g.map((w, k) => (
              <span key={k} style={{color: local >= w.s - 1 && local < w.e + 2 ? C.yellow : C.bone}}>
                {w.w}
              </span>
            ))}
          </div>
        </div>
      );
    }
  }
  return null;
};

/** Narration, music that ducks under it, and a sound for each move. */
const Sound: React.FC = () => {
  const s = (id: string) => beat(id).start;
  const problem = beat('problem');
  const whooshes = [s('problem') - 2, s('turn'), s('reveal') - 20, ...scenes().slice(2).map((x) => x.from - 2)];
  const clicks = [1, 2, 3].map((n) => problem.start + when(problem, 'Next', n) - 1);
  const pops = [beat('values'), beat('close')].flatMap((b) => (b.id === 'values' ? [when(b, 'Free'), when(b, 'open'), when(b, 'runs')] : [when(b, 'GlideDeck') - 6]).map((f) => b.start + f - 3));
  return (
    <>
      {BEATS.map((b) => (b.file ? <Sequence key={b.id} from={b.voStart} layout="none"><Audio src={staticFile(b.file)} volume={1} /></Sequence> : null))}
      {audio.music ? (
        <Audio
          src={staticFile(audio.music)}
          volume={(f) => {
            // Full at the top and tail, lower under the voice.
            const talking = BEATS.some((b) => f >= b.voStart - 8 && f <= b.voEnd + 6);
            const fade = interpolate(f, [0, 20, TOTAL - 50, TOTAL - 4], [0, 1, 1, 0], {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'});
            return fade * (talking ? audio.musicUnderVoice : audio.musicAlone);
          }}
        />
      ) : null}
      {audio.whoosh ? whooshes.map((f, i) => <Sequence key={`w${i}`} from={Math.max(0, f)} layout="none"><Audio src={staticFile(audio.whoosh!)} volume={0.5} /></Sequence>) : null}
      {audio.click ? clicks.map((f, i) => <Sequence key={`c${i}`} from={f} layout="none"><Audio src={staticFile(audio.click!)} volume={0.7} /></Sequence>) : null}
      {audio.pop ? pops.map((f, i) => <Sequence key={`p${i}`} from={f} layout="none"><Audio src={staticFile(audio.pop!)} volume={0.45} /></Sequence>) : null}
    </>
  );
};

export const Video: React.FC<{width: number; height: number}> = ({width, height}) => {
  const tall = height > width;
  return (
    <FmtProvider value={{W: width, H: height, tall}}>
      <AbsoluteFill style={{backgroundColor: C.bg}}>
        <Canvas />
        {scenes().map((sc, i) => (
          <Sequence key={i} from={sc.from} durationInFrames={sc.to - sc.from}>
            <Shell length={sc.to - sc.from} enter={sc.enter} exit={sc.exit}>
              {sc.node}
            </Shell>
          </Sequence>
        ))}
        {tall ? <Captions /> : null}
        <Sound />
      </AbsoluteFill>
    </FmtProvider>
  );
};
