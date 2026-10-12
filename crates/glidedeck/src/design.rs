//! Design intelligence: slide layouts, starter templates, alignment, and the
//! design check. Everything here is plain document manipulation, so the same
//! building blocks serve the editor, tests and (later) agents.

use glam::DVec2;
use glide_core::theme::{blend_over, builtin_themes, contrast_ratio};
use glide_core::{
    assets, Aabb, Color, Document, Element, ElementKind, EllipseShape, Frame, Icon, IconStyle, RectShape, Shadow, Step,
    TextAlign, TextBlock, TextRole, Theme, Transform,
};
use glide_render::TextEngine;

/// Layouts are authored in a 1600 × 900 frame centred on the origin.
pub const LAYOUT_W: f64 = 1600.0;
#[cfg(test)]
pub const LAYOUT_H: f64 = 900.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayoutKind {
    Title,
    Section,
    ThreePoints,
    BigNumber,
    Quote,
    Agenda,
    Timeline,
    ImageText,
}

impl LayoutKind {
    pub const ALL: [LayoutKind; 8] = [
        LayoutKind::Title,
        LayoutKind::Section,
        LayoutKind::ThreePoints,
        LayoutKind::BigNumber,
        LayoutKind::Quote,
        LayoutKind::Agenda,
        LayoutKind::Timeline,
        LayoutKind::ImageText,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LayoutKind::Title => "Title",
            LayoutKind::Section => "Section",
            LayoutKind::ThreePoints => "Three points",
            LayoutKind::BigNumber => "Big number",
            LayoutKind::Quote => "Quote",
            LayoutKind::Agenda => "Agenda",
            LayoutKind::Timeline => "Timeline",
            LayoutKind::ImageText => "Image + text",
        }
    }
}

// --- element helpers ---------------------------------------------------------------

fn at(x: f64, y: f64) -> Transform {
    Transform { x, y, scale: 1.0, rotation: 0.0 }
}

fn text(theme: &Theme, role: TextRole, content: &str, size: Option<f64>, align: TextAlign, wrap: Option<f64>) -> ElementKind {
    let mut t = theme.text_style(role);
    t.content = content.into();
    if let Some(s) = size {
        t.font_size = s;
    }
    t.align = align;
    t.max_width = wrap;
    ElementKind::Text(t)
}

fn named(kind: ElementKind, name: &str, t: Transform) -> Element {
    let mut e = Element::new(kind).with_transform(t);
    e.name = name.into();
    e
}

fn card(theme: &Theme, w: f64, h: f64) -> ElementKind {
    ElementKind::Rect(RectShape {
        width: w,
        height: h,
        corner_radius: 28.0,
        fill: Some(theme.surface),
        ..Default::default()
    })
}

fn icon(name: &str, size: f64, color: Color, style: IconStyle) -> ElementKind {
    ElementKind::Icon(Icon { name: name.into(), style, size, color })
}

fn tint(c: Color, alpha: u8) -> Color {
    Color::rgba(c.r(), c.g(), c.b(), alpha)
}

/// The content of a layout in layout-local coordinates (frame excluded).
pub fn layout_elements(kind: LayoutKind, theme: &Theme) -> Vec<Element> {
    let a = theme.accents;
    let mut v = Vec::new();
    match kind {
        LayoutKind::Title => {
            v.push(named(text(theme, TextRole::Title, "Your big idea", Some(120.0), TextAlign::Center, None), "Title", at(0.0, -60.0)));
            v.push(named(
                ElementKind::Rect(RectShape { width: 140.0, height: 10.0, corner_radius: 5.0, fill: Some(a[0]), ..Default::default() }),
                "Accent bar",
                at(0.0, 50.0),
            ));
            v.push(named(
                text(theme, TextRole::Subtitle, "A subtitle that sets the scene", Some(40.0), TextAlign::Center, None),
                "Subtitle",
                at(0.0, 130.0),
            ));
        }
        LayoutKind::Section => {
            v.push(named(text(theme, TextRole::Title, "01", Some(240.0), TextAlign::Center, None), "Number", at(-430.0, 0.0)));
            if let Some(ElementKind::Text(t)) = v.last_mut().map(|e| &mut e.kind) {
                t.color = a[0];
            }
            v.push(named(
                ElementKind::Rect(RectShape { width: 8.0, height: 300.0, corner_radius: 4.0, fill: Some(tint(theme.muted, 0x60)), ..Default::default() }),
                "Divider",
                at(-220.0, 0.0),
            ));
            v.push(named(
                text(theme, TextRole::Heading, "Section title", Some(84.0), TextAlign::Left, Some(820.0)),
                "Heading",
                at(250.0, -50.0),
            ));
            v.push(named(
                text(theme, TextRole::Subtitle, "What this part of the story is about", Some(34.0), TextAlign::Left, Some(820.0)),
                "Subtitle",
                at(250.0, 70.0),
            ));
        }
        LayoutKind::ThreePoints => {
            v.push(named(text(theme, TextRole::Heading, "Three things to remember", Some(64.0), TextAlign::Center, None), "Heading", at(0.0, -330.0)));
            let items = [("lightning", "Fast", "Explain the first point in a sentence or two."), ("shield-check", "Safe", "Back it up with a short, concrete detail."), ("heart", "Loved", "Close with why it matters to the audience.")];
            for (i, (glyph, title, body)) in items.iter().enumerate() {
                let x = -510.0 + i as f64 * 510.0;
                let mut c = named(card(theme, 440.0, 500.0), &format!("Card {}", i + 1), at(x, 70.0));
                c.shadow = Some(Shadow::soft());
                v.push(c);
                v.push(named(
                    ElementKind::Ellipse(EllipseShape { width: 120.0, height: 120.0, fill: Some(tint(a[i % 4], 0x26)), ..Default::default() }),
                    "Icon badge",
                    at(x, -70.0),
                ));
                v.push(named(icon(glyph, 64.0, a[i % 4], IconStyle::Fill), &format!("Icon {}", i + 1), at(x, -70.0)));
                v.push(named(text(theme, TextRole::Heading, title, Some(44.0), TextAlign::Center, None), "Card title", at(x, 60.0)));
                v.push(named(text(theme, TextRole::Body, body, Some(26.0), TextAlign::Center, Some(360.0)), "Card body", at(x, 170.0)));
            }
        }
        LayoutKind::BigNumber => {
            v.push(named(text(theme, TextRole::Caption, "THE NUMBER THAT MATTERS", Some(28.0), TextAlign::Center, None), "Label", at(0.0, -270.0)));
            v.push(named(text(theme, TextRole::Title, "87%", Some(300.0), TextAlign::Center, None), "Number", at(0.0, -40.0)));
            if let Some(ElementKind::Text(t)) = v.last_mut().map(|e| &mut e.kind) {
                t.color = a[0];
                t.weight = 800;
            }
            v.push(named(
                text(theme, TextRole::Subtitle, "of teams shipped faster after switching", Some(40.0), TextAlign::Center, Some(1100.0)),
                "Caption",
                at(0.0, 220.0),
            ));
        }
        LayoutKind::Quote => {
            v.push(named(icon("quotes", 150.0, a[0], IconStyle::Fill), "Quote mark", at(-600.0, -230.0)));
            v.push(named(
                text(theme, TextRole::Quote, "Simplicity is the ultimate sophistication — and the best stories zoom in on exactly one idea.", Some(56.0), TextAlign::Left, Some(1150.0)),
                "Quote",
                at(60.0, -20.0),
            ));
            v.push(named(text(theme, TextRole::Caption, "— Someone worth quoting", Some(30.0), TextAlign::Left, Some(1150.0)), "Attribution", at(60.0, 220.0)));
        }
        LayoutKind::Agenda => {
            v.push(named(text(theme, TextRole::Heading, "Agenda", Some(84.0), TextAlign::Left, Some(560.0)), "Heading", at(-460.0, -40.0)));
            v.push(named(
                ElementKind::Rect(RectShape { width: 8.0, height: 520.0, corner_radius: 4.0, fill: Some(a[0]), ..Default::default() }),
                "Divider",
                at(-140.0, 0.0),
            ));
            for (i, item) in ["Where we are", "What we learned", "Where we go next", "Questions"].iter().enumerate() {
                let y = -195.0 + i as f64 * 130.0;
                v.push(named(
                    ElementKind::Ellipse(EllipseShape { width: 76.0, height: 76.0, fill: Some(a[i % 4]), ..Default::default() }),
                    "Number chip",
                    at(-20.0, y),
                ));
                let mut n = text(theme, TextRole::Heading, &(i + 1).to_string(), Some(34.0), TextAlign::Center, None);
                if let ElementKind::Text(t) = &mut n {
                    t.color = Color::WHITE;
                    t.role = None;
                }
                v.push(named(n, "Number", at(-20.0, y)));
                v.push(named(text(theme, TextRole::Body, item, Some(40.0), TextAlign::Left, Some(640.0)), "Item", at(400.0, y)));
            }
        }
        LayoutKind::Timeline => {
            v.push(named(text(theme, TextRole::Heading, "The road so far", Some(64.0), TextAlign::Center, None), "Heading", at(0.0, -300.0)));
            v.push(named(
                ElementKind::Rect(RectShape { width: 1320.0, height: 6.0, corner_radius: 3.0, fill: Some(tint(theme.muted, 0x70)), ..Default::default() }),
                "Line",
                at(0.0, 30.0),
            ));
            for (i, (year, label)) in [("2023", "First prototype"), ("2024", "Private beta"), ("2025", "Public launch"), ("2026", "What’s next")].iter().enumerate() {
                let x = -495.0 + i as f64 * 330.0;
                v.push(named(
                    ElementKind::Ellipse(EllipseShape {
                        width: 44.0,
                        height: 44.0,
                        fill: Some(a[i % 4]),
                        stroke: Some(glide_core::Stroke { color: theme.background, width: 8.0 }),
                        ..Default::default()
                    }),
                    "Milestone",
                    at(x, 30.0),
                ));
                v.push(named(text(theme, TextRole::Heading, year, Some(48.0), TextAlign::Center, None), "Year", at(x, -60.0)));
                v.push(named(text(theme, TextRole::Body, label, Some(28.0), TextAlign::Center, Some(280.0)), "Milestone label", at(x, 120.0)));
            }
        }
        LayoutKind::ImageText => {
            let mut ph = named(
                ElementKind::Rect(RectShape { width: 680.0, height: 640.0, corner_radius: 32.0, fill: Some(tint(theme.muted, 0x22)), ..Default::default() }),
                "Image placeholder",
                at(-390.0, 0.0),
            );
            ph.shadow = Some(Shadow::soft());
            v.push(ph);
            v.push(named(icon("image", 120.0, tint(theme.muted, 0xb0), IconStyle::Light), "Image icon", at(-390.0, -30.0)));
            v.push(named(text(theme, TextRole::Caption, "Paste or drop an image here", Some(26.0), TextAlign::Center, None), "Hint", at(-390.0, 80.0)));
            v.push(named(text(theme, TextRole::Heading, "A picture and a point", Some(60.0), TextAlign::Left, Some(640.0)), "Heading", at(410.0, -170.0)));
            v.push(named(
                text(theme, TextRole::Body, "• Lead with the visual\n• Keep each line short\n• One idea per slide", Some(34.0), TextAlign::Left, Some(640.0)),
                "Points",
                at(410.0, 60.0),
            ));
        }
    }
    v
}

/// Places a layout as a new frame on the canvas: the frame is centred at
/// `center` with world `width` and `rotation` (degrees). Returns the frame id.
pub fn place_layout(doc: &mut Document, kind: LayoutKind, theme: &Theme, center: DVec2, width: f64, rotation: f64) -> String {
    let s = width / LAYOUT_W;
    let mut frame = Element::new(ElementKind::Frame(Frame::widescreen(LAYOUT_W)))
        .with_transform(Transform { x: center.x, y: center.y, scale: s, rotation });
    frame.name = format!("{} {}", kind.label(), doc.elements.iter().filter(|e| e.kind.is_frame()).count() + 1);
    let frame_id = doc.add(frame);
    let rot = DVec2::from_angle(rotation.to_radians());
    for mut el in layout_elements(kind, theme) {
        let local = el.transform.position();
        let world = center + rot.rotate(local * s);
        el.transform = Transform { x: world.x, y: world.y, scale: el.transform.scale * s, rotation: el.transform.rotation + rotation };
        doc.add(el);
    }
    frame_id
}

// --- templates -------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Template {
    Blank,
    PitchDeck,
    Journey,
    MindMap,
    Lecture,
}

impl Template {
    pub const ALL: [Template; 5] = [Template::PitchDeck, Template::Journey, Template::MindMap, Template::Lecture, Template::Blank];

    pub fn label(self) -> &'static str {
        match self {
            Template::Blank => "Blank canvas",
            Template::PitchDeck => "Pitch deck",
            Template::Journey => "Journey",
            Template::MindMap => "Big picture",
            Template::Lecture => "Lecture",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Template::Blank => "A themed, empty canvas with a title frame.",
            Template::PitchDeck => "Six slides in a grid with an overview: problem, numbers, roadmap.",
            Template::Journey => "Rotated frames along a winding path — Prezi-style storytelling.",
            Template::MindMap => "A central idea with four zoomable branches.",
            Template::Lecture => "An agenda whose topics hide deeper, nested detail.",
        }
    }

    fn theme_name(self) -> &'static str {
        match self {
            Template::Blank => "Paper",
            Template::PitchDeck => "Midnight",
            Template::Journey => "Sunset",
            Template::MindMap => "Aurora",
            Template::Lecture => "Editorial",
        }
    }
}

fn theme_named(name: &str) -> Theme {
    builtin_themes().into_iter().find(|t| t.name == name).unwrap_or_else(glide_core::theme::default_theme)
}

/// Sets the text of the first element named `name` placed after `after`.
fn retitle(doc: &mut Document, frame_id: &str, name: &str, content: &str) {
    let start = doc.index_of(frame_id).unwrap_or(0);
    if let Some(ElementKind::Text(t)) = doc.elements[start..].iter_mut().find(|e| e.name == name).map(|e| &mut e.kind) {
        t.content = content.into();
    }
}

fn overview_frame(doc: &mut Document, margin: f64) -> String {
    let mut text = TextEngine::new();
    let b = doc.elements.iter().fold(Aabb::EMPTY, |b, e| b.union(e.world_aabb(glide_render::local_size(&mut text, &e.kind))));
    let size = b.size() * (1.0 + margin);
    let w = size.x.max(size.y * 16.0 / 9.0);
    let mut f = Element::new(ElementKind::Frame(Frame::widescreen(w))).with_transform(at(b.center().x, b.center().y));
    f.name = "Overview".into();
    let id = doc.add(f);
    // Keep the overview frame at the bottom of the z-order.
    if let Some(i) = doc.index_of(&id) {
        let el = doc.elements.remove(i);
        doc.elements.insert(0, el);
    }
    id
}

pub fn build_template(t: Template) -> Document {
    let theme = theme_named(t.theme_name());
    let mut doc = Document::new(t.label());
    glide_core::theme::apply_theme(&mut doc, &theme);
    let mut path: Vec<String> = Vec::new();
    match t {
        Template::Blank => {
            path.push(place_layout(&mut doc, LayoutKind::Title, &theme, DVec2::ZERO, 1600.0, 0.0));
        }
        Template::PitchDeck => {
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
                let id = place_layout(&mut doc, kind, &theme, DVec2::new((col - 1.0) * 1900.0, (row - 0.5) * 1100.0), 1600.0, 0.0);
                if let Some((name, text)) = edit {
                    retitle(&mut doc, &id, name, text);
                }
                if kind == LayoutKind::Section {
                    retitle(&mut doc, &id, "Number", "06");
                }
                path.push(id);
            }
            let ov = overview_frame(&mut doc, 0.08);
            path.insert(0, ov.clone());
            path.push(ov);
        }
        Template::Journey => {
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
                    doc.add(named(
                        ElementKind::Ellipse(EllipseShape { width: 26.0, height: 26.0, fill: Some(tint(theme.accents[0], 0x90)), ..Default::default() }),
                        "Road",
                        at(p.x, p.y),
                    ));
                }
            }
            for (i, (kind, x, y, rot)) in stops.into_iter().enumerate() {
                let id = place_layout(&mut doc, kind, &theme, DVec2::new(x, y), 1400.0, rot);
                if kind == LayoutKind::Section {
                    retitle(&mut doc, &id, "Number", &format!("0{i}"));
                    retitle(&mut doc, &id, "Heading", ["", "Where it started", "", "The turning point"][i.min(3)]);
                }
                path.push(id);
            }
            let ov = overview_frame(&mut doc, 0.05);
            path.insert(0, ov.clone());
            path.push(ov);
        }
        Template::MindMap => {
            let center = place_layout(&mut doc, LayoutKind::Title, &theme, DVec2::ZERO, 1600.0, 0.0);
            retitle(&mut doc, &center, "Title", "The big picture");
            let hub = named(
                ElementKind::Ellipse(EllipseShape { width: 2100.0, height: 2100.0, fill: Some(tint(theme.accents[0], 0x14)), stroke: Some(glide_core::Stroke { color: tint(theme.accents[0], 0x60), width: 6.0 }), ..Default::default() }),
                "Hub",
                at(0.0, 0.0),
            );
            doc.add(hub);
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
                let end = pos - dir * 700.0;
                let mid = (start + end) * 0.5;
                let mut line = named(
                    ElementKind::Rect(RectShape { width: (end - start).length(), height: 8.0, corner_radius: 4.0, fill: Some(tint(theme.accents[i], 0x90)), ..Default::default() }),
                    "Connector",
                    at(mid.x, mid.y),
                );
                line.transform.rotation = dir.to_angle().to_degrees();
                doc.add(line);
                doc.add(named(icon(glyph, 160.0, theme.accents[i], IconStyle::Fill), &format!("{title} icon"), at(start.x + dir.x * 220.0, start.y + dir.y * 220.0)));
                let id = place_layout(&mut doc, kind, &theme, pos, 1300.0, 0.0);
                retitle(&mut doc, &id, "Heading", title);
                path.push(id);
                path.push(center.clone());
            }
            let ov = overview_frame(&mut doc, 0.04);
            path.insert(0, ov);
        }
        Template::Lecture => {
            let agenda = place_layout(&mut doc, LayoutKind::Agenda, &theme, DVec2::ZERO, 1600.0, 0.0);
            retitle(&mut doc, &agenda, "Heading", "Today");
            path.push(agenda.clone());
            // Each agenda item hides a nested slide, 1/20th the size, next to it.
            let item_ys: Vec<f64> = (0..4).map(|i| -195.0 + i as f64 * 130.0).collect();
            let kinds = [LayoutKind::Section, LayoutKind::ImageText, LayoutKind::BigNumber, LayoutKind::Quote];
            for (i, (y, kind)) in item_ys.into_iter().zip(kinds).enumerate() {
                let pos = DVec2::new(700.0, y);
                let id = place_layout(&mut doc, kind, &theme, pos, 80.0, 0.0);
                if kind == LayoutKind::Section {
                    retitle(&mut doc, &id, "Number", &format!("0{}", i + 1));
                }
                path.push(id);
                path.push(agenda.clone());
            }
        }
    }
    doc.path = path.into_iter().map(Step::new).collect();
    doc
}

// --- alignment ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignOp {
    Left,
    HCenter,
    Right,
    Top,
    VMiddle,
    Bottom,
    DistributeH,
    DistributeV,
}

/// Aligns elements in view space (so results look right in a rotated view).
/// A single element aligns to the smallest frame that contains it.
pub fn align(doc: &mut Document, text: &mut TextEngine, ids: &[String], op: AlignOp, view_rotation: f64) {
    let to_view = DVec2::from_angle(-view_rotation);
    let to_world = DVec2::from_angle(view_rotation);
    let mut boxes: Vec<(usize, Aabb)> = Vec::new();
    for id in ids {
        let Some(i) = doc.index_of(id) else { continue };
        let el = &doc.elements[i];
        if el.locked {
            continue;
        }
        let size = glide_render::local_size(text, &el.kind);
        boxes.push((i, Aabb::from_points(el.world_corners(size).map(|c| to_view.rotate(c)))));
    }
    if boxes.is_empty() {
        return;
    }
    let reference = if boxes.len() == 1 {
        let (i, _) = boxes[0];
        let center = doc.elements[i].transform.position();
        let mut best: Option<Aabb> = None;
        for (j, f) in doc.elements.iter().enumerate() {
            if j == i || !f.kind.is_frame() {
                continue;
            }
            let size = f.kind.intrinsic_size().unwrap_or_default();
            let inv = f.transform.affine().inverse();
            let local = inv.transform_point2(center);
            if local.x.abs() <= size.x / 2.0 && local.y.abs() <= size.y / 2.0 {
                let fb = Aabb::from_points(f.world_corners(size).map(|c| to_view.rotate(c)));
                if best.is_none_or(|bb| fb.size().x * fb.size().y < bb.size().x * bb.size().y) {
                    best = Some(fb);
                }
            }
        }
        match best {
            Some(f) => f,
            None => return,
        }
    } else {
        boxes.iter().fold(Aabb::EMPTY, |acc, (_, b)| acc.union(*b))
    };

    let mut deltas: Vec<(usize, DVec2)> = Vec::new();
    match op {
        AlignOp::DistributeH | AlignOp::DistributeV => {
            if boxes.len() < 3 {
                return;
            }
            let horizontal = op == AlignOp::DistributeH;
            let key = |b: &Aabb| if horizontal { b.center().x } else { b.center().y };
            boxes.sort_by(|a, b| key(&a.1).total_cmp(&key(&b.1)));
            let first = key(&boxes[0].1);
            let last = key(&boxes[boxes.len() - 1].1);
            let n = boxes.len() - 1;
            for (k, (i, b)) in boxes.iter().enumerate() {
                let target = first + (last - first) * k as f64 / n as f64;
                let d = target - key(b);
                deltas.push((*i, if horizontal { DVec2::new(d, 0.0) } else { DVec2::new(0.0, d) }));
            }
        }
        _ => {
            for (i, b) in &boxes {
                let d = match op {
                    AlignOp::Left => DVec2::new(reference.min.x - b.min.x, 0.0),
                    AlignOp::HCenter => DVec2::new(reference.center().x - b.center().x, 0.0),
                    AlignOp::Right => DVec2::new(reference.max.x - b.max.x, 0.0),
                    AlignOp::Top => DVec2::new(0.0, reference.min.y - b.min.y),
                    AlignOp::VMiddle => DVec2::new(0.0, reference.center().y - b.center().y),
                    AlignOp::Bottom => DVec2::new(0.0, reference.max.y - b.max.y),
                    _ => DVec2::ZERO,
                };
                deltas.push((*i, d));
            }
        }
    }
    for (i, d) in deltas {
        let t = &mut doc.elements[i].transform;
        t.set_position(t.position() + to_world.rotate(d));
    }
}

// --- design check ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Warning,
    Suggestion,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    TextColor { id: String, color: Color },
    ScaleText { id: String, factor: f64 },
    UnifyFonts,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub target: Option<String>,
    pub step: Option<usize>,
    pub fix: Option<Fix>,
}

/// Smallest path frame containing each point, with its step index.
fn owning_step(doc: &Document, p: DVec2) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64, f64)> = None;
    for (i, step) in doc.path.iter().enumerate() {
        let Some(f) = doc.get(&step.frame) else { continue };
        let Some(size) = f.kind.intrinsic_size() else { continue };
        let local = f.transform.affine().inverse().transform_point2(p);
        if local.x.abs() <= size.x / 2.0 && local.y.abs() <= size.y / 2.0 {
            let h = size.y * f.transform.scale.abs();
            let area = size.x * size.y * f.transform.scale * f.transform.scale;
            if best.is_none_or(|(_, a, _)| area < a) {
                best = Some((i, area, h));
            }
        }
    }
    best.map(|(i, _, h)| (i, h))
}

/// The opaque color behind a point, looking at filled shapes beneath `below`.
fn backdrop_at(doc: &Document, text: &mut TextEngine, p: DVec2, below: usize) -> Vec<Color> {
    let mut base = vec![doc.background];
    if let Some(to) = doc.background_to {
        base.push(to);
    }
    for el in doc.elements[..below].iter().rev().filter(|e| !e.hidden) {
        let fill = match &el.kind {
            ElementKind::Rect(r) => r.fill,
            ElementKind::Ellipse(e) => e.fill,
            ElementKind::Frame(f) => f.fill,
            _ => None,
        };
        let Some(fill) = fill else { continue };
        if fill.a() < 0x40 {
            continue;
        }
        let size = glide_render::local_size(text, &el.kind);
        if el.hit_test(p, size, 0.0) {
            return base.iter().map(|b| blend_over(fill, *b)).collect();
        }
    }
    base
}

pub fn design_check(doc: &Document, text: &mut TextEngine, base_dir: &std::path::Path, theme: &Theme) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut fonts = std::collections::BTreeSet::new();

    for (i, el) in doc.elements.iter().enumerate().filter(|(_, e)| !e.hidden) {
        let center = el.transform.position();
        match &el.kind {
            ElementKind::Text(t) => {
                if t.content.trim().is_empty() {
                    continue;
                }
                fonts.insert(t.font_family.clone());
                let backs = backdrop_at(doc, text, center, i);
                let fg = |b: Color| blend_over(t.color, b);
                let worst = backs.iter().map(|b| contrast_ratio(fg(*b), *b)).fold(f64::INFINITY, f64::min);
                let large = t.weight >= 600 || t.role.is_some_and(|r| r.is_display());
                let needed = if large { 3.0 } else { 4.5 };
                if worst < needed {
                    let candidates = [theme.text, theme.heading, Color::WHITE, Color::rgb(0x11, 0x11, 0x14)];
                    let best = candidates
                        .into_iter()
                        .max_by(|a, b| {
                            let ca = backs.iter().map(|bg| contrast_ratio(*a, *bg)).fold(f64::INFINITY, f64::min);
                            let cb = backs.iter().map(|bg| contrast_ratio(*b, *bg)).fold(f64::INFINITY, f64::min);
                            ca.total_cmp(&cb)
                        })
                        .unwrap_or(Color::BLACK);
                    issues.push(Issue {
                        level: Level::Warning,
                        title: format!("Low contrast: “{}”", el.display_name()),
                        detail: format!("Contrast is {worst:.1}:1; aim for at least {needed:.1}:1 so it reads on a projector."),
                        target: Some(el.id.clone()),
                        step: owning_step(doc, center).map(|(s, _)| s),
                        fix: Some(Fix::TextColor { id: el.id.clone(), color: best }),
                    });
                }
                if let Some((step, frame_h)) = owning_step(doc, center) {
                    let px = t.font_size * el.transform.scale.abs() * 1080.0 / frame_h.max(1e-12);
                    if px < 18.0 {
                        issues.push(Issue {
                            level: Level::Warning,
                            title: format!("Small text on slide {}", step + 1),
                            detail: format!("“{}” would be about {px:.0} px tall on a 1080p screen. 24 px or more reads comfortably.", el.display_name()),
                            target: Some(el.id.clone()),
                            step: Some(step),
                            fix: Some(Fix::ScaleText { id: el.id.clone(), factor: 24.0 / px.max(1e-9) }),
                        });
                    }
                }
            }
            ElementKind::Image(m) => {
                let path = assets::resolve(Some(base_dir), &m.path);
                match glide_render::image_dimensions(&path) {
                    None => issues.push(Issue {
                        level: Level::Warning,
                        title: format!("Missing image: {}", el.display_name()),
                        detail: format!("{} could not be found next to the document.", m.path),
                        target: Some(el.id.clone()),
                        step: None,
                        fix: None,
                    }),
                    Some((_, h)) => {
                        if let Some((step, frame_h)) = owning_step(doc, center) {
                            let shown = m.height * el.transform.scale.abs() * 1080.0 / frame_h.max(1e-12);
                            if shown > h as f64 * 1.6 {
                                issues.push(Issue {
                                    level: Level::Suggestion,
                                    title: format!("Low-resolution image on slide {}", step + 1),
                                    detail: format!("Shown at ~{shown:.0} px tall but the file is {h} px; it may look soft."),
                                    target: Some(el.id.clone()),
                                    step: Some(step),
                                    fix: None,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Empty slides.
    for (i, step) in doc.path.iter().enumerate() {
        let Some(f) = doc.get(&step.frame) else { continue };
        let Some(size) = f.kind.intrinsic_size() else { continue };
        let inv = f.transform.affine().inverse();
        let has_content = doc.elements.iter().any(|e| {
            !e.kind.is_frame() && !e.hidden && {
                let l = inv.transform_point2(e.transform.position());
                l.x.abs() <= size.x / 2.0 && l.y.abs() <= size.y / 2.0
            }
        });
        if !has_content {
            issues.push(Issue {
                level: Level::Suggestion,
                title: format!("Slide {} is empty", i + 1),
                detail: "Add content, or insert a layout, so the camera has something to show.".into(),
                target: Some(f.id.clone()),
                step: Some(i),
                fix: None,
            });
        }
    }

    if fonts.len() > 3 {
        issues.push(Issue {
            level: Level::Suggestion,
            title: format!("{} different fonts", fonts.len()),
            detail: format!("Using the theme's two fonts ({} / {}) gives a calmer, more consistent look.", theme.heading_font, theme.body_font),
            target: None,
            step: None,
            fix: Some(Fix::UnifyFonts),
        });
    }
    issues.sort_by_key(|i| i.level);
    issues
}

pub fn apply_fix(doc: &mut Document, theme: &Theme, fix: &Fix) {
    match fix {
        Fix::TextColor { id, color } => {
            if let Some(ElementKind::Text(t)) = doc.get_mut(id).map(|e| &mut e.kind) {
                t.color = *color;
            }
        }
        Fix::ScaleText { id, factor } => {
            if let Some(el) = doc.get_mut(id) {
                el.transform.scale *= factor;
            }
        }
        Fix::UnifyFonts => {
            for el in &mut doc.elements {
                if let ElementKind::Text(t) = &mut el.kind {
                    let display = t.role.map(|r| r.is_display()).unwrap_or(t.weight >= 600);
                    t.font_family = if display { theme.heading_font.clone() } else { theme.body_font.clone() };
                }
            }
        }
    }
}

/// A text box with a theme role, for quick inserts.
pub fn styled_text(theme: &Theme, role: TextRole) -> TextBlock {
    let mut t = theme.text_style(role);
    t.content = match role {
        TextRole::Title => "Title",
        TextRole::Heading => "Heading",
        TextRole::Subtitle => "Subtitle",
        TextRole::Body => "Body text",
        TextRole::Caption => "Caption",
        TextRole::Quote => "“A memorable quote”",
    }
    .into();
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_a_valid_path() {
        for t in Template::ALL {
            let doc = build_template(t);
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
    fn layouts_fit_inside_their_frame() {
        let theme = glide_core::theme::default_theme();
        let mut text = TextEngine::new();
        for kind in LayoutKind::ALL {
            for el in layout_elements(kind, &theme) {
                let b = el.world_aabb(glide_render::local_size(&mut text, &el.kind));
                assert!(
                    b.min.x >= -LAYOUT_W / 2.0 && b.max.x <= LAYOUT_W / 2.0 && b.min.y >= -LAYOUT_H / 2.0 && b.max.y <= LAYOUT_H / 2.0,
                    "{kind:?}: {} overflows the frame: {b:?}",
                    el.name
                );
            }
        }
    }

    #[test]
    fn align_and_distribute() {
        let mut doc = Document::new("t");
        let mut text = TextEngine::new();
        let mut ids = Vec::new();
        for (x, y, w) in [(0.0, 0.0, 10.0), (50.0, 30.0, 20.0), (200.0, -10.0, 10.0)] {
            let el = Element::new(ElementKind::Rect(RectShape { width: w, height: 10.0, ..Default::default() })).with_transform(at(x, y));
            ids.push(doc.add(el));
        }
        align(&mut doc, &mut text, &ids, AlignOp::Left, 0.0);
        let lefts: Vec<f64> = doc.elements.iter().map(|e| e.transform.x - e.kind.intrinsic_size().unwrap().x / 2.0).collect();
        assert!(lefts.iter().all(|l| (l - lefts[0]).abs() < 1e-9));
        align(&mut doc, &mut text, &ids, AlignOp::Top, 0.0);
        assert!(doc.elements.iter().all(|e| (e.transform.y - doc.elements[0].transform.y).abs() < 1e-9));
        doc.elements[2].transform.x = 300.0;
        align(&mut doc, &mut text, &ids, AlignOp::DistributeH, 0.0);
        let xs: Vec<f64> = doc.elements.iter().map(|e| e.transform.x).collect();
        assert!(((xs[1] - xs[0]) - (xs[2] - xs[1])).abs() < 1e-9, "{xs:?}");
    }

    #[test]
    fn design_check_flags_and_fixes_contrast() {
        let theme = glide_core::theme::default_theme();
        let mut doc = Document::new("t");
        doc.theme = Some(theme.clone());
        let mut t = TextBlock::new("Hard to read");
        t.color = Color::rgb(0xEE, 0xEE, 0xEE);
        let id = doc.add(Element::new(ElementKind::Text(t)));
        let mut text = TextEngine::new();
        let issues = design_check(&doc, &mut text, std::path::Path::new("."), &theme);
        let issue = issues.iter().find(|i| i.target.as_deref() == Some(id.as_str())).expect("contrast issue");
        apply_fix(&mut doc, &theme, issue.fix.as_ref().unwrap());
        let issues = design_check(&doc, &mut text, std::path::Path::new("."), &theme);
        assert!(issues.iter().all(|i| i.target.as_deref() != Some(id.as_str())), "fixed: {issues:?}");
    }
}
