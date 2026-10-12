//! Starter templates: complete zooming stories with a theme, frames and a path.
//! Each one is built from slide layouts plus a structure that only an infinite
//! canvas can do: orbits, mountains, honeycombs, spirals, nested portholes.

use crate::layouts::{
    at, bullets, circle, colored, connector, icon, named, place_layout, place_local, rect, shadowed, text, tint, turned, weighted, LayoutKind,
    LAYOUT_W,
};
use glam::DVec2;
use glide_core::theme::builtin_themes;
use glide_core::{Aabb, Color, Document, Element, ElementKind, EllipseShape, Frame, IconStyle, Shadow, Step, Stroke, TextAlign, TextRole, Theme};
use glide_render::TextEngine;
use std::f64::consts::SQRT_2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Template {
    Blank,
    PitchDeck,
    Journey,
    MindMap,
    Lecture,
    Orbit,
    Summit,
    Roadmap,
    Hive,
    DeepDive,
    Showcase,
    Branches,
    Spiral,
    Cycle,
    MoodBoard,
    Workshop,
    Chapters,
}

impl Template {
    /// Gallery order: grouped by [`Template::category`].
    pub const ALL: [Template; 17] = [
        Template::PitchDeck,
        Template::Roadmap,
        Template::Showcase,
        Template::Branches,
        Template::Journey,
        Template::Summit,
        Template::Spiral,
        Template::DeepDive,
        Template::MindMap,
        Template::Hive,
        Template::Orbit,
        Template::MoodBoard,
        Template::Lecture,
        Template::Chapters,
        Template::Workshop,
        Template::Cycle,
        Template::Blank,
    ];

    pub const CATEGORIES: [&'static str; 5] = ["Business", "Storytelling", "Ideas & maps", "Teaching", "Start fresh"];

    pub fn label(self) -> &'static str {
        match self {
            Template::Blank => "Blank canvas",
            Template::PitchDeck => "Pitch deck",
            Template::Journey => "Journey",
            Template::MindMap => "Big picture",
            Template::Lecture => "Lecture",
            Template::Orbit => "Orbit",
            Template::Summit => "Summit",
            Template::Roadmap => "Roadmap",
            Template::Hive => "Hive",
            Template::DeepDive => "Deep dive",
            Template::Showcase => "Showcase",
            Template::Branches => "Branches",
            Template::Spiral => "Spiral",
            Template::Cycle => "Cycle",
            Template::MoodBoard => "Mood board",
            Template::Workshop => "Workshop",
            Template::Chapters => "Chapters",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Template::Blank => "A themed, empty canvas with a title frame.",
            Template::PitchDeck => "Six slides in a grid with an overview: problem, numbers, roadmap.",
            Template::Journey => "Rotated frames along a winding path — Prezi-style storytelling.",
            Template::MindMap => "A central idea with four zoomable branches.",
            Template::Lecture => "An agenda whose topics hide deeper, nested detail.",
            Template::Orbit => "A sun and planets on orbits; each planet zooms into a topic.",
            Template::Summit => "Climb a mountain camp by camp to the goal at the peak.",
            Template::Roadmap => "Quarters × teams on a grid, walked column by column.",
            Template::Hive => "A honeycomb of ideas around one central hexagon.",
            Template::DeepDive => "Every slide hides the next in a porthole. Keep zooming in.",
            Template::Showcase => "A bento grid of slides for a product or portfolio launch.",
            Template::Branches => "A tree from one idea down to topics and details.",
            Template::Spiral => "Slides on a spiral that grows as the story unfolds.",
            Template::Cycle => "Six stages on a wheel; the camera turns as you go.",
            Template::MoodBoard => "Tilted polaroids pinned around a title, like a studio wall.",
            Template::Workshop => "Sticky-note clusters for ideas, questions and actions.",
            Template::Chapters => "A bookshelf: each spine opens into a chapter.",
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            Template::PitchDeck | Template::Roadmap | Template::Showcase | Template::Branches => "Business",
            Template::Journey | Template::Summit | Template::Spiral | Template::DeepDive => "Storytelling",
            Template::MindMap | Template::Hive | Template::Orbit | Template::MoodBoard => "Ideas & maps",
            Template::Lecture | Template::Chapters | Template::Workshop | Template::Cycle => "Teaching",
            Template::Blank => "Start fresh",
        }
    }

    fn theme_name(self) -> &'static str {
        match self {
            Template::Blank | Template::Roadmap | Template::MoodBoard => "Paper",
            Template::PitchDeck | Template::Orbit => "Midnight",
            Template::Lecture | Template::Chapters => "Editorial",
            Template::MindMap | Template::Hive => "Aurora",
            Template::Summit | Template::Cycle => "Forest",
            Template::Journey | Template::Spiral => "Sunset",
            Template::Showcase | Template::Workshop => "Mono",
            Template::DeepDive | Template::Branches => "Ocean",
        }
    }
}

/// Text edits for a placed layout: (element name, new content).
type Edits = &'static [(&'static str, &'static str)];
/// A Branches topic: name, subtitle and its two detail slides.
type Topic = (&'static str, &'static str, [(LayoutKind, &'static str); 2]);

fn theme_named(name: &str) -> Theme {
    builtin_themes().into_iter().find(|t| t.name == name).unwrap_or_else(glide_core::theme::default_theme)
}

/// Sets the text of the first element named `name` placed after the frame.
fn retitle(doc: &mut Document, frame_id: &str, name: &str, content: &str) {
    let start = doc.index_of(frame_id).unwrap_or(0);
    if let Some(ElementKind::Text(t)) = doc.elements[start..].iter_mut().find(|e| e.name == name).map(|e| &mut e.kind) {
        t.content = content.into();
    }
}

fn retitle_all(doc: &mut Document, frame_id: &str, edits: &[(&str, &str)]) {
    for (name, content) in edits {
        retitle(doc, frame_id, name, content);
    }
}

/// Adds an "Overview" frame around `bounds`, at the bottom of the z-order.
fn overview_around(doc: &mut Document, bounds: Aabb) -> String {
    let size = bounds.size();
    let w = size.x.max(size.y * 16.0 / 9.0);
    let mut f = Element::new(ElementKind::Frame(Frame::widescreen(w))).with_transform(at(bounds.center().x, bounds.center().y));
    f.name = "Overview".into();
    let id = doc.add(f);
    if let Some(i) = doc.index_of(&id) {
        let el = doc.elements.remove(i);
        doc.elements.insert(0, el);
    }
    id
}

/// An "Overview" frame around everything on the canvas, with a relative margin.
fn overview_frame(doc: &mut Document, text: &mut TextEngine, margin: f64) -> String {
    let b = doc.elements.iter().fold(Aabb::EMPTY, |b, e| b.union(e.world_aabb(glide_render::local_size(text, &e.kind))));
    let pad = b.size() * margin / 2.0;
    overview_around(doc, Aabb::from_center_half(b.center(), b.size() / 2.0 + pad))
}

/// Whether `p` lies inside any of the given frames (plus a margin in frame units).
fn inside_any_frame(doc: &Document, ids: &[String], p: DVec2, margin: f64) -> bool {
    ids.iter().filter_map(|id| doc.get(id)).any(|f| {
        let size = f.kind.intrinsic_size().unwrap_or_default();
        let local = f.transform.affine().inverse().transform_point2(p);
        local.x.abs() <= size.x / 2.0 + margin && local.y.abs() <= size.y / 2.0 + margin
    })
}

/// A rounded backing panel the size of a frame of `width` (16:9).
fn panel(theme: &Theme, center: DVec2, width: f64, rotation: f64, name: &str) -> Element {
    let h = width * 9.0 / 16.0;
    let el = named(rect(width, h, width * 0.03, theme.surface), name, turned(center.x, center.y, rotation));
    shadowed(el, big_shadow(width))
}

/// A soft shadow proportional to a panel's size.
fn big_shadow(width: f64) -> Shadow {
    Shadow { color: Color::rgba(0x10, 0x14, 0x24, 0x34), blur: width * 0.025, x: 0.0, y: width * 0.012 }
}

/// A tiny deterministic random generator, so templates are identical every time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

pub fn build_template(t: Template) -> Document {
    build_template_with(t, &mut TextEngine::new())
}

/// Builds a template, reusing an existing text engine for measurements.
pub fn build_template_with(t: Template, text: &mut TextEngine) -> Document {
    let theme = theme_named(t.theme_name());
    let mut doc = Document::new(t.label());
    glide_core::theme::apply_theme(&mut doc, &theme);
    let mut path: Vec<String> = Vec::new();
    let d = &mut doc;
    match t {
        Template::Blank => path.push(place_layout(d, LayoutKind::Title, &theme, DVec2::ZERO, 1600.0, 0.0)),
        Template::PitchDeck => pitch_deck(d, &theme, text, &mut path),
        Template::Journey => journey(d, &theme, text, &mut path),
        Template::MindMap => mind_map(d, &theme, text, &mut path),
        Template::Lecture => lecture(d, &theme, &mut path),
        Template::Orbit => orbit(d, &theme, text, &mut path),
        Template::Summit => summit(d, &theme, &mut path),
        Template::Roadmap => roadmap(d, &theme, text, &mut path),
        Template::Hive => hive(d, &theme, text, &mut path),
        Template::DeepDive => deep_dive(d, &theme, &mut path),
        Template::Showcase => showcase(d, &theme, text, &mut path),
        Template::Branches => branches(d, &theme, text, &mut path),
        Template::Spiral => spiral(d, &theme, text, &mut path),
        Template::Cycle => cycle(d, &theme, text, &mut path),
        Template::MoodBoard => mood_board(d, &theme, text, &mut path),
        Template::Workshop => workshop(d, &theme, text, &mut path),
        Template::Chapters => chapters(d, &theme, text, &mut path),
    }
    doc.path = path.into_iter().map(Step::new).collect();
    doc
}

// --- the original five ------------------------------------------------------------

fn pitch_deck(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let slides = [
        (LayoutKind::Title, None),
        (LayoutKind::ThreePoints, Some(("Heading", "Why now"))),
        (LayoutKind::BigNumber, None),
        (LayoutKind::Timeline, Some(("Heading", "Roadmap"))),
        (LayoutKind::Quote, None),
        (LayoutKind::Section, Some(("Heading", "Thank you"))),
    ];
    for (i, (kind, edit)) in slides.into_iter().enumerate() {
        let (col, row) = ((i % 3) as f64, (i / 3) as f64);
        let id = place_layout(doc, kind, theme, DVec2::new((col - 1.0) * 1900.0, (row - 0.5) * 1100.0), 1600.0, 0.0);
        if let Some((name, text)) = edit {
            retitle(doc, &id, name, text);
        }
        if kind == LayoutKind::Section {
            retitle(doc, &id, "Number", "06");
        }
        path.push(id);
    }
    let ov = overview_frame(doc, te, 0.08);
    path.insert(0, ov.clone());
    path.push(ov);
}

fn journey(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let stops = [
        (LayoutKind::Title, -2600.0, 0.0, -6.0),
        (LayoutKind::Section, -900.0, -700.0, 8.0),
        (LayoutKind::ImageText, 800.0, -300.0, -5.0),
        (LayoutKind::Section, 2400.0, -900.0, 10.0),
        (LayoutKind::BigNumber, 3900.0, 100.0, -4.0),
    ];
    // A winding road of dots connecting the stops.
    for w in stops.windows(2) {
        let (a, b) = (DVec2::new(w[0].1, w[0].2), DVec2::new(w[1].1, w[1].2));
        for k in 1..12 {
            let p = a.lerp(b, k as f64 / 12.0) + DVec2::new(0.0, (k as f64 / 12.0 * std::f64::consts::PI).sin() * 260.0);
            doc.add(named(circle(26.0, Some(tint(theme.accents[0], 0x90)), None), "Road", at(p.x, p.y)));
        }
    }
    for (i, (kind, x, y, rot)) in stops.into_iter().enumerate() {
        let id = place_layout(doc, kind, theme, DVec2::new(x, y), 1400.0, rot);
        if kind == LayoutKind::Section {
            retitle(doc, &id, "Number", &format!("0{i}"));
            retitle(doc, &id, "Heading", ["", "Where it started", "", "The turning point"][i.min(3)]);
        }
        path.push(id);
    }
    let ov = overview_frame(doc, te, 0.05);
    path.insert(0, ov.clone());
    path.push(ov);
}

fn mind_map(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let center = place_layout(doc, LayoutKind::Title, theme, DVec2::ZERO, 1600.0, 0.0);
    retitle(doc, &center, "Title", "The big picture");
    doc.add(named(circle(2100.0, Some(tint(theme.accents[0], 0x14)), Some((tint(theme.accents[0], 0x60), 6.0))), "Hub", at(0.0, 0.0)));
    let branches = [
        (LayoutKind::ThreePoints, "Strategy", DVec2::new(-2300.0, -1500.0), "compass"),
        (LayoutKind::BigNumber, "Impact", DVec2::new(2300.0, -1500.0), "chart-line-up"),
        (LayoutKind::Timeline, "Plan", DVec2::new(-2300.0, 1500.0), "calendar-dots"),
        (LayoutKind::Quote, "Voices", DVec2::new(2300.0, 1500.0), "chats-circle"),
    ];
    path.push(center.clone());
    for (i, (kind, title, pos, glyph)) in branches.into_iter().enumerate() {
        // Connector from the hub to the branch.
        let dir = pos.normalize();
        let start = dir * 1050.0;
        doc.add(connector(start, pos - dir * 700.0, 8.0, tint(theme.accents[i], 0x90)));
        doc.add(named(icon(glyph, 160.0, theme.accents[i], IconStyle::Fill), &format!("{title} icon"), at(start.x + dir.x * 220.0, start.y + dir.y * 220.0)));
        let id = place_layout(doc, kind, theme, pos, 1300.0, 0.0);
        retitle(doc, &id, "Heading", title);
        path.push(id);
        path.push(center.clone());
    }
    let ov = overview_frame(doc, te, 0.04);
    path.insert(0, ov);
}

fn lecture(doc: &mut Document, theme: &Theme, path: &mut Vec<String>) {
    let agenda = place_layout(doc, LayoutKind::Agenda, theme, DVec2::ZERO, 1600.0, 0.0);
    retitle(doc, &agenda, "Heading", "Today");
    path.push(agenda.clone());
    // Each agenda item hides a nested slide, 1/20th the size, next to it.
    let kinds = [LayoutKind::Section, LayoutKind::ImageText, LayoutKind::BigNumber, LayoutKind::Quote];
    for (i, kind) in kinds.into_iter().enumerate() {
        let pos = DVec2::new(700.0, -195.0 + i as f64 * 130.0);
        let id = place_layout(doc, kind, theme, pos, 80.0, 0.0);
        if kind == LayoutKind::Section {
            retitle(doc, &id, "Number", &format!("0{}", i + 1));
        }
        path.push(id);
        path.push(agenda.clone());
    }
}

// --- Orbit: a solar system ------------------------------------------------------------

fn orbit(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    let mut rng = Rng(7);
    // A star field.
    for _ in 0..110 {
        let p = DVec2::new(rng.range(-7600.0, 7600.0), rng.range(-4300.0, 4300.0));
        let size = rng.range(12.0, 38.0);
        let alpha = rng.range(48.0, 150.0) as u8;
        doc.add(named(circle(size, Some(tint(theme.heading, alpha)), None), "Star", at(p.x, p.y)));
    }
    // Orbits, tilted into perspective.
    const TILT: f64 = 0.62;
    for r in [2600.0, 3900.0, 5200.0] {
        let ring = ElementKind::Ellipse(EllipseShape {
            width: 2.0 * r,
            height: 2.0 * r * TILT,
            stroke: Some(Stroke { color: tint(theme.muted, 0x50), width: 10.0 }),
            ..Default::default()
        });
        doc.add(named(ring, "Orbit", at(0.0, 0.0)));
    }
    // The sun.
    doc.add(named(circle(3200.0, Some(tint(a[3], 0x10)), None), "Sun glow", at(0.0, 0.0)));
    doc.add(named(circle(2100.0, Some(tint(a[3], 0x1C)), Some((tint(a[3], 0x70), 12.0))), "Sun", at(0.0, 0.0)));
    let sun = place_layout(doc, LayoutKind::Title, theme, DVec2::ZERO, 1600.0, 0.0);
    retitle_all(doc, &sun, &[("Title", "Our universe"), ("Subtitle", "Everything orbits one big idea")]);
    path.push(sun);
    // Planets, from the inner orbit outwards.
    let planets = [
        (LayoutKind::ThreePoints, 2600.0, -40.0, "What we believe"),
        (LayoutKind::Dashboard, 3900.0, 200.0, "Where we are"),
        (LayoutKind::Team, 3900.0, 20.0, "Who we are"),
        (LayoutKind::Process, 5200.0, 252.0, "How we work"),
        (LayoutKind::Venn, 5200.0, 100.0, "Where we fit"),
    ];
    for (i, (kind, r, deg, heading)) in planets.into_iter().enumerate() {
        let t = f64::to_radians(deg);
        let p = DVec2::new(r * t.cos(), r * TILT * t.sin());
        let c = a[i % 4];
        doc.add(named(circle(1560.0, Some(tint(c, 0x1A)), None), "Planet halo", at(p.x, p.y)));
        doc.add(named(circle(1260.0, Some(theme.surface), Some((c, 16.0))), "Planet", at(p.x, p.y)));
        let moon = p + DVec2::from_angle(t + 2.3) * 820.0;
        doc.add(named(circle(170.0, Some(a[(i + 1) % 4]), None), "Moon", at(moon.x, moon.y)));
        let id = place_layout(doc, kind, theme, p, 1000.0, 0.0);
        retitle(doc, &id, "Heading", heading);
        path.push(id);
    }
    let ov = overview_frame(doc, te, 0.02);
    path.insert(0, ov.clone());
    path.push(ov);
}

// --- Summit: climb a mountain ---------------------------------------------------------

fn summit(doc: &mut Document, theme: &Theme, path: &mut Vec<String>) {
    let a = theme.accents;
    let ground = 1200.0;
    // A mountain is a square turned 45° whose lower half is hidden by the ground.
    let mountain = |doc: &mut Document, peak: DVec2, fill: Color, name: &str| {
        let side = (ground - peak.y) * SQRT_2;
        doc.add(named(rect(side, side, 0.0, fill), name, turned(peak.x, ground, 45.0)));
    };
    let snow = |doc: &mut Document, peak: DVec2, depth: f64| {
        let side = depth * SQRT_2;
        doc.add(named(rect(side, side, 0.0, tint(theme.heading, 0xE6)), "Snow", turned(peak.x, peak.y + depth, 45.0)));
    };
    let peaks = [(DVec2::new(-1900.0, -1500.0), 260.0), (DVec2::new(4400.0, -700.0), 200.0)];
    for (p, cap) in peaks {
        mountain(doc, p, tint(a[3], 0x55), "Far peak");
        snow(doc, p, cap);
    }
    let top = DVec2::new(1500.0, -3200.0);
    mountain(doc, top, tint(a[0], 0x40), "Mountain");
    snow(doc, top, 450.0);

    // Camps up the slope, then the summit.
    let camps: [(LayoutKind, DVec2, f64, f64, Edits); 5] = [
        (LayoutKind::Title, DVec2::new(-1000.0, 600.0), 1300.0, -3.0, &[("Title", "Base camp"), ("Subtitle", "Where our climb begins")]),
        (LayoutKind::ThreePoints, DVec2::new(2600.0, -150.0), 1250.0, 4.0, &[("Heading", "What we carry")]),
        (LayoutKind::Staircase, DVec2::new(600.0, -1200.0), 1100.0, -4.0, &[("Heading", "The ascent")]),
        (LayoutKind::Quote, DVec2::new(1500.0, -2000.0), 900.0, 3.0, &[]),
        (LayoutKind::Title, DVec2::new(1500.0, -3980.0), 1100.0, 0.0, &[("Title", "The summit"), ("Subtitle", "Where we’re headed next")]),
    ];
    // A dotted trail from the valley to the peak, hidden under the slides.
    let mut ids = Vec::new();
    let mut frames = Vec::new();
    for (kind, center, width, rot, edits) in camps {
        let id = place_layout(doc, kind, theme, center, width, rot);
        retitle_all(doc, &id, edits);
        frames.push((id.clone(), center));
        ids.push(id);
    }
    let mut trail = vec![DVec2::new(-3200.0, 1150.0)];
    trail.extend(frames.iter().take(4).map(|(_, c)| *c));
    trail.push(top);
    for w in trail.windows(2) {
        let n = ((w[1] - w[0]).length() / 120.0).ceil() as usize;
        for k in 0..n {
            let p = w[0].lerp(w[1], k as f64 / n as f64);
            if !inside_any_frame(doc, &ids, p, 60.0) {
                doc.add(named(circle(34.0, Some(tint(theme.heading, 0x80)), None), "Trail", at(p.x, p.y)));
            }
        }
    }
    let flag = 420.0;
    doc.add(named(icon("flag-banner", flag, a[2], IconStyle::Fill), "Flag", at(top.x + flag * 0.3, top.y - flag * 0.38)));

    // Ground and pine trees in front.
    doc.add(named(rect(17000.0, 5400.0, 0.0, theme.surface), "Ground", at(800.0, ground + 2700.0)));
    let mut rng = Rng(11);
    let mut x: f64 = -4700.0;
    while x < 6400.0 {
        let size = rng.range(320.0, 560.0);
        if (x + 1000.0_f64).abs() > 900.0 {
            let fill = if rng.next() < 0.5 { a[3] } else { tint(a[0], 0xC0) };
            doc.add(named(icon("tree-evergreen", size, fill, IconStyle::Fill), "Tree", at(x, ground - size * 0.4)));
        }
        x += rng.range(260.0, 520.0);
    }
    let ov = overview_around(doc, Aabb::from_center_half(DVec2::new(850.0, -1350.0), DVec2::new(5700.0, 3150.0)));
    path.push(ov.clone());
    path.extend(ids);
    path.push(ov);
}

// --- Roadmap: quarters × lanes ----------------------------------------------------------

fn roadmap(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    let lanes = [("PRODUCT", "cube"), ("MARKETING", "megaphone"), ("PEOPLE", "users-three")];
    let quarters = [("Q1", "Jan – Mar"), ("Q2", "Apr – Jun"), ("Q3", "Jul – Sep"), ("Q4", "Oct – Dec")];
    let items: [[(&str, &str); 3]; 4] = [
        [
            ("Private beta", "Invite 50 design partners\nShip the core editor\nWeekly feedback calls"),
            ("Find our voice", "Brand and messaging\nLaunch the waitlist\nFirst case study"),
            ("Core team", "Hire two engineers\nSet up team rituals\nWrite the handbook"),
        ],
        [
            ("Public launch", "Templates and themes\nExport to PowerPoint\nPerformance pass"),
            ("Launch week", "Launch video and press\nCommunity demo day\nPartner announcements"),
            ("Grow the crew", "Support lead\nDesign hire\nOnboarding program"),
        ],
        [
            ("Teams", "Shared libraries\nPresenter view\nComments"),
            ("Scale the funnel", "Webinar series\nEducation program\nReferral rewards"),
            ("Culture", "First offsite\nCareer ladders\nMentoring"),
        ],
        [
            ("Platform", "Plugin API\nMobile viewer\nAccessibility audit"),
            ("Go global", "Localized site\nRegional partners\nAnnual report"),
            ("Plan 2028", "Retro of the year\nHeadcount plan\nCelebrate"),
        ],
    ];
    let (dx, dy) = (2000.0, 1150.0);
    let title = place_layout(doc, LayoutKind::Title, theme, DVec2::new(1.5 * dx, -2350.0), 3200.0, 0.0);
    retitle_all(doc, &title, &[("Title", "Roadmap 2027"), ("Subtitle", "Four quarters, three teams, one plan")]);
    for (c, (q, months)) in quarters.iter().enumerate() {
        let x = c as f64 * dx;
        doc.add(named(weighted(text_kind(theme, TextRole::Title, q, 260.0), 800), "Quarter", at(x, -830.0)));
        doc.add(named(text_kind(theme, TextRole::Subtitle, months, 70.0), "Months", at(x, -610.0)));
    }
    for (r, (lane, _)) in lanes.iter().enumerate() {
        let y = r as f64 * dy;
        doc.add(named(text_kind(theme, TextRole::Heading, lane, 100.0), "Lane", turned(-1250.0, y, -90.0)));
        doc.add(named(rect(16.0, 900.0, 8.0, a[r]), "Lane bar", at(-1110.0, y)));
    }
    let mut cards = Vec::new();
    for (c, column) in items.iter().enumerate() {
        for (r, (title, points)) in column.iter().enumerate() {
            let center = DVec2::new(c as f64 * dx, r as f64 * dy);
            doc.add(panel(theme, center, 1600.0, 0.0, "Card"));
            let mut frame = Element::new(ElementKind::Frame(Frame::widescreen(LAYOUT_W))).with_transform(at(center.x, center.y));
            frame.name = format!("{} · {}", quarters[c].0, title);
            cards.push(doc.add(frame));
            let (lane, glyph) = lanes[r];
            let content = vec![
                named(rect(330.0, 64.0, 32.0, tint(a[r], 0x30)), "Tag", at(-515.0, -330.0)),
                named(colored(text(theme, TextRole::Caption, lane, Some(26.0), TextAlign::Center, None), theme.heading, Some(700)), "Tag text", at(-515.0, -330.0)),
                named(icon(glyph, 84.0, a[r], IconStyle::Fill), "Lane icon", at(640.0, -330.0)),
                named(text(theme, TextRole::Heading, title, Some(76.0), TextAlign::Left, Some(1360.0)), "Heading", at(0.0, -185.0)),
                named(bullets(theme, points, 46.0, 1360.0), "Points", at(0.0, 110.0)),
            ];
            place_local(doc, content, center, 1.0, 0.0);
        }
    }
    let ov = overview_frame(doc, te, 0.04);
    path.push(title);
    path.push(ov.clone());
    path.extend(cards);
    path.push(ov);
}

fn text_kind(theme: &Theme, role: TextRole, content: &str, size: f64) -> ElementKind {
    text(theme, role, content, Some(size), TextAlign::Center, None)
}

// --- Hive: a honeycomb ------------------------------------------------------------------

fn hive(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    // Phosphor's hexagon glyph is pointy-top: 208 wide and 224 tall in its 256 box.
    let s = 2400.0;
    let gap = s * 0.035;
    let (step_x, step_y) = (s * 0.8125 + gap, s * 0.625 + gap * 0.866);
    let pos = |cx: f64, cy: f64| DVec2::new(cx * step_x, cy * step_y);
    // A faint outer ring suggests the comb keeps going.
    for (cx, cy) in [(1.5, -1.0), (2.0, 0.0), (-1.5, 1.0), (-1.0, 2.0), (1.0, -2.0), (-2.0, 0.0)] {
        let p = pos(cx, cy);
        doc.add(named(icon("hexagon", s, tint(theme.muted, 0x16), IconStyle::Fill), "Empty cell", at(p.x, p.y)));
    }
    let cells: [(f64, f64, LayoutKind, Edits); 7] = [
        (0.0, 0.0, LayoutKind::Title, &[("Title", "Our ecosystem"), ("Subtitle", "Seven ideas, one hive")]),
        (0.5, -1.0, LayoutKind::ThreePoints, &[("Heading", "Why it works")]),
        (1.0, 0.0, LayoutKind::Dashboard, &[("Heading", "By the numbers")]),
        (0.5, 1.0, LayoutKind::Team, &[("Heading", "The people")]),
        (-0.5, 1.0, LayoutKind::HubSpokes, &[("Heading", "Connections")]),
        (-1.0, 0.0, LayoutKind::Pyramid, &[("Heading", "Priorities")]),
        (-0.5, -1.0, LayoutKind::Comparison, &[("Heading", "Choices")]),
    ];
    for (i, (cx, cy, kind, edits)) in cells.into_iter().enumerate() {
        let p = pos(cx, cy);
        let c = a[i % 4];
        doc.add(named(icon("hexagon", s, tint(c, if i == 0 { 0x40 } else { 0x26 }), IconStyle::Fill), "Cell", at(p.x, p.y)));
        doc.add(named(icon("hexagon", s, tint(c, 0x90), IconStyle::Light), "Cell outline", at(p.x, p.y)));
        let id = place_layout(doc, kind, theme, p, s * 0.68, 0.0);
        retitle_all(doc, &id, edits);
        path.push(id);
    }
    let ov = overview_frame(doc, te, 0.03);
    path.insert(0, ov.clone());
    path.push(ov);
}

// --- Deep dive: nested portholes --------------------------------------------------------------

fn deep_dive(doc: &mut Document, theme: &Theme, path: &mut Vec<String>) {
    let a = theme.accents;
    let levels: [(LayoutKind, Edits); 5] = [
        (LayoutKind::Title, &[("Title", "Deep dive"), ("Subtitle", "Let’s go below the surface")]),
        (LayoutKind::Section, &[("Heading", "The surface"), ("Subtitle", "What everyone already sees")]),
        (LayoutKind::BigNumber, &[("Label", "WHAT LIES BENEATH"), ("Number", "90%"), ("Caption", "of an iceberg sits below the waterline")]),
        (LayoutKind::ImageText, &[("Heading", "The details that matter")]),
        (LayoutKind::Quote, &[]),
    ];
    // Where the next level waits inside each slide, in layout units.
    let porthole = DVec2::new(600.0, 290.0);
    let (mut center, mut width, mut rot) = (DVec2::ZERO, 1600.0, 0.0);
    let mut first = None;
    for (i, (kind, edits)) in levels.into_iter().enumerate() {
        let id = place_layout(doc, kind, theme, center, width, rot);
        retitle_all(doc, &id, edits);
        first.get_or_insert_with(|| id.clone());
        path.push(id);
        let s = width / LAYOUT_W;
        if i == 0 {
            let bubbles = [(-690.0, 330.0, 64.0), (-610.0, 250.0, 36.0), (-720.0, 190.0, 24.0), (-560.0, 380.0, 28.0)];
            let els = bubbles.iter().map(|&(x, y, d)| named(circle(d, Some(tint(theme.heading, 0x12)), Some((tint(theme.heading, 0x60), 4.0))), "Bubble", at(x, y))).collect();
            place_local(doc, els, center, s, rot);
        }
        if i + 1 < levels.len() {
            let ring = named(circle(300.0, Some(tint(a[0], 0x1E)), Some((a[0], 5.0))), "Porthole", at(porthole.x, porthole.y));
            place_local(doc, vec![ring], center, s, rot);
            center += DVec2::from_angle(f64::to_radians(rot)).rotate(porthole * s);
            width /= 7.5;
            rot += 6.0;
        }
    }
    // Resurface at the end.
    path.extend(first);
}

// --- Showcase: a bento grid of slides -------------------------------------------------------------

fn showcase(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let (u, uh, gap) = (1600.0, 900.0, 160.0);
    let tiles: [(f64, f64, f64, LayoutKind, Edits); 6] = [
        (0.0, 0.0, 2.0, LayoutKind::Title, &[("Title", "Meet Nova"), ("Subtitle", "A product launch on one canvas")]),
        (2.0, 0.0, 1.0, LayoutKind::ThreePoints, &[("Heading", "Why it’s different")]),
        (2.0, 1.0, 1.0, LayoutKind::Dashboard, &[("Heading", "Early traction")]),
        (0.0, 2.0, 1.0, LayoutKind::Comparison, &[("Heading", "Before and after")]),
        (1.0, 2.0, 1.0, LayoutKind::Team, &[("Heading", "Who built it")]),
        (2.0, 2.0, 1.0, LayoutKind::Bento, &[("Heading", "Everything in one place")]),
    ];
    for (c, r, span, kind, edits) in tiles {
        let size = DVec2::new(span * u + (span - 1.0) * gap, span * uh + (span - 1.0) * gap);
        let center = DVec2::new(c * (u + gap), r * (uh + gap)) + size / 2.0;
        let back = named(rect(size.x, size.y, 60.0, theme.surface), "Tile", at(center.x, center.y));
        doc.add(shadowed(back, big_shadow(u)));
        let id = place_layout(doc, kind, theme, center, size.x, 0.0);
        retitle_all(doc, &id, edits);
        path.push(id);
    }
    let ov = overview_frame(doc, te, 0.05);
    path.insert(0, ov.clone());
    path.push(ov);
}

// --- Branches: a tree ------------------------------------------------------------------------

fn branches(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    let line = tint(theme.muted, 0x90);
    let lw = 14.0;
    let (root_w, child_w, leaf_w) = (1600.0, 1400.0, 700.0);
    let (child_y, leaf_y) = (1500.0, 2700.0);
    let child_x = [-2100.0, 0.0, 2100.0];
    let topics: [Topic; 3] = [
        ("Research", "What we set out to learn", [(LayoutKind::ThreePoints, "What we learned"), (LayoutKind::BigNumber, "")]),
        ("Product", "What we built with it", [(LayoutKind::Process, "How we build"), (LayoutKind::Quote, "")]),
        ("Growth", "Who it reaches next", [(LayoutKind::Dashboard, "Where we are"), (LayoutKind::Team, "Who drives it")]),
    ];
    // Connectors first, so the slides sit on top of them.
    let bus = 1000.0;
    doc.add(connector(DVec2::new(0.0, root_w * 9.0 / 32.0), DVec2::new(0.0, bus), lw, line));
    doc.add(connector(DVec2::new(child_x[0], bus), DVec2::new(child_x[2], bus), lw, line));
    for (k, cx) in child_x.into_iter().enumerate() {
        doc.add(connector(DVec2::new(cx, bus), DVec2::new(cx, child_y), lw, line));
        let leaf_bus = 2200.0;
        doc.add(connector(DVec2::new(cx, child_y), DVec2::new(cx, leaf_bus), lw, line));
        doc.add(connector(DVec2::new(cx - 380.0, leaf_bus), DVec2::new(cx + 380.0, leaf_bus), lw, line));
        for side in [-380.0, 380.0] {
            doc.add(connector(DVec2::new(cx + side, leaf_bus), DVec2::new(cx + side, leaf_y), lw, line));
        }
        for y in [bus, leaf_bus] {
            doc.add(named(circle(48.0, Some(a[k % 4]), None), "Joint", at(cx, y)));
        }
    }
    doc.add(panel(theme, DVec2::ZERO, root_w, 0.0, "Root panel"));
    let root = place_layout(doc, LayoutKind::Title, theme, DVec2::ZERO, root_w, 0.0);
    retitle_all(doc, &root, &[("Title", "One idea"), ("Subtitle", "and everything that grows from it")]);
    path.push(root);
    for (k, (cx, (topic, sub, leaves))) in child_x.into_iter().zip(topics).enumerate() {
        let c = DVec2::new(cx, child_y);
        doc.add(panel(theme, c, child_w, 0.0, "Topic panel"));
        let id = place_layout(doc, LayoutKind::Section, theme, c, child_w, 0.0);
        retitle_all(doc, &id, &[("Number", &format!("{:02}", k + 1)), ("Heading", topic), ("Subtitle", sub)]);
        path.push(id);
        for (side, (kind, heading)) in [-380.0, 380.0].into_iter().zip(leaves) {
            let p = DVec2::new(cx + side, leaf_y);
            doc.add(panel(theme, p, leaf_w, 0.0, "Detail panel"));
            let id = place_layout(doc, kind, theme, p, leaf_w, 0.0);
            if !heading.is_empty() {
                retitle(doc, &id, "Heading", heading);
            }
            path.push(id);
        }
    }
    let ov = overview_frame(doc, te, 0.05);
    path.insert(0, ov.clone());
    path.push(ov);
}

// --- Spiral ------------------------------------------------------------------------------

fn spiral(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    const GROWTH: f64 = 1.45;
    const TURN: f64 = 70.0;
    let radius = |deg: f64| 1300.0 * GROWTH.powf(deg / TURN);
    let stops: [(LayoutKind, Edits); 7] = [
        (LayoutKind::Title, &[("Title", "Spiral"), ("Subtitle", "Every turn takes us further")]),
        (LayoutKind::Section, &[("Heading", "A small start"), ("Subtitle", "Where the first idea came from")]),
        (LayoutKind::ThreePoints, &[("Heading", "What we found")]),
        (LayoutKind::Cycle, &[("Heading", "The loop")]),
        (LayoutKind::Staircase, &[("Heading", "Growing up")]),
        (LayoutKind::BigNumber, &[]),
        (LayoutKind::Quote, &[]),
    ];
    let mut ids = Vec::new();
    let mut placed = Vec::new();
    for (k, (kind, edits)) in stops.into_iter().enumerate() {
        let deg = k as f64 * TURN;
        let w = 1000.0 * GROWTH.powi(k as i32);
        let p = DVec2::from_angle(deg.to_radians()) * radius(deg);
        placed.push((kind, edits, p, w, deg + 90.0));
    }
    // The dotted spiral goes underneath the slides, so test it against
    // stand-in frames before the real ones are placed.
    let mut deg = -300.0;
    let last = (stops.len() - 1) as f64 * TURN + 50.0;
    let mut probe = Document::new("probe");
    let probe_ids: Vec<String> = placed
        .iter()
        .map(|(_, _, p, w, rot)| {
            let f = Element::new(ElementKind::Frame(Frame::widescreen(*w))).with_transform(turned(p.x, p.y, *rot));
            probe.add(f)
        })
        .collect();
    while deg < last {
        let r = radius(deg);
        let p = DVec2::from_angle(deg.to_radians()) * r;
        if !inside_any_frame(&probe, &probe_ids, p, 40.0) {
            let col = a[((deg + 300.0) / TURN) as usize % 4];
            doc.add(named(circle(r * 0.05, Some(tint(col, 0xA0)), None), "Spiral", at(p.x, p.y)));
        }
        deg += 7.0;
    }
    for (kind, edits, p, w, rot) in placed {
        doc.add(panel(theme, p, w * 1.06, rot, "Spiral panel"));
        let id = place_layout(doc, kind, theme, p, w, rot);
        retitle_all(doc, &id, edits);
        ids.push(id);
    }
    let ov = overview_frame(doc, te, 0.04);
    path.push(ov.clone());
    path.extend(ids);
    path.push(ov);
}

// --- Cycle: a wheel -----------------------------------------------------------------------------

fn cycle(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    let r = 3000.0;
    let ring = ElementKind::Ellipse(EllipseShape { width: 2.0 * r, height: 2.0 * r, stroke: Some(Stroke { color: tint(a[0], 0x55), width: 36.0 }), ..Default::default() });
    doc.add(named(ring, "Wheel", at(0.0, 0.0)));
    let center = place_layout(doc, LayoutKind::Title, theme, DVec2::ZERO, 2200.0, 0.0);
    retitle_all(doc, &center, &[("Title", "The flywheel"), ("Subtitle", "Six stages that keep us moving")]);
    path.push(center);
    let stages = [
        (LayoutKind::ThreePoints, "Plan"),
        (LayoutKind::Process, "Build"),
        (LayoutKind::Dashboard, "Measure"),
        (LayoutKind::Venn, "Learn"),
        (LayoutKind::Quadrants, "Decide"),
        (LayoutKind::Staircase, "Grow"),
    ];
    for (k, (kind, heading)) in stages.into_iter().enumerate() {
        let phi = -90.0 + 60.0 * k as f64;
        // Arrowhead halfway to the next stage.
        let mid = phi + 30.0;
        let m = DVec2::from_angle(mid.to_radians()) * r;
        doc.add(named(icon("caret-right", 300.0, a[k % 4], IconStyle::Fill), "Arrow", turned(m.x, m.y, mid + 90.0)));
        let p = DVec2::from_angle(phi.to_radians()) * r;
        doc.add(panel(theme, p, 1900.0, phi + 90.0, "Stage panel"));
        let id = place_layout(doc, kind, theme, p, 1800.0, phi + 90.0);
        retitle(doc, &id, "Heading", heading);
        path.push(id);
    }
    let ov = overview_frame(doc, te, 0.04);
    path.insert(0, ov.clone());
    path.push(ov);
}

// --- Mood board: polaroids on a wall -----------------------------------------------------------

fn mood_board(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    let title = place_layout(doc, LayoutKind::Title, theme, DVec2::ZERO, 2400.0, 0.0);
    retitle_all(doc, &title, &[("Title", "Mood board"), ("Subtitle", "References, textures and ideas")]);
    let slots = [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0)];
    let cards: [(LayoutKind, Edits, &str); 8] = [
        (LayoutKind::ImageText, &[("Heading", "Colour")], "warm, sunlit tones"),
        (LayoutKind::Quote, &[], "words to design by"),
        (LayoutKind::ImageText, &[("Heading", "Texture")], "paper, linen, grain"),
        (LayoutKind::BigNumber, &[("Label", "THE BRIEF"), ("Number", "3"), ("Caption", "words: calm, bold, human")], "the brief"),
        (LayoutKind::ImageText, &[("Heading", "Light")], "soft morning light"),
        (LayoutKind::Bento, &[("Heading", "The system")], "how it fits together"),
        (LayoutKind::Venn, &[("Heading", "The balance")], "finding the overlap"),
        (LayoutKind::ImageText, &[("Heading", "Form")], "simple, rounded shapes"),
    ];
    let mut rng = Rng(3);
    let mut ids = Vec::new();
    for (k, ((col, row), (kind, edits, caption))) in slots.into_iter().zip(cards).enumerate() {
        let c = DVec2::new(col * 2350.0 + rng.range(-120.0, 120.0), row * 1750.0 + rng.range(-100.0, 100.0));
        let tilt = rng.range(-7.0, 7.0);
        let polaroid = named(rect(1760.0, 1170.0, 10.0, theme.surface), "Polaroid", at(0.0, 0.0));
        place_local(doc, vec![shadowed(polaroid, big_shadow(1760.0))], c, 1.0, tilt);
        let id = place_layout(doc, kind, theme, c + DVec2::from_angle(tilt.to_radians()).rotate(DVec2::new(0.0, -55.0)), 1600.0, tilt);
        retitle_all(doc, &id, edits);
        ids.push(id);
        let mut note = text(theme, TextRole::Body, caption, Some(54.0), TextAlign::Center, None);
        if let ElementKind::Text(t) = &mut note {
            t.italic = true;
        }
        let extras = vec![named(note, "Caption", at(0.0, 490.0)), named(icon("push-pin", 140.0, a[k % 4], IconStyle::Fill), "Pin", at(0.0, -580.0))];
        place_local(doc, extras, c, 1.0, tilt);
    }
    let ov = overview_frame(doc, te, 0.03);
    path.push(ov.clone());
    path.push(title);
    path.extend(ids);
    path.push(ov);
}

// --- Workshop: sticky notes ----------------------------------------------------------------------

fn workshop(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    let title = place_layout(doc, LayoutKind::Title, theme, DVec2::ZERO, 1800.0, 0.0);
    retitle_all(doc, &title, &[("Title", "Workshop"), ("Subtitle", "Ideas, questions and next steps")]);
    path.push(title);
    // Sticky notes keep their real-world colours in every theme.
    let stickies = [0xFFE58A, 0xFFC2D1, 0xB9F2D0, 0xBFDBFE, 0xFED7AA, 0xDDD6FE].map(|c: u32| Color::rgb((c >> 16) as u8, (c >> 8) as u8, c as u8));
    let ink = Color::rgb(0x1F, 0x23, 0x28);
    let clusters: [(&str, &str, [&str; 6]); 3] = [
        ("lightbulb", "Ideas", ["Weekly demo day", "A template gallery", "Dark mode for slides", "Live captions", "Offline sharing", "Keyboard-first editing"]),
        ("question", "Questions", ["Who is it for?", "What does success look like?", "What do we stop doing?", "How do we measure it?", "What is the riskiest bet?", "Who decides?"]),
        ("check-circle", "Actions", ["Interview five users", "Draft the roadmap", "Prototype by Friday", "Share notes with the team", "Book a follow-up", "Pick one metric"]),
    ];
    let mut rng = Rng(5);
    let width = 1800.0;
    for (k, (glyph, heading, notes)) in clusters.iter().enumerate() {
        let c = DVec2::new((k as f64 - 1.0) * 2100.0, 1500.0);
        doc.add(panel(theme, c, width, 0.0, "Board"));
        let mut frame = Element::new(ElementKind::Frame(Frame::widescreen(LAYOUT_W))).with_transform(glide_core::Transform { x: c.x, y: c.y, scale: width / LAYOUT_W, rotation: 0.0 });
        frame.name = (*heading).into();
        path.push(doc.add(frame));
        let mut els = vec![
            named(icon(glyph, 72.0, a[k % 4], IconStyle::Fill), "Board icon", at(-690.0, -370.0)),
            named(text(theme, TextRole::Heading, heading, Some(60.0), TextAlign::Left, Some(600.0)), "Heading", at(-310.0, -370.0)),
        ];
        for (i, note) in notes.iter().enumerate() {
            let (x, y) = (-480.0 + (i % 3) as f64 * 480.0, -115.0 + (i / 3) as f64 * 380.0);
            let tilt = rng.range(-4.0, 4.0);
            let sticky = named(rect(330.0, 330.0, 6.0, stickies[(k * 2 + i) % stickies.len()]), "Sticky note", turned(x, y, tilt));
            els.push(shadowed(sticky, Shadow::crisp()));
            els.push(named(colored(text(theme, TextRole::Body, note, Some(36.0), TextAlign::Center, Some(270.0)), ink, Some(500)), "Note", turned(x, y, tilt)));
        }
        place_local(doc, els, c, width / LAYOUT_W, 0.0);
    }
    let next = place_layout(doc, LayoutKind::Process, theme, DVec2::new(0.0, 3000.0), 1800.0, 0.0);
    retitle(doc, &next, "Heading", "Next steps");
    path.push(next);
    let ov = overview_frame(doc, te, 0.05);
    path.insert(0, ov.clone());
    path.push(ov);
}

// --- Chapters: a bookshelf -----------------------------------------------------------------------

fn chapters(doc: &mut Document, theme: &Theme, te: &mut TextEngine, path: &mut Vec<String>) {
    let a = theme.accents;
    let shelf = 1260.0;
    let books: [(f64, f64, &str, LayoutKind); 6] = [
        (420.0, 2400.0, "Origins", LayoutKind::Section),
        (360.0, 2100.0, "The idea", LayoutKind::ThreePoints),
        (480.0, 2550.0, "Building", LayoutKind::Process),
        (380.0, 2250.0, "Setbacks", LayoutKind::Quote),
        (440.0, 2450.0, "Lessons", LayoutKind::Quadrants),
        (400.0, 2000.0, "What’s next", LayoutKind::Staircase),
    ];
    let numerals = ["I", "II", "III", "IV", "V", "VI"];
    let total: f64 = books.iter().map(|b| b.0).sum::<f64>() + 30.0 * (books.len() - 1) as f64;
    let mut x = -total / 2.0;
    let mut ids = Vec::new();
    for (k, (w, h, title, kind)) in books.into_iter().enumerate() {
        let cx = x + w / 2.0;
        let top = shelf - h;
        let paper = theme.background;
        doc.add(shadowed(named(rect(w, h, 18.0, a[k % 4]), "Book", at(cx, shelf - h / 2.0)), Shadow::soft()));
        for y in [top + 110.0, shelf - 90.0] {
            doc.add(named(rect(w - 70.0, 16.0, 8.0, tint(paper, 0x90)), "Band", at(cx, y)));
        }
        doc.add(named(colored(text(theme, TextRole::Heading, numerals[k], Some(64.0), TextAlign::Center, None), paper, Some(700)), "Numeral", at(cx, top + 220.0)));
        // A page "window" on the spine holds the chapter, turned to run along it.
        let (fh, fw) = (w - 90.0, (w - 90.0) * 16.0 / 9.0);
        let fy = shelf - 160.0 - fw / 2.0;
        doc.add(named(rect(fw + 24.0, fh + 24.0, 12.0, paper), "Page", turned(cx, fy, -90.0)));
        let id = place_layout(doc, kind, theme, DVec2::new(cx, fy), fw, -90.0);
        retitle(doc, &id, "Heading", title);
        if kind == LayoutKind::Section {
            retitle(doc, &id, "Number", "01");
        }
        ids.push(id);
        let title_y = (top + 300.0 + fy - fw / 2.0) / 2.0;
        doc.add(named(colored(text(theme, TextRole::Heading, title, Some(104.0), TextAlign::Center, None), paper, Some(700)), "Spine title", turned(cx, title_y, -90.0)));
        x += w + 30.0;
    }
    let plant = 760.0;
    doc.add(named(icon("plant", plant, a[2], IconStyle::Fill), "Plant", at(x + 380.0, shelf - plant * 0.4)));
    doc.add(shadowed(named(rect(total + 1500.0, 90.0, 10.0, theme.heading), "Shelf", at(350.0, shelf + 45.0)), Shadow::soft()));
    let title = place_layout(doc, LayoutKind::Title, theme, DVec2::new(350.0, -2150.0), 3000.0, 0.0);
    retitle_all(doc, &title, &[("Title", "The story so far"), ("Subtitle", "Six chapters, one shelf")]);
    let ov = overview_frame(doc, te, 0.05);
    path.push(title);
    path.push(ov.clone());
    path.extend(ids);
    path.push(ov);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_a_valid_path() {
        let mut text = TextEngine::new();
        for t in Template::ALL {
            let doc = build_template_with(t, &mut text);
            assert!(!doc.path.is_empty(), "{t:?}");
            for i in 0..doc.path.len() {
                assert!(doc.step_frame(i).is_some(), "{t:?} step {i} points at a frame");
            }
            let json = doc.to_json();
            let back = Document::from_json(&json).unwrap();
            assert_eq!(back, doc, "{t:?} round-trips through JSON");
        }
    }

    #[test]
    fn templates_have_unique_names_and_known_categories() {
        let mut labels = std::collections::HashSet::new();
        for t in Template::ALL {
            assert!(labels.insert(t.label().to_lowercase()), "duplicate label {}", t.label());
            assert!(Template::CATEGORIES.contains(&t.category()), "{t:?}");
            assert!(!t.blurb().is_empty());
        }
        assert_eq!(labels.len(), 17);
    }

    /// Slides on the path never overlap each other unless one is nested inside
    /// another (an overview, or a deliberate zoom-in).
    #[test]
    fn template_slides_do_not_collide() {
        let mut text = TextEngine::new();
        for t in Template::ALL {
            let doc = build_template_with(t, &mut text);
            let mut frames: Vec<&Element> = Vec::new();
            for step in &doc.path {
                let f = doc.get(&step.frame).unwrap();
                if !frames.iter().any(|g| g.id == f.id) {
                    frames.push(f);
                }
            }
            let boxes: Vec<(String, Aabb, f64)> = frames
                .iter()
                .map(|f| {
                    let size = f.kind.intrinsic_size().unwrap();
                    let b = f.world_aabb(size);
                    (f.name.clone(), b, size.x * f.transform.scale)
                })
                .collect();
            for (i, (na, a, wa)) in boxes.iter().enumerate() {
                for (nb, b, wb) in &boxes[i + 1..] {
                    let nested = |outer: &Aabb, inner: &Aabb| {
                        inner.min.x >= outer.min.x && inner.max.x <= outer.max.x && inner.min.y >= outer.min.y && inner.max.y <= outer.max.y
                    };
                    if nested(a, b) || nested(b, a) || wa / wb > 3.0 || wb / wa > 3.0 {
                        continue;
                    }
                    let overlap = a.min.x.max(b.min.x) < a.max.x.min(b.max.x) - 1.0 && a.min.y.max(b.min.y) < a.max.y.min(b.max.y) - 1.0;
                    // Rotated frames have loose bounding boxes, so only flag clear overlaps.
                    if overlap {
                        let ia = a.min.x.max(b.min.x)..a.max.x.min(b.max.x);
                        let ja = a.min.y.max(b.min.y)..a.max.y.min(b.max.y);
                        let area = (ia.end - ia.start) * (ja.end - ja.start);
                        let smaller = (a.size().x * a.size().y).min(b.size().x * b.size().y);
                        assert!(area / smaller < 0.2, "{t:?}: {na} overlaps {nb} ({:.0}%)", 100.0 * area / smaller);
                    }
                }
            }
        }
    }
}

