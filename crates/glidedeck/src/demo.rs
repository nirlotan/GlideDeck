//! The built-in welcome canvas, also written by `glidedeck new --demo`.
//!
//! A tour of what GlideDeck does, styled with the 2026 "Dusk" theme: aurora
//! glows drifting in parallax behind frosted glass cards, in-slide
//! animations, varied camera moves and a portal that zooms a million times
//! deeper.

use glide_core::theme::builtin_themes;
use glide_core::{
    Build, Color, Document, Effect, Element, ElementKind, EllipseShape, Frame, Gradient, Icon, IconStyle, RectShape, Shadow, Step, Stroke,
    TextAlign, TextBlock, TextRole, Theme, Transform, TransitionStyle, Trigger,
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

fn tint(c: Color, a: u8) -> Color {
    Color::rgba(c.r(), c.g(), c.b(), a)
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

/// A soft glowing orb of diameter `d` at `at` that drifts at `depth` in the show.
fn glow(doc: &mut Document, id: &str, d: f64, (from, to): (Color, Color), (x, y): (f64, f64), depth: f64) {
    let id = add(
        doc,
        id,
        "Aurora glow",
        ElementKind::Ellipse(EllipseShape { width: d, height: d, fill: Some(from), gradient: Some(Gradient { to, angle: 60.0 }), stroke: None }),
        at(x, y, 1.0, 0.0),
    );
    if let Some(el) = doc.get_mut(&id) {
        el.parallax = depth;
    }
}

/// Frosted glass: the surface color, mostly opaque, with a faint accent edge.
fn glass(theme: &Theme, w: f64, h: f64, r: f64, edge: Color) -> ElementKind {
    let s = theme.surface;
    ElementKind::Rect(RectShape {
        width: w,
        height: h,
        corner_radius: r,
        fill: Some(Color::rgba(s.r(), s.g(), s.b(), 0xD0)),
        stroke: Some(Stroke { color: tint(edge, 0x66), width: 2.0 }),
        ..Default::default()
    })
}

fn glass_shadow() -> Shadow {
    Shadow { color: Color::rgba(0x04, 0x02, 0x10, 0x80), blur: 40.0, x: 0.0, y: 18.0 }
}

pub fn demo_document() -> Document {
    let theme = builtin_themes().into_iter().find(|t| t.name == "Dusk").unwrap_or_else(glide_core::theme::default_theme);
    let [iris, teal, coral, lime] = theme.accents;
    let mut doc = Document::new("Welcome to GlideDeck");
    glide_core::theme::apply_theme(&mut doc, &theme);

    // Aurora: big soft glows far behind, drifting slower than the canvas.
    glow(&mut doc, "glow-iris", 1500.0, (tint(iris, 0x5C), tint(teal, 0x00)), (-760.0, -180.0), 0.45);
    glow(&mut doc, "glow-teal", 1150.0, (tint(teal, 0x44), tint(iris, 0x00)), (860.0, 80.0), 0.6);
    glow(&mut doc, "glow-coral", 950.0, (tint(coral, 0x3C), tint(lime, 0x00)), (120.0, 760.0), 0.35);
    glow(&mut doc, "glow-lime", 520.0, (tint(lime, 0x26), tint(teal, 0x00)), (980.0, -420.0), 0.7);
    // Orbit rings and sparks, nearer.
    for (i, (d, x, y, c)) in [(420.0, -1010.0, 520.0, teal), (260.0, 1030.0, 640.0, coral), (180.0, 420.0, -430.0, iris)].into_iter().enumerate() {
        let id = add(
            &mut doc,
            &format!("ring-{i}"),
            "Orbit ring",
            ElementKind::Ellipse(EllipseShape { width: d, height: d, fill: None, gradient: None, stroke: Some(Stroke { color: tint(c, 0x80), width: 3.0 }) }),
            at(x, y, 1.0, 0.0),
        );
        if let Some(el) = doc.get_mut(&id) {
            el.parallax = 0.2;
        }
    }
    for (i, (x, y, c)) in [(-980.0, -380.0, lime), (-420.0, -400.0, teal), (330.0, -330.0, coral), (1040.0, 300.0, lime), (-1050.0, 160.0, coral), (760.0, 820.0, teal), (-560.0, 860.0, iris)]
        .into_iter()
        .enumerate()
    {
        let id = add(&mut doc, &format!("spark-{i}"), "Spark", ElementKind::Ellipse(EllipseShape { width: 14.0, height: 14.0, fill: Some(tint(c, 0xD0)), ..Default::default() }), at(x, y, 1.0, 0.0));
        if let Some(el) = doc.get_mut(&id) {
            el.parallax = -0.25;
        }
    }

    // The title block.
    add(
        &mut doc,
        "badge",
        "Badge",
        ElementKind::Rect(RectShape {
            width: 560.0,
            height: 58.0,
            corner_radius: 29.0,
            fill: Some(tint(lime, 0x1C)),
            stroke: Some(Stroke { color: tint(lime, 0x88), width: 1.5 }),
            ..Default::default()
        }),
        at(0.0, -215.0, 1.0, 0.0),
    );
    add(&mut doc, "badge-icon", "Badge icon", ElementKind::Icon(Icon { name: "sparkle".into(), style: IconStyle::Fill, size: 28.0, color: lime }), at(-232.0, -215.0, 1.0, 0.0));
    add(
        &mut doc,
        "badge-text",
        "Badge text",
        ElementKind::Text(TextBlock { color: lime, weight: 600, align: TextAlign::Center, ..styled(&theme, TextRole::Caption, "New: carousel, parallax and animations", 24.0) }),
        at(18.0, -215.0, 1.0, 0.0),
    );
    add(
        &mut doc,
        "title",
        "Title",
        ElementKind::Text(TextBlock { align: TextAlign::Center, weight: 800, ..styled(&theme, TextRole::Title, "GlideDeck", 170.0) }),
        at(0.0, -75.0, 1.0, 0.0),
    );
    add(
        &mut doc,
        "tagline",
        "Tagline",
        ElementKind::Text(TextBlock { align: TextAlign::Center, color: theme.heading, weight: 600, ..styled(&theme, TextRole::Subtitle, "Presentations that move.", 56.0) }),
        at(0.0, 55.0, 1.0, 0.0),
    );
    add(
        &mut doc,
        "subtitle",
        "Subtitle",
        ElementKind::Text(TextBlock {
            align: TextAlign::Center,
            ..styled(&theme, TextRole::Caption, "Scroll to zoom  ·  Space-drag to pan  ·  Press F5 to play this canvas", 26.0)
        }),
        at(0.0, 140.0, 1.0, 0.0),
    );

    // Four frosted glass cards.
    let cards = [
        ("card-canvas", "Infinite canvas", "Zoom a million times deeper.\nEverything stays razor sharp.", iris, "infinity", -705.0),
        ("card-text", "PowerPoint in & out", "Import .pptx decks; export\nPowerPoint with Morph, or PDF.", coral, "file-ppt", -235.0),
        ("card-motion", "Motion built in", "Swoops, carousels, parallax\nand in-slide animations.", teal, "shooting-star", 235.0),
        ("card-agents", "AI-ready", "Claude and Copilot build decks\nthrough the MCP server.", lime, "robot", 705.0),
    ];
    for (id, title, body, accent, glyph, x) in cards {
        add_with_shadow(&mut doc, id, title, glass(&theme, 430.0, 300.0, 30.0, accent), at(x, 470.0, 1.0, 0.0), glass_shadow());
        add(
            &mut doc,
            &format!("{id}-badge"),
            &format!("{title} badge"),
            ElementKind::Rect(RectShape { width: 84.0, height: 84.0, corner_radius: 24.0, fill: Some(tint(accent, 0x2A)), ..Default::default() }),
            at(x, 380.0, 1.0, 0.0),
        );
        add(&mut doc, &format!("{id}-icon"), &format!("{title} icon"), ElementKind::Icon(Icon { name: glyph.into(), style: IconStyle::Fill, size: 46.0, color: accent }), at(x, 380.0, 1.0, 0.0));
        let mut heading = styled(&theme, TextRole::Heading, title, 36.0);
        heading.align = TextAlign::Center;
        heading.role = None;
        add(&mut doc, &format!("{id}-title"), &format!("{title} heading"), ElementKind::Text(heading), at(x, 465.0, 1.0, 0.0));
        add(
            &mut doc,
            &format!("{id}-body"),
            &format!("{title} body"),
            ElementKind::Text(TextBlock { align: TextAlign::Center, color: theme.muted, ..styled(&theme, TextRole::Body, body, 22.0) }),
            at(x, 545.0, 1.0, 0.0),
        );
    }

    // A tilted gradient sticker.
    add_with_shadow(
        &mut doc,
        "sticker",
        "Sticker",
        ElementKind::Rect(RectShape {
            width: 310.0,
            height: 92.0,
            corner_radius: 46.0,
            fill: Some(coral),
            gradient: Some(Gradient { to: Color::rgb(0xFF, 0xB5, 0x47), angle: 0.0 }),
            ..Default::default()
        }),
        at(640.0, -250.0, 1.0, 10.0),
        Shadow { color: Color::rgba(0xFF, 0x7A, 0x5C, 0x55), blur: 30.0, x: 0.0, y: 10.0 },
    );
    add(
        &mut doc,
        "sticker-text",
        "Sticker text",
        ElementKind::Text(TextBlock { color: Color::rgb(0x1A, 0x10, 0x24), weight: 800, align: TextAlign::Center, ..styled(&theme, TextRole::Body, "100% offline", 40.0) }),
        at(640.0, -250.0, 1.0, 10.0),
    );

    // Every script, in a glass pill.
    add(&mut doc, "scripts-pill", "Scripts pill", glass(&theme, 1000.0, 96.0, 48.0, teal), at(0.0, 735.0, 1.0, 0.0));
    add(
        &mut doc,
        "scripts",
        "Scripts",
        ElementKind::Text(TextBlock { align: TextAlign::Center, ..styled(&theme, TextRole::Caption, "Hello · Здравствуйте · こんにちは · नमस्ते · مرحبا · שלום", 30.0) }),
        at(0.0, 735.0, 1.0, 0.0),
    );

    // A portal into nested worlds: each level is 100× smaller, inside the last.
    let (x, y) = (-640.0, -250.0);
    add(
        &mut doc,
        "portal",
        "Portal",
        ElementKind::Ellipse(EllipseShape {
            width: 240.0,
            height: 240.0,
            fill: Some(tint(teal, 0x26)),
            gradient: Some(Gradient { to: tint(iris, 0x40), angle: 135.0 }),
            stroke: Some(Stroke { color: teal, width: 3.0 }),
        }),
        at(x, y, 1.0, 0.0),
    );
    add(
        &mut doc,
        "portal-hint",
        "Portal hint",
        ElementKind::Text(TextBlock { color: teal, weight: 600, ..styled(&theme, TextRole::Body, "zoom in here →", 26.0) }),
        at(x - 250.0, y, 1.0, -8.0),
    );
    let mut scale = 1.0;
    for depth in 1..=4 {
        scale /= 100.0;
        let label = match depth {
            1 => "Level 1 — 100× deeper",
            2 => "Level 2 — 10,000× deeper",
            3 => "Level 3 — 1,000,000× deeper",
            _ => "Level 4 — still crisp.",
        };
        let ring = [iris, teal, coral, lime][depth - 1];
        add(
            &mut doc,
            &format!("depth-{depth}-ring"),
            &format!("Depth {depth} ring"),
            ElementKind::Ellipse(EllipseShape {
                width: 9000.0,
                height: 9000.0,
                fill: Some(tint(ring, 0x18)),
                gradient: Some(Gradient { to: tint(ring, 0x40), angle: 120.0 }),
                stroke: Some(Stroke { color: ring, width: 40.0 }),
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

    // Waypoint frames, all invisible except the one around the scripts pill,
    // which shows how a bordered frame renders in the show and the export.
    let frame = |w: f64, h: f64| ElementKind::Frame(Frame { width: w, height: h, corner_radius: 0.0, fill: None, border: None });
    let frames = [
        ("frame-overview", "Overview", frame(2400.0, 1350.0), at(0.0, 250.0, 1.0, 0.0)),
        ("frame-title", "Title", frame(1150.0, 646.875), at(0.0, -45.0, 1.0, 0.0)),
        ("frame-canvas", "Infinite canvas", frame(600.0, 337.5), at(-705.0, 470.0, 1.0, 0.0)),
        ("frame-text", "PowerPoint in & out", frame(600.0, 337.5), at(-235.0, 470.0, 1.0, 0.0)),
        ("frame-motion", "Motion built in", frame(600.0, 337.5), at(235.0, 470.0, 1.0, 0.0)),
        ("frame-agents", "AI-ready", frame(600.0, 337.5), at(705.0, 470.0, 1.0, 0.0)),
        ("frame-sticker", "Offline", frame(440.0, 247.5), at(640.0, -250.0, 1.0, 10.0)),
        ("frame-portal", "Portal", frame(620.0, 348.75), at(-740.0, -250.0, 1.0, 0.0)),
        ("frame-depth-1", "Level 1", frame(14000.0, 7875.0), at(x, y, 0.01, 0.0)),
        ("frame-depth-2", "Level 2", frame(14000.0, 7875.0), at(x, y, 0.0001, 0.0)),
        ("frame-depth-4", "Level 4", frame(14000.0, 7875.0), at(x, y, 0.00000001, 0.0)),
    ];
    for (id, name, kind, t) in frames {
        add(&mut doc, id, name, kind, t);
    }
    add(
        &mut doc,
        "frame-scripts",
        "Every script",
        ElementKind::Frame(Frame { width: 1040.0, height: 130.0, corner_radius: 65.0, fill: None, border: Some(Stroke { color: tint(teal, 0x90), width: 2.0 }) }),
        at(0.0, 735.0, 1.0, 0.0),
    );
    // Frames sit beneath content in z-order.
    let (frames_first, rest): (Vec<_>, Vec<_>) = doc.elements.drain(..).partition(|e| e.kind.is_frame());
    doc.elements = frames_first.into_iter().chain(rest).collect();

    // The path, with a different camera move into each stop.
    let stops: [(&str, TransitionStyle, f64); 13] = [
        ("frame-overview", TransitionStyle::Smooth, 1.4),
        ("frame-title", TransitionStyle::Smooth, 1.4),
        ("frame-canvas", TransitionStyle::Swoop, 1.3),
        ("frame-text", TransitionStyle::Direct, 1.0),
        ("frame-motion", TransitionStyle::Punch, 1.1),
        ("frame-agents", TransitionStyle::Direct, 1.0),
        ("frame-scripts", TransitionStyle::Swoop, 1.3),
        ("frame-sticker", TransitionStyle::Spin, 1.6),
        ("frame-portal", TransitionStyle::Smooth, 1.4),
        ("frame-depth-1", TransitionStyle::Direct, 2.2),
        ("frame-depth-2", TransitionStyle::Direct, 2.2),
        ("frame-depth-4", TransitionStyle::Direct, 2.2),
        ("frame-overview", TransitionStyle::Fade, 1.2),
    ];
    for (id, style, duration) in stops {
        let mut step = Step::new(id);
        step.transition.style = style;
        step.transition.duration = duration;
        doc.path.push(step);
    }

    // In-slide animations: the title builds in, each card's icon pops.
    let after = |id: &str, effect: Effect, delay: f64| Build::new(id, effect).with_trigger(Trigger::AfterPrevious).with_delay(delay);
    let with = |id: &str, effect: Effect, delay: f64| Build::new(id, effect).with_trigger(Trigger::WithPrevious).with_delay(delay);
    doc.path[1].builds = vec![
        after("badge", Effect::Pop, 0.05),
        with("badge-icon", Effect::Pop, 0.0),
        with("badge-text", Effect::Fade, 0.1),
        with("title", Effect::Focus, 0.15),
        with("tagline", Effect::Words, 0.45),
        with("subtitle", Effect::Rise, 0.9),
        Build::new("tagline", Effect::Highlight),
    ];
    for (i, card) in ["card-canvas", "card-text", "card-motion", "card-agents"].into_iter().enumerate() {
        doc.path[2 + i].builds = vec![after(&format!("{card}-icon"), Effect::Pulse, 0.1), with(&format!("{card}-title"), Effect::Underline, 0.2)];
    }

    doc.path[1].notes = "# Welcome\n- Everything here runs locally: no cloud, no accounts.\n- Click once more to highlight the tagline.".into();
    doc.path[3].notes = "Mention PowerPoint import: File → Import PowerPoint, then Design → Arrange.".into();
    doc.path[4].notes = "Show the Animate tab and the Carousel template.".into();

    doc
}
