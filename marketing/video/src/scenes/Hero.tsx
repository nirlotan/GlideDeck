import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {Clip, Cut, Retimed, tidy} from '../Footage';
import {C, F} from '../theme';
import {beat, Beat} from '../timeline';
import {E, pop, tw, useFmt} from '../util';
import wide from '../footage/hero.json';
import tallClip from '../footage/hero-tall.json';

/**
 * Beats 4 to 8: the real show, full screen. The footage is retimed so that
 * each camera move starts as its line is spoken.
 */
export const Hero: React.FC = () => {
  const frame = useCurrentFrame();
  const {W, H, tall} = useFmt();
  const clip = (tall ? tallClip : wide) as unknown as Clip;
  const m = clip.marks;
  const reveal = beat('reveal');
  const lines = ['idea1', 'idea2', 'idea3', 'idea4'].map(beat);
  const at = (b: Beat, lead = 0) => b.voStart - reveal.start - lead;

  // When each key is pressed, in scene frames.
  const title = Math.max(10, at(reveal) + 4);
  const highlight = Math.max(title + 84, reveal.voEnd - reveal.start - 6);
  const overview = Math.max(highlight + 16, at(lines[0], 8));
  const detail = Math.max(overview + 34, at(lines[1], 4));
  const portal = Math.max(detail + 64, at(lines[2], 16));
  const deep = portal + 30;
  const back = Math.max(deep + 80, at(lines[3], 2));
  const end = lines[3].end - reveal.start;
  const landed = Math.min(back - 8, deep + 96);

  const cuts: Cut[] = tidy([
    {src: [m.start, m.title], at: [0, title]},
    // The flight to the promise, and its words building in, at full speed.
    {src: [m.title, m.title + 84], at: [title, title + 84]},
    {src: [m.title + 84, m.highlight], at: [title + 84, highlight]},
    {src: [m.highlight, m.highlight + 24], at: [highlight, highlight + 24]},
    {src: [m.highlight + 24, m.overview], at: [highlight + 24, overview]},
    // Back out to the big picture.
    {src: [m.overview, m.overview + 42], at: [overview, overview + 34]},
    {src: [m.overview + 42, m.detail], at: [overview + 34, detail]},
    // Into one slab.
    {src: [m.detail, m.detail + 60], at: [detail, detail + 48]},
    {src: [m.detail + 60, m.portal], at: [detail + 48, portal]},
    // Across to the portal, then all the way down.
    {src: [m.portal, m.deep], at: [portal, deep]},
    {src: [m.deep, m.deep + 108], at: [deep, landed]},
    {src: [m.deep + 108, m.back], at: [landed, back]},
    // And back out.
    {src: [m.back, m.back + 90], at: [back, back + 84]},
    {src: [m.back + 90, m.end], at: [back + 84, Math.max(end, back + 90)]},
  ]);

  // A tag for each line.
  const tags = [
    {text: 'The big picture', from: overview + 6, to: detail - 2},
    {text: 'The detail', from: detail + 8, to: portal - 2},
    {text: '1,000,000× deeper', from: deep + 6, to: back - 2},
    {text: 'Back to the big picture', from: back + 10, to: end - 6},
  ];

  return (
    <AbsoluteFill style={{backgroundColor: C.bg}}>
      <Retimed file={tall ? 'footage/hero-tall.mp4' : 'footage/hero.mp4'} clip={clip} cuts={cuts} style={{position: 'absolute', left: 0, top: 0, width: W, height: H}} />
      {tags.map((t, i) => {
        if (frame < t.from || frame > t.to) return null;
        const p = pop(frame, t.from, 13);
        const out = tw(frame, t.to - 6, t.to, 0, 1, E.in);
        return (
          <div
            key={i}
            style={{
              position: 'absolute',
              left: 0,
              right: 0,
              bottom: tall ? undefined : 22,
              top: tall ? 318 : undefined,
              display: 'flex',
              justifyContent: 'center',
              opacity: 1 - out,
              transform: `translateY(${(1 - p) * 30 + out * 14}px)`,
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: tall ? 20 : 16,
                padding: tall ? '20px 36px 22px 28px' : '10px 26px 12px 20px',
                borderRadius: 999,
                backgroundColor: 'rgba(15,17,19,0.94)',
                boxShadow: `0 0 0 2px ${C.line}, 0 18px 50px rgba(0,0,0,0.5)`,
                fontFamily: F.display,
                fontWeight: 700,
                fontSize: tall ? 56 : 36,
                letterSpacing: '-0.025em',
                color: C.bone,
              }}
            >
              <div style={{width: tall ? 22 : 18, height: tall ? 22 : 18, borderRadius: '50%', backgroundColor: C.yellow}} />
              {t.text}
            </div>
          </div>
        );
      })}
    </AbsoluteFill>
  );
};
