// GlideDeck site: the live show's controls, the command list, the download picker and
// the entrance of each section. The page reads fine without any of it.
(() => {
  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;

  // --- The live show: a real GlideDeck web export in an iframe, flown from this page. ---------
  const stage = $('#stage');
  if (stage) {
    const frame = $('iframe', stage);
    const rows = $$('#stops button');
    const count = $('#stop-count');
    const player = () => {
      try { return frame.contentWindow && frame.contentWindow.glidedeck; } catch { return null; }
    };
    const flown = () => stage.classList.add('flown');
    if (matchMedia('(hover: none)').matches) $('.stage-hint', stage).textContent = 'Tap to fly on';
    const fly = (how) => {
      const p = player();
      if (!p) return;
      if (typeof how === 'number') p.go(how); else p.nav(how);
      p.render();
      flown();
    };
    $('.stage-hit', stage).addEventListener('click', () => fly({ next: true }));
    $('#next').addEventListener('click', () => fly({ next: true }));
    $('#prev').addEventListener('click', () => fly({ prev: true }));
    rows.forEach((row) => row.addEventListener('click', () => fly(+row.dataset.step)));

    // Arrow keys fly the show while the pointer or the focus is on the demo.
    const demo = stage.closest('.demo');
    let near = false;
    demo.addEventListener('pointerenter', () => { near = true; });
    demo.addEventListener('pointerleave', () => { near = false; });
    addEventListener('keydown', (e) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (!(near || demo.contains(document.activeElement))) return;
      if (e.key === 'ArrowRight') { e.preventDefault(); fly({ next: true }); }
      if (e.key === 'ArrowLeft') { e.preventDefault(); fly({ prev: true }); }
    });

    // Swipe on a phone.
    let touch = null;
    stage.addEventListener('touchstart', (e) => { if (e.touches.length === 1) touch = [e.touches[0].clientX, e.touches[0].clientY]; }, { passive: true });
    stage.addEventListener('touchend', (e) => {
      if (!touch) return;
      const t = e.changedTouches[0], dx = t.clientX - touch[0], dy = t.clientY - touch[1];
      touch = null;
      if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy)) { e.preventDefault(); fly(dx < 0 ? { next: true } : { prev: true }); }
    });

    // Keep the list in step with the show.
    let shown = -1;
    setInterval(() => {
      const p = player();
      if (!p || p.show.step === shown) return;
      shown = p.show.step;
      rows.forEach((row, i) => {
        if (i === shown) row.setAttribute('aria-current', 'step'); else row.removeAttribute('aria-current');
      });
      const list = rows[shown] && rows[shown].closest('ol');
      if (list && list.scrollHeight > list.clientHeight) {
        const row = rows[shown];
        list.scrollTo({ top: row.offsetTop - list.offsetTop - list.clientHeight / 2 + row.offsetHeight / 2, behavior: calm ? 'auto' : 'smooth' });
      }
      count.textContent = `Stop ${shown + 1} of ${rows.length}`;
    }, 150);
  }

  // --- The film: one big button starts it, with sound. ----------------------------------------
  const film = $('#film');
  if (film) {
    const video = $('video', film);
    $('.film-play', film).addEventListener('click', () => { video.play(); });
    video.addEventListener('play', () => film.classList.add('playing'));
    video.addEventListener('pause', () => { if (video.currentTime < 0.1 || video.ended) film.classList.remove('playing'); });
    video.addEventListener('ended', () => film.classList.remove('playing'));
  }

  // --- The command list: type to filter, arrows or the pointer to pick. -----------------------
  const list = $('#command-list');
  if (list) {
    const items = $$('li', list);
    const search = $('#command-search');
    const empty = $('.palette-empty');
    const visible = () => items.filter((li) => !li.hidden);
    const pick = (li) => items.forEach((it) => it.classList.toggle('is-on', it === li));
    const fine = matchMedia('(hover: hover)').matches;
    items.forEach((li) => {
      if (fine) li.addEventListener('pointerenter', () => pick(li));
      li.addEventListener('click', () => pick(li));
      li.addEventListener('focus', () => pick(li));
    });
    const move = (by) => {
      const rows = visible();
      if (!rows.length) return;
      const at = rows.findIndex((li) => li.classList.contains('is-on'));
      pick(rows[(at + by + rows.length) % rows.length]);
    };
    $('.palette').addEventListener('keydown', (e) => {
      if (e.key === 'ArrowDown') { e.preventDefault(); move(1); }
      if (e.key === 'ArrowUp') { e.preventDefault(); move(-1); }
    });
    search.addEventListener('input', () => {
      const words = search.value.toLowerCase().split(/\s+/).filter(Boolean);
      items.forEach((li) => { li.hidden = !words.every((w) => li.textContent.toLowerCase().includes(w)); });
      const rows = visible();
      empty.hidden = rows.length > 0;
      if (rows.length && !rows.some((li) => li.classList.contains('is-on'))) pick(rows[0]);
    });
    addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); search.focus(); search.scrollIntoView({ block: 'center', behavior: calm ? 'auto' : 'smooth' }); }
    });
  }

  // --- Download: offer the visitor's own system first. -----------------------------------------
  const files = {
    windows: { href: 'https://github.com/nirlotan/GlideDeck/releases/latest/download/GlideDeck-x64.msi', label: 'Download for Windows', file: 'GlideDeck-x64.msi, about 40 MB' },
    mac: { href: 'https://github.com/nirlotan/GlideDeck/releases/latest/download/GlideDeck.dmg', label: 'Download for Mac', file: 'GlideDeck.dmg, about 30 MB' },
  };
  const ua = navigator.userAgent;
  const phone = /Android|iPhone|iPad|iPod/i.test(ua) || (/Mac/i.test(ua) && navigator.maxTouchPoints > 1);
  const mine = phone ? null : /Mac/i.test(ua) ? 'mac' : /Win/i.test(ua) ? 'windows' : null;
  const choose = (os) => {
    $$('.seg-choice button').forEach((b) => b.setAttribute('aria-checked', String(b.dataset.os === os)));
    $('#dl-button').href = files[os].href;
    $('#dl-button').textContent = files[os].label;
    $('#dl-file').textContent = files[os].file;
  };
  $$('.seg-choice button').forEach((b) => b.addEventListener('click', () => choose(b.dataset.os)));
  if (mine) {
    choose(mine);
    $$('[data-download]').forEach((a) => { a.href = files[mine].href; a.textContent = files[mine].label.replace('Download', 'Download free'); });
  } else {
    // A phone can't run it: the hero button leads to the download section instead.
    $$('[data-download]').forEach((a) => { a.href = '#download'; a.textContent = 'Get it for your computer'; });
  }

  // --- Entrances: each section's parts arrive in reading order, once. --------------------------
  const sections = $$('[data-reveal]');
  sections.forEach((section) => {
    $$('.rise, .tile, .mark, .cards details, .stages li, .values li', section).forEach((el, i) => el.style.setProperty('--d', i % 9));
  });
  if ('IntersectionObserver' in window) {
    const seen = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) { entry.target.classList.add('in'); seen.unobserve(entry.target); }
      });
    }, { rootMargin: '0px 0px -12% 0px', threshold: 0.08 });
    sections.forEach((s) => seen.observe(s));

    // Loops play only while they are on screen.
    const loops = new IntersectionObserver((entries) => {
      entries.forEach(({ target, isIntersecting }) => {
        if (isIntersecting && !calm) target.play().catch(() => {}); else target.pause();
      });
    }, { threshold: 0.25 });
    $$('video[data-autoplay]').forEach((v) => loops.observe(v));
  } else {
    sections.forEach((s) => s.classList.add('in'));
  }
})();
