// GlideDeck site: the page is one canvas, and scrolling flies a camera across it.
//
// Every section is a stop. The flight plan below places each one on the canvas at its own
// size and angle. The camera always lands square on a stop at 1:1, so text stays sharp; the
// pans, zooms and spins are what it takes to get from one placement to the next.
// Tall sections scroll as usual before the camera leaves them.
//
// Wide screens only, and only when the visitor has not asked for reduced motion. Without
// this script the page is an ordinary scrolling page.
(() => {
  const sky = document.querySelector('.sky');
  const world = sky && sky.querySelector('.world');
  if (!world || !('ResizeObserver' in window)) return;
  const root = document.documentElement;

  // go: where the stop sits, as seen on screen from the stop before it.
  // size: how big it is on the canvas (the camera zooms by the inverse). turn: degrees.
  const PLAN = [
    { sel: '#top', name: 'Start', size: 1, turn: 0 },
    { sel: '#fly', name: 'Fly it yourself', go: 'down', size: 1, turn: 0 },
    { sel: '#why', name: 'Why a canvas', go: 'right', size: 1, turn: 0 },
    { sel: '#commands', name: 'Features', go: 'down', size: 0.35, turn: -10 },
    { sel: '#start', name: 'Two minutes to takeoff', go: 'right', size: 0.35, turn: 80 },
    { sel: '#tour', name: 'Close up', go: 'up', size: 1.5, turn: 90 },
    { sel: '#works', name: 'Plays well with', go: 'left', size: 1.5, turn: 90 },
    { sel: '#values', name: 'Free, private, offline', go: 'up', size: 0.5, turn: 180 },
    { sel: '#questions', name: 'Questions', go: 'left', size: 0.5, turn: 172 },
    { sel: '#download', name: 'Download', go: 'down', size: 0.5, turn: 180 },
    { sel: '.foot', name: 'Open source', go: 'down', size: 3, turn: 360 },
  ];
  const DIR = { down: [0, 1], up: [0, -1], right: [1, 0], left: [-1, 0] };
  const stops = PLAN.map((p) => ({ el: document.querySelector(p.sel), name: p.name, go: p.go, s: p.size, r: (p.turn * Math.PI) / 180 })).filter((st) => st.el);
  if (stops.length < 2) return;

  const wide = matchMedia('(min-width: 901px) and (prefers-reduced-motion: no-preference)');
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  const turn = (x, y, a) => { const c = Math.cos(a), s = Math.sin(a); return [x * c - y * s, x * s + y * c]; };

  let W = 0, H = 0, total = 0;
  let on = false, frame = 0, clock = 0, stale = false;
  let cur = 0;        // the scroll position the camera is at; it trails the real one a little
  let lastY = 0, lastMove = 0, heading = 1, touching = false;
  let auto = null;    // a scripted scroll that finishes a flight the visitor started
  let hop = null;     // a direct flight to a stop picked from a link or the keyboard
  let shown = null;   // the camera as last drawn
  let drawn = '', here = -1;
  let hud = null, marks = [], fresh = true;

  // ---- The canvas ------------------------------------------------------------------------
  // What the camera sees when a stop is scrolled `pan` pixels down: the point it centres on,
  // the width of canvas in view, and its angle.
  const view = (st, pan) => {
    const [ox, oy] = turn(0, ((H - st.h) / 2 + pan) * st.s, st.r);
    return { x: st.cx + ox, y: st.cy + oy, w: W * st.s, r: st.r };
  };

  // True when two placed sections do not touch (separating axes of two turned rectangles).
  const apart = (a, b) => {
    for (const q of [a, b]) {
      for (const quarter of [0, Math.PI / 2]) {
        const ang = q.r + quarter;
        const reach = (o) => Math.abs(Math.cos(o.r - ang)) * o.hw + Math.abs(Math.sin(o.r - ang)) * o.hh;
        const gap = Math.abs((b.cx - a.cx) * Math.cos(ang) + (b.cy - a.cy) * Math.sin(ang));
        if (gap > reach(a) + reach(b)) return true;
      }
    }
    return false;
  };

  // Place every stop: start where the plan points and slide outward until it is clear of
  // everything already placed. Heights come from the page, so nothing here is hard-coded.
  const layout = () => {
    W = sky.clientWidth;
    H = sky.clientHeight;
    const placed = [];
    let at = 0;
    stops.forEach((st, i) => {
      st.h = st.el.offsetHeight;
      st.pan = Math.max(0, st.h - H);
      const [ox, oy] = turn(0, ((H - st.h) / 2) * st.s, st.r); // centre of the section to the centre of its first screenful
      const box = { hw: W * st.s * 0.54, hh: (st.h * st.s) / 2 + W * st.s * 0.04, r: st.r, cx: 0, cy: 0 };
      if (i === 0) {
        st.cx = -ox;
        st.cy = -oy;
      } else {
        const prev = stops[i - 1];
        const from = view(prev, prev.pan);
        const [dx, dy] = turn(DIR[st.go][0], DIR[st.go][1], prev.r);
        const step = W * Math.min(st.s, prev.s) * 0.04;
        for (let n = 0; n < 6000; n++) {
          st.cx = box.cx = from.x + dx * step * n - ox;
          st.cy = box.cy = from.y + dy * step * n - oy;
          if (placed.every((b) => apart(b, box))) break;
        }
      }
      box.cx = st.cx;
      box.cy = st.cy;
      placed.push(box);
      st.el.style.transform = `translate(${st.cx - W / 2}px,${st.cy - st.h / 2}px) rotate(${st.r}rad) scale(${st.s})`;

      // Scroll budget: the section's own height first, then the flight to the next stop.
      // Bigger moves get a longer run.
      const next = stops[i + 1];
      st.start = at;
      st.len = next ? Math.round(H * Math.min(1.9, 0.9 + (next.r !== st.r ? 0.3 : 0) + 0.3 * Math.abs(Math.log(next.s / st.s)))) : 0;
      at += st.pan + st.len;
    });
    total = at;
    document.body.style.height = `${total + H}px`;
  };

  // ---- The camera ------------------------------------------------------------------------
  // Which stop a scroll position belongs to, and how far into the flight after it (0 to 1).
  const locate = (y) => {
    y = clamp(y, 0, total);
    let i = stops.length - 1;
    while (i > 0 && stops[i].start > y) i--;
    const st = stops[i], local = y - st.start;
    return { i, local, p: st.len && local > st.pan ? Math.min(1, (local - st.pan) / st.len) : 0 };
  };

  // A flight between two views. Position and zoom follow van Wijk and Nuij's smooth path
  // (2003), the same one map apps use: it pulls back just enough to keep both ends in sight.
  const glide = (a, b, t, near) => {
    let dr = b.r - a.r;
    if (near) dr = Math.atan2(Math.sin(dr), Math.cos(dr)); // direct flights take the short way round
    const dx = b.x - a.x, dy = b.y - a.y, d = Math.hypot(dx, dy);
    let u, w;
    if (d < a.w * 1e-6) {
      u = t;
      w = a.w * Math.pow(b.w / a.w, t);
    } else {
      const k = 2; // rho squared: how eagerly the path trades panning for zooming out
      const r0 = -Math.asinh((b.w * b.w - a.w * a.w + k * k * d * d) / (2 * a.w * k * d));
      const r1 = -Math.asinh((b.w * b.w - a.w * a.w - k * k * d * d) / (2 * b.w * k * d));
      const s = r0 + t * (r1 - r0);
      u = (a.w / (k * d)) * (Math.cosh(r0) * Math.tanh(s) - Math.sinh(r0));
      w = (a.w * Math.cosh(r0)) / Math.cosh(s);
    }
    return { x: a.x + dx * u, y: a.y + dy * u, w, r: a.r + dr * t };
  };

  const cameraAt = (y) => {
    const { i, local, p } = locate(y);
    const st = stops[i];
    if (!p) return view(st, Math.min(local, st.pan));
    return glide(view(st, st.pan), view(stops[i + 1], 0), p * p * (3 - 2 * p));
  };

  const draw = (cam) => {
    shown = cam;
    const css = `translate(${W / 2}px,${H / 2}px) rotate(${-cam.r}rad) scale(${W / cam.w}) translate(${-cam.x}px,${-cam.y}px)`;
    if (css !== drawn) world.style.transform = drawn = css;
  };

  const jump = (y) => {
    y = clamp(y, 0, total);
    scrollTo({ top: y, behavior: 'instant' });
    cur = lastY = y;
    auto = null;
  };

  // When the visitor stops scrolling in mid-air, finish the flight: onward if they had set
  // off, back if they had barely moved.
  const settle = (y, now) => {
    const { i, p } = locate(y);
    if (p <= 0 || p >= 1) return;
    const st = stops[i], a = st.start + st.pan, b = a + st.len;
    if (y - a < 1 || b - y < 1) return; // close enough: browsers round scroll positions
    const to =(heading > 0 ? p > 0.08 : p > 0.92) ? b : a;
    auto = { from: y, to, last: y, t0: now, dur: clamp((Math.abs(to - y) / H) * 1100, 450, 1400) };
  };

  const tick = (now) => {
    frame = requestAnimationFrame(tick);
    const dt = now - clock;
    clock = now;
    if (stale) { stale = false; relayout(); }

    let y = scrollY;
    if (auto) {
      if (Math.abs(y - auto.last) > 2) {
        auto = null; // the visitor took over
      } else {
        const q = Math.min(1, (now - auto.t0) / auto.dur);
        y = auto.last = auto.from + (auto.to - auto.from) * (1 - Math.pow(1 - q, 3));
        scrollTo({ top: y, behavior: 'instant' });
        if (q === 1) auto = null;
      }
    }
    if (y !== lastY) {
      heading = y > lastY ? 1 : -1;
      lastY = y;
      lastMove = now;
    } else if (!auto && !hop && !touching && now - lastMove > 160) {
      settle(y, now);
    }

    // The camera trails the scroll position slightly, which turns wheel notches into a glide.
    cur = auto ? y : cur + (y - cur) * (1 - Math.exp(-dt / 90));
    if (Math.abs(y - cur) < 0.05) cur = y;

    let cam = cameraAt(cur);
    if (hop) {
      const q = Math.min(1, (now - hop.t0) / hop.dur);
      cam = glide(hop.from, cam, q < 0.5 ? 4 * q * q * q : 1 - Math.pow(-2 * q + 2, 3) / 2, true);
      if (q === 1) hop = null;
    }
    draw(cam);

    const at = locate(cur);
    const stop = at.p > 0.5 ? at.i + 1 : at.i;
    if (stop !== here) {
      here = stop;
      marks.forEach((m, i) => { if (i === stop) m.setAttribute('aria-current', 'step'); else m.removeAttribute('aria-current'); });
    }
  };

  // ---- Getting to a stop without scrolling there ------------------------------------------
  // The scroll position that shows an element: the top of its stop, or the element centred.
  const aim = (el) => {
    const st = stops.find((s) => s.el.contains(el));
    if (!st) return null;
    if (el === st.el) return st.start;
    let top = 0;
    for (let n = el; n && n !== st.el; n = n.offsetParent) top += n.offsetTop;
    return st.start + clamp(top + el.offsetHeight / 2 - H / 2, 0, st.pan);
  };

  // Fly straight there, skipping the stops in between.
  const flyTo = (y) => {
    if (y == null) return;
    y = clamp(y, 0, total);
    const far = locate(y).i !== locate(cur).i || locate(cur).p > 0;
    if (shown) hop = { from: shown, t0: performance.now(), dur: far ? 1500 : 500 };
    jump(y);
  };

  const onClick = (e) => {
    const a = e.target.closest && e.target.closest('a[href^="#"]');
    const target = a && document.getElementById(a.hash.slice(1));
    if (!target || aim(target) == null) return;
    e.preventDefault();
    flyTo(aim(target));
    history.pushState(null, '', a.hash);
    if (a.classList.contains('skip')) { target.tabIndex = -1; target.focus({ preventScroll: true }); }
  };
  const onHash = () => {
    const target = location.hash.length > 1 && document.getElementById(location.hash.slice(1));
    flyTo(target ? aim(target) : 0);
  };
  // Tabbing to something off screen takes the camera there.
  const onFocus = (e) => {
    const r = e.target.getBoundingClientRect();
    if (r.top >= 0 && r.left >= 0 && r.bottom <= H && r.right <= W) return;
    flyTo(aim(e.target));
  };
  const onTouch = (e) => { touching = e.type === 'touchstart' || e.touches.length > 0; lastMove = performance.now(); };
  const onLeave = () => {
    const at = locate(scrollY);
    try { sessionStorage.setItem('flight', JSON.stringify([at.i, at.local])); } catch {}
  };

  // ---- Keeping up with the page -----------------------------------------------------------
  // Sections change height (a question opens, the window resizes). Re-place everything and
  // keep the camera on the stop it was on.
  const relayout = () => {
    const was = locate(scrollY), old = stops[was.i], pan = old.pan, len = old.len;
    layout();
    const st = stops[was.i];
    jump(st.start + (was.local <= pan ? Math.min(was.local, st.pan) : st.pan + ((was.local - pan) / len) * st.len));
    drawn = '';
  };
  const watch = new ResizeObserver(() => { stale = true; });

  const enable = () => {
    if (on) return;
    // Hold the place: the section at the top of the window before the page becomes a canvas.
    let keep = [0, 0];
    stops.forEach((st, i) => { const top = st.el.getBoundingClientRect().top; if (top <= 1) keep = [i, -top]; });
    // On arrival only: a reload returns to the stop it left, a link with a # goes to its target.
    let target = null;
    if (fresh) {
      fresh = false;
      const nav = performance.getEntriesByType('navigation')[0];
      if (nav && nav.type !== 'navigate') {
        try { keep = JSON.parse(sessionStorage.getItem('flight')) || keep; } catch {}
      } else if (location.hash.length > 1) {
        target = document.getElementById(location.hash.slice(1));
      }
    }

    on = true;
    root.classList.add('flight');
    history.scrollRestoration = 'manual';
    world.querySelectorAll('img[loading="lazy"]').forEach((img) => { img.loading = 'eager'; });

    hud = document.createElement('nav');
    hud.className = 'path';
    hud.setAttribute('aria-label', 'Stops on this page');
    marks = stops.map((st) => {
      const b = document.createElement('button');
      b.type = 'button';
      b.innerHTML = '<span></span>';
      b.firstChild.textContent = st.name;
      b.addEventListener('click', () => flyTo(st.start));
      hud.append(b);
      return b;
    });
    document.body.append(hud);

    layout();
    const st = stops[clamp(keep[0] | 0, 0, stops.length - 1)];
    jump(target && aim(target) != null ? aim(target) : st.start + clamp(+keep[1] || 0, 0, st.pan));
    draw(cameraAt(cur));

    stops.forEach((s) => watch.observe(s.el));
    watch.observe(sky);
    document.addEventListener('click', onClick);
    sky.addEventListener('focusin', onFocus);
    addEventListener('hashchange', onHash);
    addEventListener('touchstart', onTouch, { passive: true });
    addEventListener('touchend', onTouch, { passive: true });
    addEventListener('touchcancel', onTouch, { passive: true });
    addEventListener('pagehide', onLeave);
    clock = lastMove = performance.now();
    here = -1;
    frame = requestAnimationFrame(tick);
  };

  const disable = () => {
    if (!on) return;
    const at = locate(scrollY);
    on = false;
    cancelAnimationFrame(frame);
    watch.disconnect();
    document.removeEventListener('click', onClick);
    sky.removeEventListener('focusin', onFocus);
    removeEventListener('hashchange', onHash);
    removeEventListener('touchstart', onTouch);
    removeEventListener('touchend', onTouch);
    removeEventListener('touchcancel', onTouch);
    removeEventListener('pagehide', onLeave);
    root.classList.remove('flight');
    history.scrollRestoration = 'auto';
    hud.remove();
    marks = [];
    auto = hop = shown = null;
    drawn = '';
    world.style.transform = '';
    document.body.style.height = '';
    stops.forEach((st) => { st.el.style.transform = ''; });
    scrollTo({ top: stops[at.i].el.getBoundingClientRect().top + scrollY + at.local, behavior: 'instant' });
  };

  const sync = () => (wide.matches ? enable() : disable());
  wide.addEventListener('change', sync);
  sync();
})();
