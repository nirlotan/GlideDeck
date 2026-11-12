import React from 'react';
import {AbsoluteFill} from 'remotion';
import {C} from './theme';
import {useFmt} from './util';

export type Cam = {x: number; y: number; s: number; r: number};

/**
 * The canvas: flat graphite with a dot grid, as in the app. The grid belongs
 * to the world, so it pans, turns and zooms with the camera; as the camera
 * zooms, coarser and finer levels of dots cross-fade, so it never runs out.
 */
export const Backdrop: React.FC<{cam: Cam; strength?: number}> = ({cam, strength = 1}) => {
  const {W, H} = useFmt();
  const base = 56;
  // The level whose on-screen spacing is in [48, 96).
  const level = Math.floor(Math.log2(48 / (base * cam.s))) + 1;
  const layers = [level, level + 1].map((k) => {
    const spacing = base * 2 ** k * cam.s;
    const t = (spacing - 48) / 48; // 0 at the fine end, 1 at the coarse end
    const opacity = k === level ? Math.min(1, t * 1.6) : 1;
    const dot = Math.max(1.1, Math.min(2.2, spacing / 34));
    const size = Math.hypot(W, H) + spacing * 2;
    const ox = (((-cam.x * cam.s) % spacing) + spacing) % spacing;
    const oy = (((-cam.y * cam.s) % spacing) + spacing) % spacing;
    return (
      <div
        key={k}
        style={{
          position: 'absolute',
          left: W / 2 - size / 2,
          top: H / 2 - size / 2,
          width: size,
          height: size,
          opacity: opacity * 0.2 * strength,
          transform: `rotate(${cam.r}deg)`,
          backgroundImage: `radial-gradient(circle, ${C.bone} ${dot}px, transparent ${dot + 0.6}px)`,
          backgroundSize: `${spacing}px ${spacing}px`,
          backgroundPosition: `${size / 2 + ox - spacing / 2}px ${size / 2 + oy - spacing / 2}px`,
        }}
      />
    );
  });
  return <AbsoluteFill style={{backgroundColor: C.bg, overflow: 'hidden'}}>{layers}</AbsoluteFill>;
};

/** Places its children in the world the camera looks at. */
export const World: React.FC<{cam: Cam; blur?: number; children: React.ReactNode}> = ({cam, blur = 0, children}) => {
  const {W, H} = useFmt();
  return (
    <AbsoluteFill style={{overflow: 'hidden', filter: blur > 0.3 ? `blur(${blur}px)` : undefined}}>
      <div
        style={{
          position: 'absolute',
          left: 0,
          top: 0,
          width: 0,
          height: 0,
          transformOrigin: '0 0',
          transform: `translate(${W / 2}px, ${H / 2}px) scale(${cam.s}) rotate(${cam.r}deg) translate(${-cam.x}px, ${-cam.y}px)`,
        }}
      >
        {children}
      </div>
    </AbsoluteFill>
  );
};
