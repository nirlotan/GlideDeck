//! Slide layouts: ready-made, themed slide content authored in a 1600 × 900
//! frame centred on the origin, plus the small element helpers that templates
//! share with them.

use glam::DVec2;
use glide_core::{
    Aabb, Color, Document, Element, ElementKind, EllipseShape, Frame, Icon, IconStyle, ListStyle, RectShape, Shadow, Stroke, TextAlign,
    TextRole, Theme, Transform,
};

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
    Comparison,
    Quadrants,
    Bento,
    Dashboard,
    Team,
    HubSpokes,
    Cycle,
    Venn,
    Pyramid,
    Process,
    Staircase,
    WindingRoad,
}

impl LayoutKind {
    /// Gallery order: grouped by [`LayoutKind::category`].
    pub const ALL: [LayoutKind; 20] = [
        LayoutKind::Title,
        LayoutKind::Section,
        LayoutKind::Quote,
        LayoutKind::BigNumber,
        LayoutKind::ImageText,
        LayoutKind::Agenda,
        LayoutKind::ThreePoints,
        LayoutKind::Comparison,
        LayoutKind::Quadrants,
        LayoutKind::Bento,
        LayoutKind::Dashboard,
        LayoutKind::Team,
        LayoutKind::HubSpokes,
        LayoutKind::Cycle,
        LayoutKind::Venn,
        LayoutKind::Pyramid,
        LayoutKind::Timeline,
        LayoutKind::Process,
        LayoutKind::Staircase,
        LayoutKind::WindingRoad,
    ];

    pub const CATEGORIES: [&'static str; 4] = ["Basics", "Grids", "Diagrams", "Flows"];

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
            LayoutKind::Comparison => "Comparison",
            LayoutKind::Quadrants => "Quadrants",
            LayoutKind::Bento => "Bento grid",
            LayoutKind::Dashboard => "Dashboard",
            LayoutKind::Team => "Team",
            LayoutKind::HubSpokes => "Hub & spokes",
            LayoutKind::Cycle => "Cycle",
            LayoutKind::Venn => "Venn",
            LayoutKind::Pyramid => "Pyramid",
            LayoutKind::Process => "Process",
            LayoutKind::Staircase => "Staircase",
            LayoutKind::WindingRoad => "Winding road",
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            LayoutKind::Title | LayoutKind::Section | LayoutKind::Quote | LayoutKind::BigNumber | LayoutKind::ImageText | LayoutKind::Agenda => "Basics",
            LayoutKind::ThreePoints | LayoutKind::Comparison | LayoutKind::Quadrants | LayoutKind::Bento | LayoutKind::Dashboard | LayoutKind::Team => "Grids",
            LayoutKind::HubSpokes | LayoutKind::Cycle | LayoutKind::Venn | LayoutKind::Pyramid => "Diagrams",
            LayoutKind::Timeline | LayoutKind::Process | LayoutKind::Staircase | LayoutKind::WindingRoad => "Flows",
        }
    }
}

// --- element helpers ---------------------------------------------------------------

pub(crate) fn at(x: f64, y: f64) -> Transform {
    Transform { x, y, scale: 1.0, rotation: 0.0 }
}

pub(crate) fn turned(x: f64, y: f64, rotation: f64) -> Transform {
    Transform { x, y, scale: 1.0, rotation }
}

pub(crate) fn text(theme: &Theme, role: TextRole, content: &str, size: Option<f64>, align: TextAlign, wrap: Option<f64>) -> ElementKind {
    let mut t = theme.text_style(role);
    t.content = content.into();
    if let Some(s) = size {
        t.font_size = s;
    }
    t.align = align;
    t.max_width = wrap;
    ElementKind::Text(t)
}

/// A bulleted text box.
pub(crate) fn bullets(theme: &Theme, content: &str, size: f64, wrap: f64) -> ElementKind {
    let mut kind = text(theme, TextRole::Body, content, Some(size), TextAlign::Left, Some(wrap));
    if let ElementKind::Text(t) = &mut kind {
        t.list = ListStyle::Bullet;
    }
    kind
}

/// Gives a text box a fixed color (and weight). The role is dropped so a theme
/// switch remaps the color through the theme slots instead of resetting it.
pub(crate) fn colored(mut kind: ElementKind, color: Color, weight: Option<u16>) -> ElementKind {
    if let ElementKind::Text(t) = &mut kind {
        t.color = color;
        t.role = None;
        if let Some(w) = weight {
            t.weight = w;
        }
    }
    kind
}

pub(crate) fn weighted(mut kind: ElementKind, weight: u16) -> ElementKind {
    if let ElementKind::Text(t) = &mut kind {
        t.weight = weight;
    }
    kind
}

pub(crate) fn named(kind: ElementKind, name: &str, t: Transform) -> Element {
    let mut e = Element::new(kind).with_transform(t);
    e.name = name.into();
    e
}

pub(crate) fn shadowed(mut el: Element, shadow: Shadow) -> Element {
    el.shadow = Some(shadow);
    el
}

pub(crate) fn card(theme: &Theme, w: f64, h: f64) -> ElementKind {
    rect(w, h, 28.0, theme.surface)
}

pub(crate) fn rect(w: f64, h: f64, corner_radius: f64, fill: Color) -> ElementKind {
    ElementKind::Rect(RectShape { width: w, height: h, corner_radius, fill: Some(fill), ..Default::default() })
}

pub(crate) fn circle(d: f64, fill: Option<Color>, stroke: Option<(Color, f64)>) -> ElementKind {
    ElementKind::Ellipse(EllipseShape { width: d, height: d, fill, stroke: stroke.map(|(color, width)| Stroke { color, width }), ..Default::default() })
}

pub(crate) fn icon(name: &str, size: f64, color: Color, style: IconStyle) -> ElementKind {
    ElementKind::Icon(Icon { name: name.into(), style, size, color })
}

pub(crate) fn tint(c: Color, alpha: u8) -> Color {
    Color::rgba(c.r(), c.g(), c.b(), alpha)
}

/// A straight line from `a` to `b`, drawn as a thin rotated pill.
pub(crate) fn connector(a: DVec2, b: DVec2, width: f64, color: Color) -> Element {
    let d = b - a;
    let mid = (a + b) * 0.5;
    named(rect(d.length(), width, width / 2.0, color), "Connector", turned(mid.x, mid.y, d.to_angle().to_degrees()))
}

/// The content of a layout in layout-local coordinates (frame excluded).
pub fn layout_elements(kind: LayoutKind, theme: &Theme) -> Vec<Element> {
    let a = theme.accents;
    let mut v = Vec::new();
    match kind {
        LayoutKind::Title => {
            v.push(named(text(theme, TextRole::Title, "Your big idea", Some(120.0), TextAlign::Center, None), "Title", at(0.0, -60.0)));
            v.push(named(rect(140.0, 10.0, 5.0, a[0]), "Accent bar", at(0.0, 50.0)));
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
            v.push(named(rect(8.0, 300.0, 4.0, tint(theme.muted, 0x60)), "Divider", at(-220.0, 0.0)));
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
                v.push(shadowed(named(card(theme, 440.0, 500.0), &format!("Card {}", i + 1), at(x, 70.0)), Shadow::soft()));
                v.push(named(circle(120.0, Some(tint(a[i % 4], 0x26)), None), "Icon badge", at(x, -70.0)));
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
            v.push(named(rect(8.0, 520.0, 4.0, a[0]), "Divider", at(-140.0, 0.0)));
            for (i, item) in ["Where we are", "What we learned", "Where we go next", "Questions"].iter().enumerate() {
                let y = -195.0 + i as f64 * 130.0;
                v.push(named(circle(76.0, Some(a[i % 4]), None), "Number chip", at(-20.0, y)));
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
            v.push(named(rect(1320.0, 6.0, 3.0, tint(theme.muted, 0x70)), "Line", at(0.0, 30.0)));
            for (i, (year, label)) in [("2023", "First prototype"), ("2024", "Private beta"), ("2025", "Public launch"), ("2026", "What’s next")].iter().enumerate() {
                let x = -495.0 + i as f64 * 330.0;
                v.push(named(circle(44.0, Some(a[i % 4]), Some((theme.background, 8.0))), "Milestone", at(x, 30.0)));
                v.push(named(text(theme, TextRole::Heading, year, Some(48.0), TextAlign::Center, None), "Year", at(x, -60.0)));
                v.push(named(text(theme, TextRole::Body, label, Some(28.0), TextAlign::Center, Some(280.0)), "Milestone label", at(x, 120.0)));
            }
        }
        LayoutKind::ImageText => {
            v.push(shadowed(named(rect(680.0, 640.0, 32.0, tint(theme.muted, 0x22)), "Image placeholder", at(-390.0, 0.0)), Shadow::soft()));
            v.push(named(icon("image", 120.0, tint(theme.muted, 0xb0), IconStyle::Light), "Image icon", at(-390.0, -30.0)));
            v.push(named(text(theme, TextRole::Caption, "Paste or drop an image here", Some(26.0), TextAlign::Center, None), "Hint", at(-390.0, 80.0)));
            v.push(named(text(theme, TextRole::Heading, "A picture and a point", Some(60.0), TextAlign::Left, Some(640.0)), "Heading", at(410.0, -170.0)));
            v.push(named(
                text(theme, TextRole::Body, "• Lead with the visual\n• Keep each line short\n• One idea per slide", Some(34.0), TextAlign::Left, Some(640.0)),
                "Points",
                at(410.0, 60.0),
            ));
        }
        LayoutKind::Comparison => {
            v.push(named(text(theme, TextRole::Heading, "Two ways forward", Some(60.0), TextAlign::Center, None), "Heading", at(0.0, -350.0)));
            let sides = [
                ("check-circle", "Option A", "Faster to ship\nProven technology\nLower upfront cost"),
                ("rocket-launch", "Option B", "Bigger long-term upside\nNew capabilities\nRoom to grow"),
            ];
            for (i, (glyph, title, body)) in sides.iter().enumerate() {
                let x = if i == 0 { -380.0 } else { 380.0 };
                v.push(shadowed(named(card(theme, 660.0, 600.0), &format!("Card {}", i + 1), at(x, 60.0)), Shadow::soft()));
                v.push(named(circle(110.0, Some(tint(a[i], 0x26)), None), "Icon badge", at(x, -140.0)));
                v.push(named(icon(glyph, 60.0, a[i], IconStyle::Fill), &format!("Icon {}", i + 1), at(x, -140.0)));
                v.push(named(text(theme, TextRole::Heading, title, Some(46.0), TextAlign::Center, None), "Card title", at(x, -40.0)));
                v.push(named(bullets(theme, body, 36.0, 500.0), "Points", at(x, 150.0)));
            }
            v.push(named(circle(120.0, Some(theme.heading), None), "VS badge", at(0.0, 60.0)));
            v.push(named(colored(text(theme, TextRole::Heading, "VS", Some(40.0), TextAlign::Center, None), theme.background, Some(800)), "VS", at(0.0, 60.0)));
        }
        LayoutKind::Quadrants => {
            v.push(named(text(theme, TextRole::Heading, "SWOT at a glance", Some(56.0), TextAlign::Center, None), "Heading", at(0.0, -375.0)));
            let cells = [
                ("trend-up", "Strengths", "What we do better than anyone else, and why it lasts."),
                ("warning", "Weaknesses", "Where we are still catching up, honestly stated."),
                ("lightbulb", "Opportunities", "Openings in the market we can move on this year."),
                ("lightning", "Threats", "Risks and competitors we need to watch closely."),
            ];
            for (i, (glyph, title, body)) in cells.iter().enumerate() {
                let (cx, cy) = (-365.0 + (i % 2) as f64 * 730.0, -150.0 + (i / 2) as f64 * 360.0);
                v.push(named(rect(700.0, 330.0, 28.0, tint(a[i], 0x1C)), &format!("Quadrant {}", i + 1), at(cx, cy)));
                v.push(named(circle(96.0, Some(tint(a[i], 0x30)), None), "Icon badge", at(cx - 262.0, cy - 80.0)));
                v.push(named(icon(glyph, 52.0, a[i], IconStyle::Fill), &format!("Icon {}", i + 1), at(cx - 262.0, cy - 80.0)));
                v.push(named(text(theme, TextRole::Heading, title, Some(40.0), TextAlign::Left, Some(480.0)), "Quadrant title", at(cx + 50.0, cy - 80.0)));
                v.push(named(text(theme, TextRole::Body, body, Some(28.0), TextAlign::Left, Some(600.0)), "Quadrant body", at(cx, cy + 70.0)));
            }
        }
        LayoutKind::Bento => {
            let (gap, x0, y0) = (24.0, -740.0, -400.0);
            let (cw, rh) = ((1480.0 - 3.0 * gap) / 4.0, (800.0 - 2.0 * gap) / 3.0);
            // (column, row, column span, row span) → (center, size)
            let cell = |c: f64, r: f64, cs: f64, rs: f64| {
                let size = DVec2::new(cs * cw + (cs - 1.0) * gap, rs * rh + (rs - 1.0) * gap);
                (DVec2::new(x0 + c * (cw + gap), y0 + r * (rh + gap)) + size / 2.0, size)
            };
            let tile = |v: &mut Vec<Element>, (p, s): (DVec2, DVec2), fill: Option<Color>, name: &str| {
                let el = named(rect(s.x, s.y, 28.0, fill.unwrap_or(theme.surface)), name, at(p.x, p.y));
                v.push(if fill.is_none() { shadowed(el, Shadow::soft()) } else { el });
            };
            // Hero tile.
            let (p, s) = cell(0.0, 0.0, 2.0, 2.0);
            tile(&mut v, (p, s), Some(tint(a[0], 0x22)), "Hero tile");
            let (left, top, w) = (p.x - s.x / 2.0, p.y - s.y / 2.0, s.x - 100.0);
            v.push(named(weighted(text(theme, TextRole::Caption, "NEW IN 2026", Some(22.0), TextAlign::Left, Some(w)), 700), "Eyebrow", at(p.x, top + 70.0)));
            v.push(named(text(theme, TextRole::Heading, "The headline feature", Some(56.0), TextAlign::Left, Some(w)), "Heading", at(p.x, top + 150.0)));
            v.push(named(
                text(theme, TextRole::Body, "One sentence that says why it matters to the people in the room.", Some(28.0), TextAlign::Left, Some(w - 120.0)),
                "Body",
                at(p.x - 60.0, top + 260.0),
            ));
            v.push(named(icon("sparkle", 150.0, tint(a[0], 0xB0), IconStyle::Fill), "Hero icon", at(left + s.x - 120.0, top + s.y - 110.0)));
            // Wide tile: icon + title.
            let (p, s) = cell(2.0, 0.0, 2.0, 1.0);
            tile(&mut v, (p, s), None, "Wide tile");
            let left = p.x - s.x / 2.0;
            v.push(named(icon("lightning", 64.0, a[1], IconStyle::Fill), "Icon", at(left + 80.0, p.y)));
            let tw = s.x - 190.0;
            v.push(named(text(theme, TextRole::Heading, "Instant", Some(40.0), TextAlign::Left, Some(tw)), "Tile title", at(left + 150.0 + tw / 2.0, p.y - 28.0)));
            v.push(named(text(theme, TextRole::Body, "Opens in under a second", Some(24.0), TextAlign::Left, Some(tw)), "Tile body", at(left + 150.0 + tw / 2.0, p.y + 28.0)));
            // Number tile.
            let (p, s) = cell(2.0, 1.0, 1.0, 1.0);
            tile(&mut v, (p, s), None, "Number tile");
            v.push(named(weighted(text(theme, TextRole::Title, "4×", Some(100.0), TextAlign::Center, None), 800), "Number", at(p.x, p.y - 20.0)));
            v.push(named(text(theme, TextRole::Caption, "faster exports", Some(24.0), TextAlign::Center, None), "Number caption", at(p.x, p.y + 65.0)));
            // Tall tile.
            let (p, s) = cell(3.0, 1.0, 1.0, 2.0);
            tile(&mut v, (p, s), Some(tint(a[1], 0x22)), "Tall tile");
            let top = p.y - s.y / 2.0;
            v.push(named(icon("shield-check", 90.0, a[1], IconStyle::Fill), "Icon", at(p.x, top + 110.0)));
            v.push(named(text(theme, TextRole::Heading, "Private", Some(40.0), TextAlign::Center, None), "Tile title", at(p.x, top + 220.0)));
            v.push(named(
                text(theme, TextRole::Body, "Everything stays on your device. No accounts, no tracking.", Some(24.0), TextAlign::Center, Some(s.x - 70.0)),
                "Tile body",
                at(p.x, top + 330.0),
            ));
            // Small tile.
            let (p, s) = cell(0.0, 2.0, 1.0, 1.0);
            tile(&mut v, (p, s), None, "Small tile");
            v.push(named(icon("heart", 64.0, a[2], IconStyle::Fill), "Icon", at(p.x, p.y - 35.0)));
            v.push(named(text(theme, TextRole::Heading, "Loved by teams", Some(32.0), TextAlign::Center, Some(s.x - 40.0)), "Tile title", at(p.x, p.y + 50.0)));
            // Wide bottom tile.
            let (p, s) = cell(1.0, 2.0, 2.0, 1.0);
            tile(&mut v, (p, s), Some(tint(a[3], 0x22)), "Banner tile");
            v.push(named(text(theme, TextRole::Heading, "Works with what you have", Some(40.0), TextAlign::Left, Some(s.x - 100.0)), "Tile title", at(p.x, p.y - 28.0)));
            v.push(named(
                text(theme, TextRole::Body, "Images, video, SVG and PowerPoint export", Some(24.0), TextAlign::Left, Some(s.x - 100.0)),
                "Tile body",
                at(p.x, p.y + 32.0),
            ));
        }
        LayoutKind::Dashboard => {
            v.push(named(text(theme, TextRole::Heading, "Quarter at a glance", Some(56.0), TextAlign::Left, Some(900.0)), "Heading", at(-280.0, -350.0)));
            v.push(named(text(theme, TextRole::Caption, "Q3 · 2026", Some(26.0), TextAlign::Center, None), "Period", at(620.0, -350.0)));
            let kpis = [("REVENUE", "$4.2M", "+18% vs last quarter"), ("ACTIVE USERS", "128k", "+9% vs last quarter"), ("NPS", "62", "+4 points"), ("CHURN", "1.8%", "−0.6 points")];
            for (i, (label, number, delta)) in kpis.iter().enumerate() {
                let x = -555.0 + i as f64 * 370.0;
                v.push(shadowed(named(card(theme, 340.0, 240.0), &format!("KPI {}", i + 1), at(x, -120.0)), Shadow::soft()));
                v.push(named(rect(56.0, 8.0, 4.0, a[i]), "KPI accent", at(x - 112.0, -212.0)));
                v.push(named(weighted(text(theme, TextRole::Caption, label, Some(20.0), TextAlign::Left, Some(280.0)), 700), "KPI label", at(x, -178.0)));
                v.push(named(weighted(text(theme, TextRole::Title, number, Some(72.0), TextAlign::Left, Some(280.0)), 800), "KPI value", at(x, -110.0)));
                v.push(named(text(theme, TextRole::Caption, delta, Some(22.0), TextAlign::Left, Some(280.0)), "KPI delta", at(x, -40.0)));
            }
            v.push(shadowed(named(card(theme, 1480.0, 330.0), "Chart card", at(0.0, 245.0)), Shadow::soft()));
            v.push(named(weighted(text(theme, TextRole::Caption, "MONTHLY ACTIVE USERS", Some(22.0), TextAlign::Left, Some(600.0)), 700), "Chart title", at(-390.0, 120.0)));
            let heights = [70.0, 95.0, 85.0, 120.0, 110.0, 140.0, 130.0, 165.0, 150.0, 185.0, 175.0, 215.0];
            for (j, h) in heights.iter().enumerate() {
                let x = -632.5 + j as f64 * 115.0;
                let fill = if j == heights.len() - 1 { a[0] } else { tint(a[0], 0x55) };
                v.push(named(rect(70.0, *h, 10.0, fill), "Bar", at(x, 380.0 - h / 2.0)));
            }
        }
        LayoutKind::Team => {
            v.push(named(text(theme, TextRole::Heading, "Meet the team", Some(64.0), TextAlign::Center, None), "Heading", at(0.0, -340.0)));
            let people = [
                ("Avery Chen", "Founder & CEO", "Sets the vision and keeps us honest."),
                ("Sam Rivera", "Head of Design", "Turns complex ideas into simple screens."),
                ("Noa Levi", "Lead Engineer", "Builds the engine under the hood."),
                ("Jordan Park", "Growth", "Brings the story to new audiences."),
            ];
            for (i, (name, role, bio)) in people.iter().enumerate() {
                let x = -555.0 + i as f64 * 370.0;
                v.push(named(circle(210.0, Some(tint(a[i], 0x2A)), Some((a[i], 6.0))), "Avatar", at(x, -110.0)));
                v.push(named(icon("user", 110.0, a[i], IconStyle::Fill), "Avatar icon", at(x, -110.0)));
                v.push(named(text(theme, TextRole::Heading, name, Some(36.0), TextAlign::Center, None), "Name", at(x, 50.0)));
                v.push(named(text(theme, TextRole::Caption, role, Some(24.0), TextAlign::Center, None), "Role", at(x, 100.0)));
                v.push(named(text(theme, TextRole::Body, bio, Some(24.0), TextAlign::Center, Some(300.0)), "Bio", at(x, 185.0)));
            }
        }
        LayoutKind::HubSpokes => {
            v.push(named(circle(300.0, Some(tint(a[0], 0x1E)), Some((a[0], 6.0))), "Hub", at(0.0, 0.0)));
            v.push(named(text(theme, TextRole::Heading, "Core idea", Some(44.0), TextAlign::Center, Some(240.0)), "Heading", at(0.0, 0.0)));
            let spokes = [("users-three", "People"), ("flow-arrow", "Process"), ("cube", "Product"), ("stack", "Platform"), ("handshake", "Partners"), ("tag", "Pricing")];
            for (k, (glyph, label)) in spokes.iter().enumerate() {
                let ang = (k as f64 * 60.0).to_radians();
                let p = DVec2::new(ang.cos() * 560.0, ang.sin() * 255.0);
                let dir = p.normalize();
                let c = a[k % 4];
                v.push(connector(dir * 160.0, p - dir * 95.0, 6.0, tint(c, 0x80)));
                v.push(shadowed(named(circle(170.0, Some(theme.surface), Some((c, 6.0))), "Spoke", at(p.x, p.y)), Shadow::soft()));
                v.push(named(icon(glyph, 70.0, c, IconStyle::Fill), "Spoke icon", at(p.x, p.y)));
                let ly = if p.y < -1.0 { p.y - 125.0 } else { p.y + 125.0 };
                v.push(named(weighted(text(theme, TextRole::Body, label, Some(28.0), TextAlign::Center, None), 650), "Spoke label", at(p.x, ly)));
            }
        }
        LayoutKind::Cycle => {
            let (c, r) = (DVec2::new(0.0, 20.0), 270.0);
            v.push(named(text(theme, TextRole::Heading, "The loop", Some(56.0), TextAlign::Center, None), "Heading", at(c.x, c.y - 20.0)));
            v.push(named(text(theme, TextRole::Caption, "Repeat every sprint", Some(26.0), TextAlign::Center, None), "Caption", at(c.x, c.y + 45.0)));
            let nodes = [("calendar-check", "Plan"), ("hammer", "Build"), ("chart-bar", "Measure"), ("lightbulb", "Learn"), ("sliders-horizontal", "Adjust")];
            let n = nodes.len();
            let angle = |k: f64| (-90.0 + k * 360.0 / n as f64).to_radians();
            for (k, (glyph, label)) in nodes.iter().enumerate() {
                let col = a[k % 4];
                // A dotted arc to the next node, ending in an arrowhead.
                let (from, to) = (angle(k as f64) + 0.36, angle(k as f64 + 1.0) - 0.34);
                for s in 0..5 {
                    let t = from + (to - from - 0.1) * s as f64 / 5.0;
                    let p = c + DVec2::from_angle(t) * r;
                    v.push(named(circle(12.0, Some(tint(col, 0x90)), None), "Arc", at(p.x, p.y)));
                }
                let head = c + DVec2::from_angle(to) * r;
                v.push(named(icon("caret-right", 36.0, col, IconStyle::Fill), "Arrow", turned(head.x, head.y, to.to_degrees() + 90.0)));
                let dir = DVec2::from_angle(angle(k as f64));
                let p = c + dir * r;
                v.push(shadowed(named(circle(150.0, Some(theme.surface), Some((col, 6.0))), "Node", at(p.x, p.y)), Shadow::soft()));
                v.push(named(icon(glyph, 60.0, col, IconStyle::Fill), "Node icon", at(p.x, p.y)));
                let l = p + dir * 140.0;
                v.push(named(weighted(text(theme, TextRole::Body, label, Some(30.0), TextAlign::Center, None), 700), "Node label", at(l.x, l.y)));
            }
        }
        LayoutKind::Venn => {
            v.push(named(text(theme, TextRole::Heading, "Where ideas meet", Some(56.0), TextAlign::Center, None), "Heading", at(0.0, -375.0)));
            let sets = [(DVec2::new(-150.0, -45.0), "Desirable", DVec2::new(-255.0, -115.0)), (DVec2::new(150.0, -45.0), "Feasible", DVec2::new(255.0, -115.0)), (DVec2::new(0.0, 205.0), "Viable", DVec2::new(0.0, 330.0))];
            for (i, (p, _, _)) in sets.iter().enumerate() {
                v.push(named(circle(440.0, Some(tint(a[i], 0x33)), Some((a[i], 4.0))), &format!("Set {}", i + 1), at(p.x, p.y)));
            }
            for (_, label, l) in sets {
                v.push(named(text(theme, TextRole::Heading, label, Some(36.0), TextAlign::Center, None), "Set label", at(l.x, l.y)));
            }
            v.push(named(icon("sparkle", 56.0, theme.heading, IconStyle::Fill), "Sweet spot icon", at(0.0, 25.0)));
            v.push(named(weighted(text(theme, TextRole::Body, "Sweet spot", Some(24.0), TextAlign::Center, None), 700), "Sweet spot", at(0.0, 80.0)));
            v.push(named(
                text(theme, TextRole::Caption, "Great products sit where all three overlap.", Some(26.0), TextAlign::Left, Some(300.0)),
                "Note",
                at(-560.0, 260.0),
            ));
        }
        LayoutKind::Pyramid => {
            v.push(named(text(theme, TextRole::Heading, "What matters most", Some(64.0), TextAlign::Left, Some(560.0)), "Heading", at(-440.0, -110.0)));
            v.push(named(
                text(theme, TextRole::Body, "Start from the vision at the top. Every layer below holds up the one above it.", Some(28.0), TextAlign::Left, Some(560.0)),
                "Body",
                at(-440.0, 90.0),
            ));
            for (i, label) in ["Vision", "Strategy", "Goals", "Daily work"].iter().enumerate() {
                let (w, y) = (250.0 + i as f64 * 160.0, -246.0 + i as f64 * 164.0);
                v.push(named(rect(w, 150.0, 22.0, tint(a[i], 0x40)), &format!("Tier {}", i + 1), at(380.0, y)));
                v.push(named(text(theme, TextRole::Heading, label, Some(34.0), TextAlign::Center, None), "Tier label", at(380.0, y)));
            }
        }
        LayoutKind::Process => {
            v.push(named(text(theme, TextRole::Heading, "How it works", Some(60.0), TextAlign::Center, None), "Heading", at(0.0, -340.0)));
            let steps = [
                ("magnifying-glass", "Discover", "Listen to users and map the problem."),
                ("lightbulb", "Design", "Sketch options and pick the strongest."),
                ("gear", "Build", "Ship small, testable increments."),
                ("rocket-launch", "Launch", "Release, measure and iterate."),
            ];
            for (i, (glyph, title, body)) in steps.iter().enumerate() {
                let x = -540.0 + i as f64 * 360.0;
                v.push(named(weighted(text(theme, TextRole::Caption, &format!("STEP {:02}", i + 1), Some(22.0), TextAlign::Center, None), 700), "Step label", at(x, -220.0)));
                v.push(named(circle(130.0, Some(tint(a[i], 0x26)), None), "Icon badge", at(x, -110.0)));
                v.push(named(icon(glyph, 64.0, a[i], IconStyle::Fill), &format!("Icon {}", i + 1), at(x, -110.0)));
                if i < steps.len() - 1 {
                    v.push(named(icon("arrow-right", 44.0, tint(theme.muted, 0xB0), IconStyle::Bold), "Arrow", at(x + 180.0, -110.0)));
                }
                v.push(shadowed(named(card(theme, 310.0, 260.0), &format!("Card {}", i + 1), at(x, 150.0)), Shadow::soft()));
                v.push(named(text(theme, TextRole::Heading, title, Some(38.0), TextAlign::Center, None), "Card title", at(x, 75.0)));
                v.push(named(text(theme, TextRole::Body, body, Some(24.0), TextAlign::Center, Some(250.0)), "Card body", at(x, 180.0)));
            }
        }
        LayoutKind::Staircase => {
            v.push(named(text(theme, TextRole::Heading, "Step by step", Some(64.0), TextAlign::Left, Some(700.0)), "Heading", at(-400.0, -345.0)));
            v.push(named(text(theme, TextRole::Subtitle, "Each step builds on the last.", Some(30.0), TextAlign::Left, Some(700.0)), "Subtitle", at(-400.0, -270.0)));
            let steps = [("Idea", "Sketch it"), ("Prototype", "Make it real"), ("Pilot", "Test with users"), ("Launch", "Ship it"), ("Scale", "Grow it")];
            for (i, (title, body)) in steps.iter().enumerate() {
                let (x, h) = (-560.0 + i as f64 * 280.0, 140.0 + i as f64 * 100.0);
                let top = 400.0 - h;
                v.push(named(rect(250.0, h, 18.0, tint(a[0], 0x28 + i as u8 * 0x20)), &format!("Step {}", i + 1), at(x, 400.0 - h / 2.0)));
                v.push(named(text(theme, TextRole::Heading, title, Some(34.0), TextAlign::Center, None), "Step title", at(x, top + 45.0)));
                v.push(named(text(theme, TextRole::Body, body, Some(22.0), TextAlign::Center, Some(220.0)), "Step body", at(x, top + 92.0)));
            }
            v.push(named(icon("flag-banner", 84.0, a[1], IconStyle::Fill), "Flag", at(575.0, -200.0)));
        }
        LayoutKind::WindingRoad => {
            v.push(named(text(theme, TextRole::Heading, "Our journey", Some(60.0), TextAlign::Center, None), "Heading", at(0.0, -375.0)));
            let stops = [("2022", "The spark"), ("2023", "First customers"), ("2024", "Series A"), ("2025", "Going global"), ("2026", "What’s next")];
            let pos = |k: usize| DVec2::new(-600.0 + k as f64 * 300.0, if k.is_multiple_of(2) { -40.0 } else { 170.0 });
            // The road: an eased S-curve of dots through every stop.
            let mut points = vec![DVec2::new(-740.0, 170.0)];
            points.extend((0..stops.len()).map(pos));
            points.push(DVec2::new(740.0, 170.0));
            for w in points.windows(2) {
                for s in 1..10 {
                    let t = s as f64 / 10.0;
                    let x = w[0].x + (w[1].x - w[0].x) * t;
                    let y = w[0].y + (w[1].y - w[0].y) * (1.0 - (std::f64::consts::PI * t).cos()) / 2.0;
                    v.push(named(circle(16.0, Some(tint(theme.muted, 0x80)), None), "Road", at(x, y)));
                }
            }
            for (k, (year, label)) in stops.iter().enumerate() {
                let p = pos(k);
                let up = k.is_multiple_of(2);
                v.push(named(circle(60.0, Some(a[k % 4]), Some((theme.background, 8.0))), "Milestone", at(p.x, p.y)));
                let (year_y, label_y) = if up { (p.y - 150.0, p.y - 92.0) } else { (p.y + 85.0, p.y + 140.0) };
                v.push(named(text(theme, TextRole::Heading, year, Some(44.0), TextAlign::Center, None), "Year", at(p.x, year_y)));
                v.push(named(text(theme, TextRole::Body, label, Some(26.0), TextAlign::Center, Some(270.0)), "Milestone label", at(p.x, label_y)));
            }
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
    place_local(doc, layout_elements(kind, theme), center, s, rotation);
    frame_id
}

/// Adds elements authored in a local space, mapped by `center`, scale `s` and
/// `rotation` (degrees) into the world.
pub(crate) fn place_local(doc: &mut Document, elements: Vec<Element>, center: DVec2, s: f64, rotation: f64) {
    let rot = DVec2::from_angle(rotation.to_radians());
    for mut el in elements {
        let world = center + rot.rotate(el.transform.position() * s);
        el.transform = Transform { x: world.x, y: world.y, scale: el.transform.scale * s, rotation: el.transform.rotation + rotation };
        doc.add(el);
    }
}

/// Where a new slide goes after `anchor`: the same size and rotation, one
/// slide-width further along, skipping past frames already there.
/// Returns (center, width, rotation in degrees).
pub fn slot_after(doc: &Document, anchor: &Element) -> (DVec2, f64, f64) {
    let size = anchor.kind.intrinsic_size().unwrap_or_default() * anchor.transform.scale.abs();
    let rotation = anchor.transform.rotation;
    let dir = DVec2::from_angle(rotation.to_radians());
    (free_spot(doc, anchor.transform.position() + dir * size.x * 1.15, size.x, rotation), size.x, rotation)
}

/// Steps a 16:9 slide of `width` along its rotation from `center` until it no
/// longer overlaps a frame of similar size.
pub fn free_spot(doc: &Document, mut center: DVec2, width: f64, rotation: f64) -> DVec2 {
    let step = DVec2::from_angle(rotation.to_radians()) * width * 1.15;
    let half = DVec2::new(width, width * 9.0 / 16.0) * 0.5;
    for _ in 0..40 {
        let candidate = Aabb::from_center_half(center, half);
        let overlaps = doc.elements.iter().filter(|e| e.kind.is_frame()).any(|f| {
            let b = f.world_aabb(f.kind.intrinsic_size().unwrap_or_default());
            b.intersects(&candidate) && b.size().x < width * 4.0
        });
        if !overlaps {
            break;
        }
        center += step;
    }
    center
}
