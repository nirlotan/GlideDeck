import React from 'react';
import {Composition} from 'remotion';
import {FPS, TOTAL} from './timeline';
import {Video} from './Video';

/** The same story twice: 16:9 for a computer screen and GitHub, 9:16 for a phone. */
export const Root: React.FC = () => (
  <>
    <Composition id="Desktop" component={Video} durationInFrames={TOTAL} fps={FPS} width={1920} height={1080} defaultProps={{width: 1920, height: 1080}} />
    <Composition id="Mobile" component={Video} durationInFrames={TOTAL} fps={FPS} width={1080} height={1920} defaultProps={{width: 1080, height: 1920}} />
  </>
);
