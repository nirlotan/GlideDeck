import React from 'react';
import {AbsoluteFill, Img, staticFile, useCurrentFrame} from 'remotion';
import {C, F} from '../theme';
import {after, beat, when} from '../timeline';
import {E, lerp, pop, Rise, tw, useFmt} from '../util';

/** Beat 14: three things GlideDeck does without, then three things it is. */
export const Values: React.FC = () => {
  const frame = useCurrentFrame();
  const {tall} = useFmt();
  const b = beat('values');
  const nos = [
    {text: 'No account.', at: when(b, 'No', 1) - 3, done: after(b, 'account')},
    {text: 'No cloud.', at: when(b, 'No', 2) - 3, done: after(b, 'cloud')},
    {text: 'No subscription.', at: when(b, 'No', 3) - 3, done: after(b, 'subscription')},
  ];
  const free = when(b, 'Free') - 4;
  const yes = [
    {text: 'Free', at: free, rot: -4},
    {text: 'Open source', at: when(b, 'open') - 4, rot: 3},
    {text: '100% offline', at: when(b, 'runs') - 2, rot: -2},
  ];
  // The "no" lines step aside when the good news arrives.
  const aside = tw(frame, free - 8, free + 10, 0, 1, E.inOut);
  const size = tall ? 108 : 118;
  return (
    <AbsoluteFill>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: tall ? lerp(600, 360, aside) : lerp(270, 200, aside),
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          transform: `scale(${lerp(1, tall ? 0.6 : 0.46, aside)})`,
          transformOrigin: 'center top',
          opacity: lerp(1, 0.55, aside),
        }}
      >
        {nos.map((n, i) => {
          const strike = tw(frame, n.done - 6, n.done + 6, 0, 1, E.inOut);
          return (
            <div key={i} style={{position: 'relative', width: 'fit-content', marginBottom: tall ? 26 : 14}}>
              <Rise frame={frame} size={size} color={C.bone} font={F.display} words={n.text.split(' ').map((text, k) => ({text, at: n.at + k * 3}))} />
              <div style={{position: 'absolute', left: -10, top: '54%', height: tall ? 12 : 13, width: `calc(${strike * 100}% + ${strike * 20}px)`, borderRadius: 7, backgroundColor: C.coral, transform: 'rotate(-1.5deg)'}} />
            </div>
          );
        })}
      </div>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: tall ? 690 : 490,
          display: 'flex',
          flexDirection: tall ? 'column' : 'row',
          alignItems: 'center',
          justifyContent: 'center',
          gap: tall ? 36 : 40,
        }}
      >
        {yes.map((y, i) => {
          const p = pop(frame, y.at, 9);
          return (
            <div
              key={i}
              style={{
                padding: tall ? '26px 56px 30px' : '26px 54px 30px',
                borderRadius: 999,
                backgroundColor: i === 0 ? C.yellow : 'transparent',
                boxShadow: `inset 0 0 0 5px ${C.yellow}`,
                color: i === 0 ? C.ink : C.yellow,
                fontFamily: F.display,
                fontWeight: 800,
                fontSize: tall ? 104 : 100,
                letterSpacing: '-0.035em',
                whiteSpace: 'nowrap',
                transform: `rotate(${y.rot}deg) scale(${p})`,
                opacity: Math.min(1, p * 2),
              }}
            >
              {y.text}
            </div>
          );
        })}
      </div>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          textAlign: 'center',
          top: tall ? 1405 : 760,
          fontFamily: F.display,
          fontWeight: 500,
          fontSize: tall ? 46 : 56,
          letterSpacing: '-0.015em',
          color: C.soft,
          opacity: tw(frame, when(b, 'entirely') - 4, when(b, 'entirely') + 8, 0, 1),
          transform: `translateY(${tw(frame, when(b, 'entirely') - 4, when(b, 'entirely') + 10, 20, 0)}px)`,
        }}
      >
        It runs entirely on your computer.
      </div>
    </AbsoluteFill>
  );
};

/** Beat 15: the line, then the wordmark and where to get it. */
export const Close: React.FC = () => {
  const frame = useCurrentFrame();
  const {W, H, tall} = useFmt();
  const b = beat('close');
  const stop = when(b, 'Stop') - 3;
  const start = when(b, 'Start') - 3;
  const brand = when(b, 'GlideDeck') - 8;
  const freeAt = when(b, 'Free') - 4;
  // The line lifts away as the wordmark lands.
  const lift = tw(frame, brand - 4, brand + 12, 0, 1, E.inOut);
  const logo = pop(frame, brand, 12);
  const mark = tall ? 190 : 170;
  return (
    <AbsoluteFill>
      <div
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: tall ? H * 0.34 : H * 0.27,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          opacity: 1 - lift,
          transform: `translateY(${-lift * 120}px) scale(${lerp(1, 0.8, lift)})`,
        }}
      >
        <Rise
          frame={frame}
          align="center"
          size={tall ? 150 : 190}
          color={C.muted}
          font={F.display}
          lineHeight={0.98}
          words={[
            {text: 'Stop', at: stop},
            {text: 'flipping.', at: stop + 4},
          ]}
        />
        <div style={{transform: `translateX(${tw(frame, start, start + 18, -W * 0.18, 0)}px) rotate(${tw(frame, start, start + 18, -4, -2)}deg)`}}>
          <Rise
            frame={frame}
            align="center"
            size={tall ? 150 : 190}
            color={C.yellow}
            font={F.display}
            lineHeight={0.98}
            words={[
              {text: 'Start', at: start},
              {text: 'flying.', at: start + 4},
            ]}
          />
        </div>
      </div>

      <AbsoluteFill style={{alignItems: 'center', justifyContent: 'center', flexDirection: 'column', paddingBottom: tall ? 160 : 0}}>
        <div style={{display: 'flex', flexDirection: tall ? 'column' : 'row', alignItems: 'center', gap: tall ? 30 : 44, transform: `scale(${logo})`, opacity: Math.min(1, logo * 2)}}>
          <Img src={staticFile('icon.png')} style={{width: mark, height: mark}} />
          <div style={{fontFamily: F.brand, fontWeight: 900, fontSize: tall ? 172 : 200, letterSpacing: '-0.035em', color: C.bone, lineHeight: 1}}>GlideDeck</div>
        </div>
        <div
          style={{
            marginTop: tall ? 36 : 30,
            fontFamily: F.display,
            fontWeight: 500,
            fontSize: tall ? 54 : 50,
            letterSpacing: '-0.02em',
            color: C.soft,
            opacity: tw(frame, brand + 10, brand + 22, 0, 1),
          }}
        >
          Presentations that move.
        </div>
        <div
          style={{
            marginTop: tall ? 86 : 66,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            gap: tall ? 30 : 24,
            transform: `scale(${pop(frame, freeAt, 11)})`,
            opacity: tw(frame, freeAt, freeAt + 6, 0, 1),
          }}
        >
          <div
            style={{
              padding: tall ? '24px 58px 28px' : '20px 52px 24px',
              borderRadius: 999,
              backgroundColor: C.yellow,
              color: C.ink,
              fontFamily: F.display,
              fontWeight: 800,
              fontSize: tall ? 60 : 52,
              letterSpacing: '-0.025em',
            }}
          >
            Free on GitHub
          </div>
          <div style={{fontFamily: F.text, fontWeight: 500, fontSize: tall ? 38 : 34, color: C.bone}}>github.com/nirlotan/GlideDeck</div>
          <div style={{fontFamily: F.text, fontWeight: 400, fontSize: tall ? 30 : 26, color: C.muted}}>Windows and macOS · Open source · 100% offline</div>
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};
