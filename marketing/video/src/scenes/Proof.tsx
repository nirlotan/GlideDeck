import React from 'react';
import {AbsoluteFill, useCurrentFrame} from 'remotion';
import {C, F} from '../theme';
import {beat, when} from '../timeline';
import {pop, Rise, tw, useFmt} from '../util';

/** Beat 9: the Harvard study, in type. Three slabs drift behind it, as on the welcome canvas. */
export const Proof: React.FC = () => {
  const frame = useCurrentFrame();
  const {W, H, tall} = useFmt();
  const b = beat('proof');
  const study = when(b, 'Harvard') - 4;
  const engaging = when(b, 'engaging') - 5;
  const persuasive = when(b, 'persuasive') - 5;
  const than = when(b, 'than') - 2;
  const lead = when(b, 'audiences') - 3;
  const sticker = pop(frame, study, 10);

  const slabs = [
    {x: tall ? 0.78 : 0.84, y: tall ? 0.165 : 0.2, w: tall ? 0.18 : 0.2, fill: C.surface, rot: -5, depth: 0.5, at: 0},
    {x: tall ? 0.42 : 0.93, y: tall ? 0.105 : 0.52, w: tall ? 0.14 : 0.2, fill: C.yellow, rot: 4, depth: 1, at: engaging},
    {x: tall ? 0.16 : 0.8, y: tall ? 0.19 : 0.8, w: tall ? 0.1 : 0.13, fill: C.bone, rot: 3, depth: 0.75, at: persuasive},
  ];
  const unit = tall ? W * 1.9 : W;

  return (
    <AbsoluteFill>
      {slabs.map((s, i) => {
        const p = pop(frame, s.at + 2, 14);
        return (
          <div
            key={i}
            style={{
              position: 'absolute',
              left: s.x * W - (s.w * unit) / 2 - frame * 0.5 * s.depth,
              top: s.y * H - (s.w * unit * 0.63) / 2 + Math.sin(frame / 40 + i) * 10 * s.depth,
              width: s.w * unit,
              height: s.w * unit * 0.63,
              borderRadius: 22,
              backgroundColor: s.fill,
              transform: `rotate(${s.rot + Math.sin(frame / 60 + i * 2) * 1.5}deg) scale(${p})`,
              opacity: Math.min(1, p * 2) * (i === 0 ? 1 : 0.95),
              boxShadow: `0 0 0 ${s.fill === C.surface ? 2 : 0}px ${C.line}, 0 30px 80px rgba(0,0,0,0.5)`,
            }}
          />
        );
      })}
      <div style={{position: 'absolute', left: tall ? 76 : 150, top: tall ? 440 : 170, right: tall ? 60 : 500}}>
        <div
          style={{
            display: 'inline-block',
            padding: tall ? '14px 34px' : '10px 28px',
            borderRadius: 999,
            border: `3px solid ${C.yellow}`,
            color: C.yellow,
            fontFamily: F.display,
            fontWeight: 700,
            fontSize: tall ? 40 : 34,
            letterSpacing: '0.01em',
            transform: `rotate(-4deg) scale(${sticker})`,
            transformOrigin: 'left center',
            opacity: Math.min(1, sticker * 2),
            marginBottom: tall ? 44 : 34,
          }}
        >
          Harvard study
        </div>
        <Rise
          frame={frame}
          size={tall ? 58 : 56}
          weight={500}
          tracking={-0.02}
          color={C.soft}
          font={F.display}
          words={[
            {text: 'Audiences', at: lead},
            {text: 'rated', at: lead + 3},
            {text: 'zooming', at: when(b, 'zooming') - 2},
            {text: 'presentations', at: when(b, 'presentations') - 2},
          ]}
        />
        <div style={{height: tall ? 26 : 18}} />
        <Rise
          frame={frame}
          size={tall ? 132 : 150}
          color={C.bone}
          font={F.display}
          lineHeight={1}
          words={[
            {text: 'More', at: engaging},
            {text: 'engaging.', at: engaging + 3},
            {text: '\n', at: 0},
            {text: 'More', at: persuasive, color: C.yellow},
            {text: 'persuasive.', at: persuasive + 3, color: C.yellow},
          ]}
        />
        <div style={{height: tall ? 34 : 26}} />
        <Rise
          frame={frame}
          size={tall ? 60 : 56}
          weight={500}
          tracking={-0.02}
          color={C.soft}
          font={F.display}
          words={[
            {text: 'than', at: than},
            {text: 'slides.', at: than + 3},
          ]}
        />
      </div>
      <div
        style={{
          position: 'absolute',
          left: tall ? 76 : 150,
          right: tall ? 76 : 150,
          top: tall ? 1340 : undefined,
          bottom: tall ? undefined : 62,
          fontFamily: F.text,
          fontWeight: 400,
          fontSize: tall ? 25 : 22,
          lineHeight: 1.4,
          color: C.muted,
          opacity: tw(frame, study + 10, study + 24, 0, 1),
        }}
      >
        Moulton, Türkay &amp; Kosslyn, PLOS ONE (2017). A randomized study at Harvard{tall ? <br /> : ' '}comparing zooming (Prezi), slide and spoken presentations.
      </div>
    </AbsoluteFill>
  );
};
