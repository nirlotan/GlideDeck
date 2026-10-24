// GlideDeck HTML player. Plays an exported presentation exactly like the
// app's show: this is a straight port of the app's camera maths
// (glide-core/camera.rs), animations (glidedeck/anim.rs) and show rules
// (glidedeck/present.rs). Keep the formulas in sync with those files.
(() => {
  'use strict';
  const deck = JSON.parse(document.getElementById('deck').textContent);
  const canvas = document.getElementById('stage');
  const { PI, sin, cos, abs, min, max, exp, log, pow, sqrt, round, floor, ceil } = Math;
  const TAU = PI * 2;
  const clamp = (v, lo, hi) => (v < lo ? lo : v > hi ? hi : v);
  const clamp01 = (v) => clamp(v, 0, 1);

  // --- colours -------------------------------------------------------------------------
  const colorCache = new Map();
  function rgba(hex) {
    if (hex == null) return null;
    let c = colorCache.get(hex);
    if (!c) {
      const h = hex.replace('#', '');
      c = [0, 2, 4, 6].map((i) => parseInt(h.slice(i, i + 2) || 'ff', 16));
      colorCache.set(hex, c);
    }
    return c;
  }
  const css = (c, k = 1) => `rgba(${c[0]},${c[1]},${c[2]},${(c[3] / 255) * k})`;
  const mixColor = (a, b, t) => a.map((x, i) => round(x + (b[i] - x) * clamp01(t)));
  const withAlpha = (c, k) => [c[0], c[1], c[2], round(c[3] * clamp01(k))];
  const accent = rgba(deck.accent);
  const background = rgba(deck.bg);
  const backgroundTo = rgba(deck.bgTo);

  // --- easing (glide-core/easing.rs) ----------------------------------------------------
  const EASE = {
    linear: (t) => t,
    ease_in_out_sine: (t) => -(cos(PI * t) - 1) / 2,
    ease_in_out_cubic: (t) => (t < 0.5 ? 4 * t * t * t : 1 - pow(-2 * t + 2, 3) / 2),
    ease_in_out_quint: (t) => (t < 0.5 ? 16 * pow(t, 5) : 1 - pow(-2 * t + 2, 5) / 2),
    ease_out_expo: (t) => (t >= 1 ? 1 : 1 - pow(2, -10 * t)),
  };
  const ease = (name, t) => (EASE[name] || EASE.ease_in_out_cubic)(clamp01(t));

  // --- camera (glide-core/camera.rs) ----------------------------------------------------
  const MIN_ZOOM = 1e-9, MAX_ZOOM = 1e9, RHO = 1.25;
  function wrapAngle(a) {
    let r = (((a + PI) % TAU) + TAU) % TAU - PI;
    return r <= -PI ? r + TAU : r;
  }
  function forFrame(el, vp, margin = 0) {
    const sx = el.w * abs(el.s), sy = el.h * abs(el.s);
    if (sx <= 0 || sy <= 0) return null;
    const ax = max(vp.w - 2 * margin, 1), ay = max(vp.h - 2 * margin, 1);
    return { x: el.x, y: el.y, zoom: clamp(min(ax / sx, ay / sy), MIN_ZOOM, MAX_ZOOM), rot: wrapAngle((el.r * PI) / 180) };
  }
  // World → screen as a canvas matrix [a, b, c, d, e, f].
  function viewMatrix(cam, vp) {
    const cr = cos(cam.rot), sr = sin(cam.rot), z = cam.zoom;
    const a = z * cr, b = -z * sr, c = z * sr, d = z * cr;
    return [a, b, c, d, vp.w / 2 - (a * cam.x + c * cam.y), vp.h / 2 - (b * cam.x + d * cam.y)];
  }
  function modelMatrix(t) {
    const th = (t.r * PI) / 180, s = t.s, cs = cos(th) * s, sn = sin(th) * s;
    return [cs, sn, -sn, cs, t.x, t.y];
  }
  const mul = (m, n) => [
    m[0] * n[0] + m[2] * n[1], m[1] * n[0] + m[3] * n[1],
    m[0] * n[2] + m[2] * n[3], m[1] * n[2] + m[3] * n[3],
    m[0] * n[4] + m[2] * n[5] + m[4], m[1] * n[4] + m[3] * n[5] + m[5],
  ];
  function screenToWorld(cam, px, py, vp) {
    const dx = (px - vp.w / 2) / cam.zoom, dy = (py - vp.h / 2) / cam.zoom;
    const cr = cos(cam.rot), sr = sin(cam.rot);
    return [cam.x + cr * dx - sr * dy, cam.y + sr * dx + cr * dy];
  }
  function zoomAbout(cam, px, py, factor, vp) {
    const before = screenToWorld(cam, px, py, vp);
    cam.zoom = clamp(cam.zoom * factor, MIN_ZOOM, MAX_ZOOM);
    const after = screenToWorld(cam, px, py, vp);
    cam.x += before[0] - after[0];
    cam.y += before[1] - after[1];
  }
  function visibleAabb(cam, vp) {
    const pts = [[0, 0], [vp.w, 0], [vp.w, vp.h], [0, vp.h]].map((p) => screenToWorld(cam, p[0], p[1], vp));
    const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
    return [min(...xs), min(...ys), max(...xs), max(...ys)];
  }
  const camEq = (a, b) => a && b && a.x === b.x && a.y === b.y && a.zoom === b.zoom && a.rot === b.rot;

  function makeTween(from, to, duration, easing, style, vp) {
    const tw = { from: { ...from }, to: { ...to }, dur: max(duration, 1e-6), ease: easing, style, elapsed: 0, extent: max(vp.w, vp.h, 1) };
    if (style === 'cut') tw.dur = 1e-6;
    return tw;
  }
  const finished = (tw) => tw.elapsed >= tw.dur;
  function sampleDirect(tw, u) {
    const a = tw.from, b = tw.to;
    const zoom = exp(log(a.zoom) + (log(b.zoom) - log(a.zoom)) * u);
    const ia = 1 / a.zoom, ib = 1 / b.zoom;
    const w = abs(ib - ia) > 1e-12 * max(ia, ib) ? (1 / zoom - ia) / (ib - ia) : u;
    return { x: a.x + (b.x - a.x) * w, y: a.y + (b.y - a.y) * w, zoom, rot: a.rot + wrapAngle(b.rot - a.rot) * u };
  }
  function sampleSmooth(tw, u) {
    const a = tw.from, b = tw.to;
    const w0 = tw.extent / a.zoom, w1 = tw.extent / b.zoom;
    const dx = b.x - a.x, dy = b.y - a.y, dist = sqrt(dx * dx + dy * dy);
    const rot = a.rot + wrapAngle(b.rot - a.rot) * u;
    const rho2 = RHO * RHO;
    if (dist < 1e-9 * max(w0, w1)) {
      const w = w0 * pow(w1 / w0, u);
      return { x: a.x + dx * u, y: a.y + dy * u, zoom: tw.extent / w, rot };
    }
    const b0 = (w1 * w1 - w0 * w0 + rho2 * rho2 * dist * dist) / (2 * w0 * rho2 * dist);
    const b1 = (w1 * w1 - w0 * w0 - rho2 * rho2 * dist * dist) / (2 * w1 * rho2 * dist);
    const r0 = -Math.asinh(b0), r1 = -Math.asinh(b1);
    const s = (u * (r1 - r0)) / RHO;
    const along = (w0 / rho2) * (Math.cosh(r0) * Math.tanh(RHO * s + r0) - Math.sinh(r0));
    const w = (w0 * Math.cosh(r0)) / Math.cosh(RHO * s + r0);
    return { x: a.x + dx * (along / dist), y: a.y + dy * (along / dist), zoom: clamp(tw.extent / w, MIN_ZOOM, MAX_ZOOM), rot };
  }
  function sampleTween(tw, t) {
    const u = ease(tw.ease, t);
    if (u >= 1) return { ...tw.to };
    switch (tw.style) {
      case 'cut': return { ...tw.to };
      case 'smooth': return sampleSmooth(tw, u);
      case 'swoop': {
        const c = sampleDirect(tw, u), dx = tw.to.x - tw.from.x, dy = tw.to.y - tw.from.y, bow = sin(PI * u);
        c.x += -dy * 0.2 * bow; c.y += dx * 0.2 * bow; c.rot += 0.12 * bow;
        return c;
      }
      case 'spin': { const c = sampleSmooth(tw, u); c.rot += TAU * u; return c; }
      case 'fade': return t < 0.5 ? { ...tw.from } : { ...tw.to };
      case 'punch': {
        const c = sampleDirect(tw, u);
        c.zoom = clamp(c.zoom * (1 + 0.09 * sin(PI * t) * t), MIN_ZOOM, MAX_ZOOM);
        return c;
      }
      default: return sampleDirect(tw, u);
    }
  }
  function fadeAmount(tw) {
    if (!tw || tw.style !== 'fade' || finished(tw)) return 0;
    const t = clamp01(tw.elapsed / tw.dur);
    return 1 - abs(2 * t - 1);
  }

  // --- builds (glidedeck/anim.rs, BuildClock) --------------------------------------------
  const DONE = -Infinity;
  const els = deck.elements;
  const path = deck.path;
  function clickGroups(builds) {
    const groups = [];
    builds.forEach((b, i) => {
      const last = groups[groups.length - 1];
      if (last && b.trigger !== 'on_click') last[1] = i + 1;
      else groups.push([i, i + 1]);
    });
    return groups;
  }
  const buildsOf = (step) => (path[step] ? path[step].builds : []);
  const freshClock = (step) => ({ step, starts: buildsOf(step).map(() => null) });
  const finishedClock = (step) => ({ step, starts: buildsOf(step).map(() => DONE) });
  const nextGroup = (c) => clickGroups(buildsOf(c.step)).find((g) => c.starts[g[0]] === null);
  function nextIsAutomatic(c) {
    const g = nextGroup(c);
    return !!g && buildsOf(c.step)[g[0]].trigger !== 'on_click';
  }
  function startGroup(c, g, now) {
    const builds = buildsOf(c.step);
    let prevStart = now, prevEnd = now;
    for (let i = g[0]; i < g[1]; i++) {
      const b = builds[i];
      const base = b.trigger === 'with_previous' ? prevStart : b.trigger === 'after_previous' ? prevEnd : now;
      const start = base + max(b.delay, 0);
      c.starts[i] = start;
      prevStart = start;
      prevEnd = start + max(b.dur, 0);
    }
  }
  const running = (c, now) => buildsOf(c.step).some((b, i) => c.starts[i] !== null && now < c.starts[i] + b.dur);
  function finishRunning(c) {
    c.starts = c.starts.map((s) => (s === null ? null : DONE));
  }
  function lastStartedGroup(c) {
    const gs = clickGroups(buildsOf(c.step));
    for (let k = gs.length - 1; k >= 0; k--) if (c.starts[gs[k][0]] !== null) return gs[k];
    return null;
  }

  // Easing for effects (anim.rs).
  const easeOutCubic = (t) => 1 - pow(1 - t, 3);
  const easeInCubic = (t) => t * t * t;
  const easeInOut = (t) => (t < 0.5 ? 4 * t * t * t : 1 - pow(-2 * t + 2, 3) / 2);
  const easeOutBack = (t) => 1 + 2.70158 * pow(t - 1, 3) + 1.70158 * pow(t - 1, 2);
  function easeOutBounce(t) {
    const n = 7.5625, d = 2.75;
    if (t < 1 / d) return n * t * t;
    if (t < 2 / d) { t -= 1.5 / d; return n * t * t + 0.75; }
    if (t < 2.5 / d) { t -= 2.25 / d; return n * t * t + 0.9375; }
    t -= 2.625 / d;
    return n * t * t + 0.984375;
  }
  const DIRS = { left: [-1, 0], right: [1, 0], up: [0, -1], down: [0, 1] };
  const anyBuilds = path.some((s) => s.builds.length);
  const anyParallax = els.some((e) => e.px);

  // The scene: what every object looks like at `now` (anim.rs `scene`).
  function scene(clock, now) {
    const items = els.map((el, i) => ({ el, i, id: el.id, hidden: !!el.hidden, o: el.o == null ? 1 : el.o, x: el.x, y: el.y, s: el.s, r: el.r }));
    if (!anyBuilds) return items;
    const mine = new Map();
    path.forEach((step, si) => step.builds.forEach((b, bi) => {
      let phase;
      if (si < clock.step) phase = 1;
      else if (si > clock.step) phase = null;
      else {
        const t0 = clock.starts[bi];
        const d = max(b.dur, 1e-6);
        if (t0 === null || t0 === undefined || now < t0) phase = null;
        else if (now >= t0 + d) phase = 1;
        else phase = { p: clamp01((now - t0) / d) };
      }
      if (!mine.has(b.el)) mine.set(b.el, []);
      mine.get(b.el).push({ si, b, phase });
    }));
    const frameOf = (si) => {
      const f = els[path[si].frame];
      return { t: f, size: [f.w * abs(f.s), f.h * abs(f.s)] };
    };
    const behind = new Map();
    let spot = null;
    const toFront = new Set();
    for (const it of items) {
      const list = mine.get(it.i);
      if (!list) continue;
      let visible = list[0].b.kind !== 'appear';
      const effects = [];
      for (const { si, b, phase } of list) {
        if (phase === 1 && b.kind === 'appear') visible = true;
        else if (phase === 1 && b.kind === 'disappear') visible = false;
        else if (phase && phase.p !== undefined) { visible = true; effects.push({ si, b, p: phase.p }); }
        else if (phase === 1 && b.kind === 'emphasis' && si === clock.step && ['spotlight', 'highlight', 'underline', 'color_pop'].includes(b.fx)) effects.push({ si, b, p: 1 });
      }
      if (!visible) { it.hidden = true; continue; }
      const el = it.el;
      const local = [el.w, el.h];
      const size = [local[0] * abs(it.s), local[1] * abs(it.s)];
      for (const { si, b, p } of effects) {
        const { t: ft, size: fsize } = frameOf(si);
        const fa = (ft.r * PI) / 180, tc = cos(fa), ts = sin(fa);
        const rot = (x, y) => [tc * x - ts * y, ts * x + tc * y];
        const down = rot(0, 1);
        const q = easeOutCubic(p);
        switch (b.fx) {
          case 'fade': it.o *= easeInOut(p); break;
          case 'rise': it.o *= q; it.x += down[0] * (1 - q) * fsize[1] * 0.08; it.y += down[1] * (1 - q) * fsize[1] * 0.08; break;
          case 'focus': it.o *= q; it.s *= 1 + 0.18 * (1 - q); break;
          case 'zoom': it.o *= q; it.s *= 0.3 + 0.7 * q; break;
          case 'pop': it.o *= min(p * 3, 1); it.s *= max(easeOutBack(p), 0); it.r += (1 - q) * -10; break;
          case 'drop': { it.o *= min(p * 4, 1); const k = (1 - easeOutBounce(p)) * fsize[1] * 0.55; it.x -= down[0] * k; it.y -= down[1] * k; break; }
          case 'fly': { const d = DIRS[b.dir] || DIRS.left; it.o *= min(p * 2, 1); const v = rot(d[0] * fsize[0], d[1] * fsize[1]); it.x += v[0] * 0.75 * (1 - q); it.y += v[1] * 0.75 * (1 - q); break; }
          case 'spin': it.o *= q; it.s *= 0.2 + 0.8 * q; it.r += (1 - q) * -200; break;
          case 'typewriter':
            if (el.k === 'text' && el.glyphs) {
              const shown = round(p * el.chars.length);
              if (shown < el.chars.length) { const cut = el.chars[shown]; it.glyphAlpha = (s) => (s !== null && s >= cut ? 0 : 1); }
            } else if (el.k !== 'text') it.o *= easeInOut(p);
            break;
          case 'words':
            if (el.k === 'text' && el.glyphs) {
              const n = max(el.words.length, 1);
              const alphas = el.words.map((w, k) => clamp01((p * (n + 1) - k) / 2));
              it.glyphAlpha = (s) => {
                if (s === null) return 1;
                for (let k = 0; k < el.words.length; k++) if (s >= el.words[k][0] && s < el.words[k][1]) return alphas[k];
                return 1;
              };
            } else if (el.k !== 'text') it.o *= easeInOut(p);
            break;
          case 'count_up':
            if (el.k === 'text' && el.count) {
              if (p < 1) it.countFrame = el.count[min(el.count.length - 1, floor(p * el.count.length))];
              it.o *= min(p * 4, 1);
            } else if (el.k !== 'text') it.o *= easeInOut(p);
            break;
          case 'pulse': it.s *= 1 + 0.14 * sin(PI * p); break;
          case 'wiggle': it.r += 7 * sin(6 * PI * p) * (1 - p); break;
          case 'glow': {
            const k = sin(PI * p);
            if (el.k === 'text' || el.k === 'icon') it.baseMix = k;
            else it.shadow = { c: withAlpha(accent, 0.85 * k), b: (min(size[0], size[1]) * 0.25) / max(abs(it.s), 1e-12), x: 0, y: 0 };
            break;
          }
          case 'color_pop':
            if (el.k === 'text') { it.baseMix = q; it.spanMix = q; }
            else if (el.k === 'icon' || el.k === 'rect' || el.k === 'ellipse') it.baseMix = q;
            else it.s *= 1 + 0.06 * q;
            break;
          case 'spotlight': it.s *= 1 + 0.06 * q; spot = { k: q, ft, fsize }; toFront.add(it.id); break;
          case 'highlight':
          case 'underline': {
            const underline = b.fx === 'underline';
            let lines = el.lines && el.lines.length ? el.lines : [[0, 0, local[0], local[1]]];
            const k = abs(it.s), er = (it.r * PI) / 180, rc = cos(er), rs = sin(er);
            const total = lines.reduce((acc, l) => acc + max(l[2] - l[0], 0), 0);
            let budget = total * q;
            const marks = behind.get(it.id) || [];
            lines.forEach((l, n) => {
              if (budget <= 0 && n > 0) return;
              const lw = max(l[2] - l[0], 0), lh = max(l[3] - l[1], 1e-9), pad = lh * 0.03;
              const w = min(budget, lw + 2 * pad);
              budget -= w;
              if (w <= 0) { budget = 0; return; }
              const y0 = underline ? l[1] + lh * 0.88 : l[1] + lh * 0.42, h = underline ? max(lh * 0.07, 1e-9) : lh * 0.5;
              const lx = (l[0] - pad + w / 2 - local[0] / 2) * k, ly = (y0 + h / 2 - local[1] / 2) * k;
              marks.push({
                mark: true, id: `${it.id}~${underline ? 'underline' : 'highlight'}${n}`, o: 1, hidden: false,
                x: it.x + rc * lx - rs * ly, y: it.y + rs * lx + rc * ly, s: 1, r: it.r,
                el: { k: 'rect', w: w * k, h: h * k, rad: h * k * 0.35, fill: null, fillC: underline ? accent : withAlpha(accent, 0.38) },
              });
            });
            behind.set(it.id, marks);
            break;
          }
          case 'fade_out': it.o *= 1 - easeInOut(p); break;
          case 'sink': it.o *= 1 - q; it.x += down[0] * q * fsize[1] * 0.08; it.y += down[1] * q * fsize[1] * 0.08; break;
          case 'zoom_out': it.o *= 1 - q; it.s *= 1 + 0.35 * q; break;
          case 'fly_out': { const d = DIRS[b.dir] || DIRS.right; const k = easeInCubic(p); it.o *= 1 - pow(p, 3); const v = rot(d[0] * fsize[0], d[1] * fsize[1]); it.x += v[0] * 0.75 * k; it.y += v[1] * 0.75 * k; break; }
          case 'shrink': it.o *= 1 - p * p; it.s *= max(1 - easeInCubic(p) * 1.05 + 0.05 * sin(PI * p), 0); break;
        }
      }
    }
    let out = items;
    if (behind.size) {
      out = [];
      for (const it of items) {
        for (const m of behind.get(it.id) || []) out.push(m);
        out.push(it);
      }
    }
    if (spot) {
      const front = out.filter((it) => toFront.has(it.id) || [...toFront].some((id) => it.id.startsWith(`${id}~`)));
      out = out.filter((it) => !front.includes(it));
      out.push({
        mark: true, id: '~spotlight', o: 1, hidden: false, x: spot.ft.x, y: spot.ft.y, s: 1, r: spot.ft.r,
        el: { k: 'rect', w: spot.fsize[0] * 12, h: spot.fsize[1] * 12, rad: 0, fill: null, fillC: [0, 0, 0, floor(175 * spot.k)] },
      });
      out.push(...front);
    }
    return out;
  }

  // Parallax (anim.rs `apply_parallax`).
  const slideWidths = path.map((s) => els[s.frame]).map((f) => f.w * abs(f.s)).sort((a, b) => a - b);
  const medianSlideW = slideWidths[floor(slideWidths.length / 2)];
  function applyParallax(items, cam, vp) {
    if (!anyParallax || vp.w <= 0 || !medianSlideW) return;
    const ratio = (medianSlideW * cam.zoom) / vp.w;
    const t = clamp01((ratio - 0.25) / (0.8 - 0.25));
    const strength = t * t * (3 - 2 * t);
    if (strength <= 0) return;
    for (const it of items) {
      const px = it.el.px;
      if (px) {
        const depth = clamp(px, -2, 1) * strength;
        it.x += (cam.x - it.x) * depth;
        it.y += (cam.y - it.y) * depth;
      }
    }
  }

  // --- media -------------------------------------------------------------------------------
  const images = new Map();
  const svgRasters = new Map();
  const videos = new Map();
  let needsFrame = true;
  function imageFor(key) {
    if (!key) return null;
    let img = images.get(key);
    if (!img) {
      img = new Image();
      img.onload = () => { needsFrame = true; };
      img.src = deck.media[key];
      images.set(key, img);
    }
    return img.complete && img.naturalWidth ? img : null;
  }
  // SVGs are re-rasterised whenever their on-screen size crosses a power of two.
  function svgRaster(el, px) {
    const img = imageFor(el.src);
    if (!img) return null;
    const level = clamp(pow(2, ceil(Math.log2(max(px, 1)))), 64, 4096);
    const key = `${el.id}@${level}`;
    let c = svgRasters.get(key);
    if (!c) {
      c = document.createElement('canvas');
      const aspect = el.h > 0 ? el.w / el.h : 1;
      c.width = aspect >= 1 ? level : max(1, round(level * aspect));
      c.height = aspect >= 1 ? max(1, round(level / aspect)) : level;
      c.getContext('2d').drawImage(img, 0, 0, c.width, c.height);
      svgRasters.set(key, c);
    }
    return c;
  }
  function videoFor(el) {
    if (!el.src) return null;
    let v = videos.get(el.id);
    if (!v) {
      v = document.createElement('video');
      v.src = el.src;
      v.preload = 'auto';
      v.playsInline = true;
      v.loop = !!el.loop;
      v.muted = !!el.muted;
      v.addEventListener('loadeddata', () => { needsFrame = true; });
      videos.set(el.id, v);
    }
    return v;
  }
  const anyVideoPlaying = () => [...videos.values()].some((v) => !v.paused && !v.ended);
  function autoplayStepVideos(step) {
    const f = els[path[step].frame];
    const fa = (-f.r * PI) / 180, s = abs(f.s) || 1;
    els.forEach((el) => {
      if ((el.k !== 'video' && el.k !== 'embed') || el.hidden) return;
      const dx = el.x - f.x, dy = el.y - f.y;
      const lx = (cos(fa) * dx - sin(fa) * dy) / s, ly = (sin(fa) * dx + cos(fa) * dy) / s;
      const inside = abs(lx) <= f.w / 2 && abs(ly) <= f.h / 2;
      if (el.k === 'embed') {
        if (inside && el.autoplay && !opensOutside(el)) playEmbed(el);
        else if (!inside) stopEmbed(el);
        return;
      }
      const v = videoFor(el);
      if (!v) return;
      if (inside && el.autoplay) v.play().catch(() => {});
      else v.pause();
    });
  }

  // --- web videos (YouTube, Vimeo, Loom, video links) ---------------------------------------
  // A site's player is a page from that site, which can't be drawn into the
  // canvas: the poster draws like an image, and a layer that follows the
  // camera holds a play badge and, once played, the site's own player.
  const embedLayer = document.createElement('div');
  embedLayer.id = 'embeds';
  canvas.after(embedLayer);
  const embeds = new Map();
  const PLAY_SVG = '<svg viewBox="0 0 64 64" aria-hidden="true"><circle cx="32" cy="32" r="32" fill="rgba(0,0,0,.55)"/><path d="M26 19 L47 32 L26 45 Z" fill="#fff"/></svg>';
  function embedFor(el) {
    let e = embeds.get(el.id);
    if (!e) {
      const box = document.createElement('div');
      box.className = 'embed';
      const badge = document.createElement('div');
      badge.className = 'play';
      badge.innerHTML = PLAY_SVG;
      box.appendChild(badge);
      embedLayer.appendChild(box);
      e = { box, badge, node: null, size: '' };
      embeds.set(el.id, e);
    }
    return e;
  }
  // YouTube refuses to play embeds on pages opened as local files (they send
  // no referrer), so there the video opens on YouTube instead.
  const opensOutside = (el) => el.site === 'youtube' && location.protocol === 'file:';
  function playEmbed(el) {
    const e = embedFor(el);
    if (e.node) return;
    if (opensOutside(el)) { window.open(el.url, '_blank', 'noopener'); return; }
    let node;
    if (el.player) {
      node = document.createElement('iframe');
      node.src = el.player;
      node.allow = 'autoplay; fullscreen; picture-in-picture; encrypted-media';
      node.referrerPolicy = 'strict-origin-when-cross-origin';
      node.title = el.title || 'Video';
    } else {
      node = document.createElement('video');
      node.src = el.media;
      node.controls = true;
      node.autoplay = true;
      node.playsInline = true;
      node.loop = !!el.loop;
      node.muted = !!el.muted;
      // Keys stay with the show.
      node.addEventListener('focus', () => canvas.focus({ preventScroll: true }));
    }
    e.node = node;
    e.box.appendChild(node);
    e.badge.style.display = 'none';
  }
  function stopEmbed(el) {
    const e = embeds.get(el.id);
    if (!e || !e.node) return;
    e.node.remove();
    e.node = null;
    e.badge.style.display = '';
  }
  // Places each web video's layer over its element. The layer is sized in
  // screen pixels (so a site's player lays out its controls at their normal
  // size) and the matrix keeps only the rotation and position.
  function layoutEmbeds(items, vm, vis, fade) {
    const shown = new Set();
    if (!show.blank) {
      for (const it of items) {
        if (it.el.k !== 'embed') continue;
        const G = itemGeometry(it, vm, 1, vis, show.cam.zoom);
        if (!G) continue;
        const e = embedFor(it.el), m = G.m, k = Math.hypot(m[0], m[1]) || 1;
        const w = G.w * k, h = G.h * k;
        const size = `${round(w)}x${round(h)}`;
        if (size !== e.size) {
          e.size = size;
          e.box.style.width = `${w}px`;
          e.box.style.height = `${h}px`;
          e.box.style.borderRadius = it.el.rad > 0 ? `${it.el.rad * k}px` : '';
        }
        e.box.style.transform = `matrix(${m[0] / k},${m[1] / k},${m[2] / k},${m[3] / k},${m[4]},${m[5]}) translate(${-w / 2}px,${-h / 2}px)`;
        e.box.style.opacity = String(clamp01(it.o) * (1 - fade));
        e.box.style.display = 'block';
        shown.add(it.el.id);
      }
    }
    for (const [id, e] of embeds) if (!shown.has(id)) e.box.style.display = 'none';
  }
  // Clicking a site's player gives it the keyboard; take it back so the
  // arrow keys keep driving the show.
  canvas.tabIndex = -1;
  window.addEventListener('blur', () => setTimeout(() => {
    const a = document.activeElement;
    if (a && a.tagName === 'IFRAME' && embedLayer.contains(a)) canvas.focus({ preventScroll: true });
  }, 0));

  // --- drawing -------------------------------------------------------------------------------
  // Two renderers share the same scene: WebGL2, which matches the app pixel for
  // pixel (linear-light blending into a 4× MSAA sRGB target, the app's shaders
  // ported), and a 2D-canvas fallback for browsers without WebGL2.
  const MIN_VISIBLE_PX = 0.35, TOLERANCE_PX = 0.2;

  // Culling, as glide-render does it: off-screen, or too small to see.
  function itemGeometry(it, vm, dpr, vis, zoom) {
    const el = it.el;
    if (it.hidden || it.o <= 0) return null;
    if (el.k === 'frame' && el.invisible) return null;
    const w = it.countFrame ? it.countFrame.w : el.w, h = it.countFrame ? it.countFrame.h : el.h;
    const mm = modelMatrix(it);
    const xs = [], ys = [];
    for (const [lx, ly] of [[-w / 2, -h / 2], [w / 2, -h / 2], [w / 2, h / 2], [-w / 2, h / 2]]) {
      xs.push(mm[0] * lx + mm[2] * ly + mm[4]);
      ys.push(mm[1] * lx + mm[3] * ly + mm[5]);
    }
    if (max(...xs) < vis[0] || min(...xs) > vis[2] || max(...ys) < vis[1] || min(...ys) > vis[3]) return null;
    const pxPerLocal = zoom * dpr * abs(it.s);
    if (max(w, h) * pxPerLocal < MIN_VISIBLE_PX) return null;
    const m0 = mul(vm, mm);
    return { w, h, pxPerLocal, m: [m0[0] * dpr, m0[1] * dpr, m0[2] * dpr, m0[3] * dpr, m0[4] * dpr, m0[5] * dpr] };
  }
  function textColor(it, base, c) {
    let col = c ? rgba(c) : base;
    if (!c && it.baseMix) col = mixColor(col, accent, it.baseMix);
    if (c && it.spanMix) col = mixColor(col, accent, it.spanMix);
    return col;
  }

  // ===== WebGL2 ================================================================================
  function makeGL() {
    const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, stencil: false, premultipliedAlpha: false, preserveDrawingBuffer: true });
    if (!gl) return null;
    const HEAD = `#version 300 es
precision highp float;
vec3 s2l(vec3 c) { return mix(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, step(c, vec3(0.04045))); }
vec3 l2s(vec3 c) { return mix(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, step(c, vec3(0.0031308))); }
float rbox(vec2 p, vec2 h, float r) { vec2 q = abs(p) - h + vec2(r); return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r; }
`;
    function program(vs, fs) {
      const p = gl.createProgram();
      for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
        const s = gl.createShader(type);
        gl.shaderSource(s, HEAD + src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
        gl.attachShader(p, s);
      }
      gl.bindAttribLocation(p, 0, 'pos');
      gl.linkProgram(p);
      if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
      const locs = {};
      return { p, u: (n) => (n in locs ? locs[n] : (locs[n] = gl.getUniformLocation(p, n))) };
    }
    let P;
    try {
      P = {
        // Fills: stencil (non-zero winding), then cover with the colour.
        stencil: program(`in vec2 pos; uniform mat3 M; void main() { gl_Position = vec4((M * vec3(pos, 1.0)).xy, 0.0, 1.0); }`,
          `out vec4 o; void main() { o = vec4(0.0); }`),
        cover: program(`in vec2 pos; uniform mat3 M; out vec2 lp; void main() { lp = pos; gl_Position = vec4((M * vec3(pos, 1.0)).xy, 0.0, 1.0); }`,
          // g: gradient direction, extent and mode: 0 solid; 2 plain rect (corner colours
          // blended in linear light); 3 ellipse / rounded rect, coloured the way the app's
          // tessellator does it: the gradient is evaluated on the outline and blended
          // (premultiplied, linear) across each horizontal span. sh: half size, radius, ellipse.
          `in vec2 lp; uniform vec4 cA; uniform vec4 cB; uniform vec4 g; uniform vec4 sh; uniform float op; out vec4 o;
           vec4 at(vec2 p) {
             float t = g.z > 0.0 ? clamp(dot(p, g.xy) / g.z * 0.5 + 0.5, 0.0, 1.0) : 0.0;
             vec4 m = mix(cA, cB, t);
             return vec4(s2l(m.rgb) * m.a, m.a);
           }
           void main() {
             if (g.w < 0.5) { float a = cA.a * op; o = vec4(s2l(cA.rgb) * a, a); return; }
             if (g.w < 2.5) {
               float t = g.z > 0.0 ? clamp(dot(lp, g.xy) / g.z * 0.5 + 0.5, 0.0, 1.0) : 0.0;
               vec4 pa = vec4(s2l(cA.rgb) * cA.a, cA.a), pb = vec4(s2l(cB.rgb) * cB.a, cB.a);
               o = mix(pa, pb, t) * op;
               return;
             }
             float y = clamp(lp.y, -sh.y, sh.y), xr;
             if (sh.w > 0.5) xr = sh.x * sqrt(max(0.0, 1.0 - (y / sh.y) * (y / sh.y)));
             else { float r = min(sh.z, min(sh.x, sh.y)); float dy = abs(y) - (sh.y - r); xr = dy <= 0.0 ? sh.x : sh.x - r + sqrt(max(0.0, r * r - dy * dy)); }
             float s = xr > 0.0 ? clamp((lp.x + xr) / (2.0 * xr), 0.0, 1.0) : 0.5;
             o = mix(at(vec2(-xr, y)), at(vec2(xr, y)), s) * op;
           }`),
        // canvas.wgsl vs_image / fs_image.
        image: program(`uniform mat3 M; out vec2 uv;
           void main() { uv = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1)); vec2 l = uv - vec2(0.5); gl_Position = vec4((M * vec3(l, 1.0)).xy, 0.0, 1.0); }`,
          `in vec2 uv; uniform sampler2D tex; uniform vec4 prm; uniform float op; out vec4 o;
           void main() {
             vec4 c = texture(tex, uv);
             float mask = 1.0;
             if (prm.y > 0.0) {
               vec2 p = (uv - vec2(0.5)) * vec2(prm.x, 1.0);
               float d = rbox(p, vec2(prm.x * 0.5, 0.5), min(prm.y, min(prm.x, 1.0) * 0.5));
               float aa = max(fwidth(d), 1e-6);
               mask = 1.0 - smoothstep(-aa, aa, d);
             }
             float a = c.a * op * mask;
             o = vec4(c.rgb * a, a);
           }`),
        // canvas.wgsl vs_shadow / fs_shadow.
        shadow: program(`uniform mat3 M; uniform vec2 hf; uniform vec4 prm; uniform vec2 off; out vec2 p;
           void main() {
             vec2 uv = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1));
             vec2 reach = hf + vec2(prm.y * 2.5);
             p = (uv * 2.0 - vec2(1.0)) * reach;
             gl_Position = vec4((M * vec3(p + off, 1.0)).xy, 0.0, 1.0);
           }`,
          `in vec2 p; uniform vec2 hf; uniform vec4 prm; uniform vec4 col; out vec4 o;
           void main() {
             float blur = max(prm.y, 1e-4);
             float d = prm.z > 0.5 ? (length(p / hf) - 1.0) * min(hf.x, hf.y) : rbox(p, hf, min(prm.x, min(hf.x, hf.y)));
             float falloff = 0.5 - 0.5 * tanh(d / (blur * 0.55));
             float a = col.a * falloff;
             o = vec4(s2l(col.rgb) * a, a);
           }`),
        // canvas.wgsl fs_backdrop (frag.y counts from the top there).
        backdrop: program(`void main() { vec2 uv = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); gl_Position = vec4(uv * 2.0 - 1.0, 0.0, 1.0); }`,
          `uniform vec3 top; uniform vec3 bot; uniform float H; out vec4 o;
           void main() { float t = clamp((H - gl_FragCoord.y) / max(H, 1.0), 0.0, 1.0); o = vec4(mix(s2l(top), s2l(bot), t), 1.0); }`),
        // The finished linear image, encoded to sRGB for display.
        // Overlays (the Fade dip, black/white screens) are blended on the encoded
        // image, as the app's interface layer draws them.
        present: program(`void main() { vec2 uv = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2)); gl_Position = vec4(uv * 2.0 - 1.0, 0.0, 1.0); }`,
          `uniform sampler2D tex; uniform vec4 ov1; uniform vec4 ov2; out vec4 o;
           void main() {
             vec3 c = l2s(texelFetch(tex, ivec2(gl_FragCoord.xy), 0).rgb);
             c = mix(c, ov1.rgb, ov1.a);
             c = mix(c, ov2.rgb, ov2.a);
             o = vec4(c, 1.0);
           }`),
      };
    } catch (e) {
      console.warn('GlideDeck: WebGL2 shaders failed, using the 2D canvas', e);
      return null;
    }
    const samples = min(4, gl.getParameter(gl.MAX_SAMPLES) || 1);
    let fb = null;
    function targets(W, H) {
      if (fb && fb.W === W && fb.H === H) return fb;
      if (fb) { gl.deleteFramebuffer(fb.ms); gl.deleteFramebuffer(fb.res); gl.deleteRenderbuffer(fb.color); gl.deleteRenderbuffer(fb.ds); gl.deleteTexture(fb.tex); }
      const color = gl.createRenderbuffer();
      gl.bindRenderbuffer(gl.RENDERBUFFER, color);
      gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, gl.SRGB8_ALPHA8, W, H);
      const ds = gl.createRenderbuffer();
      gl.bindRenderbuffer(gl.RENDERBUFFER, ds);
      gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, gl.DEPTH24_STENCIL8, W, H);
      const ms = gl.createFramebuffer();
      gl.bindFramebuffer(gl.FRAMEBUFFER, ms);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, color);
      gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_STENCIL_ATTACHMENT, gl.RENDERBUFFER, ds);
      const tex = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texStorage2D(gl.TEXTURE_2D, 1, gl.SRGB8_ALPHA8, W, H);
      const res = gl.createFramebuffer();
      gl.bindFramebuffer(gl.FRAMEBUFFER, res);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
      fb = { W, H, color, ds, ms, tex, res };
      return fb;
    }
    const quadBuf = gl.createBuffer();
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    gl.enableVertexAttribArray(0);

    // Path geometry: parsed once, flattened per level of detail, cached on the GPU.
    const parsed = new Map();
    function parse(d) {
      let cmds = parsed.get(d);
      if (cmds) return cmds;
      cmds = [];
      const re = /([MLQCZ])([^MLQCZ]*)/g;
      let m;
      while ((m = re.exec(d))) cmds.push([m[1], ...m[2].trim().split(/[\s,]+/).filter(Boolean).map(Number)]);
      parsed.set(d, cmds);
      return cmds;
    }
    function flattenCmds(cmds, tol) {
      const contours = [];
      let cur = null, x = 0, y = 0;
      for (const c of cmds) {
        switch (c[0]) {
          case 'M': cur = [c[1], c[2]]; contours.push(cur); x = c[1]; y = c[2]; break;
          case 'L': if (cur) cur.push(c[1], c[2]); x = c[1]; y = c[2]; break;
          case 'Q': {
            if (!cur) break;
            const [cx, cy, ex, ey] = c.slice(1);
            const dd = Math.hypot(x - 2 * cx + ex, y - 2 * cy + ey);
            const n = clamp(ceil(sqrt((0.25 * dd) / tol)), 1, 512);
            for (let i = 1; i <= n; i++) { const t = i / n, u = 1 - t; cur.push(u * u * x + 2 * u * t * cx + t * t * ex, u * u * y + 2 * u * t * cy + t * t * ey); }
            x = ex; y = ey;
            break;
          }
          case 'C': {
            if (!cur) break;
            const [ax, ay, bx, by, ex, ey] = c.slice(1);
            const dd = max(Math.hypot(x - 2 * ax + bx, y - 2 * ay + by), Math.hypot(ax - 2 * bx + ex, ay - 2 * by + ey));
            const n = clamp(ceil(sqrt((0.75 * dd) / tol)), 1, 512);
            for (let i = 1; i <= n; i++) {
              const t = i / n, u = 1 - t;
              cur.push(u * u * u * x + 3 * u * u * t * ax + 3 * u * t * t * bx + t * t * t * ex, u * u * u * y + 3 * u * u * t * ay + 3 * u * t * t * by + t * t * t * ey);
            }
            x = ex; y = ey;
            break;
          }
          case 'Z': cur = null; break;
        }
      }
      return contours;
    }
    // Arc of a circle or ellipse, flattened within `tol`.
    function arcPoints(out, cx, cy, rx, ry, a0, a1, tol) {
      const r = max(rx, ry);
      const step = r > tol ? 2 * Math.acos(clamp(1 - tol / r, -1, 1)) : PI / 2;
      const n = clamp(ceil(abs(a1 - a0) / max(step, 1e-3)), 1, 1024);
      for (let i = 0; i <= n; i++) { const a = a0 + ((a1 - a0) * i) / n; out.push(cx + cos(a) * rx, cy + sin(a) * ry); }
    }
    function roundRectContour(w, h, r, tol) {
      const hw = w / 2, hh = h / 2, rr = clamp(r || 0, 0, min(hw, hh));
      if (rr <= 0) return [-hw, -hh, hw, -hh, hw, hh, -hw, hh];
      const c = [];
      arcPoints(c, hw - rr, -hh + rr, rr, rr, -PI / 2, 0, tol);
      arcPoints(c, hw - rr, hh - rr, rr, rr, 0, PI / 2, tol);
      arcPoints(c, -hw + rr, hh - rr, rr, rr, PI / 2, PI, tol);
      arcPoints(c, -hw + rr, -hh + rr, rr, rr, PI, 1.5 * PI, tol);
      return c;
    }
    function ellipseContour(w, h, tol) { const c = []; arcPoints(c, 0, 0, w / 2, h / 2, 0, TAU, tol); return c; }
    const reverse = (c) => { const out = []; for (let i = c.length - 2; i >= 0; i -= 2) out.push(c[i], c[i + 1]); return out; };
    const geometry = new Map();
    let frameNo = 0;
    function upload(key, makeContours) {
      let g = geometry.get(key);
      if (!g) {
        const contours = makeContours();
        const tris = [];
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
        for (const c of contours) {
          for (let i = 0; i < c.length; i += 2) { x0 = min(x0, c[i]); x1 = max(x1, c[i]); y0 = min(y0, c[i + 1]); y1 = max(y1, c[i + 1]); }
          for (let i = 2; i + 3 < c.length; i += 2) tris.push(c[0], c[1], c[i], c[i + 1], c[i + 2], c[i + 3]);
        }
        const buf = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, buf);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(tris), gl.STATIC_DRAW);
        g = { buf, count: tris.length / 2, box: [x0, y0, x1, y1], used: 0 };
        geometry.set(key, g);
        if (geometry.size > 4000) {
          for (const [k, v] of geometry) if (frameNo - v.used > 120) { gl.deleteBuffer(v.buf); geometry.delete(k); }
        }
      }
      g.used = frameNo;
      return g;
    }
    const lodOf = (pxPerLocal) => clamp(ceil(Math.log2(max(pxPerLocal, 1e-300))), -60, 60);
    const tolFor = (lod, size) => max(TOLERANCE_PX / pow(2, lod), size * 2e-6);
    let W = 1, H = 1;
    let overlays = [];
    // Local → clip space, from a matrix in canvas pixels (y down).
    const clipM = (m) => new Float32Array([(2 * m[0]) / W, (-2 * m[1]) / H, 0, (2 * m[2]) / W, (-2 * m[3]) / H, 0, (2 * m[4]) / W - 1, 1 - (2 * m[5]) / H, 1]);
    const lin01 = (c) => [c[0] / 255, c[1] / 255, c[2] / 255, c[3] / 255];
    function fill(g, M, colA, colB, grad, opacity, shape) {
      if (!g.count) return;
      gl.useProgram(P.stencil.p);
      gl.uniformMatrix3fv(P.stencil.u('M'), false, M);
      gl.bindBuffer(gl.ARRAY_BUFFER, g.buf);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      gl.enable(gl.STENCIL_TEST);
      gl.colorMask(false, false, false, false);
      gl.stencilFunc(gl.ALWAYS, 0, 0xff);
      gl.stencilOpSeparate(gl.FRONT, gl.KEEP, gl.KEEP, gl.INCR_WRAP);
      gl.stencilOpSeparate(gl.BACK, gl.KEEP, gl.KEEP, gl.DECR_WRAP);
      gl.drawArrays(gl.TRIANGLES, 0, g.count);
      gl.colorMask(true, true, true, true);
      gl.stencilFunc(gl.NOTEQUAL, 0, 0xff);
      gl.stencilOp(gl.ZERO, gl.ZERO, gl.ZERO);
      const [x0, y0, x1, y1] = g.box;
      gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([x0, y0, x1, y0, x0, y1, x1, y1]), gl.STREAM_DRAW);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      gl.useProgram(P.cover.p);
      gl.uniformMatrix3fv(P.cover.u('M'), false, M);
      gl.uniform4fv(P.cover.u('cA'), lin01(colA));
      gl.uniform4fv(P.cover.u('cB'), lin01(colB || colA));
      gl.uniform4fv(P.cover.u('g'), grad || [0, 0, 0, 0]);
      gl.uniform4fv(P.cover.u('sh'), shape || [1, 1, 0, 0]);
      gl.uniform1f(P.cover.u('op'), opacity);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.disable(gl.STENCIL_TEST);
    }
    function gradientParams(g, w, h, plainRect) {
      const a = (g.a * PI) / 180, gc = cos(a), gs = sin(a);
      return [gc, gs, abs((w / 2) * gc) + abs((h / 2) * gs), plainRect ? 2 : 3];
    }
    // Textures (sRGB with straight alpha, mipmapped, as glide-render loads them).
    const textures = new Map();
    function texture(key, source, mip, w, h) {
      let t = textures.get(key);
      if (!t) {
        t = gl.createTexture();
        textures.set(key, t);
        gl.bindTexture(gl.TEXTURE_2D, t);
        gl.pixelStorei(gl.UNPACK_COLORSPACE_CONVERSION_WEBGL, gl.NONE);
        gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.SRGB8_ALPHA8, gl.RGBA, gl.UNSIGNED_BYTE, source);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        if (mip) { gl.generateMipmap(gl.TEXTURE_2D); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR); }
        else gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      }
      return t;
    }
    function drawTexture(tex, m, w, h, rad, opacity) {
      const M = clipM([m[0] * w, m[1] * w, m[2] * h, m[3] * h, m[4], m[5]]);
      gl.useProgram(P.image.p);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.uniform1i(P.image.u('tex'), 0);
      gl.uniformMatrix3fv(P.image.u('M'), false, M);
      const hh = max(abs(h), 1e-12);
      gl.uniform4fv(P.image.u('prm'), [w / hh, (rad || 0) / hh, 0, 0]);
      gl.uniform1f(P.image.u('op'), opacity);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    }
    function shadow(sh, m, w, h, radius, ellipse, opacity) {
      const c = typeof sh.c === 'string' ? rgba(sh.c) : sh.c;
      gl.useProgram(P.shadow.p);
      gl.uniformMatrix3fv(P.shadow.u('M'), false, clipM(m));
      gl.uniform2fv(P.shadow.u('hf'), [w / 2, h / 2]);
      gl.uniform4fv(P.shadow.u('prm'), [radius || 0, max(sh.b, 0), ellipse ? 1 : 0, 0]);
      gl.uniform2fv(P.shadow.u('off'), [sh.x, sh.y]);
      const cc = lin01(c);
      cc[3] *= opacity;
      gl.uniform4fv(P.shadow.u('col'), cc);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    }
    return {
      kind: 'webgl2',
      begin(w, h) {
        W = w; H = h;
        frameNo++;
        overlays = [];
        const f = targets(w, h);
        gl.bindFramebuffer(gl.FRAMEBUFFER, f.ms);
        gl.viewport(0, 0, w, h);
        gl.disable(gl.CULL_FACE);
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
        gl.bindVertexArray(vao);
        gl.clearBufferfv(gl.COLOR, 0, [toLinear(background[0]), toLinear(background[1]), toLinear(background[2]), 1]);
        gl.clearBufferfi(gl.DEPTH_STENCIL, 0, 1, 0);
        if (backgroundTo) {
          gl.useProgram(P.backdrop.p);
          gl.uniform3fv(P.backdrop.u('top'), background.slice(0, 3).map((v) => v / 255));
          gl.uniform3fv(P.backdrop.u('bot'), backgroundTo.slice(0, 3).map((v) => v / 255));
          gl.uniform1f(P.backdrop.u('H'), h);
          gl.drawArrays(gl.TRIANGLES, 0, 3);
        }
      },
      item(it, vm, dpr, vis, zoom) {
        const G = itemGeometry(it, vm, dpr, vis, zoom);
        if (!G) return;
        const el = it.el, { w, h, m, pxPerLocal } = G;
        const opacity = clamp01(it.o);
        const lod = lodOf(pxPerLocal), tol = tolFor(lod, max(w, h));
        const M = clipM(m);
        const sh = it.shadow || el.sh;
        if (sh && (el.k === 'rect' || el.k === 'frame' || el.k === 'image' || el.k === 'ellipse')) shadow(sh, m, w, h, el.k === 'ellipse' ? 0 : el.rad, el.k === 'ellipse', opacity);
        const pathFill = (d, col) => fill(upload(`${d}|${lod}`, () => flattenCmds(parse(d), tol)), M, col, null, null, opacity);
        switch (el.k) {
          case 'rect':
          case 'ellipse':
          case 'frame': {
            const ell = el.k === 'ellipse';
            const shape = ell ? `e:${w}:${h}:${lod}` : `r:${w}:${h}:${el.rad}:${lod}`;
            let colA = el.fillC || rgba(el.fill);
            if (colA && it.baseMix) colA = mixColor(colA, accent, it.baseMix);
            if (colA) {
              const g = upload(shape, () => [ell ? ellipseContour(w, h, tol) : roundRectContour(w, h, el.rad, tol)]);
              if (el.grad) fill(g, M, colA, rgba(el.grad.to), gradientParams(el.grad, w, h, !ell && !(el.rad > 0)), opacity, [w / 2, h / 2, el.rad || 0, ell ? 1 : 0]);
              else fill(g, M, colA, null, null, opacity);
            }
            if (el.stroke && el.stroke.w > 0) {
              const sw = el.stroke.w / 2, r = el.rad || 0;
              const g = upload(`${shape}|s${el.stroke.w}`, () => ell
                ? [ellipseContour(w + 2 * sw, h + 2 * sw, tol), reverse(ellipseContour(max(w - 2 * sw, 0), max(h - 2 * sw, 0), tol))]
                : [roundRectContour(w + 2 * sw, h + 2 * sw, r + sw, tol), reverse(roundRectContour(max(w - 2 * sw, 0), max(h - 2 * sw, 0), max(r - sw, 0), tol))]);
              fill(g, M, rgba(el.stroke.c), null, null, opacity);
            }
            break;
          }
          case 'icon': {
            let col = rgba(el.color);
            if (it.baseMix) col = mixColor(col, accent, it.baseMix);
            if (el.d) pathFill(el.d, col);
            break;
          }
          case 'text': {
            const base = rgba(el.color);
            if (it.glyphAlpha && el.glyphs) {
              for (const g of el.glyphs) {
                const a = it.glyphAlpha(g.s);
                if (a > 0) pathFill(g.d, withAlpha(textColor(it, base, g.c), a));
              }
            } else {
              for (const run of (it.countFrame ? it.countFrame.runs : el.runs)) pathFill(run.d, textColor(it, base, run.c));
            }
            break;
          }
          case 'embed':
          case 'image': {
            const img = imageFor(el.src);
            if (img) drawTexture(texture(el.src, img, true), m, w, h, el.rad, opacity);
            break;
          }
          case 'svg': {
            const c = svgRaster(el, max(w, h) * pxPerLocal);
            if (c) drawTexture(texture(`${el.id}@${c.width}`, c, true), m, w, h, 0, opacity);
            break;
          }
          case 'video': {
            const v = videoFor(el);
            if (v && v.readyState >= 2) {
              const key = `video:${el.id}`;
              const t = texture(key, v, false);
              if (!v.paused) { gl.bindTexture(gl.TEXTURE_2D, t); gl.texImage2D(gl.TEXTURE_2D, 0, gl.SRGB8_ALPHA8, gl.RGBA, gl.UNSIGNED_BYTE, v); }
              drawTexture(t, m, w, h, el.rad, opacity);
            } else fill(upload(`r:${w}:${h}:${el.rad}:${lod}`, () => [roundRectContour(w, h, el.rad, tol)]), M, [20, 20, 26, 230], null, null, opacity);
            break;
          }
        }
      },
      overlay(c, alpha) {
        overlays.push([c[0] / 255, c[1] / 255, c[2] / 255, alpha]);
      },
      end() {
        const f = fb;
        gl.bindFramebuffer(gl.READ_FRAMEBUFFER, f.ms);
        gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, f.res);
        gl.blitFramebuffer(0, 0, W, H, 0, 0, W, H, gl.COLOR_BUFFER_BIT, gl.NEAREST);
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        gl.viewport(0, 0, W, H);
        gl.disable(gl.BLEND);
        gl.useProgram(P.present.p);
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, f.tex);
        gl.uniform1i(P.present.u('tex'), 0);
        gl.uniform4fv(P.present.u('ov1'), overlays[0] || [0, 0, 0, 0]);
        gl.uniform4fv(P.present.u('ov2'), overlays[1] || [0, 0, 0, 0]);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
      },
    };
  }

  // ===== 2D canvas fallback =====================================================================
  function make2D() {
    const ctx = canvas.getContext('2d');
    const paths = new Map();
    const path2d = (d) => {
      let p = paths.get(d);
      if (!p) { p = new Path2D(d); paths.set(d, p); }
      return p;
    };
    function roundRect(w, h, r) {
      const hw = w / 2, hh = h / 2, rr = clamp(r || 0, 0, min(hw, hh));
      const p = new Path2D();
      if (rr <= 0) { p.rect(-hw, -hh, w, h); return p; }
      p.moveTo(-hw + rr, -hh);
      p.arcTo(hw, -hh, hw, hh, rr);
      p.arcTo(hw, hh, -hw, hh, rr);
      p.arcTo(-hw, hh, -hw, -hh, rr);
      p.arcTo(-hw, -hh, hw, -hh, rr);
      p.closePath();
      return p;
    }
    function ellipsePath(w, h) { const p = new Path2D(); p.ellipse(0, 0, w / 2, h / 2, 0, 0, TAU); return p; }
    function gradientFill(from, g, w, h) {
      const a = (g.a * PI) / 180, gc = cos(a), gs = sin(a);
      const ext = abs((w / 2) * gc) + abs((h / 2) * gs);
      const grad = ctx.createLinearGradient(-gc * ext, -gs * ext, gc * ext, gs * ext);
      grad.addColorStop(0, css(from));
      grad.addColorStop(1, css(rgba(g.to)));
      return grad;
    }
    function drawShadow(shape, sh, m, opacity) {
      const c = typeof sh.c === 'string' ? rgba(sh.c) : sh.c;
      const lin = sqrt(abs(m[0] * m[3] - m[1] * m[2]));
      const ox = m[0] * sh.x + m[2] * sh.y, oy = m[1] * sh.x + m[3] * sh.y;
      const FAR = 1e5;
      ctx.save();
      ctx.setTransform(m[0], m[1], m[2], m[3], m[4] + FAR, m[5]);
      ctx.shadowColor = css(c, opacity);
      ctx.shadowBlur = 2 * 0.478 * max(sh.b, 0) * lin;
      ctx.shadowOffsetX = -FAR + ox;
      ctx.shadowOffsetY = oy;
      ctx.fillStyle = '#000';
      ctx.fill(shape);
      ctx.restore();
    }
    let W = 1, H = 1;
    return {
      kind: '2d',
      begin(w, h) {
        W = w; H = h;
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.globalAlpha = 1;
        if (!backgroundTo) { ctx.fillStyle = css([background[0], background[1], background[2], 255]); ctx.fillRect(0, 0, w, h); return; }
        const g = ctx.createLinearGradient(0, 0, 0, h);
        for (let i = 0; i <= 16; i++) {
          const t = i / 16;
          const c = [0, 1, 2].map((k) => toSrgb(toLinear(background[k]) + (toLinear(backgroundTo[k]) - toLinear(background[k])) * t));
          g.addColorStop(t, `rgb(${c[0]},${c[1]},${c[2]})`);
        }
        ctx.fillStyle = g;
        ctx.fillRect(0, 0, w, h);
      },
      item(it, vm, dpr, vis, zoom) {
        const G = itemGeometry(it, vm, dpr, vis, zoom);
        if (!G) return;
        const el = it.el, { w, h, m, pxPerLocal } = G;
        const opacity = clamp01(it.o);
        const sh = it.shadow || el.sh;
        let shape = null;
        if (el.k === 'rect' || el.k === 'frame' || el.k === 'image' || el.k === 'embed') shape = roundRect(w, h, el.rad);
        else if (el.k === 'ellipse') shape = ellipsePath(w, h);
        if (sh && shape && el.k !== 'embed') drawShadow(shape, sh, m, opacity);
        ctx.setTransform(m[0], m[1], m[2], m[3], m[4], m[5]);
        ctx.globalAlpha = opacity;
        switch (el.k) {
          case 'rect':
          case 'ellipse':
          case 'frame': {
            let fillC = el.fillC || rgba(el.fill);
            if (fillC && it.baseMix) fillC = mixColor(fillC, accent, it.baseMix);
            if (fillC) { ctx.fillStyle = el.grad ? gradientFill(fillC, el.grad, w, h) : css(fillC); ctx.fill(shape); }
            if (el.stroke && el.stroke.w > 0) { ctx.lineWidth = el.stroke.w; ctx.lineJoin = 'round'; ctx.strokeStyle = css(rgba(el.stroke.c)); ctx.stroke(shape); }
            break;
          }
          case 'icon': {
            let col = rgba(el.color);
            if (it.baseMix) col = mixColor(col, accent, it.baseMix);
            ctx.fillStyle = css(col);
            if (el.d) ctx.fill(path2d(el.d));
            break;
          }
          case 'text': {
            const base = rgba(el.color);
            if (it.glyphAlpha && el.glyphs) {
              for (const g of el.glyphs) {
                const a = it.glyphAlpha(g.s);
                if (a <= 0) continue;
                ctx.fillStyle = css(withAlpha(textColor(it, base, g.c), a));
                ctx.fill(path2d(g.d));
              }
            } else {
              for (const run of (it.countFrame ? it.countFrame.runs : el.runs)) { ctx.fillStyle = css(textColor(it, base, run.c)); ctx.fill(path2d(run.d)); }
            }
            break;
          }
          case 'embed':
          case 'image': {
            const img = imageFor(el.src);
            if (img) { ctx.save(); if (el.rad > 0) ctx.clip(shape); ctx.drawImage(img, -w / 2, -h / 2, w, h); ctx.restore(); }
            break;
          }
          case 'svg': {
            const c = svgRaster(el, max(w, h) * pxPerLocal);
            if (c) ctx.drawImage(c, -w / 2, -h / 2, w, h);
            break;
          }
          case 'video': {
            const v = videoFor(el);
            ctx.save();
            const clip = roundRect(w, h, el.rad);
            ctx.clip(clip);
            if (v && v.readyState >= 2) ctx.drawImage(v, -w / 2, -h / 2, w, h);
            else { ctx.fillStyle = 'rgba(20,20,26,0.9)'; ctx.fill(clip); }
            ctx.restore();
            break;
          }
        }
        ctx.globalAlpha = 1;
      },
      overlay(c, alpha) {
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.fillStyle = `rgba(${c[0]},${c[1]},${c[2]},${alpha})`;
        ctx.fillRect(0, 0, W, H);
      },
      end() {},
    };
  }

  const toLinear = (c) => { c /= 255; return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4); };
  const toSrgb = (c) => round(255 * clamp01(c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1 / 2.4) - 0.055));
  const forced = new URLSearchParams(location.hash.slice(1)).get('renderer');
  const R = (forced !== '2d' && makeGL()) || make2D();

  // --- the show (glidedeck/present.rs) ---------------------------------------------------------
  const vp = { w: 0, h: 0 };
  let frozenView = false;
  let dpr = 1;
  const show = { step: 0, clock: freshClock(0), cam: { x: 0, y: 0, zoom: 1, rot: 0 }, tween: null, blank: null, now: 0 };
  const stepCamera = (i) => (path[i] ? forFrame(els[path[i].frame], vp, 0) : null);
  function resize() {
    dpr = window.devicePixelRatio || 1;
    const w = window.innerWidth, h = window.innerHeight;
    canvas.width = round(w * dpr);
    canvas.height = round(h * dpr);
    const first = vp.w === 0;
    const changed = abs(vp.w - w) > 0.5 || abs(vp.h - h) > 0.5;
    vp.w = w; vp.h = h;
    if ((changed && !show.tween) || first) {
      const c = stepCamera(show.step);
      if (c) show.cam = c;
    }
    needsFrame = true;
    // A frozen (test) view is rebuilt for the new size and redrawn.
    if (frozenView) { fromHash(); render(); }
  }
  function go(index) {
    if (!path.length) return;
    const from = show.step;
    index = clamp(index, 0, path.length - 1);
    const target = stepCamera(index);
    if (!target) return;
    const t = path[max(index, from)];
    show.tween = makeTween(show.cam, target, t.dur, t.ease, t.style, vp);
    if (index !== from) show.clock = index < from ? finishedClock(index) : freshClock(index);
    show.step = index;
    show.blank = null;
  }
  function startArrivalBuilds() {
    const c = show.clock;
    if (nextIsAutomatic(c)) { const g = nextGroup(c); if (g) startGroup(c, g, show.now); }
  }
  function nav({ next, prev, first, last, black, white }) {
    if (black || white) {
      const color = black ? '#000' : '#fff';
      show.blank = show.blank === color ? null : color;
      return;
    }
    if (show.blank && (next || prev)) { show.blank = null; return; }
    const n = path.length, step = show.step, c = show.clock, now = show.now;
    if (next) {
      if (running(c, now)) { finishRunning(c); return; }
      const g = nextGroup(c);
      if (g) { startGroup(c, g, now); return; }
    }
    if (prev) {
      const g = lastStartedGroup(c);
      if (g && buildsOf(step)[g[0]].trigger === 'on_click') { for (let i = g[0]; i < g[1]; i++) c.starts[i] = null; return; }
    }
    if (next && step + 1 < n) go(step + 1);
    else if (prev && step > 0) go(step - 1);
    else if (first) go(0);
    else if (last) go(n - 1);
    else if (next && step + 1 >= n && !show.tween && !camEq(stepCamera(step), show.cam)) go(step);
  }

  function render() {
    R.begin(canvas.width, canvas.height);
    let layout = null;
    if (path.length) {
      const items = scene(show.clock, show.now);
      applyParallax(items, show.cam, vp);
      const vm = viewMatrix(show.cam, vp);
      const vis = visibleAabb(show.cam, vp);
      for (const it of items) R.item(it, vm, dpr, vis, show.cam.zoom);
      const fade = fadeAmount(show.tween);
      if (fade > 0) R.overlay(background, round(fade * 255) / 255);
      if (show.blank) R.overlay(show.blank === '#000' ? [0, 0, 0] : [255, 255, 255], 1);
      layout = [items, vm, vis, fade];
    }
    R.end();
    if (layout) layoutEmbeds(...layout);
    count.textContent = path.length ? `${show.step + 1} / ${path.length}` : '';
  }

  // One simulation step (the app's nav.tick + arrival handling).
  function tick(dt) {
    const wasMoving = !!show.tween;
    if (show.tween) {
      const tw = show.tween;
      tw.elapsed = min(tw.elapsed + dt, tw.dur);
      show.cam = sampleTween(tw, tw.elapsed / tw.dur);
      if (finished(tw)) show.tween = null;
    }
    if (wasMoving && !show.tween) {
      autoplayStepVideos(show.step);
      startArrivalBuilds();
    }
  }

  // --- input ---------------------------------------------------------------------------------
  const hud = document.getElementById('hud');
  const count = document.getElementById('count');
  const help = document.getElementById('help');
  let hudTimer = 0;
  function poke() {
    hud.classList.remove('hidden');
    canvas.style.cursor = 'default';
    clearTimeout(hudTimer);
    hudTimer = setTimeout(() => { hud.classList.add('hidden'); canvas.style.cursor = 'none'; }, 2500);
  }
  const act = (n) => { nav(n); needsFrame = true; };
  document.getElementById('prev').onclick = (e) => { e.stopPropagation(); act({ prev: true }); };
  document.getElementById('next').onclick = (e) => { e.stopPropagation(); act({ next: true }); };
  const toggleFull = () => (document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen()).catch(() => {});
  document.getElementById('full').onclick = (e) => { e.stopPropagation(); toggleFull(); };
  document.getElementById('helpbtn').onclick = (e) => { e.stopPropagation(); help.classList.toggle('show'); };
  help.onclick = () => help.classList.remove('show');
  window.addEventListener('keydown', (e) => {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const k = e.key;
    const map = {
      ArrowRight: 'next', ArrowDown: 'next', ' ': 'next', PageDown: 'next', Enter: 'next', n: 'next', N: 'next',
      ArrowLeft: 'prev', ArrowUp: 'prev', PageUp: 'prev', Backspace: 'prev', p: 'prev', P: 'prev',
      Home: 'first', End: 'last', b: 'black', B: 'black', '.': 'black', w: 'white', W: 'white', ',': 'white',
    };
    if (map[k]) { e.preventDefault(); act({ [map[k]]: true }); return; }
    if (k === 'f' || k === 'F') toggleFull();
    if (k === '?') help.classList.toggle('show');
    if (k === 'Escape') help.classList.remove('show');
  });
  // The topmost video or web video under a point.
  function videoAt(px, py) {
    const [wx, wy] = screenToWorld(show.cam, px, py, vp);
    for (let i = els.length - 1; i >= 0; i--) {
      const el = els[i];
      if ((el.k !== 'video' && el.k !== 'embed') || el.hidden) continue;
      const th = (-el.r * PI) / 180, s = abs(el.s) || 1, dx = wx - el.x, dy = wy - el.y;
      const lx = (cos(th) * dx - sin(th) * dy) / s, ly = (sin(th) * dx + cos(th) * dy) / s;
      if (abs(lx) <= el.w / 2 && abs(ly) <= el.h / 2) return el;
    }
    return null;
  }
  canvas.addEventListener('click', (e) => {
    const el = videoAt(e.clientX, e.clientY);
    if (el && el.k === 'embed') { playEmbed(el); needsFrame = true; return; }
    const v = el && videoFor(el);
    if (v) { if (v.paused) v.play().catch(() => {}); else v.pause(); needsFrame = true; return; }
    act({ next: true });
  });
  canvas.addEventListener('contextmenu', (e) => { e.preventDefault(); act({ prev: true }); });
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const scale = e.deltaMode === 1 ? 40 : e.deltaMode === 2 ? 400 : 1;
    show.tween = null;
    zoomAbout(show.cam, e.clientX, e.clientY, exp(-e.deltaY * scale * 0.004 * (e.ctrlKey ? 2.5 : 1)), vp);
    needsFrame = true;
  }, { passive: false });
  let touch = null;
  canvas.addEventListener('touchstart', (e) => { if (e.touches.length === 1) touch = [e.touches[0].clientX, e.touches[0].clientY]; }, { passive: true });
  canvas.addEventListener('touchend', (e) => {
    if (!touch) return;
    const t = e.changedTouches[0], dx = t.clientX - touch[0], dy = t.clientY - touch[1];
    touch = null;
    if (abs(dx) > 50 && abs(dx) > abs(dy)) act(dx < 0 ? { next: true } : { prev: true });
  });
  window.addEventListener('mousemove', poke);
  window.addEventListener('resize', resize);

  // --- loop ------------------------------------------------------------------------------------
  let last = performance.now();
  function frame(t) {
    const dt = clamp((t - last) / 1000, 0, 0.1);
    last = t;
    show.now = t / 1000;
    const active = show.tween || running(show.clock, show.now) || anyVideoPlaying();
    tick(dt);
    if (active || needsFrame || show.tween) render();
    needsFrame = false;
    requestAnimationFrame(frame);
  }

  // Deterministic states for checks and screenshots: `#step=3`, plus
  // `&from=2&t=0.5` (mid-transition at fraction t) or `&built=1` (all builds
  // played) / `&clicks=2&elapsed=0.4` (simulate clicks, then time passing).
  function fromHash() {
    const q = new URLSearchParams(location.hash.slice(1));
    if (!q.has('step')) return false;
    const step = clamp(+q.get('step') - 1, 0, path.length - 1);
    if (q.has('from')) {
      const from = clamp(+q.get('from') - 1, 0, path.length - 1);
      show.step = from;
      show.cam = stepCamera(from);
      show.clock = finishedClock(from);
      go(step);
      const tw = show.tween;
      tw.elapsed = clamp01(+(q.get('t') || 0)) * tw.dur;
      show.cam = sampleTween(tw, tw.elapsed / tw.dur);
      return true;
    }
    show.step = step;
    show.cam = stepCamera(step);
    show.clock = q.get('built') === '1' ? finishedClock(step) : freshClock(step);
    show.now = 1000;
    if (q.get('built') !== '1') {
      startArrivalBuilds();
      for (let i = 0; i < +(q.get('clicks') || 0); i++) { finishRunning(show.clock); nav({ next: true }); }
      show.now += +(q.get('elapsed') || 0);
    }
    return true;
  }

  resize();
  const frozen = fromHash();
  frozenView = frozen;
  window.glidedeck = { show, go, nav: act, render, scene, sampleTween, viewMatrix, renderer: R.kind };
  if (frozen) {
    // Wait for images, then draw the requested state once.
    Promise.all([...Object.keys(deck.media)].map((k) => new Promise((res) => { const i = imageFor(k) || images.get(k); if (!i || i.complete) res(); else { i.onload = i.onerror = res; } })))
      .then(() => { render(); document.title += ` (ready ${R.kind})`; });
  } else {
    show.cam = stepCamera(0) || show.cam;
    startArrivalBuilds();
    autoplayStepVideos(0);
    poke();
    requestAnimationFrame((t) => { last = t; show.now = t / 1000; show.clock = freshClock(0); startArrivalBuilds(); frame(t); });
  }
})();
