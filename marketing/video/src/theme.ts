import {loadFont as loadDisplay} from '@remotion/google-fonts/BricolageGrotesque';
import {loadFont as loadText} from '@remotion/google-fonts/Inter';

const display = loadDisplay('normal', {weights: ['500', '700', '800'], subsets: ['latin']});
const text = loadText('normal', {weights: ['400', '500', '600'], subsets: ['latin']});

/** GlideDeck's house style, "Darkroom": flat graphite, bone-white type, one signal yellow. */
export const C = {
  bg: '#0F1113',
  surface: '#1D2125',
  line: '#2B3036',
  bone: '#F1EFE9',
  soft: '#DAD8D2',
  muted: '#8F969C',
  yellow: '#FFD23F',
  aqua: '#35D0BA',
  coral: '#FF6B4A',
  violet: '#9DA7FF',
  ink: '#14110A',
};

export const F = {
  display: `${display.fontFamily}, "Segoe UI", sans-serif`,
  text: `${text.fontFamily}, "Segoe UI", sans-serif`,
  /** The wordmark is set the way the app draws it. */
  brand: '"Segoe UI", "Helvetica Neue", sans-serif',
};
