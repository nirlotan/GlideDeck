//! Playing in-slide animations ("builds").
//!
//! [`scene`] answers "what does the document look like at this moment of the
//! show?": objects whose appear-build hasn't played yet are hidden, running
//! builds are mid-effect, and emphasis overlays (spotlight, marker, underline)
//! are added. The show renders the scene instead of the document itself.
//! [`BuildClock`] tracks which builds of the current slide have started.

use glide_core::DVec2;
use glide_core::{Build, BuildKind, Color, Document, Effect, Element, ElementKind, RectShape, Shadow, Transform};
use std::ops::Range;

/// Measures objects for the animations: sizes, and where text lines are.
pub trait Measure {
    /// Local size (text needs laying out).
    fn size(&mut self, e: &Element) -> DVec2;
    /// Text line boxes (x0, top, x1, bottom), local units from the top-left.
    fn lines(&mut self, _e: &Element) -> Vec<[f64; 4]> {
        Vec::new()
    }
}

impl<F: FnMut(&Element) -> DVec2> Measure for F {
    fn size(&mut self, e: &Element) -> DVec2 {
        self(e)
    }
}

impl Measure for glide_render::TextEngine {
    fn size(&mut self, e: &Element) -> DVec2 {
        glide_render::local_size(self, &e.kind)
    }
    fn lines(&mut self, e: &Element) -> Vec<[f64; 4]> {
        match &e.kind {
            ElementKind::Text(t) => self.layout(t).line_boxes(),
            _ => Vec::new(),
        }
    }
}

impl Measure for glide_render::CanvasRenderer {
    fn size(&mut self, e: &Element) -> DVec2 {
        self.text.size(e)
    }
    fn lines(&mut self, e: &Element) -> Vec<[f64; 4]> {
        self.text.lines(e)
    }
}

/// A start time meaning "finished long ago".
pub const DONE: f64 = f64::NEG_INFINITY;

/// Start times (seconds on the show clock) of the current slide's builds.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildClock {
    pub step: usize,
    pub starts: Vec<Option<f64>>,
}

impl BuildClock {
    /// A slide whose builds haven't played.
    pub fn fresh(doc: &Document, step: usize) -> Self {
        BuildClock { step, starts: vec![None; doc.path.get(step).map_or(0, |s| s.builds.len())] }
    }

    /// A slide whose builds have all played (e.g. when stepping back to it).
    pub fn finished(doc: &Document, step: usize) -> Self {
        BuildClock { step, starts: vec![Some(DONE); doc.path.get(step).map_or(0, |s| s.builds.len())] }
    }

    fn builds<'d>(&self, doc: &'d Document) -> &'d [Build] {
        doc.path.get(self.step).map_or(&[], |s| s.builds.as_slice())
    }

    /// The next group of builds that hasn't started.
    pub fn next_group(&self, doc: &Document) -> Option<Range<usize>> {
        glide_core::build::click_groups(self.builds(doc)).into_iter().find(|g| self.starts.get(g.start).is_some_and(Option::is_none))
    }

    /// True when the next group starts by itself (its first build isn't on click).
    pub fn next_is_automatic(&self, doc: &Document) -> bool {
        self.next_group(doc).is_some_and(|g| self.builds(doc)[g.start].trigger != glide_core::Trigger::OnClick)
    }

    /// Starts a group at `now`: "with previous" builds share a start, "after
    /// previous" ones wait for the one before; delays add on top. Returns when
    /// the group's last build ends.
    pub fn start(&mut self, doc: &Document, group: Range<usize>, now: f64) -> f64 {
        let builds = self.builds(doc).to_vec();
        let (mut prev_start, mut prev_end) = (now, now);
        let mut end = now;
        for i in group {
            let b = &builds[i];
            let base = match b.trigger {
                glide_core::Trigger::OnClick => now,
                glide_core::Trigger::WithPrevious => prev_start,
                glide_core::Trigger::AfterPrevious => prev_end,
            };
            let start = base + b.delay.max(0.0);
            if let Some(slot) = self.starts.get_mut(i) {
                *slot = Some(start);
            }
            prev_start = start;
            prev_end = start + b.duration.max(0.0);
            end = end.max(prev_end);
        }
        end
    }

    /// Plays every group in turn from `from`, as if clicked through with a
    /// short pause between them (the editor's preview). Returns the end time.
    pub fn play_all(&mut self, doc: &Document, from: f64) -> f64 {
        let mut t = from;
        for g in glide_core::build::click_groups(self.builds(doc)) {
            t = self.start(doc, g, t) + 0.35;
        }
        t
    }

    /// Whether any started build is still running (or waiting for its delay).
    pub fn running(&self, doc: &Document, now: f64) -> bool {
        self.builds(doc).iter().zip(&self.starts).any(|(b, s)| s.is_some_and(|t| now < t + b.duration))
    }

    /// Jumps every started build to its end.
    pub fn finish_running(&mut self) {
        for s in self.starts.iter_mut().flatten() {
            *s = DONE;
        }
    }

    /// Un-plays the last group that started. Returns false if none had.
    pub fn undo_last(&mut self, doc: &Document) -> bool {
        let groups = glide_core::build::click_groups(self.builds(doc));
        let Some(g) = groups.into_iter().rev().find(|g| self.starts.get(g.start).is_some_and(Option::is_some)) else { return false };
        for i in g {
            self.starts[i] = None;
        }
        true
    }

    #[cfg(test)]
    pub fn all_started(&self) -> bool {
        self.starts.iter().all(Option::is_some)
    }
}

/// The slide an object animates on: the slide it already animates on, else
/// the smallest slide showing it (`prefer` wins ties, e.g. the same frame
/// visited twice).
pub fn owner_step(doc: &Document, id: &str, prefer: Option<usize>) -> Option<usize> {
    if let Some(i) = doc.path.iter().position(|s| s.builds.iter().any(|b| b.element == id)) {
        return Some(i);
    }
    let p = doc.get(id)?.transform.position();
    let area = |i: usize| {
        let f = doc.step_frame(i)?;
        let size = f.kind.intrinsic_size()?;
        let l = f.transform.affine().inverse().transform_point2(p);
        (l.x.abs() <= size.x / 2.0 && l.y.abs() <= size.y / 2.0).then_some(size.x * size.y * f.transform.scale * f.transform.scale)
    };
    let best = (0..doc.path.len()).filter_map(|i| area(i).map(|a| (i, a))).min_by(|a, b| a.1.total_cmp(&b.1))?;
    match prefer.and_then(|i| area(i).map(|a| (i, a))) {
        Some((i, a)) if a <= best.1 * 1.0001 => Some(i),
        _ => Some(best.0),
    }
}

/// Drops animations whose object no longer exists. Returns true if any were.
pub fn prune(doc: &mut Document) -> bool {
    if doc.path.iter().all(|s| s.builds.is_empty()) {
        return false;
    }
    let ids: std::collections::HashSet<String> = doc.elements.iter().map(|e| e.id.clone()).collect();
    let mut changed = false;
    for s in &mut doc.path {
        let before = s.builds.len();
        s.builds.retain(|b| ids.contains(&b.element));
        changed |= s.builds.len() != before;
    }
    changed
}

// --- easing ----------------------------------------------------------------------------

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_cubic(t: f64) -> f64 {
    t * t * t
}

fn ease_in_out(t: f64) -> f64 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn ease_out_back(t: f64) -> f64 {
    let (c1, c3) = (1.70158, 2.70158);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn ease_out_bounce(t: f64) -> f64 {
    let (n, d) = (7.5625, 2.75);
    if t < 1.0 / d {
        n * t * t
    } else if t < 2.0 / d {
        let t = t - 1.5 / d;
        n * t * t + 0.75
    } else if t < 2.5 / d {
        let t = t - 2.25 / d;
        n * t * t + 0.9375
    } else {
        let t = t - 2.625 / d;
        n * t * t + 0.984375
    }
}

fn mix(a: Color, b: Color, t: f64) -> Color {
    let l = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t.clamp(0.0, 1.0)).round() as u8;
    Color::rgba(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

fn with_alpha(c: Color, k: f64) -> Color {
    Color::rgba(c.r(), c.g(), c.b(), (c.a() as f64 * k.clamp(0.0, 1.0)).round() as u8)
}

// --- text effects ------------------------------------------------------------------------

/// Fades parts of a text box: `alpha(byte_range)` for each word or character.
fn fade_text(t: &mut glide_core::TextBlock, ranges: &[(Range<usize>, f64)]) {
    let base = t.color;
    for (r, a) in ranges {
        if *a < 1.0 {
            t.format_range(r.clone(), |s| s.color = Some(with_alpha(s.color.unwrap_or(base), *a)));
        }
    }
}

/// Number tokens in text: digits with `,` or `.` between them.
fn numbers(s: &str) -> Vec<Range<usize>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || ((b[i] == b',' || b[i] == b'.') && i + 1 < b.len() && b[i + 1].is_ascii_digit())) {
                i += 1;
            }
            out.push(start..i);
        } else {
            i += 1;
        }
    }
    out
}

/// "12,432.5" at fraction `k` of its value, keeping its format.
fn scaled_number(token: &str, k: f64) -> String {
    let grouped = token.contains(',');
    let decimals = token.split_once('.').map_or(0, |(_, d)| d.len());
    let value: f64 = token.replace(',', "").parse().unwrap_or(0.0) * k;
    let mut s = format!("{value:.decimals$}");
    if grouped {
        let (int, frac) = s.split_once('.').map(|(a, b)| (a.to_owned(), format!(".{b}"))).unwrap_or((s.clone(), String::new()));
        let mut g = String::new();
        for (n, c) in int.chars().rev().enumerate() {
            if n > 0 && n % 3 == 0 {
                g.push(',');
            }
            g.push(c);
        }
        s = g.chars().rev().collect::<String>() + &frac;
    }
    s
}

/// The text with every number scaled toward its value at progress `p` of a
/// count-up animation (formats such as `$12,400` and `87.5%` are kept).
pub fn count_up_text(content: &str, p: f64) -> String {
    let mut rebuilt = String::new();
    let mut at = 0;
    let k = if p >= 1.0 { 1.0 } else { 1.0 - 2f64.powf(-10.0 * p) };
    for r in numbers(content) {
        rebuilt.push_str(&content[at..r.start]);
        rebuilt.push_str(&scaled_number(&content[r.clone()], k));
        at = r.end;
    }
    rebuilt.push_str(&content[at..]);
    rebuilt
}

// --- the scene --------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    NotYet,
    Running(f64),
    Done,
}

/// How the document looks at `now` with the show on `clock.step`.
/// `size_of` returns an element's local size (text needs measuring).
pub fn scene(doc: &Document, clock: &BuildClock, now: f64, measure: &mut dyn Measure) -> Document {
    let mut out = doc.clone();
    let accent = doc.theme.as_ref().map(|t| t.accents[0]).unwrap_or(Color::rgb(0x8B, 0x7C, 0xFF));
    // Every build in show order, with its phase at this moment.
    let mut timeline: Vec<(usize, &Build, Phase)> = Vec::new();
    for (si, step) in doc.path.iter().enumerate() {
        for (bi, b) in step.builds.iter().enumerate() {
            let phase = if si < clock.step {
                Phase::Done
            } else if si > clock.step {
                Phase::NotYet
            } else {
                match clock.starts.get(bi).copied().flatten() {
                    None => Phase::NotYet,
                    Some(t0) if now < t0 => Phase::NotYet,
                    Some(t0) if now >= t0 + b.duration.max(1e-6) => Phase::Done,
                    Some(t0) => Phase::Running(((now - t0) / b.duration.max(1e-6)).clamp(0.0, 1.0)),
                }
            };
            timeline.push((si, b, phase));
        }
    }
    if timeline.is_empty() {
        return out;
    }
    // The slide each build plays on, for distances and the spotlight.
    let frame_of = |si: usize| doc.step_frame(si).map(|f| (f.transform, f.kind.intrinsic_size().unwrap_or_default() * f.transform.scale.abs()));

    let mut behind: Vec<(String, Element)> = Vec::new(); // overlays drawn just under an element
    let mut spot: Option<(f64, Transform, DVec2)> = None; // strength, slide transform, slide size
    let mut to_front: Vec<String> = Vec::new();

    for el in &mut out.elements {
        let mine: Vec<&(usize, &Build, Phase)> = timeline.iter().filter(|(_, b, _)| b.element == el.id).collect();
        if mine.is_empty() {
            continue;
        }
        let mut visible = mine[0].1.effect.kind() != BuildKind::Appear;
        let mut effects: Vec<(usize, &Build, f64)> = Vec::new();
        for (si, b, phase) in mine.iter().map(|t| **t) {
            match (b.effect.kind(), phase) {
                (BuildKind::Appear, Phase::Done) => visible = true,
                (BuildKind::Disappear, Phase::Done) => visible = false,
                (_, Phase::Running(p)) => {
                    visible = true;
                    effects.push((si, b, p));
                }
                // Emphasis that leaves a mark keeps it for the rest of its slide.
                (BuildKind::Emphasis, Phase::Done) if si == clock.step && matches!(b.effect, Effect::Spotlight | Effect::Highlight | Effect::Underline | Effect::ColorPop) => {
                    effects.push((si, b, 1.0));
                }
                _ => {}
            }
        }
        if !visible {
            el.hidden = true;
            continue;
        }
        let local = measure.size(el);
        let size = local * el.transform.scale.abs();
        for (si, b, p) in effects {
            let Some((ft, fsize)) = frame_of(si) else { continue };
            let turn = DVec2::from_angle(ft.rotation.to_radians());
            let down = turn.rotate(DVec2::Y);
            let q = ease_out_cubic(p);
            let t = &mut el.transform;
            match b.effect {
                Effect::Fade => el.opacity *= ease_in_out(p),
                Effect::Rise => {
                    el.opacity *= q;
                    t.set_position(t.position() + down * (1.0 - q) * fsize.y * 0.08);
                }
                Effect::Focus => {
                    el.opacity *= q;
                    t.scale *= 1.0 + 0.18 * (1.0 - q);
                }
                Effect::Zoom => {
                    el.opacity *= q;
                    t.scale *= 0.3 + 0.7 * q;
                }
                Effect::Pop => {
                    el.opacity *= (p * 3.0).min(1.0);
                    t.scale *= ease_out_back(p).max(0.0);
                    t.rotation += (1.0 - q) * -10.0;
                }
                Effect::Drop => {
                    el.opacity *= (p * 4.0).min(1.0);
                    t.set_position(t.position() - down * (1.0 - ease_out_bounce(p)) * fsize.y * 0.55);
                }
                Effect::Fly => {
                    let (x, y) = b.direction.vector();
                    el.opacity *= (p * 2.0).min(1.0);
                    t.set_position(t.position() + turn.rotate(DVec2::new(x * fsize.x, y * fsize.y)) * 0.75 * (1.0 - q));
                }
                Effect::Spin => {
                    el.opacity *= q;
                    t.scale *= 0.2 + 0.8 * q;
                    t.rotation += (1.0 - q) * -200.0;
                }
                Effect::Typewriter | Effect::Words | Effect::CountUp => match &mut el.kind {
                    ElementKind::Text(text) => match b.effect {
                        Effect::Typewriter => {
                            let chars: Vec<usize> = text.content.char_indices().map(|(i, _)| i).collect();
                            let shown = (p * chars.len() as f64).round() as usize;
                            if let Some(&cut) = chars.get(shown) {
                                let end = text.content.len();
                                fade_text(text, &[(cut..end, 0.0)]);
                            }
                        }
                        Effect::Words => {
                            let words: Vec<Range<usize>> = text
                                .content
                                .split_inclusive(char::is_whitespace)
                                .scan(0, |at, w| {
                                    let r = *at..*at + w.len();
                                    *at += w.len();
                                    Some(r)
                                })
                                .collect();
                            let n = words.len().max(1) as f64;
                            // Each word fades in over the time of ~2 words.
                            let ranges: Vec<(Range<usize>, f64)> = words.into_iter().enumerate().map(|(k, r)| (r, ((p * (n + 1.0) - k as f64) / 2.0).clamp(0.0, 1.0))).collect();
                            fade_text(text, &ranges);
                        }
                        _ => {
                            let rebuilt = count_up_text(&text.content, p);
                            text.set_content(rebuilt);
                            el.opacity *= (p * 4.0).min(1.0);
                        }
                    },
                    // Not text: a simple fade in.
                    _ => el.opacity *= ease_in_out(p),
                },
                Effect::Pulse => t.scale *= 1.0 + 0.14 * (std::f64::consts::PI * p).sin(),
                Effect::Wiggle => t.rotation += 7.0 * (6.0 * std::f64::consts::PI * p).sin() * (1.0 - p),
                Effect::Glow => {
                    let k = (std::f64::consts::PI * p).sin();
                    match &mut el.kind {
                        ElementKind::Text(text) => text.color = mix(text.color, accent, k),
                        ElementKind::Icon(icon) => icon.color = mix(icon.color, accent, k),
                        _ => {
                            el.shadow = Some(Shadow { color: with_alpha(accent, 0.85 * k), blur: size.x.min(size.y) * 0.25 / el.transform.scale.abs().max(1e-12), x: 0.0, y: 0.0 });
                        }
                    }
                }
                Effect::ColorPop => match &mut el.kind {
                    ElementKind::Text(text) => {
                        text.color = mix(text.color, accent, q);
                        for s in &mut text.spans {
                            s.style.color = s.style.color.map(|c| mix(c, accent, q));
                        }
                    }
                    ElementKind::Icon(icon) => icon.color = mix(icon.color, accent, q),
                    ElementKind::Rect(r) => r.fill = r.fill.map(|c| mix(c, accent, q)),
                    ElementKind::Ellipse(e) => e.fill = e.fill.map(|c| mix(c, accent, q)),
                    _ => t.scale *= 1.0 + 0.06 * q,
                },
                Effect::Spotlight => {
                    // The object steps forward a little while the rest dims.
                    t.scale *= 1.0 + 0.06 * q;
                    spot = Some((q, ft, fsize));
                    to_front.push(el.id.clone());
                }
                Effect::Highlight | Effect::Underline => {
                    let underline = b.effect == Effect::Underline;
                    // Line by line for text (hugging the words), else the whole box.
                    let mut lines = measure.lines(el);
                    if lines.is_empty() {
                        lines.push([0.0, 0.0, local.x, local.y]);
                    }
                    let et = el.transform;
                    let k = et.scale.abs();
                    let r = DVec2::from_angle(et.rotation.to_radians());
                    // The marker sweeps across the lines in reading order.
                    let total: f64 = lines.iter().map(|l| (l[2] - l[0]).max(0.0)).sum();
                    let mut budget = total * q;
                    for (n, l) in lines.iter().enumerate() {
                        let (lw, lh) = ((l[2] - l[0]).max(0.0), (l[3] - l[1]).max(1e-9));
                        let pad = lh * 0.03;
                        let w = budget.min(lw + 2.0 * pad);
                        budget -= w;
                        if w <= 0.0 {
                            break;
                        }
                        // A marker over the lower half of the line, or a bar under it.
                        let (y0, h) = if underline { (l[1] + lh * 0.88, (lh * 0.07).max(1e-9)) } else { (l[1] + lh * 0.42, lh * 0.5) };
                        let local_center = DVec2::new(l[0] - pad + w / 2.0, y0 + h / 2.0) - local / 2.0;
                        let center = et.position() + r.rotate(local_center * k);
                        let fill = if underline { accent } else { with_alpha(accent, 0.38) };
                        let mut mark = Element::new(ElementKind::Rect(RectShape { width: w * k, height: h * k, corner_radius: h * k * 0.35, fill: Some(fill), ..Default::default() }))
                            .with_transform(Transform { x: center.x, y: center.y, scale: 1.0, rotation: et.rotation });
                        mark.id = format!("{}~{}{n}", el.id, if underline { "underline" } else { "highlight" });
                        behind.push((el.id.clone(), mark));
                    }
                }
                Effect::FadeOut => el.opacity *= 1.0 - ease_in_out(p),
                Effect::Sink => {
                    el.opacity *= 1.0 - q;
                    t.set_position(t.position() + down * q * fsize.y * 0.08);
                }
                Effect::ZoomOut => {
                    el.opacity *= 1.0 - q;
                    t.scale *= 1.0 + 0.35 * q;
                }
                Effect::FlyOut => {
                    let (x, y) = b.direction.vector();
                    let k = ease_in_cubic(p);
                    el.opacity *= 1.0 - p.powi(3);
                    t.set_position(t.position() + turn.rotate(DVec2::new(x * fsize.x, y * fsize.y)) * 0.75 * k);
                }
                Effect::Shrink => {
                    el.opacity *= 1.0 - p * p;
                    t.scale *= (1.0 - ease_in_cubic(p) * 1.05 + 0.05 * (std::f64::consts::PI * p).sin()).max(0.0);
                }
            }
        }
    }

    // Overlays go just under their element; spotlighted elements go on top of the dimming.
    if !behind.is_empty() {
        let mut els = Vec::with_capacity(out.elements.len() + behind.len());
        for el in std::mem::take(&mut out.elements) {
            for (_, mark) in behind.iter().filter(|(id, _)| *id == el.id) {
                els.push(mark.clone());
            }
            els.push(el);
        }
        out.elements = els;
    }
    if let Some((k, ft, fsize)) = spot {
        let front: Vec<Element> = out.elements.iter().filter(|e| to_front.iter().any(|id| *id == e.id || e.id.starts_with(&format!("{id}~")))).cloned().collect();
        out.elements.retain(|e| !front.iter().any(|f| f.id == e.id));
        // Far larger than the slide, so the dimming has no visible edge.
        let mut dim = Element::new(ElementKind::Rect(RectShape { width: fsize.x * 12.0, height: fsize.y * 12.0, fill: Some(Color::rgba(0, 0, 0, (175.0 * k) as u8)), ..Default::default() }))
            .with_transform(Transform { x: ft.x, y: ft.y, scale: 1.0, rotation: ft.rotation });
        dim.id = "~spotlight".into();
        out.elements.push(dim);
        out.elements.extend(front);
    }
    out
}

/// Moves objects with a parallax depth for a camera looking at `cam`: far
/// objects drift toward the camera's center (so they move slower than the
/// canvas), near ones away from it. The effect fades out as the camera zooms
/// out well past slide size, so overviews show the layout as designed.
pub fn apply_parallax(doc: &mut Document, cam: &glide_core::Camera, viewport: DVec2) {
    if doc.elements.iter().all(|e| e.parallax == 0.0) || viewport.x <= 0.0 {
        return;
    }
    // A typical slide's width on screen, relative to the view.
    let mut widths: Vec<f64> = (0..doc.path.len())
        .filter_map(|i| doc.step_frame(i))
        .filter_map(|f| f.kind.intrinsic_size().map(|s| s.x * f.transform.scale.abs()))
        .collect();
    widths.sort_by(f64::total_cmp);
    let Some(&slide_w) = widths.get(widths.len() / 2) else { return };
    let ratio = slide_w * cam.zoom / viewport.x;
    let t = ((ratio - 0.25) / (0.8 - 0.25)).clamp(0.0, 1.0);
    let strength = t * t * (3.0 - 2.0 * t);
    if strength <= 0.0 {
        return;
    }
    for el in &mut doc.elements {
        if el.parallax != 0.0 {
            let p = el.transform.position();
            let depth = el.parallax.clamp(-2.0, 1.0) * strength;
            el.transform.set_position(p + (cam.center - p) * depth);
        }
    }
}

/// A tasteful cascade for a whole slide: titles rise, icons pop, pictures and
/// cards zoom, in reading order, flowing in one after another on arrival.
pub fn auto_builds(doc: &Document, step: usize, measure: &mut dyn Measure) -> Vec<Build> {
    let Some(frame) = doc.step_frame(step) else { return Vec::new() };
    let fsize = frame.kind.intrinsic_size().unwrap_or_default();
    let inv = frame.transform.affine().inverse();
    let frame_area = fsize.x * fsize.y * frame.transform.scale * frame.transform.scale;
    // An object belongs to the smallest slide showing it (an overview slide
    // doesn't take the content of the slides inside it).
    let path_frames: Vec<(glam::DAffine2, DVec2, f64)> = (0..doc.path.len())
        .filter_map(|i| doc.step_frame(i))
        .filter_map(|f| {
            let s = f.kind.intrinsic_size()?;
            Some((f.transform.affine().inverse(), s, s.x * s.y * f.transform.scale * f.transform.scale))
        })
        .collect();
    let in_smaller_slide = |p: DVec2| {
        path_frames.iter().any(|(inv, s, area)| {
            let l = inv.transform_point2(p);
            *area < frame_area * 0.999 && l.x.abs() <= s.x / 2.0 && l.y.abs() <= s.y / 2.0
        })
    };
    // Content of this slide: inside it, smaller than it, not a frame or a trail.
    let mut items: Vec<(DVec2, &Element)> = doc
        .elements
        .iter()
        .filter(|e| !e.kind.is_frame() && !e.hidden && e.name != crate::arrange::TRAIL)
        .filter_map(|e| {
            let local = inv.transform_point2(e.transform.position());
            let inside = local.x.abs() <= fsize.x / 2.0 && local.y.abs() <= fsize.y / 2.0;
            let s = measure.size(e) * e.transform.scale.abs();
            (inside && s.x * s.y < frame_area * 0.7 && !in_smaller_slide(e.transform.position())).then_some((local, e))
        })
        .collect();
    // Reading order: top to bottom in bands, then left to right.
    let band = fsize.y * 0.08;
    items.sort_by(|(a, _), (b, _)| ((a.y / band).floor(), a.x).partial_cmp(&((b.y / band).floor(), b.x)).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    for (i, (_, e)) in items.into_iter().enumerate() {
        let effect = match &e.kind {
            ElementKind::Text(t) if t.role.is_some_and(|r| r == glide_core::TextRole::Title) => Effect::Focus,
            ElementKind::Text(t) if t.content.chars().any(|c| c.is_ascii_digit()) && t.content.chars().count() < 12 => Effect::CountUp,
            ElementKind::Text(_) => Effect::Rise,
            ElementKind::Icon(_) => Effect::Pop,
            ElementKind::Image(_) | ElementKind::Svg(_) | ElementKind::Video(_) => Effect::Zoom,
            _ => Effect::Fade,
        };
        // The first plays on arrival; the rest overlap in a quick cascade.
        let trigger = if i == 0 { glide_core::Trigger::AfterPrevious } else { glide_core::Trigger::WithPrevious };
        let delay = if i == 0 { 0.05 } else { 0.12 };
        let mut b = Build::new(e.id.clone(), effect).with_trigger(trigger).with_delay(delay);
        if effect == Effect::CountUp {
            b.duration = 1.2;
        }
        out.push(b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use glide_core::{Frame, Step, TextBlock, Trigger};

    fn doc() -> Document {
        let mut d = Document::new("t");
        for (i, x) in [0.0, 2000.0].iter().enumerate() {
            let mut f = Element::new(ElementKind::Frame(Frame::widescreen(1600.0))).with_transform(Transform { x: *x, ..Default::default() });
            f.id = format!("f{i}");
            d.add(f);
            d.path.push(Step::new(format!("f{i}")));
        }
        for (id, x, text) in [("a", 0.0, "Title"), ("b", 100.0, "Revenue 12,400"), ("c", 2000.0, "Later")] {
            let mut t = Element::new(ElementKind::Text(TextBlock::new(text))).with_transform(Transform { x, ..Default::default() });
            t.id = id.into();
            d.add(t);
        }
        d.path[0].builds = vec![Build::new("a", Effect::Rise), Build::new("b", Effect::CountUp).with_trigger(Trigger::AfterPrevious)];
        d.path[1].builds = vec![Build::new("c", Effect::Pop), Build::new("a", Effect::FadeOut)];
        d
    }

    fn size(_: &Element) -> DVec2 {
        DVec2::new(200.0, 60.0)
    }

    fn el<'d>(d: &'d Document, id: &str) -> &'d Element {
        d.get(id).unwrap()
    }

    #[test]
    fn hidden_until_played_and_gone_after_disappearing() {
        let d = doc();
        let mut clock = BuildClock::fresh(&d, 0);
        let s = scene(&d, &clock, 0.0, &mut size);
        assert!(el(&s, "a").hidden && el(&s, "b").hidden, "appear-builds hide their objects first");
        assert!(el(&s, "c").hidden, "objects appearing on later slides are hidden too");
        // Click: "a" rises, then "b" counts up after it.
        let g = clock.next_group(&d).unwrap();
        assert_eq!(g, 0..2);
        clock.start(&d, g, 10.0);
        let mid = scene(&d, &clock, 10.3, &mut size);
        let a = el(&mid, "a");
        assert!(!a.hidden && a.opacity < 1.0 && a.transform.y > 0.0, "rising: {:?}", a.transform);
        assert!(el(&mid, "b").hidden, "the second waits for the first");
        let counting = scene(&d, &clock, 10.6 + 0.5, &mut size);
        let ElementKind::Text(t) = &el(&counting, "b").kind else { unreachable!() };
        assert!(t.content.starts_with("Revenue ") && t.content != "Revenue 12,400" && t.content.contains(','), "{}", t.content);
        assert!(clock.running(&d, 11.0));
        let done = scene(&d, &clock, 20.0, &mut size);
        let ElementKind::Text(t) = &el(&done, "b").kind else { unreachable!() };
        assert_eq!(t.content, "Revenue 12,400");
        assert_eq!((el(&done, "a").opacity, el(&done, "a").transform.y), (1.0, 0.0));
        assert!(!clock.running(&d, 20.0) && clock.all_started());
        // On the next slide "a" fades out and is then hidden.
        let mut next = BuildClock::fresh(&d, 1);
        assert!(!scene(&d, &next, 0.0, &mut size).get("a").unwrap().hidden);
        next.start(&d, 0..1, 0.0);
        next.start(&d, 1..2, 0.0);
        assert!(scene(&d, &next, 5.0, &mut size).get("a").unwrap().hidden);
        // Undo un-plays the last group.
        assert!(next.undo_last(&d));
        assert!(!scene(&d, &next, 5.0, &mut size).get("a").unwrap().hidden);
    }

    #[test]
    fn spotlight_and_highlight_add_overlays() {
        let mut d = doc();
        d.path[0].builds = vec![Build::new("b", Effect::Spotlight), Build::new("a", Effect::Highlight).with_trigger(Trigger::WithPrevious)];
        let mut clock = BuildClock::fresh(&d, 0);
        clock.start(&d, 0..2, 0.0);
        let s = scene(&d, &clock, 5.0, &mut size);
        let ids: Vec<&str> = s.elements.iter().map(|e| e.id.as_str()).collect();
        let dim = ids.iter().position(|i| *i == "~spotlight").expect("dimming layer");
        assert_eq!(ids.last(), Some(&"b"), "the spotlighted object is above the dimming");
        assert!(dim < ids.len() - 1);
        let hl = ids.iter().position(|i| *i == "a~highlight0").expect("marker");
        assert_eq!(ids[hl + 1], "a", "the marker sits right under its text");
    }

    #[test]
    fn words_and_typewriter_keep_the_layout() {
        let mut d = doc();
        d.path[0].builds = vec![Build::new("b", Effect::Words)];
        let mut clock = BuildClock::fresh(&d, 0);
        clock.start(&d, 0..1, 0.0);
        let s = scene(&d, &clock, 0.3, &mut size);
        let ElementKind::Text(t) = &el(&s, "b").kind else { unreachable!() };
        assert_eq!(t.content, "Revenue 12,400", "words are faded, not removed, so nothing reflows");
        assert!(t.spans.iter().any(|s| s.style.color.is_some_and(|c| c.a() < 255)));
    }

    #[test]
    fn preview_plays_groups_in_turn_and_owner_is_the_smallest_slide() {
        let mut d = doc();
        d.path[0].builds.push(Build::new("b", Effect::Pulse));
        let mut clock = BuildClock::fresh(&d, 0);
        let end = clock.play_all(&d, 0.0);
        assert!(clock.all_started());
        assert!(clock.starts[2].unwrap() > clock.starts[1].unwrap() + 0.3, "the second click waits for the first");
        assert!(end > clock.starts[2].unwrap());
        assert_eq!(owner_step(&d, "c", None), Some(1));
        assert_eq!(owner_step(&d, "a", None), Some(0));
        d.elements.retain(|e| e.id != "c");
        assert!(prune(&mut d));
        assert_eq!(d.path[1].builds.len(), 1);
    }

    #[test]
    fn numbers_keep_their_format() {
        assert_eq!(scaled_number("12,400", 0.5), "6,200");
        assert_eq!(scaled_number("87.5", 1.0), "87.5");
        assert_eq!(scaled_number("1,234,567", 1.0), "1,234,567");
        assert_eq!(numbers("Up 18% to $4.2M, 12,400 users"), vec![3..5, 11..14, 17..23]);
    }

    #[test]
    fn auto_animation_cascades_in_reading_order() {
        let d = doc();
        let builds = auto_builds(&d, 0, &mut size);
        let order: Vec<&str> = builds.iter().map(|b| b.element.as_str()).collect();
        assert_eq!(order, ["a", "b"]);
        assert_eq!(builds[0].trigger, Trigger::AfterPrevious, "plays on arrival");
        assert_eq!(builds[1].effect, Effect::Rise);
        assert!(builds[1].delay > 0.0 && builds[1].trigger == Trigger::WithPrevious);
    }
}
