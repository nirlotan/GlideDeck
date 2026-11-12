import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {AppWindow, Clip, Cut, tidy, View} from '../Footage';
import {C, F} from '../theme';
import {beat, when} from '../timeline';
import {E, lerp, mix, pop, Rise, tw, useFmt} from '../util';
import arrangeClip from '../footage/arrange.json';
import animateClip from '../footage/animate.json';

/** The editor is recorded at this size. */
const SOURCE: [number, number] = [2560, 1440];

/** Eases the view from one keyframe to the next. */
const viewAt = (frame: number, keys: {at: number; view: View}[]): View => {
  let v = keys[0].view;
  for (let i = 1; i < keys.length; i++) {
    const t = tw(frame, keys[i].at - 16, keys[i].at + 6, 0, 1, E.inOut);
    v = {cx: lerp(v.cx, keys[i].view.cx, t), cy: lerp(v.cy, keys[i].view.cy, t), scale: lerp(v.scale, keys[i].view.scale, t)};
  }
  return v;
};

const Chip: React.FC<{label: string; on: number; size: number}> = ({label, on, size}) => (
  <div
    style={{
      padding: `${size * 0.28}px ${size * 0.62}px ${size * 0.32}px`,
      borderRadius: 999,
      fontFamily: F.display,
      fontWeight: 700,
      fontSize: size,
      letterSpacing: '-0.02em',
      color: mix(C.muted, C.ink, on),
      backgroundColor: mix(C.surface, C.yellow, on),
      boxShadow: `0 0 0 2px ${mix(C.line, C.yellow, on)}`,
      transform: `scale(${1 + Math.sin(Math.PI * Math.min(1, on * 1.4)) * 0.08})`,
    }}
  >
    {label}
  </div>
);

/** The frame shared by both flows: a headline that changes, the app window, a row of chips. */
const Stage: React.FC<{
  frame: number;
  headlines: {words: string[]; at: number; accent?: number}[];
  chips: {label: string; at: number}[];
  chipsFrom: number;
  children: (size: {width: number; height: number}) => React.ReactNode;
}> = ({frame, headlines, chips, chipsFrom, children}) => {
  const {W, tall} = useFmt();
  const win = tall ? {width: 1040, height: 780} : {width: 1600, height: 900};
  const enter = tw(frame, 0, 16, 0, 1);
  return (
    <AbsoluteFill>
      {headlines.map((h, i) => {
        const next = headlines[i + 1]?.at ?? 1e7;
        const out = tw(frame, next - 8, next - 1, 0, 1, E.in);
        return (
          <div key={i} style={{position: 'absolute', left: tall ? 76 : 160, top: tall ? 300 : 38, width: tall ? 940 : 1100, opacity: 1 - out, transform: `translateY(${-out * 24}px)`}}>
            <Rise
              frame={frame}
              size={tall ? 92 : 68}
              color={C.bone}
              font={F.display}
              words={h.words.map((text, k) => ({text, at: h.at + k * 3, color: h.accent !== undefined && k >= h.accent ? C.yellow : undefined}))}
            />
          </div>
        );
      })}
      <div
        style={{
          position: 'absolute',
          left: (W - win.width) / 2,
          top: tall ? 560 : 150,
          opacity: Math.min(1, enter * 1.5),
          transform: `translateY(${(1 - enter) * 60}px) scale(${lerp(0.94, 1, enter)})`,
        }}
      >
        {children(win)}
      </div>
      <div
        style={{
          position: 'absolute',
          display: 'flex',
          gap: tall ? 20 : 16,
          ...(tall ? {left: 0, right: 0, top: 1372, justifyContent: 'center'} : {right: 160, top: 58}),
        }}
      >
        {chips.map((c, i) => {
          const shown = pop(frame, chipsFrom + i * 3, 13);
          return (
            <div key={i} style={{transform: `scale(${shown})`, opacity: Math.min(1, shown * 2)}}>
              <Chip label={c.label} on={tw(frame, c.at - 3, c.at + 5, 0, 1)} size={tall ? 44 : 34} />
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};

/** Beats 10 and 11: a PowerPoint deck drops in, and Arrange reshapes it three ways. */
export const Arrange: React.FC = () => {
  const frame = useCurrentFrame();
  const {W, tall} = useFmt();
  const clip = arrangeClip as unknown as Clip;
  const m = clip.marks;
  const a = beat('flow1a');
  const b = beat('flow1b');
  const b0 = b.start - a.start;
  const end = b.end - a.start;

  const drop = when(a, 'Drop') + 4;
  const open = b.voStart - a.start - 12;
  const spiral = b0 + when(b, 'spiral') - 5;
  const orbit = b0 + when(b, 'orbit') - 5;
  const carousel = b0 + when(b, 'carousel') - 5;
  const present = carousel + 30;

  const cuts: Cut[] = tidy([
    {src: [0, m.import], at: [0, drop]},
    {src: [m.import, m.import + 60], at: [drop, drop + 46]},
    {src: [m['open-Spiral'], m['applied-Spiral']], at: [open, spiral]},
    {src: [m['applied-Spiral'], m['applied-Spiral'] + 44], at: [spiral, orbit]},
    {src: [m['applied-Orbits'] - 2, m['applied-Orbits'] + 44], at: [orbit, carousel]},
    {src: [m['applied-Carousel'] - 2, m['applied-Carousel'] + 50], at: [carousel, present]},
    ...(tall ? [] : [{src: [m.present + 4, clip.frames - 1] as [number, number], at: [present, Math.max(end + 12, present + 30)] as [number, number]}]),
  ]);

  const whole: View = {cx: 1280, cy: 720, scale: 0};
  const canvas: View = tall ? {cx: 1060, cy: 665, scale: 0.6} : {cx: 1060, cy: 665, scale: 0.78};
  const dialog: View = tall ? {cx: 1298, cy: 720, scale: 0} : {cx: 1298, cy: 720, scale: 0.64};
  const view = viewAt(frame, [
    {at: 0, view: tall ? {...canvas, scale: 0} : whole},
    {at: open + 12, view: dialog},
    {at: spiral + 4, view: canvas},
    ...(tall ? [] : [{at: present + 4, view: whole}]),
  ]);

  // The .pptx file that gets dropped in.
  const fileIn = pop(frame, when(a, 'PowerPoint') - 8, 12);
  const fall = tw(frame, drop - 12, drop + 2, 0, 1, E.in);
  const fileX = lerp(tall ? W - 250 : W - 420, W / 2 - 60, fall);
  const fileY = lerp(tall ? 330 : 50, tall ? 900 : 560, fall);

  return (
    <AbsoluteFill>
      <Stage
        frame={frame}
        headlines={[
          {words: ['Got', 'a', 'PowerPoint?'], at: Math.max(2, when(a, 'Got') - 4), accent: 2},
          {words: ['One', 'click.', 'A', 'new', 'shape.'], at: b.voStart - a.start - 4, accent: 2},
        ]}
        chips={[
          {label: 'Spiral', at: spiral},
          {label: 'Orbits', at: orbit},
          {label: 'Carousel', at: carousel},
        ]}
        chipsFrom={b.voStart - a.start - 2}
      >
        {(size) => <AppWindow file="footage/arrange.mp4" clip={clip} cuts={cuts} source={SOURCE} view={view} cursor={tall || frame < present} {...size} />}
      </Stage>
      <div
        style={{
          position: 'absolute',
          left: fileX,
          top: fileY,
          width: 120,
          height: 150,
          transform: `scale(${fileIn * lerp(tall ? 1.25 : 1, 0.3, fall)}) rotate(${lerp(8, -4, fall)}deg)`,
          opacity: Math.min(1, fileIn * 2) * (1 - tw(frame, drop, drop + 5, 0, 1)),
          borderRadius: 14,
          backgroundColor: C.bone,
          boxShadow: '0 24px 60px rgba(0,0,0,0.55)',
          overflow: 'hidden',
        }}
      >
        <div style={{position: 'absolute', right: 0, top: 0, width: 34, height: 34, backgroundColor: C.bg, borderBottomLeftRadius: 12}} />
        <div style={{position: 'absolute', left: 16, top: 34, width: 60, height: 9, borderRadius: 4, backgroundColor: '#B9B7B1'}} />
        <div style={{position: 'absolute', left: 16, top: 54, width: 80, height: 9, borderRadius: 4, backgroundColor: '#CFCDC7'}} />
        <div
          style={{
            position: 'absolute',
            left: 0,
            right: 0,
            bottom: 0,
            height: 54,
            backgroundColor: C.coral,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontFamily: F.display,
            fontWeight: 800,
            fontSize: 30,
            color: C.ink,
            letterSpacing: '0.02em',
          }}
        >
          .pptx
        </div>
      </div>
    </AbsoluteFill>
  );
};

/** Beats 12 and 13: Auto-animate builds a slide in, and the Share menu lists every export. */
export const Animate: React.FC = () => {
  const frame = useCurrentFrame();
  const {tall} = useFmt();
  const clip = animateClip as unknown as Clip;
  const m = clip.marks;
  const a = beat('flow2a');
  const b = beat('flow2b');
  const b0 = b.start - a.start;
  const end = b.end - a.start;

  const share = b.voStart - a.start - 10;
  const web = b0 + when(b, 'web') - 6;
  const pdf = b0 + when(b, 'PDF') - 6;
  const ppt = b0 + when(b, 'PowerPoint') - 6;

  const cuts: Cut[] = tidy([
    {src: [m.auto - 14, m.share - 6], at: [0, share]},
    {src: [m.share, m['Web page (HTML)']], at: [share, web]},
    {src: [m['Web page (HTML)'], m.PDF], at: [web, pdf]},
    {src: [m.PDF, m['PowerPoint (Morph)']], at: [pdf, ppt]},
    {src: [m['PowerPoint (Morph)'], clip.frames - 1], at: [ppt, Math.max(end + 12, ppt + 30)]},
  ]);

  const click = Math.round(((m.auto + 27 - (m.auto - 14)) / (m.share - 6 - (m.auto - 14))) * share);
  const panel: View = tall ? {cx: 1866, cy: 720, scale: 0.75} : {cx: 1280, cy: 720, scale: 0};
  const canvas: View = tall ? {cx: 1060, cy: 665, scale: 0.6} : {cx: 1060, cy: 665, scale: 0.78};
  const menu: View = tall ? {cx: 2035, cy: 260, scale: 1.5} : {cx: 1945, cy: 346, scale: 1.3};
  const view = viewAt(frame, [
    {at: 0, view: panel},
    {at: click + 14, view: canvas},
    {at: share + 12, view: menu},
  ]);

  return (
    <Stage
      frame={frame}
      headlines={[
        {words: ['Animate', 'it', 'in', 'one', 'click.'], at: Math.max(2, when(a, 'Animate') - 4), accent: 3},
        {words: ['Share', 'it', 'anywhere.'], at: b.voStart - a.start - 4, accent: 2},
      ]}
      chips={[
        {label: 'Web page', at: web},
        {label: 'PDF', at: pdf},
        {label: 'PowerPoint', at: ppt},
      ]}
      chipsFrom={b.voStart - a.start - 2}
    >
      {(size) => <AppWindow file="footage/animate.mp4" clip={clip} cuts={cuts} source={SOURCE} view={view} {...size} />}
    </Stage>
  );
};
