//! Rearranging all slides into a new layout and path: spirals, rings,
//! honeycombs, trees, zoom tunnels and more. Each arrangement is a formula, so
//! it works for any number of slides. A slide's content travels with its
//! frame. Typical use: after importing a PowerPoint deck, which arrives as a
//! plain grid.

use glam::DVec2;
use glide_core::{Aabb, Color, Document, Element, ElementKind, EllipseShape, Frame, RectShape, Step, Transform};
use std::f64::consts::{PI, TAU};

/// Name given to the trail dots and tree lines this module draws, so a later
/// arrangement can replace them.
pub const TRAIL: &str = "Path trail";
/// Name of the parallax backdrop the carousel draws, replaced the same way.
pub const PARALLAX: &str = "Parallax layer";
const OVERVIEW: &str = "Overview";
/// Zoom tunnel: each slide is this fraction of its parent's width…
const TUNNEL_SCALE: f64 = 0.2;
/// …and a new tunnel starts after this many levels (floating-point depth limit).
const TUNNEL_DEPTH: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arrangement {
    Grid,
    Snake,
    Spiral,
    Ring,
    Orbits,
    Wave,
    Journey,
    Staircase,
    Honeycomb,
    Sunflower,
    Tree,
    Tunnel,
    Carousel,
}

impl Arrangement {
    pub const ALL: [Arrangement; 13] = [
        Arrangement::Carousel,
        Arrangement::Spiral,
        Arrangement::Orbits,
        Arrangement::Sunflower,
        Arrangement::Ring,
        Arrangement::Honeycomb,
        Arrangement::Journey,
        Arrangement::Wave,
        Arrangement::Staircase,
        Arrangement::Tree,
        Arrangement::Tunnel,
        Arrangement::Snake,
        Arrangement::Grid,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Arrangement::Grid => "Grid",
            Arrangement::Snake => "Snake",
            Arrangement::Spiral => "Spiral",
            Arrangement::Ring => "Ring",
            Arrangement::Orbits => "Orbits",
            Arrangement::Wave => "Wave",
            Arrangement::Journey => "Journey",
            Arrangement::Staircase => "Staircase",
            Arrangement::Honeycomb => "Honeycomb",
            Arrangement::Sunflower => "Sunflower",
            Arrangement::Tree => "Tree",
            Arrangement::Tunnel => "Zoom tunnel",
            Arrangement::Carousel => "Carousel",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Arrangement::Grid => "Rows, read left to right.",
            Arrangement::Snake => "Rows that turn back at each end, so the path never jumps.",
            Arrangement::Spiral => "Outward along a spiral, slides turning with it.",
            Arrangement::Ring => "One circle that grows with the deck; the camera spins around it.",
            Arrangement::Orbits => "The first slide at the center, the rest on rings around it.",
            Arrangement::Wave => "A gentle sine-wave road, wrapping into rows.",
            Arrangement::Journey => "An irregular, meandering road with tilted slides.",
            Arrangement::Staircase => "Climbing upward in switchback flights.",
            Arrangement::Honeycomb => "A hex pattern spiralling out from the center.",
            Arrangement::Sunflower => "Golden-angle seeds: every step swings around the center.",
            Arrangement::Tree => "Branches like an org chart, visited branch by branch.",
            Arrangement::Tunnel => "Each slide inside the previous one: zoom ever deeper.",
            Arrangement::Carousel => "Slides glide past side by side over a parallax backdrop of glowing layers.",
        }
    }

    /// Stable id for the CLI and MCP (`spiral`, `zoom-tunnel`, …).
    pub fn id(self) -> String {
        self.label().to_lowercase().replace(' ', "-")
    }

    pub fn from_id(s: &str) -> Option<Arrangement> {
        let want = s.trim().to_lowercase().replace([' ', '_'], "-");
        Arrangement::ALL.into_iter().find(|a| a.id() == want || (want == "tunnel" && *a == Arrangement::Tunnel))
    }

    /// Whether this arrangement draws a trail when asked to.
    pub fn has_trail(self) -> bool {
        !matches!(self, Arrangement::Tunnel | Arrangement::Sunflower)
    }

    /// Whether "tilt slides along the path" changes this arrangement.
    pub fn can_tilt(self) -> bool {
        matches!(self, Arrangement::Spiral | Arrangement::Ring | Arrangement::Orbits | Arrangement::Wave | Arrangement::Journey | Arrangement::Staircase | Arrangement::Sunflower | Arrangement::Tunnel)
    }

    /// The tilt setting that suits the arrangement best.
    pub fn default_tilt(self) -> bool {
        matches!(self, Arrangement::Spiral | Arrangement::Ring | Arrangement::Journey | Arrangement::Wave | Arrangement::Tunnel)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrangeOptions {
    /// Gap between slides as a fraction of the slide size.
    pub spacing: f64,
    /// Turn slides to follow the path (the show's camera turns with them).
    pub tilt: bool,
    /// Draw a dotted trail between slides (tree lines for Tree).
    pub trail: bool,
    /// Keep an overview of everything at the start and end of the path.
    pub overview: bool,
}

impl Default for ArrangeOptions {
    fn default() -> Self {
        ArrangeOptions { spacing: 0.25, tilt: true, trail: true, overview: true }
    }
}

/// Where one slide goes: its center, rotation (degrees) and size relative to the cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub center: DVec2,
    pub rotation: f64,
    pub scale: f64,
}

fn at(center: DVec2, rotation: f64) -> Placement {
    Placement { center, rotation, scale: 1.0 }
}

/// Snake order for rows of `per_row`: (column, row) of item `i`.
fn snake(i: usize, per_row: usize) -> (usize, usize) {
    let (row, j) = (i / per_row, i % per_row);
    (if row % 2 == 1 { per_row - 1 - j } else { j }, row)
}

/// Hex rings around the origin in axial coordinates: 0, then 6, 12, 18…
fn hex_spiral(n: usize) -> Vec<(i64, i64)> {
    const DIRS: [(i64, i64); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];
    let mut out = vec![(0, 0)];
    let mut k = 1;
    while out.len() < n {
        let (mut q, mut r) = (-k, k);
        for (dq, dr) in DIRS {
            for _ in 0..k {
                out.push((q, r));
                q += dq;
                r += dr;
            }
        }
        k += 1;
    }
    out.truncate(n);
    out
}

/// Preorder (depth-first) visit of a binary heap of `n` nodes.
fn preorder(n: usize) -> Vec<usize> {
    let mut out = Vec::with_capacity(n);
    let mut stack = vec![0];
    while let Some(h) = stack.pop() {
        if h >= n {
            continue;
        }
        out.push(h);
        stack.push(2 * h + 2);
        stack.push(2 * h + 1);
    }
    out
}

/// Tree lines as pairs of path positions (parent, child).
pub fn tree_edges(n: usize) -> Vec<(usize, usize)> {
    let order = preorder(n);
    let mut pos = vec![0; n];
    for (i, h) in order.iter().enumerate() {
        pos[*h] = i;
    }
    (1..n).map(|h| (pos[(h - 1) / 2], pos[h])).collect()
}

/// Positions for `n` slides of size `cell` (world units), in path order.
pub fn placements(kind: Arrangement, n: usize, cell: DVec2, opts: &ArrangeOptions) -> Vec<Placement> {
    if n == 0 {
        return Vec::new();
    }
    let (w, h) = (cell.x.max(1e-9), cell.y.max(1e-9));
    let s = opts.spacing.clamp(0.0, 3.0);
    let (pw, ph) = (w * (1.0 + s), h * (1.0 + s));
    // Two slides at least this far apart never overlap, whatever their rotation.
    let safe = pw.hypot(ph);
    let tilt = opts.tilt && kind.can_tilt();
    match kind {
        Arrangement::Grid | Arrangement::Snake => {
            let cols = ((n as f64 * (16.0 / 9.0) * ph / pw).sqrt().round() as usize).clamp(1, n);
            (0..n)
                .map(|i| {
                    let (c, r) = if kind == Arrangement::Snake { snake(i, cols) } else { (i % cols, i / cols) };
                    at(DVec2::new((c as f64 - (cols - 1) as f64 / 2.0) * pw, r as f64 * ph), 0.0)
                })
                .collect()
        }
        Arrangement::Spiral => {
            // r = r0 + a·θ; turns are one safe distance apart.
            let gap = if tilt { ph * 1.2 } else { safe };
            let a = gap / TAU;
            let r0 = safe;
            let mut out = vec![at(DVec2::ZERO, 0.0)];
            let mut theta: f64 = 0.0;
            while out.len() < n {
                let r = r0 + a * theta;
                let dir = DVec2::from_angle(theta);
                out.push(at(dir * r, if tilt { theta.to_degrees() + 90.0 } else { 0.0 }));
                // Step along the curve; tilted slides need room on their inner edge.
                let step = if tilt { pw * r / (r - h / 2.0).max(h * 0.25) } else { safe };
                theta += step / r;
            }
            out
        }
        Arrangement::Ring => {
            if n == 1 {
                return vec![at(DVec2::ZERO, 0.0)];
            }
            let half = PI / n as f64;
            let r = if tilt { (pw / (2.0 * half.sin()) + h / 2.0).max(safe / 2.0) } else { safe / (2.0 * half.sin()) };
            (0..n)
                .map(|i| {
                    let ang = -PI / 2.0 + TAU * i as f64 / n as f64;
                    at(DVec2::from_angle(ang) * r, if tilt { ang.to_degrees() + 90.0 } else { 0.0 })
                })
                .collect()
        }
        Arrangement::Orbits => {
            let mut out = vec![at(DVec2::ZERO, 0.0)];
            let mut k = 1;
            while out.len() < n {
                let r = k as f64 * safe;
                // How many fit on this ring without touching.
                let cap = if tilt { (TAU * (r - h / 2.0) / pw).floor() } else { (PI / (safe / (2.0 * r)).min(1.0).asin()).floor() };
                let m = (cap.max(1.0) as usize).min(n - out.len());
                let offset = k as f64 * 0.6;
                for j in 0..m {
                    let ang = -PI / 2.0 + offset + TAU * j as f64 / m as f64;
                    out.push(at(DVec2::from_angle(ang) * r, if tilt { ang.to_degrees() + 90.0 } else { 0.0 }));
                }
                k += 1;
            }
            out
        }
        Arrangement::Wave | Arrangement::Journey => {
            let journey = kind == Arrangement::Journey;
            let per_row = ((n as f64 * 2.5).sqrt().ceil() as usize).max(5);
            let step = if tilt { pw * 1.2 } else { pw };
            let amp = if journey { ph * 1.35 } else { ph * 0.9 };
            let row_pitch = 2.0 * amp + ph * 1.5;
            // y and tilt as functions of the column, so every row has the same shape.
            let y_at = |c: f64| {
                if journey {
                    amp * (0.62 * (0.9 * c + 0.3).sin() + 0.38 * (2.3 * c + 1.7).sin())
                } else {
                    amp * (TAU * c / 6.0).sin()
                }
            };
            (0..n)
                .map(|i| {
                    let (c, r) = snake(i, per_row);
                    let c = c as f64;
                    let x = (c - (per_row - 1) as f64 / 2.0) * step;
                    let slope = (y_at(c + 0.05) - y_at(c - 0.05)) / (0.1 * step);
                    let rot = if !tilt {
                        0.0
                    } else if journey {
                        10.0 * (1.7 * c + 0.5).sin()
                    } else {
                        (slope.atan().to_degrees() * 0.6).clamp(-15.0, 15.0)
                    };
                    at(DVec2::new(x, r as f64 * row_pitch + y_at(c)), rot)
                })
                .collect()
        }
        Arrangement::Staircase => {
            const FLIGHT: usize = 5;
            let mut out = Vec::with_capacity(n);
            let mut x = 0.0;
            for i in 0..n {
                let dir = if (i / FLIGHT).is_multiple_of(2) { 1.0 } else { -1.0 };
                if i > 0 && i % FLIGHT != 0 {
                    x += dir * pw * 0.62;
                }
                out.push(at(DVec2::new(x, -(i as f64) * ph * 1.12), if tilt { -dir * 6.0 } else { 0.0 }));
            }
            out
        }
        Arrangement::Honeycomb => hex_spiral(n)
            .into_iter()
            .map(|(q, r)| at(DVec2::new((q as f64 + r as f64 / 2.0) * pw, r as f64 * ph), 0.0))
            .collect(),
        Arrangement::Sunflower => {
            // Vogel's model: r = c·√i, θ = i·137.5°. Grow c until every pair is a safe distance apart.
            let golden = PI * (3.0 - 5f64.sqrt());
            let points = |c: f64| -> Vec<DVec2> { (0..n).map(|i| DVec2::from_angle(i as f64 * golden) * c * (i as f64).sqrt()).collect() };
            let mut c = safe * 0.5;
            let mut pts = points(c);
            for _ in 0..60 {
                let min = pts.iter().enumerate().flat_map(|(i, a)| pts[i + 1..].iter().map(move |b| a.distance(*b))).fold(f64::INFINITY, f64::min);
                if min >= safe {
                    break;
                }
                c *= 1.06;
                pts = points(c);
            }
            pts.into_iter()
                .enumerate()
                .map(|(i, p)| at(p, if tilt && i > 0 { (i as f64 * golden).to_degrees() + 90.0 } else { 0.0 }))
                .collect()
        }
        Arrangement::Tree => {
            let depth = (usize::BITS - n.leading_zeros()) as i32 - 1; // floor(log2 n)
            let span = 2f64.powi(depth) * pw;
            let heap_pos = |hix: usize| {
                let d = (usize::BITS - (hix + 1).leading_zeros()) as i32 - 1;
                let p = (hix + 1 - (1usize << d)) as f64;
                let slot = 2f64.powi(depth - d) * pw;
                DVec2::new((p + 0.5) * slot - span / 2.0, d as f64 * ph * 1.6)
            };
            preorder(n).into_iter().map(|hix| at(heap_pos(hix), 0.0)).collect()
        }
        Arrangement::Carousel => (0..n).map(|i| at(DVec2::new((i as f64 - (n - 1) as f64 / 2.0) * pw, 0.0), 0.0)).collect(),
        Arrangement::Tunnel => {
            // Each slide sits in a corner of the previous one, at a fifth of its size.
            let corners = [DVec2::new(1.0, 1.0), DVec2::new(-1.0, 1.0), DVec2::new(-1.0, -1.0), DVec2::new(1.0, -1.0)];
            let mut out = Vec::with_capacity(n);
            let mut prev = at(DVec2::ZERO, 0.0);
            for i in 0..n {
                let level = i % TUNNEL_DEPTH;
                let p = if level == 0 {
                    // A new tunnel starts beside the first one.
                    at(DVec2::new((i / TUNNEL_DEPTH) as f64 * pw * 1.3, 0.0), 0.0)
                } else {
                    let k = TUNNEL_SCALE;
                    let corner = corners[level % 4];
                    let local = DVec2::new(corner.x * (w / 2.0) * (1.0 - k - 0.08), corner.y * (h / 2.0) * (1.0 - k - 0.12)) * prev.scale;
                    let center = prev.center + DVec2::from_angle(prev.rotation.to_radians()).rotate(local);
                    Placement { center, rotation: prev.rotation + if tilt { 8.0 } else { 0.0 }, scale: prev.scale * k }
                };
                out.push(p);
                prev = p;
            }
            out
        }
    }
}

pub struct ArrangeReport {
    pub slides: usize,
}

/// The frames on the path in order (each once), split into slides and the
/// overview frames that show many slides at once.
pub fn path_frames(doc: &Document) -> (Vec<String>, Vec<String>) {
    let mut ids: Vec<String> = Vec::new();
    for step in &doc.path {
        if !ids.contains(&step.frame) && doc.get(&step.frame).is_some_and(|e| e.kind.is_frame()) {
            ids.push(step.frame.clone());
        }
    }
    let contains = |outer: &Element, p: DVec2| {
        let size = outer.kind.intrinsic_size().unwrap_or_default();
        let l = outer.transform.affine().inverse().transform_point2(p);
        l.x.abs() <= size.x / 2.0 && l.y.abs() <= size.y / 2.0
    };
    let (mut slides, mut overviews) = (Vec::new(), Vec::new());
    for id in &ids {
        let f = doc.get(id).expect("checked above");
        let inside = ids.iter().filter(|o| *o != id).filter_map(|o| doc.get(o)).filter(|o| contains(f, o.transform.position())).count();
        if f.name == OVERVIEW || inside >= 2 {
            overviews.push(id.clone());
        } else {
            slides.push(id.clone());
        }
    }
    (slides, overviews)
}

/// Rearranges every slide on the path. Content moves (and turns) with its
/// slide; trails from an earlier arrangement are replaced.
pub fn arrange(doc: &mut Document, kind: Arrangement, opts: &ArrangeOptions) -> Result<ArrangeReport, String> {
    let (slides, overviews) = path_frames(doc);
    if slides.is_empty() {
        return Err("there are no slides on the path to arrange".into());
    }
    doc.elements.retain(|e| e.name != TRAIL && e.name != PARALLAX);

    // Each element belongs to the smallest slide containing its center.
    let frames: Vec<(String, glam::DAffine2, DVec2, f64)> = slides
        .iter()
        .filter_map(|id| doc.get(id))
        .map(|f| {
            let size = f.kind.intrinsic_size().unwrap_or_default();
            (f.id.clone(), f.transform.affine().inverse(), size, size.x * size.y * f.transform.scale * f.transform.scale)
        })
        .collect();
    let owner_of = |el: &Element| -> Option<usize> {
        frames
            .iter()
            .enumerate()
            .filter(|(_, (id, inv, size, _))| *id != el.id && {
                let l = inv.transform_point2(el.transform.position());
                l.x.abs() <= size.x / 2.0 && l.y.abs() <= size.y / 2.0
            })
            .min_by(|a, b| a.1 .3.total_cmp(&b.1 .3))
            .map(|(i, _)| i)
    };
    let owners: Vec<Option<usize>> = doc
        .elements
        .iter()
        .map(|e| if slides.contains(&e.id) || overviews.contains(&e.id) { None } else { owner_of(e) })
        .collect();

    // The largest slide sets the grid cell.
    let cell = slides
        .iter()
        .filter_map(|id| doc.get(id))
        .map(|f| f.kind.intrinsic_size().unwrap_or_default() * f.transform.scale.abs())
        .fold(DVec2::ZERO, DVec2::max);
    let places = placements(kind, slides.len(), cell, opts);

    // Old → new transform of every slide.
    let moves: Vec<(DVec2, f64, DVec2, f64, f64)> = slides
        .iter()
        .zip(&places)
        .map(|(id, p)| {
            let f = doc.get(id).expect("slide exists");
            let size = f.kind.intrinsic_size().unwrap_or_default() * f.transform.scale.abs();
            // Slides keep their size, except in the tunnel, where each is a
            // fraction of the one before.
            let k = if p.scale < 1.0 && size.x > 0.0 { p.scale * cell.x / size.x } else { 1.0 };
            (f.transform.position(), f.transform.rotation, p.center, p.rotation, k)
        })
        .collect();
    let apply = |t: &mut Transform, (p0, r0, p1, r1, k): (DVec2, f64, DVec2, f64, f64), is_frame: bool| {
        let d = (r1 - r0).to_radians();
        let offset = DVec2::from_angle(d).rotate(t.position() - p0) * k;
        let pos = if is_frame { p1 } else { p1 + offset };
        t.set_position(pos);
        t.rotation = if is_frame { r1 } else { t.rotation + (r1 - r0) };
        t.scale *= k;
    };
    for (el, owner) in doc.elements.iter_mut().zip(&owners) {
        if let Some(i) = slides.iter().position(|id| *id == el.id) {
            apply(&mut el.transform, moves[i], true);
        } else if let Some(i) = owner {
            apply(&mut el.transform, moves[*i], false);
        }
    }

    // Path order stays as it was; trails and overview follow the new layout.
    // The tunnel nests and the sunflower's path swings across the center on
    // purpose: a straight trail would only add clutter.
    if opts.trail && kind.has_trail() {
        if kind == Arrangement::Carousel {
            draw_parallax(doc, &slides);
        } else {
            draw_trail(doc, &slides, kind);
        }
    }
    // The carousel glides sideways from slide to slide.
    if kind == Arrangement::Carousel {
        for step in doc.path.iter_mut().filter(|s| slides.contains(&s.frame)) {
            step.transition.style = glide_core::TransitionStyle::Direct;
            step.transition.duration = 1.2;
            step.transition.easing = glide_core::Easing::EaseInOutCubic;
        }
    }
    if opts.overview {
        let bounds = slides.iter().filter_map(|id| doc.get(id)).fold(Aabb::EMPTY, |b, f| b.union(f.world_aabb(f.kind.intrinsic_size().unwrap_or_default())));
        let size = bounds.size() * 1.06;
        let width = size.x.max(size.y * 16.0 / 9.0);
        let id = match overviews.first() {
            Some(id) => id.clone(),
            None => {
                let mut f = Element::new(ElementKind::Frame(Frame::widescreen(width)));
                f.name = OVERVIEW.into();
                let id = doc.add(f);
                // Under everything, so clicks reach the slides.
                if let Some(i) = doc.index_of(&id) {
                    let el = doc.elements.remove(i);
                    doc.elements.insert(0, el);
                }
                id
            }
        };
        for ov in std::iter::once(&id).chain(overviews.iter()) {
            if let Some(f) = doc.get_mut(ov) {
                f.kind = ElementKind::Frame(Frame { width, height: width * 9.0 / 16.0, ..match &f.kind {
                    ElementKind::Frame(fr) => fr.clone(),
                    _ => Frame::widescreen(width),
                } });
                f.transform = Transform { x: bounds.center().x, y: bounds.center().y, scale: 1.0, rotation: 0.0 };
            }
        }
        if doc.path.first().is_none_or(|s| s.frame != id) {
            doc.path.insert(0, Step::new(id.clone()));
        }
        if doc.path.last().is_none_or(|s| s.frame != id) {
            doc.path.push(Step::new(id));
        }
    } else {
        doc.path.retain(|s| !overviews.contains(&s.frame));
        doc.elements.retain(|e| !(overviews.contains(&e.id) && e.name == OVERVIEW));
    }
    Ok(ArrangeReport { slides: slides.len() })
}

/// The carousel's backdrop: three layers at different depths that drift at
/// different speeds as the camera glides along (see `Element::parallax`):
/// big soft orbs far behind, rings in the middle, and sparks up close.
fn draw_parallax(doc: &mut Document, slides: &[String]) {
    let frames: Vec<Element> = slides.iter().filter_map(|id| doc.get(id).cloned()).collect();
    let Some(first) = frames.first() else { return };
    let size = first.kind.intrinsic_size().unwrap_or_default() * first.transform.scale.abs();
    let (w, h) = (size.x, size.y);
    let (x0, x1) = frames.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), f| (lo.min(f.transform.x), hi.max(f.transform.x)));
    let y = frames.iter().map(|f| f.transform.y).sum::<f64>() / frames.len() as f64;
    let accents = doc.theme.as_ref().map(|t| t.accents).unwrap_or([Color::rgb(0x8F, 0x7B, 0xFF), Color::rgb(0x2E, 0xE6, 0xC5), Color::rgb(0xFF, 0x7A, 0x5C), Color::rgb(0xE4, 0xFF, 0x6B)]);
    let tint = |c: Color, a: u8| Color::rgba(c.r(), c.g(), c.b(), a);
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };
    let mut add: Vec<Element> = Vec::new();
    let mut put = |kind: ElementKind, at: DVec2, depth: f64| {
        let mut el = Element::new(kind).with_transform(Transform { x: at.x, y: at.y, scale: 1.0, rotation: 0.0 });
        el.name = PARALLAX.into();
        el.parallax = depth;
        add.push(el);
    };
    let span = (x0 - w, x1 + w);
    // Far: glowing orbs, alternating above and below the track.
    let mut x = span.0;
    let mut k = 0;
    while x <= span.1 {
        let d = h * (0.55 + 0.4 * rnd());
        let c = accents[k % 4];
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let fill = Some(tint(c, 0x46));
        let gradient = Some(glide_core::Gradient { to: tint(accents[(k + 1) % 4], 0x00), angle: 90.0 });
        put(ElementKind::Ellipse(EllipseShape { width: d, height: d, fill, gradient, stroke: None }), DVec2::new(x + w * 0.3 * (rnd() - 0.5), y + side * h * (0.3 + 0.25 * rnd())), 0.5);
        x += w * (0.85 + 0.3 * rnd());
        k += 1;
    }
    // Middle: thin rings.
    let mut x = span.0 + w * 0.45;
    while x <= span.1 {
        let d = h * (0.22 + 0.22 * rnd());
        let c = accents[(k + 2) % 4];
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        put(ElementKind::Ellipse(EllipseShape { width: d, height: d, fill: None, gradient: None, stroke: Some(glide_core::Stroke { color: tint(c, 0x70), width: h * 0.006 }) }), DVec2::new(x, y + side * h * (0.62 + 0.12 * rnd())), 0.25);
        x += w * (0.9 + 0.4 * rnd());
        k += 1;
    }
    // Near: small sparks in the margins, moving faster than the slides.
    let mut x = span.0;
    while x <= span.1 {
        let d = h * (0.012 + 0.02 * rnd());
        let c = accents[k % 4];
        let side = if rnd() < 0.5 { -1.0 } else { 1.0 };
        put(ElementKind::Ellipse(EllipseShape { width: d, height: d, fill: Some(tint(c, 0xC0)), gradient: None, stroke: None }), DVec2::new(x, y + side * h * (0.56 + 0.18 * rnd())), -0.3);
        x += w * (0.18 + 0.25 * rnd());
        k += 1;
    }
    // Under everything (and under the trail).
    for (i, el) in add.into_iter().enumerate() {
        let id = doc.add(el);
        if let Some(at) = doc.index_of(&id) {
            let el = doc.elements.remove(at);
            doc.elements.insert(i.min(doc.elements.len()), el);
        }
    }
}

/// Dots between consecutive slides (lines between parent and child for Tree).
fn draw_trail(doc: &mut Document, slides: &[String], kind: Arrangement) {
    let frames: Vec<Element> = slides.iter().filter_map(|id| doc.get(id).cloned()).collect();
    let Some(first) = frames.first() else { return };
    let w = first.kind.intrinsic_size().unwrap_or_default().x * first.transform.scale.abs();
    let color = doc.theme.as_ref().map(|t| t.accents[0]).unwrap_or(Color::rgb(0x8B, 0x7C, 0xFF));
    let dot_color = Color::rgba(color.r(), color.g(), color.b(), 0xA0);
    let inside_any = |p: DVec2| {
        frames.iter().any(|f| {
            let size = f.kind.intrinsic_size().unwrap_or_default();
            let l = f.transform.affine().inverse().transform_point2(p);
            l.x.abs() <= size.x / 2.0 + size.x * 0.03 && l.y.abs() <= size.y / 2.0 + size.x * 0.03
        })
    };
    let mut add = Vec::new();
    if kind == Arrangement::Tree {
        for (a, b) in tree_edges(frames.len()) {
            let (pa, pb) = (frames[a].transform.position(), frames[b].transform.position());
            let half_h = frames[a].kind.intrinsic_size().unwrap_or_default().y * frames[a].transform.scale / 2.0;
            // Elbow: down from the parent, across, down to the child.
            let mid_y = (pa.y + half_h + pb.y - half_h) / 2.0;
            let pts = [DVec2::new(pa.x, pa.y + half_h), DVec2::new(pa.x, mid_y), DVec2::new(pb.x, mid_y), DVec2::new(pb.x, pb.y - half_h)];
            for seg in pts.windows(2) {
                let (s, e) = (seg[0], seg[1]);
                let d = e - s;
                if d.length() < 1e-9 {
                    continue;
                }
                let thick = w * 0.008;
                let mid = (s + e) / 2.0;
                let mut el = Element::new(ElementKind::Rect(RectShape { width: d.length() + thick, height: thick, corner_radius: thick / 2.0, fill: Some(dot_color), ..Default::default() }))
                    .with_transform(Transform { x: mid.x, y: mid.y, scale: 1.0, rotation: d.to_angle().to_degrees() });
                el.name = TRAIL.into();
                add.push(el);
            }
        }
    } else {
        let spacing = w * 0.06;
        let size = w * 0.022;
        for pair in frames.windows(2) {
            let (a, b) = (pair[0].transform.position(), pair[1].transform.position());
            let n = ((b - a).length() / spacing).ceil() as usize;
            for k in 1..n {
                let p = a.lerp(b, k as f64 / n as f64);
                if inside_any(p) {
                    continue;
                }
                let mut el = Element::new(ElementKind::Ellipse(EllipseShape { width: size, height: size, fill: Some(dot_color), ..Default::default() }))
                    .with_transform(Transform { x: p.x, y: p.y, scale: 1.0, rotation: 0.0 });
                el.name = TRAIL.into();
                add.push(el);
            }
        }
    }
    // Trails go under the slides.
    for (i, el) in add.into_iter().enumerate() {
        let id = doc.add(el);
        if let Some(at) = doc.index_of(&id) {
            let el = doc.elements.remove(at);
            doc.elements.insert(i.min(doc.elements.len()), el);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Separating-axis test for two rotated rectangles (with a little tolerance).
    fn overlap(a: (DVec2, DVec2, f64), b: (DVec2, DVec2, f64)) -> bool {
        let corners = |(c, size, rot): (DVec2, DVec2, f64)| {
            let r = DVec2::from_angle(rot.to_radians());
            let h = size / 2.0;
            [DVec2::new(-h.x, -h.y), DVec2::new(h.x, -h.y), DVec2::new(h.x, h.y), DVec2::new(-h.x, h.y)].map(|p| c + r.rotate(p))
        };
        let (ca, cb) = (corners(a), corners(b));
        let axes = [a.2, a.2 + 90.0, b.2, b.2 + 90.0].map(|d| DVec2::from_angle(d.to_radians()));
        axes.iter().all(|ax| {
            let proj = |pts: &[DVec2; 4]| pts.iter().map(|p| p.dot(*ax)).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
            let ((a0, a1), (b0, b1)) = (proj(&ca), proj(&cb));
            a0 < b1 - 1e-6 && b0 < a1 - 1e-6
        })
    }

    #[test]
    fn no_arrangement_overlaps_slides_at_any_count() {
        let cell = DVec2::new(1280.0, 720.0);
        for kind in Arrangement::ALL {
            for tilt in [false, true] {
                let opts = ArrangeOptions { spacing: 0.2, tilt, ..Default::default() };
                for n in (1..=40).chain([64, 120]) {
                    let ps = placements(kind, n, cell, &opts);
                    assert_eq!(ps.len(), n, "{kind:?} n={n}");
                    for (i, a) in ps.iter().enumerate() {
                        for (j, b) in ps.iter().enumerate().skip(i + 1) {
                            if kind == Arrangement::Tunnel {
                                continue; // nested on purpose; checked below
                            }
                            let ra = (a.center, cell * a.scale, a.rotation);
                            let rb = (b.center, cell * b.scale, b.rotation);
                            assert!(!overlap(ra, rb), "{kind:?} tilt={tilt} n={n}: slides {i} and {j} overlap: {a:?} {b:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn tunnel_slides_nest_inside_their_parent() {
        let cell = DVec2::new(1600.0, 900.0);
        let ps = placements(Arrangement::Tunnel, 25, cell, &ArrangeOptions::default());
        for i in 1..ps.len() {
            if i % TUNNEL_DEPTH == 0 {
                assert_eq!(ps[i].scale, 1.0, "a new tunnel starts at full size");
                continue;
            }
            let (p, c) = (ps[i - 1], ps[i]);
            let local = DVec2::from_angle(-p.rotation.to_radians()).rotate(c.center - p.center);
            let half = cell * p.scale / 2.0;
            let child = cell * c.scale / 2.0;
            assert!(local.x.abs() + child.x <= half.x * 1.001 && local.y.abs() + child.y <= half.y * 1.001, "level {i} sticks out");
        }
    }

    #[test]
    fn carousel_glides_over_a_parallax_backdrop() {
        let mut doc = deck(5);
        let report = arrange(&mut doc, Arrangement::Carousel, &ArrangeOptions::default()).unwrap();
        assert_eq!(report.slides, 5);
        let (slides, _) = path_frames(&doc);
        let ys: Vec<f64> = slides.iter().map(|id| doc.get(id).unwrap().transform.y).collect();
        assert!(ys.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-6), "one row");
        let layers: Vec<&Element> = doc.elements.iter().filter(|e| e.name == PARALLAX).collect();
        assert!(layers.len() > 10);
        assert!(layers.iter().any(|e| e.parallax > 0.4) && layers.iter().any(|e| e.parallax < 0.0), "far and near layers");
        let count = layers.len();
        let first_slide = doc.index_of(&slides[0]).unwrap();
        assert!(doc.elements.iter().position(|e| e.name == PARALLAX).unwrap() < first_slide, "the backdrop is behind");
        assert!(doc.path.iter().filter(|s| slides.contains(&s.frame)).all(|s| s.transition.style == glide_core::TransitionStyle::Direct));
        // Arranging again replaces the backdrop instead of stacking it.
        arrange(&mut doc, Arrangement::Carousel, &ArrangeOptions::default()).unwrap();
        assert_eq!(doc.elements.iter().filter(|e| e.name == PARALLAX).count(), count);
        arrange(&mut doc, Arrangement::Grid, &ArrangeOptions::default()).unwrap();
        assert_eq!(doc.elements.iter().filter(|e| e.name == PARALLAX).count(), 0);
    }

    #[test]
    fn ids_round_trip() {
        for a in Arrangement::ALL {
            assert_eq!(Arrangement::from_id(&a.id()), Some(a));
        }
        assert_eq!(Arrangement::from_id("Zoom tunnel"), Some(Arrangement::Tunnel));
    }

    #[test]
    fn tree_edges_connect_parents_to_children() {
        let edges = tree_edges(7);
        assert_eq!(edges.len(), 6);
        // Depth-first: 0 → 1 → 2, then back to 1's sibling.
        assert!(edges.contains(&(0, 1)) && edges.contains(&(1, 2)));
        assert_eq!(preorder(7), vec![0, 1, 3, 4, 2, 5, 6]);
    }

    /// Slides with a text box each, in a row.
    fn deck(n: usize) -> Document {
        let mut doc = Document::new("t");
        for i in 0..n {
            let x = i as f64 * 2000.0;
            let mut f = Element::new(ElementKind::Frame(Frame::widescreen(1600.0))).with_transform(Transform { x, y: 0.0, scale: 1.0, rotation: 0.0 });
            f.id = format!("f{i}");
            doc.add(f);
            let mut t = Element::new(ElementKind::Text(glide_core::TextBlock::new(format!("Slide {i}"))))
                .with_transform(Transform { x: x + 300.0, y: -200.0, scale: 1.0, rotation: 0.0 });
            t.id = format!("t{i}");
            doc.add(t);
            doc.path.push(Step::new(format!("f{i}")));
        }
        doc
    }

    #[test]
    fn content_moves_and_turns_with_its_slide() {
        let mut doc = deck(9);
        arrange(&mut doc, Arrangement::Spiral, &ArrangeOptions { tilt: true, trail: true, overview: true, spacing: 0.25 }).unwrap();
        for i in 0..9 {
            let f = doc.get(&format!("f{i}")).unwrap().transform;
            let t = doc.get(&format!("t{i}")).unwrap().transform;
            // The text keeps its place relative to the slide: (300, -200) in the slide's own frame.
            let local = DVec2::from_angle(-f.rotation.to_radians()).rotate(t.position() - f.position());
            assert!((local - DVec2::new(300.0, -200.0)).length() < 1e-6, "slide {i}: {local:?}");
            assert!((t.rotation - f.rotation).abs() < 1e-9);
        }
        // The path is unchanged, with an overview at both ends.
        let frames: Vec<&str> = doc.path.iter().map(|s| s.frame.as_str()).collect();
        assert_eq!(frames.len(), 11);
        assert_eq!(frames[0], frames[10]);
        assert_eq!(doc.get(frames[0]).unwrap().name, "Overview");
        assert_eq!(&frames[1..10], &["f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8"]);
        let trail = doc.elements.iter().filter(|e| e.name == TRAIL).count();
        assert!(trail > 0);
        // Arranging again replaces the trail and reuses the overview.
        arrange(&mut doc, Arrangement::Honeycomb, &ArrangeOptions::default()).unwrap();
        assert_eq!(doc.elements.iter().filter(|e| e.name == "Overview").count(), 1);
        assert_eq!(doc.path.len(), 11);
        assert!(doc.elements.iter().filter(|e| e.name == TRAIL).count() > 0);
        // Turning the overview and trail off removes them.
        arrange(&mut doc, Arrangement::Grid, &ArrangeOptions { trail: false, overview: false, ..Default::default() }).unwrap();
        assert_eq!(doc.path.len(), 9);
        assert!(!doc.elements.iter().any(|e| e.name == TRAIL || e.name == "Overview"));
    }

    #[test]
    fn tunnel_shrinks_slides_and_their_content() {
        let mut doc = deck(3);
        arrange(&mut doc, Arrangement::Tunnel, &ArrangeOptions { overview: false, ..Default::default() }).unwrap();
        let s = |id: &str| doc.get(id).unwrap().transform.scale;
        assert!((s("f1") - TUNNEL_SCALE).abs() < 1e-9 && (s("t1") - TUNNEL_SCALE).abs() < 1e-9);
        assert!((s("f2") - TUNNEL_SCALE * TUNNEL_SCALE).abs() < 1e-12);
        assert!(arrange(&mut Document::new("empty"), Arrangement::Ring, &ArrangeOptions::default()).is_err());
    }
}
