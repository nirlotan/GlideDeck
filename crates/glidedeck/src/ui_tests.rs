//! End-to-end UI tests: the real app driven through egui_kittest with a real
//! wgpu device, so clicks and drags go through the same code paths as a user.
//!
//! Set `GLIDEDECK_SNAPSHOTS=<dir>` to also write screenshots for review.

use crate::app::{Tool, GlideDeckApp};
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use glide_core::{Document, ElementKind};

fn harness(doc: Option<Document>) -> Harness<'static, GlideDeckApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1480.0, 920.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| {
            let mut app = GlideDeckApp::new(cc, None).expect("app starts");
            if let Some(doc) = doc {
                app.doc = doc;
                app.history.reset(&app.doc);
                app.current_step = None;
            }
            app
        });
    h.ctx.set_theme(egui::ThemePreference::Light);
    h.run_steps(4);
    h
}

fn snapshot(h: &mut Harness<'_, GlideDeckApp>, name: &str) {
    let Some(dir) = std::env::var_os("GLIDEDECK_SNAPSHOTS") else { return };
    h.run_steps(3);
    let img = h.render().expect("render");
    let path = std::path::Path::new(&dir).join(format!("{name}.png"));
    img.save(&path).expect("save snapshot");
}

fn drag(h: &mut Harness<'_, GlideDeckApp>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.step();
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    let steps = 8;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        h.event(Event::PointerMoved(from + (to - from) * t));
        h.step();
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.step();
    h.step();
}

fn click(h: &mut Harness<'_, GlideDeckApp>, at: Pos2) {
    h.event(Event::PointerMoved(at));
    h.step();
    h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.step();
    h.step();
}

fn canvas_center(h: &Harness<'_, GlideDeckApp>) -> Pos2 {
    h.state().canvas_rect.center()
}

#[test]
fn drawing_shapes_by_drag_and_click() {
    let mut h = harness(Some(Document::new("Shapes")));
    // Pick the rectangle tool from the ribbon, then drag it out.
    h.get_by_label("Insert").click();
    h.run_steps(2);
    h.get_by_label("Rectangle").click();
    h.run_steps(2);
    assert_eq!(h.state().tool, Tool::Rect);

    let c = canvas_center(&h);
    drag(&mut h, c + egui::vec2(-300.0, -100.0), c + egui::vec2(-100.0, 20.0));
    let doc = &h.state().doc;
    assert_eq!(doc.elements.len(), 1, "drag should create a rectangle");
    let el = &doc.elements[0];
    let ElementKind::Rect(r) = &el.kind else { panic!("expected rect, got {:?}", el.kind) };
    let on_screen = egui::vec2((r.width * el.transform.scale * h.state().camera.zoom) as f32, (r.height * el.transform.scale * h.state().camera.zoom) as f32);
    assert!((on_screen.x - 200.0).abs() < 2.0 && (on_screen.y - 120.0).abs() < 2.0, "drawn size {on_screen:?}");
    assert_eq!(h.state().tool, Tool::Select, "tool returns to Select after drawing");

    // Ellipse by click (default size), via keyboard shortcut.
    h.key_press(Key::O);
    h.run_steps(2);
    click(&mut h, c + egui::vec2(200.0, 50.0));
    assert!(h.state().doc.elements.iter().any(|e| matches!(e.kind, ElementKind::Ellipse(_))));

    // Both actually render (drawn by the GPU renderer, not culled).
    h.run_steps(2);
    assert_eq!(h.state().stats.drawn, 2);
    snapshot(&mut h, "shapes");
}

#[test]
fn frames_become_slides_and_present() {
    let mut h = harness(Some(Document::new("Frames")));
    let c = canvas_center(&h);
    // Two frames: one drawn, one from the current view.
    h.key_press(Key::F);
    h.run_steps(2);
    drag(&mut h, c + egui::vec2(-320.0, -160.0), c + egui::vec2(-20.0, 10.0));
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
    h.run_steps(3);
    {
        let app = h.state();
        assert_eq!(app.doc.path.len(), 2, "each frame becomes a slide");
        let frame = app.doc.step_frame(0).expect("first slide has a frame");
        let ElementKind::Frame(f) = &frame.kind else { unreachable!() };
        assert!(f.is_invisible(), "frames are invisible by default");
        assert!((f.width / f.height - 16.0 / 9.0).abs() < 1e-6, "frames default to 16:9");
    }
    // Invisible frames are not drawn by the renderer.
    assert_eq!(h.state().stats.drawn, 0);
    snapshot(&mut h, "frames");

    // Present, advance, and exit.
    h.key_press(Key::F5);
    h.run_steps(3);
    assert!(h.state().presenting.is_some());
    h.key_press(Key::ArrowRight);
    for _ in 0..120 {
        h.step();
    }
    let step = h.state().presenting.as_ref().map(|p| p.step);
    assert_eq!(step, Some(1));
    snapshot(&mut h, "presenting");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.state().presenting.is_none());
    assert_eq!(h.state().current_step, Some(1));
}

#[test]
fn paste_text_and_import_image() {
    let mut h = harness(Some(Document::new("Media")));
    h.event(Event::Paste("Pasted from another app".into()));
    h.run_steps(3);
    assert!(h.state().doc.elements.iter().any(|e| matches!(&e.kind, ElementKind::Text(t) if t.content == "Pasted from another app")));

    // Import an image file: it is copied into assets/ and referenced relatively.
    let dir = std::env::temp_dir().join(format!("zc-ui-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("gradient.png");
    let img = image::RgbaImage::from_fn(320, 180, |x, y| image::Rgba([(x * 255 / 320) as u8, (y * 255 / 180) as u8, 200, 255]));
    img.save(&src).unwrap();
    let ctx = h.ctx.clone();
    h.state_mut().import_image_files(&ctx, &[src.clone()], None);
    h.run_steps(2);
    // Wait for the background decode.
    for _ in 0..200 {
        h.step();
        if h.state_mut().renderer.poll_images() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        } else {
            break;
        }
    }
    h.run_steps(2);
    let app = h.state();
    let img_el = app.doc.elements.iter().find(|e| matches!(e.kind, ElementKind::Image(_))).expect("image element");
    let ElementKind::Image(m) = &img_el.kind else { unreachable!() };
    assert!(m.path.starts_with("assets/"), "media is referenced relatively: {}", m.path);
    assert!(glide_core::assets::resolve(Some(&app.base_dir()), &m.path).exists());
    let json = app.doc.to_json();
    assert!(!json.contains("base64") && json.len() < 4000, "the JSON holds no media bytes");
    snapshot(&mut h, "media");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn demo_overview_snapshot() {
    let mut h = harness(None);
    h.run_steps(6);
    snapshot(&mut h, "editor");
    h.get_by_label("Transitions").click();
    h.run_steps(2);
    h.state_mut().go_to_step(6);
    for _ in 0..90 {
        h.step();
    }
    snapshot(&mut h, "editor-slide");
    h.state_mut().selection = vec!["frame-sticker".into()];
    let ctx = h.ctx.clone();
    ctx.set_theme(egui::ThemePreference::Dark);
    h.run_steps(4);
    snapshot(&mut h, "editor-dark");
}

#[test]
fn icon_library_inserts_vector_icons() {
    let mut h = harness(Some(Document::new("Icons")));
    h.get_by_label("Insert").click();
    h.run_steps(2);
    h.get_by_label("Icons").click();
    h.run_steps(3);
    assert!(h.state().icon_picker.open);
    h.state_mut().icon_picker.query = "rocket".into();
    h.state_mut().icon_picker.style = glide_core::IconStyle::Fill;
    h.run_steps(3);
    snapshot(&mut h, "icon-picker");
    h.get_by_label("Icon rocket-launch").click();
    h.run_steps(3);
    let icon = h.state().doc.elements.iter().find_map(|e| match &e.kind {
        ElementKind::Icon(i) => Some(i.clone()),
        _ => None,
    });
    let icon = icon.expect("icon inserted");
    assert_eq!(icon.name, "rocket-launch");
    assert_eq!(icon.style, glide_core::IconStyle::Fill);
    h.run_steps(2);
    assert_eq!(h.state().stats.drawn, 1, "icon renders as a vector mesh");
}

#[test]
fn layouts_themes_and_templates() {
    let mut h = harness(Some(Document::new("Design")));
    // Insert a layout: it becomes a frame + content and a new slide.
    h.get_by_label("Insert").click();
    h.run_steps(2);
    h.get_by_label("Layouts").click();
    h.run_steps(4);
    snapshot(&mut h, "layout-gallery");
    h.get_by_label("Insert layout Three points").click();
    for _ in 0..80 {
        h.step();
    }
    assert_eq!(h.state().doc.path.len(), 1);
    assert!(h.state().doc.elements.iter().any(|e| matches!(e.kind, ElementKind::Icon(_))));
    assert!(h.state().doc.elements.iter().any(|e| e.shadow.is_some()), "cards get soft shadows");
    snapshot(&mut h, "layout-inserted");

    // A non-linear layout from further down the gallery.
    h.get_by_label("Layouts").click();
    h.run_steps(4);
    h.get_by_label("Insert layout Venn").scroll_to_me();
    h.run_steps(4);
    h.get_by_label("Insert layout Venn").click();
    for _ in 0..80 {
        h.step();
    }
    assert_eq!(h.state().doc.path.len(), 2);
    assert_eq!(h.state().doc.elements.iter().filter(|e| matches!(e.name.as_str(), "Set 1" | "Set 2" | "Set 3")).count(), 3, "three Venn sets");

    // Switch theme from the Design tab.
    h.get_by_label("Design").click();
    h.run_steps(2);
    h.get_by_label("Theme Midnight").click();
    h.run_steps(4);
    let doc = &h.state().doc;
    assert_eq!(doc.theme.as_ref().map(|t| t.name.as_str()), Some("Midnight"));
    assert!(doc.background_to.is_some(), "Midnight has a gradient background");
    snapshot(&mut h, "theme-midnight");

    // Start from a template.
    h.get_by_label("Templates").click();
    h.run_steps(6);
    snapshot(&mut h, "template-gallery");
    h.get_by_label("Use template Pitch deck").click();
    h.run_steps(6);
    let doc = &h.state().doc;
    assert_eq!(doc.title, "Pitch deck");
    assert!(doc.path.len() >= 6);
    snapshot(&mut h, "template-pitch");

    // One of the structural templates.
    h.get_by_label("Templates").click();
    h.run_steps(6);
    h.get_by_label("Use template Orbit").click();
    h.run_steps(6);
    let doc = &h.state().doc;
    assert_eq!(doc.title, "Orbit");
    assert_eq!(doc.elements.iter().filter(|e| e.name == "Planet").count(), 5);
}

#[test]
fn smart_guides_snap_while_dragging() {
    let mut doc = Document::new("Guides");
    for (id, x, y) in [("a", 0.0, 0.0), ("b", 300.0, 200.0)] {
        let mut el = glide_core::Element::new(ElementKind::Rect(glide_core::RectShape { width: 100.0, height: 100.0, fill: Some(glide_core::Color::rgb(90, 80, 230)), ..Default::default() }))
            .with_transform(glide_core::Transform { x, y, scale: 1.0, rotation: 0.0 });
        el.id = id.into();
        doc.add(el);
    }
    let mut h = harness(Some(doc));
    h.state_mut().camera = glide_core::Camera::default();
    h.run_steps(2);
    let vp = h.state().viewport();
    let rect = h.state().canvas_rect;
    let to_screen = |w: glam::DVec2| rect.min + egui::vec2(((w.x - 0.0) + vp.x / 2.0) as f32, ((w.y - 0.0) + vp.y / 2.0) as f32);
    // Drag b left so its x lands 4 px away from a's x: it should snap exactly.
    let from = to_screen(glam::DVec2::new(300.0, 200.0));
    let to = to_screen(glam::DVec2::new(4.0, 200.0));
    drag(&mut h, from, to);
    let b = h.state().doc.get("b").unwrap().transform;
    assert!((b.x - 0.0).abs() < 1e-6, "snapped to x=0, got {}", b.x);
    assert!((b.y - 200.0).abs() < 1e-6);
}

fn screen_of(h: &Harness<'_, GlideDeckApp>, world: glam::DVec2) -> Pos2 {
    let app = h.state();
    let s = app.camera.world_to_screen(world, app.viewport());
    app.canvas_rect.min + egui::vec2(s.x as f32, s.y as f32)
}

fn one_element(kind: ElementKind, id: &str) -> Document {
    let mut doc = Document::new("t");
    let mut el = zui_el(kind);
    el.id = id.into();
    doc.add(el);
    doc
}

fn zui_el(kind: ElementKind) -> glide_core::Element {
    glide_core::Element::new(kind)
}

#[test]
fn corner_handles_resize_uniformly_and_knob_rotates() {
    let icon = ElementKind::Icon(glide_core::Icon { name: "star".into(), style: glide_core::IconStyle::Fill, size: 100.0, color: glide_core::Color::rgb(240, 160, 30) });
    let mut h = harness(Some(one_element(icon, "i")));
    h.state_mut().camera = glide_core::Camera::default();
    h.state_mut().selection = vec!["i".into()];
    h.run_steps(2);
    // Drag the bottom-right corner (50, 50) out to (150, 150): scale doubles, top-left stays.
    let from = screen_of(&h, glam::DVec2::new(50.0, 50.0));
    let to = screen_of(&h, glam::DVec2::new(150.0, 150.0));
    drag(&mut h, from, to);
    let t = h.state().doc.get("i").unwrap().transform;
    assert!((t.scale - 2.0).abs() < 0.02, "scale {}", t.scale);
    assert!((t.x - 50.0).abs() < 1.0 && (t.y - 50.0).abs() < 1.0, "anchored at the opposite corner: {t:?}");
    // Rotate with the knob above the top edge, snapping with Shift is optional; free rotate here.
    h.state_mut().doc.get_mut("i").unwrap().transform = glide_core::Transform::default();
    h.run_steps(2);
    let knob = screen_of(&h, glam::DVec2::new(0.0, -50.0)) + egui::vec2(0.0, -26.0);
    let right = screen_of(&h, glam::DVec2::new(120.0, 0.0));
    drag(&mut h, knob, right);
    let r = h.state().doc.get("i").unwrap().transform.rotation;
    assert!((r - 90.0).abs() < 2.0, "rotated a quarter turn, got {r}");
}

#[test]
fn in_place_editing_types_hebrew_and_bullets() {
    let theme = glide_core::theme::default_theme();
    let mut t = theme.text_style(glide_core::TextRole::Body);
    t.content = "Hello".into();
    let mut h = harness(Some(one_element(ElementKind::Text(t), "t")));
    h.state_mut().camera = glide_core::Camera::default();
    h.run_steps(2);
    // Select it and press Enter to edit in place (everything selected), then type Hebrew.
    h.state_mut().selection = vec!["t".into()];
    h.run_steps(1);
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert!(h.state().text_edit.is_some(), "editing started");
    h.event(Event::Text("שלום".into()));
    h.run_steps(1);
    h.key_press(Key::Enter);
    h.event(Event::Text("עולם".into()));
    h.run_steps(2);
    let content = match &h.state().doc.get("t").unwrap().kind {
        ElementKind::Text(t) => t.content.clone(),
        _ => unreachable!(),
    };
    assert_eq!(content, "שלום\nעולם");
    snapshot(&mut h, "editing-hebrew");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.state().text_edit.is_none());
    // Bullets from the ribbon's contextual text group.
    h.get_by_label("Home").click();
    h.run_steps(2);
    h.state_mut().selection = vec!["t".into()];
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().perform(&ctx, crate::app::Action::Duplicate);
    let id = h.state().selection[0].clone();
    if let Some(ElementKind::Text(t)) = h.state_mut().doc.get_mut(&id).map(|e| &mut e.kind) {
        t.list = glide_core::ListStyle::Bullet;
    }
    h.run_steps(3);
    snapshot(&mut h, "bullets");
}

#[test]
fn svg_and_video_import_and_play() {
    let dir = std::env::temp_dir().join(format!("gd-media-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let svg = dir.join("logo.svg");
    std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120"><defs><radialGradient id="g"><stop offset="0" stop-color="#ffd166"/><stop offset="1" stop-color="#ef476f"/></radialGradient></defs><circle cx="60" cy="60" r="56" fill="url(#g)"/><path d="M35 62 L54 80 L88 40" fill="none" stroke="#fff" stroke-width="10" stroke-linecap="round" stroke-linejoin="round"/></svg>"##).unwrap();
    let mut h = harness(Some(Document::new("Media")));
    let ctx = h.ctx.clone();
    h.state_mut().import_image_files(&ctx, &[svg], None);
    h.run_steps(3);
    assert!(h.state().doc.elements.iter().any(|e| matches!(e.kind, ElementKind::Svg(_))));
    assert_eq!(h.state().stats.drawn, 1, "svg renders as a vector mesh");

    let Some(ffmpeg) = glide_render::video::ffmpeg_path() else {
        eprintln!("FFmpeg not installed; skipping video playback");
        return;
    };
    let clip = dir.join("clip.mp4");
    let ok = std::process::Command::new(ffmpeg)
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=30:duration=3", "-f", "lavfi", "-i", "sine=frequency=440:duration=3", "-shortest", "-pix_fmt", "yuv420p"])
        .arg(&clip)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(ok, "generate test clip");
    h.state_mut().import_video_files(&ctx, &[clip], None);
    let id = h.state().selection[0].clone();
    let path = match &h.state().doc.get(&id).unwrap().kind {
        ElementKind::Video(m) => m.path.clone(),
        _ => unreachable!(),
    };
    for _ in 0..300 {
        h.step();
        if matches!(h.state_mut().renderer.video_status(&path), glide_render::VideoStatus::Ready { .. }) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(h.state_mut().renderer.video_status(&path), glide_render::VideoStatus::Ready { .. }));
    assert!(h.state_mut().toggle_video(&id), "playback starts");
    for _ in 0..60 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let position = match h.state_mut().renderer.video_status(&path) {
        glide_render::VideoStatus::Ready { position, .. } => position,
        other => panic!("{other:?}"),
    };
    assert!(position > 0.3, "video advanced to {position}s");
    snapshot(&mut h, "video");
    h.state_mut().renderer.pause_all_videos();
    std::fs::remove_dir_all(&dir).ok();
}

/// Regenerates the README screenshots:
/// `GLIDEDECK_SCREENSHOTS=docs/screenshots cargo test -p glidedeck readme_screenshots -- --ignored`
#[test]
#[ignore = "writes README screenshots; run explicitly"]
fn readme_screenshots() {
    let Some(dir) = std::env::var_os("GLIDEDECK_SCREENSHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shot = |h: &mut Harness<'_, GlideDeckApp>, name: &str| {
        h.run_steps(4);
        h.render().expect("render").save(dir.join(format!("{name}.png"))).expect("save");
    };
    let build = |doc: Option<Document>| {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1600.0, 960.0))
            .with_pixels_per_point(1.25)
            .wgpu()
            .build_eframe(move |cc| {
                let mut app = GlideDeckApp::new(cc, None).expect("app starts");
                if let Some(doc) = doc {
                    app.doc = doc;
                    app.history.reset(&app.doc);
                    app.current_step = Some(0);
                    app.pending_fit = true;
                }
                app
            });
        h.ctx.set_theme(egui::ThemePreference::Light);
        h.run_steps(6);
        h
    };
    let settle = |h: &mut Harness<'_, GlideDeckApp>| {
        for _ in 0..120 {
            h.step();
        }
    };

    // 1. The editor with the welcome canvas.
    let mut h = build(None);
    shot(&mut h, "editor");

    // 2. Design tab + a themed template in dark mode.
    let mut h = build(Some(crate::design::build_template(crate::design::Template::PitchDeck)));
    h.ctx.set_theme(egui::ThemePreference::Dark);
    h.state_mut().ribbon_tab = crate::app::RibbonTab::Design;
    h.state_mut().go_to_step(2);
    settle(&mut h);
    shot(&mut h, "design-dark");

    // 3. Template gallery.
    let mut h = build(None);
    h.state_mut().show_templates = true;
    for _ in 0..3 {
        shot(&mut h, "templates");
    }

    // 4. Slide layouts.
    let mut h = build(Some(crate::design::build_template(crate::design::Template::Blank)));
    h.state_mut().ribbon_tab = crate::app::RibbonTab::Insert;
    h.state_mut().show_layouts = true;
    for _ in 0..3 {
        shot(&mut h, "layouts");
    }

    // 5. Icon library.
    let mut h = build(Some(crate::design::build_template(crate::design::Template::Blank)));
    h.state_mut().ribbon_tab = crate::app::RibbonTab::Insert;
    h.state_mut().icon_picker.open = true;
    h.state_mut().icon_picker.query = "chart".into();
    h.state_mut().icon_picker.style = glide_core::IconStyle::Fill;
    shot(&mut h, "icons");

    // 6. Right-to-left editing in place, with bullets.
    let theme = glide_core::theme::default_theme();
    let mut doc = Document::new("עברית");
    let mut heading = theme.text_style(glide_core::TextRole::Heading);
    heading.content = "תוכנית העבודה".into();
    let mut h1 = glide_core::Element::new(ElementKind::Text(heading)).with_transform(glide_core::Transform { x: 0.0, y: -120.0, scale: 1.0, rotation: 0.0 });
    h1.id = "h".into();
    doc.add(h1);
    let mut body = theme.text_style(glide_core::TextRole::Body);
    body.content = "חזון ומטרות\nלוח זמנים ואבני דרך\nצוות ותקציב".into();
    body.list = glide_core::ListStyle::Numbered;
    body.font_size = 40.0;
    let mut b1 = glide_core::Element::new(ElementKind::Text(body)).with_transform(glide_core::Transform { x: 0.0, y: 40.0, scale: 1.0, rotation: 0.0 });
    b1.id = "b".into();
    doc.add(b1);
    let mut h = build(Some(doc));
    h.state_mut().selection = vec!["b".into()];
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::End);
    shot(&mut h, "rtl-editing");

    // 7. Presenting (fullscreen show, mid-path).
    let mut h = build(Some(crate::design::build_template(crate::design::Template::MindMap)));
    let ctx = h.ctx.clone();
    h.state_mut().start_presenting(&ctx, 2, Some(false));
    settle(&mut h);
    shot(&mut h, "present");

    // 8. Design check.
    let mut doc = crate::design::build_template(crate::design::Template::Journey);
    if let Some(ElementKind::Text(t)) = doc.elements.iter_mut().find(|e| matches!(e.kind, ElementKind::Text(_))).map(|e| &mut e.kind) {
        t.color = glide_core::Color::rgb(0xFF, 0xE8, 0xE0);
    }
    let mut h = build(Some(doc));
    h.state_mut().ribbon_tab = crate::app::RibbonTab::Design;
    h.state_mut().design_check.open = true;
    shot(&mut h, "design-check");
}

#[test]
fn delete_works_after_dragging_and_clicking_buttons() {
    let rect = |w| ElementKind::Rect(glide_core::RectShape { width: w, height: 100.0, fill: Some(glide_core::Color::rgb(90, 80, 230)), ..Default::default() });
    let mut doc = one_element(rect(100.0), "a");
    let mut b = zui_el(rect(100.0));
    b.id = "b".into();
    b.transform.x = 300.0;
    doc.add(b);
    let mut h = harness(Some(doc));
    h.state_mut().camera = glide_core::Camera::default();
    h.run_steps(2);
    // Move an object by dragging it, then press Delete.
    let a = screen_of(&h, glam::DVec2::ZERO);
    drag(&mut h, a, a + egui::vec2(40.0, 0.0));
    assert_eq!(h.state().selection, vec!["a".to_string()]);
    h.key_press(Key::Delete);
    h.run_steps(2);
    assert!(h.state().doc.get("a").is_none(), "Delete removes the dragged object");
    // Click a ribbon button (it keeps keyboard focus), select on the canvas, press Backspace.
    h.get_by_label("Home").click();
    h.run_steps(2);
    let at = screen_of(&h, glam::DVec2::new(300.0, 0.0));
    click(&mut h, at);
    let button = h.get_by_label_contains("Bring to front").rect();
    click(&mut h, button.center());
    assert_eq!(h.state().selection, vec!["b".to_string()]);
    h.key_press(Key::Backspace);
    h.run_steps(2);
    assert!(h.state().doc.get("b").is_none(), "Backspace removes it after a button click");

    // Edit a field in Properties, then go back to the canvas and press Delete.
    let mut c = zui_el(rect(100.0));
    c.id = "c".into();
    h.state_mut().doc.add(c);
    let at = screen_of(&h, glam::DVec2::ZERO);
    click(&mut h, at);
    assert_eq!(h.state().selection, vec!["c".to_string()]);
    let fields: Vec<_> = h.query_all(egui_kittest::kittest::By::new().role(egui::accesskit::Role::TextInput)).map(|n| n.rect()).collect();
    let field = fields[0];
    click(&mut h, field.center());
    h.event(Event::Text("x".into()));
    h.run_steps(2);
    let at = screen_of(&h, glam::DVec2::ZERO);
    click(&mut h, at);
    h.key_press(Key::Delete);
    h.run_steps(2);
    assert!(h.state().doc.get("c").is_none(), "Delete works after editing a Properties field");

    // Edit text in place on the canvas, finish, then delete another object.
    let mut t = zui_el(ElementKind::Text(glide_core::TextBlock::new("Hello")));
    t.id = "t".into();
    t.transform.y = 200.0;
    h.state_mut().doc.add(t);
    let mut d = zui_el(rect(100.0));
    d.id = "d".into();
    h.state_mut().doc.add(d);
    h.run_steps(2);
    h.state_mut().selection = vec!["t".into()];
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert!(h.state().text_edit.is_some(), "editing in place");
    h.event(Event::Text("!".into()));
    h.run_steps(2);
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.state().text_edit.is_none());
    let at = screen_of(&h, glam::DVec2::ZERO);
    click(&mut h, at);
    assert_eq!(h.state().selection, vec!["d".to_string()]);
    h.key_press(Key::Delete);
    h.run_steps(2);
    assert!(h.state().doc.get("d").is_none(), "Delete works after editing text in place");
}

fn cursor_at(h: &mut Harness<'_, GlideDeckApp>, at: Pos2) -> egui::CursorIcon {
    h.event(Event::PointerMoved(at));
    h.run_steps(2);
    h.output().platform_output.cursor_icon
}

#[test]
fn resize_cursors_follow_each_corner_and_side() {
    let rect = ElementKind::Rect(glide_core::RectShape { width: 200.0, height: 120.0, fill: Some(glide_core::Color::rgb(90, 80, 230)), ..Default::default() });
    let mut h = harness(Some(one_element(rect, "r")));
    h.state_mut().camera = glide_core::Camera::default();
    h.state_mut().selection = vec!["r".into()];
    h.run_steps(2);
    let at = |h: &Harness<'_, GlideDeckApp>, x, y| screen_of(h, glam::DVec2::new(x, y));
    use egui::CursorIcon as C;
    let p = at(&h, -100.0, -60.0);
    assert_eq!(cursor_at(&mut h, p), C::ResizeNwSe, "top-left");
    let p = at(&h, 100.0, -60.0);
    assert_eq!(cursor_at(&mut h, p), C::ResizeNeSw, "top-right");
    let p = at(&h, 100.0, 60.0);
    assert_eq!(cursor_at(&mut h, p), C::ResizeNwSe, "bottom-right");
    let p = at(&h, -100.0, 60.0);
    assert_eq!(cursor_at(&mut h, p), C::ResizeNeSw, "bottom-left");
    let p = at(&h, 100.0, 0.0);
    assert_eq!(cursor_at(&mut h, p), C::ResizeHorizontal, "right side");
    let p = at(&h, 0.0, 60.0);
    assert_eq!(cursor_at(&mut h, p), C::ResizeVertical, "bottom side");

    // Dragging the bottom side stretches the height only; the top edge stays.
    let from = at(&h, 0.0, 60.0);
    let to = at(&h, 0.0, 160.0);
    drag(&mut h, from, to);
    let el = h.state().doc.get("r").unwrap().clone();
    let ElementKind::Rect(r) = &el.kind else { unreachable!() };
    assert!((r.width - 200.0).abs() < 1e-6 && (r.height - 220.0).abs() < 1.0, "{} × {}", r.width, r.height);
    assert!((el.transform.y - r.height / 2.0 - -60.0).abs() < 1.0, "top edge fixed at -60");
}

#[test]
fn text_side_handles_change_the_wrap_width() {
    let mut t = glide_core::TextBlock::new("A long sentence that will wrap onto several lines");
    t.font_size = 30.0;
    let mut h = harness(Some(one_element(ElementKind::Text(t), "t")));
    h.state_mut().camera = glide_core::Camera::default();
    h.state_mut().selection = vec!["t".into()];
    h.run_steps(2);
    let el0 = h.state().doc.get("t").unwrap().clone();
    let size0 = h.state_mut().renderer.local_size(&el0);
    let top0 = h.state().doc.get("t").unwrap().transform.y - size0.y / 2.0;
    let left0 = h.state().doc.get("t").unwrap().transform.x - size0.x / 2.0;
    // Drag the right side in to about a third of the width.
    let from = screen_of(&h, glam::DVec2::new(left0 + size0.x, top0 + size0.y / 2.0));
    let to = screen_of(&h, glam::DVec2::new(left0 + size0.x / 3.0, top0 + size0.y / 2.0));
    assert_eq!(cursor_at(&mut h, from), egui::CursorIcon::ResizeHorizontal);
    drag(&mut h, from, to);
    let el = h.state().doc.get("t").unwrap().clone();
    let ElementKind::Text(t) = &el.kind else { unreachable!() };
    let w = t.max_width.expect("a wrap width is set");
    assert!((w - size0.x / 3.0).abs() < 2.0, "width {w} vs {}", size0.x / 3.0);
    assert!((t.font_size - 30.0).abs() < 1e-9 && (el.transform.scale - 1.0).abs() < 1e-9, "the font is not scaled");
    let size1 = h.state_mut().renderer.local_size(&el);
    assert!(size1.y > size0.y * 2.0, "text reflowed onto more lines: {} → {}", size0.y, size1.y);
    assert!((el.transform.y - size1.y / 2.0 - top0).abs() < 1.0, "top edge stays put");
    assert!((el.transform.x - size1.x / 2.0 - left0).abs() < 1.0, "left edge stays put");
    snapshot(&mut h, "text-wrap-handles");
}

#[test]
fn drawing_a_frame_keeps_its_start_corner() {
    let mut h = harness(Some(Document::new("t")));
    h.state_mut().camera = glide_core::Camera::default();
    h.state_mut().tool = Tool::Frame;
    h.run_steps(2);
    let start = screen_of(&h, glam::DVec2::new(-200.0, -100.0));
    // A tall drag: the 16:9 frame must grow away from the start point, not around the middle.
    let end = screen_of(&h, glam::DVec2::new(-150.0, 150.0));
    drag(&mut h, start, end);
    let f = h.state().doc.elements.iter().find(|e| e.kind.is_frame()).expect("a frame").clone();
    let size = f.kind.intrinsic_size().unwrap() * f.transform.scale;
    let top_left = f.transform.position() - size / 2.0;
    assert!((top_left - glam::DVec2::new(-200.0, -100.0)).length() < 1.5, "start corner fixed: {top_left:?}");
    assert!((size.x / size.y - 16.0 / 9.0).abs() < 1e-6);
    // Dragging up and left grows the frame that way instead.
    h.state_mut().tool = Tool::Frame;
    h.run_steps(1);
    let start = screen_of(&h, glam::DVec2::new(300.0, 300.0));
    let end = screen_of(&h, glam::DVec2::new(100.0, 250.0));
    drag(&mut h, start, end);
    let f = h.state().doc.elements.iter().rfind(|e| e.kind.is_frame()).unwrap().clone();
    let size = f.kind.intrinsic_size().unwrap() * f.transform.scale;
    let bottom_right = f.transform.position() + size / 2.0;
    assert!((bottom_right - glam::DVec2::new(300.0, 300.0)).length() < 1.5, "start corner fixed: {bottom_right:?}");
}

#[test]
fn presenter_view_shows_notes_next_slide_and_controls() {
    let mut doc = crate::design::build_template(crate::design::Template::PitchDeck);
    doc.path[1].notes = "# Open strong\n- Thank the hosts\n- **One** number to remember\n1. Pause for questions".into();
    let mut h = harness(Some(doc));
    let ctx = h.ctx.clone();
    h.state_mut().start_presenting(&ctx, 1, Some(true));
    h.run_steps(6);
    assert!(h.state().presenting.as_ref().is_some_and(|p| p.console.is_some()), "presenter view is on");
    h.get_by_label_contains("Presenter view");
    h.get_by_label_contains("Slide 2 of");
    h.get_by_label_contains("Open strong");
    h.get_by_label_contains("Thank the hosts");
    for _ in 0..60 {
        h.step();
    }
    snapshot(&mut h, "presenter-view");

    // Keys pressed on the console drive the show.
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(h.state().presenting.as_ref().unwrap().step, 2);
    h.get_by_label_contains("Slide 3 of");
    h.key_press(Key::B);
    h.run_steps(2);
    assert_eq!(h.state().presenting.as_ref().unwrap().blank, Some(egui::Color32::BLACK));
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    let p = h.state().presenting.as_ref().unwrap();
    assert_eq!((p.blank, p.step), (None, 2), "the first key after black just reveals the show");
    // Buttons.
    h.get_by_label_contains("Previous").click();
    h.run_steps(2);
    assert_eq!(h.state().presenting.as_ref().unwrap().step, 1);
    h.get_by_label_contains("End show").click();
    h.run_steps(3);
    assert!(h.state().presenting.is_none());
    assert_eq!(h.state().current_step, Some(1), "the editor returns to the last slide shown");
}

#[test]
fn rich_text_formatting_while_editing_in_place() {
    let mut t = glide_core::TextBlock::new("Make this bold");
    t.font_size = 48.0;
    let mut h = harness(Some(one_element(ElementKind::Text(t), "t")));
    h.state_mut().camera = glide_core::Camera::default();
    h.state_mut().selection = vec!["t".into()];
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    // Select "bold" (the last word) with Shift+Ctrl+Left, then Ctrl+B.
    h.key_press_modifiers(Modifiers::COMMAND, Key::End);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowLeft);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::B);
    h.run_steps(2);
    let text = |h: &Harness<'_, GlideDeckApp>| match &h.state().doc.get("t").unwrap().kind {
        ElementKind::Text(t) => t.clone(),
        _ => unreachable!(),
    };
    let t = text(&h);
    assert_eq!(t.spans.len(), 1, "{:?}", t.spans);
    assert_eq!(&t.content[t.spans[0].start..t.spans[0].end], "bold");
    assert_eq!(t.spans[0].style.bold, Some(true));
    // The floating format bar shows Bold as on.
    assert!(h.state().current_format_state().unwrap().bold);
    snapshot(&mut h, "format-bar");

    // With no selection, Ctrl+U applies to what is typed next.
    h.key_press_modifiers(Modifiers::COMMAND, Key::End);
    h.run_steps(1);
    h.key_press_modifiers(Modifiers::COMMAND, Key::U);
    h.event(Event::Text(" now".into()));
    h.run_steps(2);
    let t = text(&h);
    assert_eq!(t.content, "Make this bold now");
    let now = t.spans.iter().find(|s| &t.content[s.start..s.end] == " now").expect("typed text is formatted");
    assert!(now.style.underline && now.style.bold == Some(true), "{:?}", t.spans);

    // The floating bar's buttons work on a selection too.
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(2);
    h.get_by_label_contains("Italic").click();
    h.run_steps(2);
    let t = text(&h);
    assert!(t.range_all(0..t.content.len(), |s| t.is_italic(s)), "{:?}", t.spans);
    assert!(h.state().text_edit.is_some(), "clicking the bar keeps editing");

    // Properties edits keep formatting attached to its text.
    h.key_press(Key::Escape);
    h.run_steps(2);
    if let Some(ElementKind::Text(t)) = h.state_mut().doc.get_mut("t").map(|e| &mut e.kind) {
        t.set_content("Please make this bold now");
    }
    let t = text(&h);
    let bold: String = t.spans.iter().filter(|s| s.style.bold == Some(true)).map(|s| &t.content[s.start..s.end]).collect();
    assert_eq!(bold, "bold now");
}

#[test]
fn import_powerpoint_export_pdf_and_recent_files() {
    let dir = std::env::temp_dir().join(format!("glidedeck-ui-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pptx = dir.join("Quarterly.pptx");
    std::fs::write(&pptx, include_bytes!("testdata/sample.pptx")).unwrap();

    let mut h = harness(Some(Document::new("Before")));
    let ctx = h.ctx.clone();
    h.state_mut().import_pptx(&ctx, &pptx);
    h.run_steps(4);
    let app = h.state();
    assert_eq!(app.doc.path.len(), 5);
    assert!(app.is_dirty(), "an import is unsaved until you save it");
    assert!(app.recent.paths.iter().any(|p| p.ends_with("Quarterly.pptx")), "imports are listed as recent");
    let pic = app.doc.elements.iter().find_map(|e| match &e.kind {
        ElementKind::Image(m) => Some(m.path.clone()),
        _ => None,
    });
    let pic = pic.expect("the picture was imported");
    assert!(glide_core::assets::resolve(Some(&app.base_dir()), &pic).exists(), "pictures are stored with the document: {pic}");

    // File → Open recent lists it.
    h.get_by_label("File").click();
    h.run_steps(2);
    h.get_by_label_contains("Open recent").hover();
    h.run_steps(3);
    let entries = h.query_all(egui_kittest::kittest::By::new().label_contains("Quarterly.pptx").role(egui::accesskit::Role::Button)).count();
    assert_eq!(entries, 1, "one recent-file entry");

    // PDF export renders every slide with the editor's renderer.
    let out = dir.join("deck.pdf");
    let doc = h.state().doc.clone();
    let report = crate::pdf::export(&mut h.state_mut().renderer, &doc, &out, crate::pdf::PdfOptions { width_px: 640, quality: 80 }).unwrap();
    assert_eq!(report.pages, 5);
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"%PDF-1.7") && bytes.len() == report.bytes);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn arrange_slides_dialog_rearranges_an_imported_deck() {
    let dir = std::env::temp_dir().join(format!("glidedeck-ui-arrange-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pptx = dir.join("deck.pptx");
    std::fs::write(&pptx, include_bytes!("testdata/sample.pptx")).unwrap();
    let mut h = harness(Some(Document::new("x")));
    let ctx = h.ctx.clone();
    h.state_mut().import_pptx(&ctx, &pptx);
    h.run_steps(3);
    let before = h.state().doc.clone();

    h.get_by_label("Design").click();
    h.run_steps(2);
    h.get_by_label("Arrange").click();
    h.run_steps(3);
    assert!(h.state().arrange_dialog.is_some());
    h.get_by_label("Arrange as Orbits").click();
    h.run_steps(3);
    snapshot(&mut h, "arrange-dialog");
    h.get_by_label_contains("Arrange 5 slides").click();
    h.run_steps(4);
    let app = h.state();
    assert!(app.arrange_dialog.is_none());
    assert_eq!(app.doc.path.len(), 7, "five slides plus an overview at each end");
    let first = app.doc.step_frame(1).unwrap().transform.position();
    assert!(first.length() < 1.0, "Orbits puts the first slide at the center: {first:?}");
    // One undo restores the import.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(3);
    assert_eq!(h.state().doc, before);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn animate_tab_adds_builds_previews_and_the_show_plays_them() {
    let doc = crate::design::build_template(crate::design::Template::PitchDeck);
    let mut h = harness(Some(doc));
    h.state_mut().go_to_step(1);
    h.run_steps(90);
    h.get_by_label("Animate").click();
    h.run_steps(2);
    h.get_by_label("Auto-animate").click();
    h.run_steps(3);
    let auto = h.state().doc.path[1].builds.clone();
    assert!(auto.len() >= 2, "the slide's content cascades in: {auto:?}");
    assert_eq!(auto[0].trigger, glide_core::Trigger::AfterPrevious, "and plays on arrival");
    assert!(h.state().anim_preview.is_some(), "auto-animate previews the slide on the canvas");
    snapshot(&mut h, "animate-preview");
    for _ in 0..600 {
        if h.state().anim_preview.is_none() {
            break;
        }
        h.step();
    }
    assert!(h.state().anim_preview.is_none(), "the preview ends by itself");

    // Spotlight the first object on a click, from the ribbon menu.
    let target = auto[0].element.clone();
    h.state_mut().selection = vec![target.clone()];
    h.run_steps(2);
    h.get_by_label("Emphasis").click();
    h.run_steps(2);
    h.get_by_label_contains("Spotlight").click();
    h.run_steps(3);
    let builds = h.state().doc.path[1].builds.clone();
    let last = builds.last().unwrap();
    assert_eq!((last.element.as_str(), last.effect, last.trigger), (target.as_str(), glide_core::Effect::Spotlight, glide_core::Trigger::OnClick));
    h.get_by_label_contains("Slide 2 · click 2");
    for _ in 0..600 {
        if h.state().anim_preview.is_none() {
            break;
        }
        h.step();
    }
    snapshot(&mut h, "animate-tab");

    // The show: the cascade plays on arrival, a click plays the spotlight,
    // the next click moves on; going back undoes the spotlight first.
    let ctx = h.ctx.clone();
    h.state_mut().start_presenting(&ctx, 1, Some(false));
    let settle = |h: &mut Harness<'_, GlideDeckApp>| {
        for _ in 0..600 {
            h.step();
            let app = h.state();
            let p = app.presenting.as_ref().unwrap();
            if !app.nav.is_animating() && !p.builds.running(&app.doc, h.ctx.input(|i| i.time)) {
                break;
            }
        }
    };
    settle(&mut h);
    let starts = |h: &Harness<'_, GlideDeckApp>| h.state().presenting.as_ref().unwrap().builds.starts.clone();
    let s = starts(&h);
    assert!(s[0].is_some() && s.last().unwrap().is_none(), "arrived: cascade played, spotlight waiting: {s:?}");
    h.key_press(Key::ArrowRight);
    h.run_steps(4);
    snapshot(&mut h, "show-spotlight");
    settle(&mut h);
    assert_eq!(h.state().presenting.as_ref().unwrap().step, 1, "the click played the spotlight instead of moving on");
    assert!(starts(&h).iter().all(Option::is_some));
    h.key_press(Key::ArrowLeft);
    h.run_steps(2);
    assert_eq!(h.state().presenting.as_ref().unwrap().step, 1);
    assert!(starts(&h).last().unwrap().is_none(), "back takes the spotlight back");
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    settle(&mut h);
    h.key_press(Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(h.state().presenting.as_ref().unwrap().step, 2, "then the next click moves on");
    h.key_press(Key::ArrowLeft);
    h.run_steps(2);
    let p = h.state().presenting.as_ref().unwrap();
    assert_eq!(p.step, 1);
    assert!(p.builds.starts.iter().all(Option::is_some), "coming back shows the slide fully built");
    h.key_press(Key::Escape);
    h.run_steps(2);
}
