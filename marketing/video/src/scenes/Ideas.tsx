import React from 'react';
import {AbsoluteFill, Img, staticFile, useCurrentFrame} from 'remotion';
import {Backdrop, Cam, World} from '../Backdrop';
import {C, F} from '../theme';
import {after, beat, when} from '../timeline';
import {E, lerp, mix, pop, Rise, rnd, tw, useFmt} from '../util';

/**
 * Beats 1 to 3, in one world and one camera move:
 * ideas scattered and linked → squeezed into a row of slides → set free on a canvas.
 * It ends with the camera diving into the middle card, which is the first frame of the app footage.
 */

const IDEAS = ['Why', 'Vision', 'Proof', 'Story', 'Data', 'Team', 'Roadmap', 'Risks', 'Demo', 'Market', 'What if?', 'The ask'];
const FILLS = [C.yellow, C.surface, C.bone, C.aqua, C.surface, C.coral, C.bone, C.violet, C.surface, C.yellow, C.aqua, C.bone];
const SLIDE = '#C9C8C3';

// Slides in a row.
const SW = 640;
const SH = 360;
const DX = 760;
const MX = 5.5 * DX;

type Layout = {
  chips: [number, number][];
  cards: [number, number][];
  card: [number, number];
  poster: [number, number];
  sB: number;
  sC: number;
  oyB: number;
  oyC: number;
};

const WIDE: Layout = {
  chips: [
    [-730, -385], [-340, -425], [60, -395], [440, -430], [770, -335], [-790, 5],
    [800, 35], [-710, 370], [-320, 425], [60, 395], [430, 430], [750, 345],
  ],
  cards: [
    [-880, -900], [0, -940], [880, -900], [1900, -760], [1960, 0], [1900, 760],
    [880, 900], [0, 940], [-880, 900], [-1900, 760], [-1960, 0], [-1900, -760],
  ],
  card: [760, 428],
  poster: [1280, 720],
  sB: 2.0,
  sC: 0.34,
  oyB: -30,
  oyC: 104,
};

const TALL: Layout = {
  chips: [
    [-300, -690], [210, -715], [-110, -560], [320, -560], [-330, -425], [100, -415],
    [-320, 320], [20, 335], [340, 310], [-200, 450], [190, 455], [-60, -820],
  ],
  cards: [
    [-690, -1010], [0, -1040], [690, -1010], [960, -520], [990, 60], [960, 640],
    [690, 1130], [0, 1160], [-690, 1130], [-960, 640], [-990, 60], [-960, -520],
  ],
  card: [600, 338],
  poster: [720, 1280],
  sB: 1.42,
  sC: 0.335,
  oyB: -170,
  oyC: 30,
};

/** Pairs of ideas joined by a line in the opening shot. */
const LINKS: [number, number][] = [
  [0, 1], [1, 2], [2, 3], [3, 4], [4, 6], [6, 11], [11, 10], [10, 9], [9, 8], [8, 7], [7, 5], [5, 0],
  [1, 9], [2, 7], [3, 8], [0, 10], [4, 9], [5, 2], [6, 8], [11, 1],
];

export const Ideas: React.FC = () => {
  const g = useCurrentFrame();
  const {W, H, tall} = useFmt();
  const L = tall ? TALL : WIDE;
  const hook = beat('hook');
  const problem = beat('problem');
  const turn = beat('turn');
  const P = problem.start;
  const T = turn.start;
  const R = turn.end;

  // Moments, as frames in the video.
  const next = [1, 2, 3].map((n) => P + when(problem, 'Next', n));
  const by = P + when(problem, 'By');
  const nobody = P + when(problem, 'nobody');
  const one = T + when(turn, 'One');
  const home = T + when(turn, 'every');
  const see = T + when(turn, 'where') + 4;
  const dive = R - 24;

  // The camera.
  const sweep = tw(g, by, by + 20, 0, 1, E.inOut);
  const toMap = tw(g, T, T + 30, 0, 1, E.inOut);
  const diving = tw(g, dive, R, 0, 1, E.inOut);
  const sDive = W / L.poster[0];
  const logS =
    tw(g, 0, P, 0, Math.log(1.05), (t) => t) +
    tw(g, P, P + 20, 0, Math.log(L.sB / 1.05), E.inOut) +
    toMap * Math.log(L.sC / L.sB) +
    tw(g, T + 30, dive, 0, Math.log(1.05), (t) => t) +
    diving * Math.log(sDive / (L.sC * 1.05));
  const s = Math.exp(logS);
  const offset = tw(g, P, P + 20, 0, L.oyB, E.inOut) + toMap * (L.oyC - L.oyB) - diving * L.oyC;
  const cam: Cam = {
    x: next.reduce((x, n) => x + tw(g, n, n + 8, 0, DX, E.snap), 0) + sweep * 8 * DX + toMap * (MX - 11 * DX),
    y: -offset / s,
    s,
    r: lerp(-1.4, 1.2, tw(g, T + 20, dive, 0, 1, (t) => t)) * toMap * (1 - diving),
  };
  const blur = Math.sin(Math.PI * sweep) * 9;

  // Each idea: chip → slide → card.
  const items = IDEAS.map((label, i) => {
    const toRow = tw(g, P - 2 + i * 0.6, P + 15 + i * 0.6, 0, 1, E.inOut);
    const order = (i * 5) % 12;
    const toCard = tw(g, T + 12 + order * 1.6, T + 38 + order * 1.6, 0, 1, E.inOut);
    const born = pop(g, -14 + ((i * 7) % 12) * 3.4, 11);
    const chipW = (tall ? 128 : 136) + label.length * (tall ? 23 : 25);
    const chipH = tall ? 86 : 92;
    const a = L.chips[i];
    const float = [Math.sin(g / 34 + i * 1.7) * 9, Math.cos(g / 41 + i * 2.3) * 7];
    const b = [i * DX, 0];
    const tilt = (rnd(i + 3) - 0.5) * 9;
    const c = [MX + L.cards[i][0] + (rnd(i) - 0.5) * 70, L.cards[i][1] + (rnd(i + 20) - 0.5) * 60];
    const x = lerp(lerp(a[0] + float[0], b[0], toRow), c[0], toCard);
    const y = lerp(lerp(a[1] + float[1], b[1], toRow), c[1], toCard);
    const w = lerp(lerp(chipW, SW, toRow), L.card[0], toCard);
    const h = lerp(lerp(chipH, SH, toRow), L.card[1], toCard);
    const radius = lerp(lerp(chipH / 2, 6, toRow), 30, toCard);
    const dark = FILLS[i] === C.surface;
    const fill = toCard > 0 ? mix(SLIDE, FILLS[i], toCard) : mix(FILLS[i], SLIDE, toRow);
    const ink = dark ? C.bone : C.ink;
    // Slides you have left behind go dim.
    const current = (cam.x - b[0]) / DX;
    const forgotten = tw(g, nobody, nobody + 20, 0, 1) * (1 - toCard);
    const dim = (current > 0.5 ? 0.45 : 0) * toRow * (1 - toCard) + forgotten * (i === 11 ? 0.25 : 0.5);
    return (
      <div
        key={i}
        style={{
          position: 'absolute',
          left: x - w / 2,
          top: y - h / 2,
          width: w,
          height: h,
          borderRadius: radius,
          backgroundColor: fill,
          transform: `rotate(${lerp((rnd(i + 9) - 0.5) * 7 * (1 - toRow), tilt, toCard)}deg) scale(${born})`,
          opacity: (1 - dim) * Math.min(1, born * 2),
          boxShadow: `0 ${18 * toCard + 6}px ${50 * toCard + 16}px rgba(0,0,0,${0.35 + 0.2 * toCard})`,
          outline: dark ? `${lerp(2, 3, toCard)}px solid ${C.line}` : undefined,
          overflow: 'hidden',
        }}
      >
        {/* The idea, as a chip. */}
        <div
          style={{
            position: 'absolute',
            inset: 0,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontFamily: F.display,
            fontWeight: 700,
            fontSize: tall ? 38 : 40,
            letterSpacing: '-0.02em',
            color: ink,
            opacity: 1 - tw(g, P - 2 + i * 0.6, P + 6 + i * 0.6, 0, 1),
            whiteSpace: 'nowrap',
          }}
        >
          {label}
        </div>
        {/* The same idea, flattened into a slide. */}
        <div style={{position: 'absolute', inset: 0, opacity: toRow * (1 - tw(g, T + 12 + order * 1.6, T + 24 + order * 1.6, 0, 1))}}>
          <div style={{position: 'absolute', left: 44, top: 34, fontFamily: F.text, fontWeight: 600, fontSize: 54, letterSpacing: '-0.03em', color: '#5F5E5A'}}>Slide {i + 1}</div>
          <div style={{position: 'absolute', right: 40, top: 40, width: 74, height: 50, borderRadius: 6, backgroundColor: '#B4B3AE'}} />
          {[0, 1, 2, 3].slice(0, 2 + (i % 3)).map((k) => (
            <React.Fragment key={k}>
              <div style={{position: 'absolute', left: 48, top: 146 + k * 46, width: 12, height: 12, borderRadius: 6, backgroundColor: '#9C9B96'}} />
              <div style={{position: 'absolute', left: 76, top: 144 + k * 46, width: [420, 350, 460, 290, 380][(k * 2 + i) % 5], height: 16, borderRadius: 3, backgroundColor: '#A9A8A3'}} />
            </React.Fragment>
          ))}
        </div>
        {/* And as a card with a place of its own. */}
        <div
          style={{
            position: 'absolute',
            left: L.card[0] * 0.09,
            bottom: L.card[1] * 0.14,
            fontFamily: F.display,
            fontWeight: 800,
            fontSize: L.card[0] * 0.15,
            letterSpacing: '-0.035em',
            color: ink,
            opacity: tw(g, T + 24 + order * 1.6, T + 38 + order * 1.6, 0, 1),
            whiteSpace: 'nowrap',
          }}
        >
          {label}
        </div>
      </div>
    );
  });

  // Lines between the scattered ideas.
  const chipLinks = LINKS.map(([a, b], i) => {
    const p = tw(g, 6 + i * 2.2, 26 + i * 2.2, 0, 1, E.inOut);
    const [x1, y1] = L.chips[a];
    const [x2, y2] = L.chips[b];
    return (
      <line
        key={i}
        x1={x1}
        y1={y1}
        x2={lerp(x1, x2, p)}
        y2={lerp(y1, y2, p)}
        stroke={i % 3 === 0 ? C.yellow : C.muted}
        strokeWidth={i % 3 === 0 ? 3 : 2}
        strokeLinecap="round"
        opacity={(i % 3 === 0 ? 0.75 : 0.4) * (1 - tw(g, P - 6, P + 4, 0, 1))}
      />
    );
  });

  // Lines across the map: a spoke to every card, and a ring joining neighbours.
  const cardAt = (i: number) => [MX + L.cards[i][0] + (rnd(i) - 0.5) * 70, L.cards[i][1] + (rnd(i + 20) - 0.5) * 60];
  const mapLinks = L.cards.flatMap((_, i) => {
    const [x, y] = cardAt(i);
    const [nx, ny] = cardAt((i + 1) % 12);
    const spoke = tw(g, see - 6 + i * 1.5, see + 16 + i * 1.5, 0, 1, E.inOut);
    const ring = tw(g, see + 10 + i * 1.5, see + 30 + i * 1.5, 0, 1, E.inOut);
    return [
      <line key={`s${i}`} x1={MX} y1={0} x2={lerp(MX, x, spoke)} y2={lerp(0, y, spoke)} stroke={C.yellow} strokeWidth={9} strokeLinecap="round" opacity={spoke > 0 ? 0.95 : 0} />,
      <line key={`r${i}`} x1={x} y1={y} x2={lerp(x, nx, ring)} y2={lerp(y, ny, ring)} stroke={C.yellow} strokeWidth={5} strokeDasharray="2 22" strokeLinecap="round" opacity={ring > 0 ? 0.8 : 0} />,
    ];
  });

  // The middle of the map: the app's own welcome canvas.
  const posterIn = pop(g, T + 30, 14);
  const poster = (
    <div
      style={{
        position: 'absolute',
        left: MX - L.poster[0] / 2,
        top: -L.poster[1] / 2,
        width: L.poster[0],
        height: L.poster[1],
        transform: `scale(${posterIn})`,
        opacity: Math.min(1, posterIn * 1.5),
        borderRadius: 34 * (1 - diving),
        overflow: 'hidden',
        boxShadow: `0 0 0 ${5 * (1 - diving)}px ${C.yellow}, 0 40px 120px rgba(0,0,0,${0.6 * (1 - diving)})`,
      }}
    >
      <Img src={staticFile(tall ? 'footage/hero-tall-poster.png' : 'footage/hero-poster.png')} style={{width: '100%', height: '100%'}} />
    </div>
  );

  // Type on the screen, not in the world.
  const w = (b: typeof hook, word: string, nth = 1) => b.start + when(b, word, nth);
  const hookOut = tw(g, P - 8, P + 2, 0, 1, E.in);
  const slideNo = Math.min(12, Math.round(cam.x / DX) + 1);
  const problemUi = tw(g, P + 14, P + 24, 0, 1) * (1 - tw(g, T - 2, T + 8, 0, 1));
  const pressed = next.reduce((v, n) => Math.max(v, tw(g, n - 1, n + 2, 0, 1) * (1 - tw(g, n + 2, n + 9, 0, 1))), 0);
  const slideTop = H / 2 + L.oyB - (SH * L.sB) / 2;
  const slideBottom = H / 2 + L.oyB + (SH * L.sB) / 2;
  const turnOut = tw(g, dive - 8, dive + 2, 0, 1, E.in);

  return (
    <AbsoluteFill>
      <Backdrop cam={cam} />
      <World cam={cam} blur={blur}>
        <svg style={{position: 'absolute', left: 0, top: 0, overflow: 'visible'}} width={1} height={1}>
          {chipLinks}
          {mapLinks}
        </svg>
        {items}
        {poster}
      </World>

      {/* Beat 1: the hook. */}
      <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center', opacity: 1 - hookOut, transform: `translateY(${-hookOut * 40}px)`}}>
        <div style={{position: 'absolute', width: tall ? 1000 : 1300, height: tall ? 620 : 520, background: `radial-gradient(closest-side, ${C.bg} 55%, rgba(15,17,19,0) 100%)`}} />
        <div style={{width: tall ? 900 : 1120, position: 'relative'}}>
          <Rise
            frame={g}
            align="center"
            size={tall ? 104 : 112}
            color={C.bone}
            font={F.display}
            words={[
              {text: 'Your', at: w(hook, 'Your')},
              {text: 'best', at: w(hook, 'best')},
              {text: 'ideas', at: w(hook, 'ideas')},
              {text: '\n', at: 0},
              {text: "don't", at: w(hook, "don't")},
              {text: 'arrive', at: w(hook, 'arrive')},
              {text: 'in', at: w(hook, 'in')},
              {text: 'a', at: w(hook, 'a')},
              {text: '\n', at: 0},
              {text: 'straight', at: w(hook, 'straight'), color: C.yellow},
              {text: 'line.', at: w(hook, 'line'), color: C.yellow},
            ]}
          />
        </div>
      </AbsoluteFill>

      {/* Beat 2: the slide counter and the Next button. */}
      <div
        style={{
          position: 'absolute',
          left: tall ? 76 : 320,
          top: slideTop - (tall ? 84 : 74),
          opacity: problemUi,
          fontFamily: F.text,
          fontWeight: 600,
          fontSize: tall ? 34 : 30,
          color: C.muted,
          fontVariantNumeric: 'tabular-nums',
        }}
      >
        Slide <span style={{color: C.bone}}>{slideNo}</span> of 12
      </div>
      <div
        style={{
          position: 'absolute',
          right: tall ? 76 : 320,
          top: slideBottom + (tall ? 34 : 26),
          opacity: problemUi * (1 - tw(g, by, by + 8, 0, 1)),
          transform: `scale(${1 - pressed * 0.1})`,
          padding: tall ? '20px 40px' : '18px 40px',
          borderRadius: 14,
          border: `2px solid ${mix(C.line, C.bone, pressed)}`,
          backgroundColor: mix(C.surface, '#3A4046', pressed),
          fontFamily: F.text,
          fontWeight: 600,
          fontSize: tall ? 38 : 34,
          color: C.bone,
        }}
      >
        Next ›
      </div>
      <div style={{position: 'absolute', left: tall ? 76 : 320, top: slideBottom + (tall ? 150 : 22), opacity: 1 - tw(g, T - 2, T + 8, 0, 1)}}>
        <Rise
          frame={g}
          size={tall ? 76 : 58}
          color={C.bone}
          font={F.display}
          maxWidth={tall ? 920 : 1000}
          words={[
            {text: 'How', at: nobody + 2},
            {text: 'does', at: nobody + 5},
            {text: 'it', at: nobody + 8},
            {text: 'all', at: nobody + 11},
            {text: 'connect?', at: nobody + 14, color: C.coral},
          ]}
        />
      </div>

      {/* Beat 3: the turn. */}
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: tall ? 250 : 54,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: tall ? 16 : 6,
          opacity: 1 - turnOut,
          transform: `translateY(${-turnOut * 30}px)`,
        }}
      >
        <Rise
          frame={g}
          align="center"
          size={tall ? 108 : 96}
          color={C.bone}
          font={F.display}
          words={[
            {text: 'One', at: one},
            {text: 'infinite', at: T + when(turn, 'infinite'), color: C.yellow},
            {text: 'canvas.', at: T + when(turn, 'canvas'), color: C.yellow},
          ]}
        />
        <Rise
          frame={g}
          align="center"
          size={tall ? 44 : 38}
          weight={500}
          tracking={-0.015}
          color={C.soft}
          font={F.display}
          words={[
            {text: 'Every', at: home},
            {text: 'idea', at: home + 3},
            {text: 'has', at: home + 6},
            {text: 'a', at: home + 8},
            {text: 'home.', at: T + after(turn, 'a', 1)},
          ]}
        />
      </div>
    </AbsoluteFill>
  );
};
