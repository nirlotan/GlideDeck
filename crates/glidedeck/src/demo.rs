//! The built-in welcome canvas, also written by `glidedeck new --demo`.
//!
//! A tour of what GlideDeck does in the "Darkroom" house style: a flat
//! graphite canvas, four tilted slabs, one signal yellow, a full-width
//! wordmark and a portal that zooms a million times deeper. The camera
//! straightens each tilted slab as it flies in.

use glam::DVec2;
use glide_core::{
    Build, Color, Document, Effect, Element, ElementKind, EllipseShape, Frame, RectShape, Shadow, Step, Stroke, TextAlign, TextBlock, TextRole,
    Theme, Transform, TransitionStyle, Trigger,
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

/// Text ink on the yellow and bone slabs.
const INK: Color = Color::rgb(0x14, 0x11, 0x0A);

/// A tilted slab with a title and one line under it. `(cx, cy)` is its
/// center; the text turns with the slab.
fn slab(doc: &mut Document, theme: &Theme, id: &str, (title, body): (&str, &str), (cx, cy): (f64, f64), rotation: f64, fill: Color, ink: Color) {
    const W: f64 = 470.0;
    const H: f64 = 296.0;
    let rect = add(
        doc,
        id,
        title,
        ElementKind::Rect(RectShape { width: W, height: H, corner_radius: 20.0, fill: Some(fill), ..Default::default() }),
        at(cx, cy, 1.0, rotation),
    );
    if let Some(el) = doc.get_mut(&rect) {
        el.shadow = Some(Shadow { color: Color::rgba(0, 0, 0, 0x70), blur: 36.0, x: 0.0, y: 16.0 });
    }
    // Slab-local offsets, turned with the slab.
    let turn = DVec2::from_angle(rotation.to_radians());
    let place = |local: DVec2| {
        let p = DVec2::new(cx, cy) + turn.rotate(local);
        at(p.x, p.y, 1.0, rotation)
    };
    let width = W - 88.0;
    let lines = title.lines().count() as f64;
    let mut heading = styled(theme, TextRole::Heading, title, 50.0);
    heading.role = None;
    heading.weight = 800;
    heading.line_height = 1.05;
    heading.color = ink;
    heading.max_width = Some(width);
    // The text block sits on the slab's bottom edge: body line, then the title above it.
    let body_y = H / 2.0 - 62.0;
    let title_y = body_y - 34.0 - lines * 50.0 * 1.05 / 2.0;
    add(doc, &format!("{id}-title"), &format!("{title} heading"), ElementKind::Text(heading), place(DVec2::new(0.0, title_y)));
    let mut text = styled(theme, TextRole::Body, body, 25.0);
    text.role = None;
    text.color = Color::rgba(ink.r(), ink.g(), ink.b(), 0xD8);
    text.max_width = Some(width);
    add(doc, &format!("{id}-body"), &format!("{title} body"), ElementKind::Text(text), place(DVec2::new(0.0, body_y)));
}

pub fn demo_document() -> Document {
    let theme = glide_core::theme::theme_named(glide_core::theme::HOUSE_DARK).unwrap_or_else(glide_core::theme::default_theme);
    let [yellow, aqua, _, _] = theme.accents;
    let (bone, graphite) = (theme.heading, theme.surface);
    let mut doc = Document::new("Welcome to GlideDeck");
    glide_core::theme::apply_theme(&mut doc, &theme);

    // The overview is a 2400 × 1350 poster centered on the origin.
    const LEFT: f64 = -1085.0;
    let left_text = |t: TextBlock, width: f64| TextBlock { max_width: Some(width), ..t };

    // Top left: the promise, and how to drive the canvas.
    add(
        &mut doc,
        "tagline",
        "Tagline",
        ElementKind::Text(left_text(TextBlock { color: bone, weight: 600, ..styled(&theme, TextRole::Subtitle, "Presentations that move.", 86.0) }, 1040.0)),
        at(LEFT + 520.0, -500.0, 1.0, 0.0),
    );
    add(
        &mut doc,
        "subtitle",
        "Subtitle",
        ElementKind::Text(left_text(styled(&theme, TextRole::Caption, "Scroll to zoom. Space-drag to pan. Press F5 to play this canvas.", 34.0), 1040.0)),
        at(LEFT + 520.0, -398.0, 1.0, 0.0),
    );

    // Four tilted slabs: two graphite, one signal yellow, one bone.
    slab(&mut doc, &theme, "card-canvas", ("Infinite canvas", "A million times deeper, still sharp."), (236.0, -436.0), -3.0, graphite, bone);
    slab(&mut doc, &theme, "card-motion", ("Motion built in", "Swoops, carousels, parallax."), (776.0, -422.0), 3.0, yellow, INK);
    slab(&mut doc, &theme, "card-text", ("PowerPoint\nin and out", "Morph export, or PDF."), (70.0, -82.0), 2.0, bone, INK);
    slab(&mut doc, &theme, "card-agents", ("AI-ready", "Claude and Copilot build decks."), (596.0, -68.0), -2.0, graphite, bone);

    // The wordmark, across the whole poster.
    add(
        &mut doc,
        "title",
        "Wordmark",
        ElementKind::Text(TextBlock { align: TextAlign::Center, weight: 800, line_height: 1.0, ..styled(&theme, TextRole::Title, "GlideDeck", 430.0) }),
        at(-40.0, 352.0, 1.0, 0.0),
    );

    // One quiet line under the wordmark.
    add(
        &mut doc,
        "scripts",
        "Scripts",
        ElementKind::Text(TextBlock { align: TextAlign::Center, ..styled(&theme, TextRole::Caption, "Type in any language, right to left too. Every script stays sharp at any zoom.", 30.0) }),
        at(-560.0, 612.0, 1.0, 0.0),
    );

    // A tilted outline sticker.
    add(
        &mut doc,
        "sticker",
        "Sticker",
        ElementKind::Rect(RectShape { width: 300.0, height: 76.0, corner_radius: 38.0, fill: None, stroke: Some(Stroke { color: yellow, width: 3.0 }), ..Default::default() }),
        at(880.0, 606.0, 1.0, -6.0),
    );
    add(
        &mut doc,
        "sticker-text",
        "Sticker text",
        ElementKind::Text(TextBlock { color: yellow, weight: 700, align: TextAlign::Center, role: None, ..styled(&theme, TextRole::Body, "100% offline", 32.0) }),
        at(880.0, 606.0, 1.0, -6.0),
    );

    // A portal into nested worlds: each level is 100× smaller, inside the last.
    let (x, y) = (1012.0, -62.0);
    add(&mut doc, "portal", "Portal", ElementKind::Ellipse(EllipseShape { width: 240.0, height: 240.0, fill: Some(yellow), ..Default::default() }), at(x, y, 1.0, 0.0));
    add(&mut doc, "portal-core", "Portal core", ElementKind::Ellipse(EllipseShape { width: 112.0, height: 112.0, fill: Some(theme.background), ..Default::default() }), at(x, y, 1.0, 0.0));
    add(
        &mut doc,
        "portal-hint",
        "Portal hint",
        ElementKind::Text(TextBlock { align: TextAlign::Center, ..styled(&theme, TextRole::Caption, "Zoom in here", 32.0) }),
        at(x, y + 168.0, 1.0, 0.0),
    );
    let mut scale = 1.0;
    for depth in 1..=4 {
        scale /= 100.0;
        let label = match depth {
            1 => "Level 1: 100× deeper",
            2 => "Level 2: 10,000× deeper",
            3 => "Level 3: 1,000,000× deeper",
            _ => "Level 4: still crisp.",
        };
        let ring = [yellow, bone, aqua, yellow][depth - 1];
        add(
            &mut doc,
            &format!("depth-{depth}-ring"),
            &format!("Depth {depth} ring"),
            ElementKind::Ellipse(EllipseShape { width: 9000.0, height: 9000.0, fill: Some(theme.background), gradient: None, stroke: Some(Stroke { color: ring, width: 260.0 }) }),
            at(x, y, scale, 0.0),
        );
        add(
            &mut doc,
            &format!("depth-{depth}-label"),
            &format!("Depth {depth} label"),
            ElementKind::Text(TextBlock { align: TextAlign::Center, weight: 800, ..styled(&theme, TextRole::Heading, label, 560.0) }),
            at(x, y - 1500.0 * scale, scale, 0.0),
        );
    }

    // Waypoint frames. All are invisible; the slab frames are tilted with
    // their slab, so the camera turns to face each one.
    let frame = |w: f64| ElementKind::Frame(Frame { width: w, height: w * 9.0 / 16.0, corner_radius: 0.0, fill: None, border: None });
    let frames = [
        ("frame-overview", "Overview", frame(2400.0), at(0.0, 0.0, 1.0, 0.0)),
        ("frame-title", "Presentations that move", frame(1120.0), at(LEFT + 520.0, -460.0, 1.0, 0.0)),
        ("frame-canvas", "Infinite canvas", frame(640.0), at(236.0, -436.0, 1.0, -3.0)),
        ("frame-text", "PowerPoint in and out", frame(640.0), at(70.0, -82.0, 1.0, 2.0)),
        ("frame-motion", "Motion built in", frame(640.0), at(776.0, -422.0, 1.0, 3.0)),
        ("frame-agents", "AI-ready", frame(640.0), at(596.0, -68.0, 1.0, -2.0)),
        ("frame-scripts", "Every script", frame(1180.0), at(-560.0, 612.0, 1.0, 0.0)),
        ("frame-sticker", "Offline", frame(460.0), at(880.0, 606.0, 1.0, -6.0)),
        ("frame-portal", "Portal", frame(620.0), at(x, y + 30.0, 1.0, 0.0)),
        ("frame-depth-1", "Level 1", frame(14000.0), at(x, y, 0.01, 0.0)),
        ("frame-depth-2", "Level 2", frame(14000.0), at(x, y, 0.0001, 0.0)),
        ("frame-depth-4", "Level 4", frame(14000.0), at(x, y, 0.00000001, 0.0)),
    ];
    for (id, name, kind, t) in frames {
        add(&mut doc, id, name, kind, t);
    }
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

    // In-slide animations: the promise builds in, each slab's words follow.
    let after = |id: &str, effect: Effect, delay: f64| Build::new(id, effect).with_trigger(Trigger::AfterPrevious).with_delay(delay);
    let with = |id: &str, effect: Effect, delay: f64| Build::new(id, effect).with_trigger(Trigger::WithPrevious).with_delay(delay);
    doc.path[1].builds = vec![after("tagline", Effect::Words, 0.1), with("subtitle", Effect::Rise, 0.7), Build::new("tagline", Effect::Highlight)];
    for (i, card) in ["card-canvas", "card-text", "card-motion", "card-agents"].into_iter().enumerate() {
        doc.path[2 + i].builds = vec![after(&format!("{card}-title"), Effect::Rise, 0.1), with(&format!("{card}-body"), Effect::Fade, 0.35)];
    }
    doc.path[7].builds = vec![after("sticker", Effect::Pop, 0.1), with("sticker-text", Effect::Pop, 0.1)];

    doc.path[1].notes = "# Welcome\n- Everything here runs locally: no cloud, no accounts.\n- Click once more to highlight the tagline.".into();
    doc.path[3].notes = "Mention PowerPoint import: File → Import PowerPoint, then Arrange on the path strip.".into();
    doc.path[4].notes = "Show the Animate tab and the Carousel template.".into();

    doc
}
