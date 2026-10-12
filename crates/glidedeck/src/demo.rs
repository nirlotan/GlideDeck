//! The built-in welcome canvas, also written by `glidedeck new --demo`.

use glide_core::theme::default_theme;
use glide_core::{
    Color, Document, Element, ElementKind, EllipseShape, Frame, Gradient, Icon, IconStyle, RectShape, Shadow, Step,
    Stroke, TextAlign, TextBlock, TextRole, Theme, Transform, TransitionStyle,
};

fn at(x: f64, y: f64, scale: f64, rotation: f64) -> Transform {
    Transform { x, y, scale, rotation }
}

fn styled(theme: &Theme, role: TextRole, content: &str, size: f64) -> TextBlock {
    let mut t = theme.text_style(role);
    t.content = content.into();
    t.font_size = size;
    t
}

fn add(doc: &mut Document, id: &str, name: &str, kind: ElementKind, t: Transform) -> String {
    let mut el = Element::new(kind).with_transform(t);
    el.id = id.into();
    el.name = name.into();
    doc.add(el)
}

fn add_with_shadow(doc: &mut Document, id: &str, name: &str, kind: ElementKind, t: Transform, shadow: Shadow) {
    let id = add(doc, id, name, kind, t);
    if let Some(el) = doc.get_mut(&id) {
        el.shadow = Some(shadow);
    }
}

pub fn demo_document() -> Document {
    let theme = default_theme();
    let [teal, orange, indigo, _amber] = theme.accents;
    let mut doc = Document::new("Welcome to GlideDeck");
    glide_core::theme::apply_theme(&mut doc, &theme);

    add(&mut doc, "title", "Title", ElementKind::Text(TextBlock { align: TextAlign::Center, ..styled(&theme, TextRole::Title, "GlideDeck", 140.0) }), at(0.0, -40.0, 1.0, 0.0));
    add(
        &mut doc,
        "subtitle",
        "Subtitle",
        ElementKind::Text(TextBlock {
            align: TextAlign::Center,
            ..styled(&theme, TextRole::Subtitle, "A local-first, native zooming canvas.\nScroll to zoom · Space-drag to pan · Alt-scroll to rotate", 34.0)
        }),
        at(0.0, 90.0, 1.0, 0.0),
    );

    let cards = [
        ("card-canvas", "Infinite canvas", "Unbounded in every direction,\nprecise at any zoom depth.", teal, "infinity", -620.0),
        ("card-text", "Vector everything", "Text and 1,500 icons are outlines,\nso they stay razor sharp.", orange, "pen-nib", 0.0),
        ("card-agents", "Agent-ready", "Every edit is a JSON op —\nscriptable from the CLI.", indigo, "robot", 620.0),
    ];
    for (id, title, body, accent, glyph, x) in cards {
        add_with_shadow(
            &mut doc,
            id,
            title,
            ElementKind::Rect(RectShape { width: 520.0, height: 330.0, corner_radius: 28.0, fill: Some(theme.surface), ..Default::default() }),
            at(x, 430.0, 1.0, 0.0),
            Shadow::soft(),
        );
        add(
            &mut doc,
            &format!("{id}-badge"),
            &format!("{title} badge"),
            ElementKind::Ellipse(EllipseShape { width: 84.0, height: 84.0, fill: Some(Color::rgba(accent.r(), accent.g(), accent.b(), 0x22)), ..Default::default() }),
            at(x, 320.0, 1.0, 0.0),
        );
        add(
            &mut doc,
            &format!("{id}-icon"),
            &format!("{title} icon"),
            ElementKind::Icon(Icon { name: glyph.into(), style: IconStyle::Fill, size: 48.0, color: accent }),
            at(x, 320.0, 1.0, 0.0),
        );
        let mut heading = styled(&theme, TextRole::Heading, title, 40.0);
        heading.align = TextAlign::Center;
        heading.color = accent;
        heading.role = None;
        add(&mut doc, &format!("{id}-title"), &format!("{title} heading"), ElementKind::Text(heading), at(x, 410.0, 1.0, 0.0));
        add(
            &mut doc,
            &format!("{id}-body"),
            &format!("{title} body"),
            ElementKind::Text(TextBlock { align: TextAlign::Center, ..styled(&theme, TextRole::Body, body, 26.0) }),
            at(x, 500.0, 1.0, 0.0),
        );
    }

    // A rotated gradient sticker.
    add_with_shadow(
        &mut doc,
        "sticker",
        "Sticker",
        ElementKind::Rect(RectShape {
            width: 300.0,
            height: 90.0,
            corner_radius: 45.0,
            fill: Some(orange),
            gradient: Some(Gradient { to: Color::rgb(0xF2, 0xA5, 0x41), angle: 0.0 }),
            ..Default::default()
        }),
        at(560.0, -190.0, 1.0, 12.0),
        Shadow::lifted(),
    );
    add(
        &mut doc,
        "sticker-text",
        "Sticker text",
        ElementKind::Text(TextBlock { color: Color::WHITE, weight: 700, ..styled(&theme, TextRole::Body, "100% offline", 40.0) }),
        at(560.0, -190.0, 1.0, 12.0),
    );

    // Multilingual shaping sample (uses whatever fonts are installed locally).
    add(
        &mut doc,
        "scripts",
        "Scripts",
        ElementKind::Text(styled(&theme, TextRole::Caption, "Hello · Здравствуйте · こんにちは · नमस्ते · مرحبا · שלום", 30.0)),
        at(0.0, 720.0, 1.0, 0.0),
    );

    // Nested worlds: each level is 100x smaller and sits inside the previous one.
    add(
        &mut doc,
        "portal",
        "Portal",
        ElementKind::Ellipse(EllipseShape {
            width: 240.0,
            height: 240.0,
            fill: Some(Color::rgba(teal.r(), teal.g(), teal.b(), 0x30)),
            stroke: Some(Stroke { color: teal, width: 3.0 }),
            ..Default::default()
        }),
        at(-560.0, -200.0, 1.0, 0.0),
    );
    add(
        &mut doc,
        "portal-hint",
        "Portal hint",
        ElementKind::Text(TextBlock { color: teal, weight: 600, ..styled(&theme, TextRole::Body, "zoom in here →", 26.0) }),
        at(-800.0, -200.0, 1.0, -8.0),
    );
    let (x, y) = (-560.0, -200.0);
    let mut scale = 1.0;
    for depth in 1..=4 {
        scale /= 100.0;
        let label = match depth {
            1 => "Level 1 — 100× deeper",
            2 => "Level 2 — 10,000× deeper",
            3 => "Level 3 — 1,000,000× deeper",
            _ => "Level 4 — still crisp.",
        };
        add(
            &mut doc,
            &format!("depth-{depth}-ring"),
            &format!("Depth {depth} ring"),
            ElementKind::Ellipse(EllipseShape {
                width: 9000.0,
                height: 9000.0,
                fill: Some(Color::rgba(indigo.r(), indigo.g(), indigo.b(), 0x18)),
                stroke: Some(Stroke { color: indigo, width: 40.0 }),
                ..Default::default()
            }),
            at(x, y, scale, 0.0),
        );
        add(
            &mut doc,
            &format!("depth-{depth}-label"),
            &format!("Depth {depth} label"),
            ElementKind::Text(TextBlock { align: TextAlign::Center, ..styled(&theme, TextRole::Heading, label, 600.0) }),
            at(x, y - 1500.0 * scale, scale, 0.0),
        );
    }

    // Waypoint frames. All invisible except the one around the scripts line,
    // which shows how a bordered frame renders in the show and the export.
    let frame = |w: f64, h: f64| ElementKind::Frame(Frame { width: w, height: h, corner_radius: 0.0, fill: None, border: None });
    let frames = [
        ("frame-overview", "Overview", frame(2200.0, 1237.5), at(-20.0, 250.0, 1.0, 0.0)),
        ("frame-title", "Title", frame(1100.0, 618.75), at(0.0, 20.0, 1.0, 0.0)),
        ("frame-canvas", "Infinite canvas", frame(640.0, 360.0), at(-620.0, 430.0, 1.0, 0.0)),
        ("frame-text", "Vector everything", frame(640.0, 360.0), at(0.0, 430.0, 1.0, 0.0)),
        ("frame-agents", "Agent-ready", frame(640.0, 360.0), at(620.0, 430.0, 1.0, 0.0)),
        ("frame-sticker", "Offline", frame(420.0, 236.25), at(560.0, -190.0, 1.0, 12.0)),
        ("frame-portal", "Portal", frame(560.0, 315.0), at(-640.0, -200.0, 1.0, 0.0)),
        ("frame-depth-1", "Level 1", frame(14000.0, 7875.0), at(-560.0, -200.0, 0.01, 0.0)),
        ("frame-depth-2", "Level 2", frame(14000.0, 7875.0), at(-560.0, -200.0, 0.0001, 0.0)),
        ("frame-depth-4", "Level 4", frame(14000.0, 7875.0), at(-560.0, -200.0, 0.00000001, 0.0)),
    ];
    for (id, name, kind, t) in frames {
        add(&mut doc, id, name, kind, t);
    }
    add(
        &mut doc,
        "frame-scripts",
        "Every script",
        ElementKind::Frame(Frame {
            width: 960.0,
            height: 110.0,
            corner_radius: 18.0,
            fill: None,
            border: Some(Stroke { color: Color::rgba(0x5b, 0x4f, 0xe9, 0x90), width: 2.0 }),
        }),
        at(0.0, 720.0, 1.0, 0.0),
    );
    // Frames sit beneath content in z-order.
    let (frames_first, rest): (Vec<_>, Vec<_>) = doc.elements.drain(..).partition(|e| e.kind.is_frame());
    doc.elements = frames_first.into_iter().chain(rest).collect();

    for id in [
        "frame-overview", "frame-title", "frame-canvas", "frame-text", "frame-agents", "frame-scripts", "frame-sticker",
        "frame-portal", "frame-depth-1", "frame-depth-2", "frame-depth-4", "frame-overview",
    ] {
        let mut step = Step::new(id);
        if id.starts_with("frame-depth") {
            step.transition.style = TransitionStyle::Direct;
            step.transition.duration = 2.2;
        }
        doc.path.push(step);
    }
    doc.path[1].notes = "Welcome everyone. Everything here runs locally — no cloud, no accounts.".into();

    doc
}
