//! Renders the GlideDeck app icon with GlideDeck's own renderer and writes
//! PNGs (16–1024 px) plus a multi-size Windows .ico into `assets/icon/`.
//!
//! cargo run -p glidedeck --example make_icon

use glide_core::{Camera, Color, Document, Element, ElementKind, EllipseShape, Gradient, RectShape, Shadow, Transform};
use glide_render::HeadlessRenderer;
use glam::DVec2;

const SIZE: u32 = 1024;
const RADIUS: f64 = 228.0;

fn rect(w: f64, h: f64, r: f64, fill: Color, at: (f64, f64), rotation: f64) -> Element {
    Element::new(ElementKind::Rect(RectShape { width: w, height: h, corner_radius: r, fill: Some(fill), ..Default::default() }))
        .with_transform(Transform { x: at.0, y: at.1, scale: 1.0, rotation })
}

fn disc(d: f64, fill: Color, at: (f64, f64)) -> Element {
    Element::new(ElementKind::Ellipse(EllipseShape { width: d, height: d, fill: Some(fill), ..Default::default() }))
        .with_transform(Transform { x: at.0, y: at.1, scale: 1.0, rotation: 0.0 })
}

fn main() -> anyhow::Result<()> {
    // The house style in one mark: a graphite tile, a tilted bone-white
    // slide, and the signal-yellow portal you zoom into.
    let graphite = Color::rgb(0x0F, 0x11, 0x13);
    let yellow = Color::rgb(0xFF, 0xD2, 0x3F);
    let bone = Color::rgb(0xF1, 0xEF, 0xE9);
    let mut doc = Document::new("icon");
    doc.background = graphite;
    let mut bg = rect(SIZE as f64, SIZE as f64, RADIUS, Color::rgb(0x23, 0x28, 0x2D), (0.0, 0.0), 0.0);
    if let ElementKind::Rect(r) = &mut bg.kind {
        r.gradient = Some(Gradient { to: graphite, angle: 90.0 });
    }
    doc.add(bg);
    let shadow = |blur: f64, y: f64| Some(Shadow { color: Color::rgba(0, 0, 0, 0x90), blur, x: 0.0, y });

    // The slide, tilted, low and left.
    let mut slide = rect(600.0, 372.0, 64.0, bone, (-96.0, 150.0), -9.0);
    slide.shadow = shadow(60.0, 26.0);
    doc.add(slide);
    // Two quiet lines of "text" on it.
    let turn = DVec2::from_angle((-9.0f64).to_radians());
    for (w, local) in [(250.0, DVec2::new(-125.0, 62.0)), (170.0, DVec2::new(-165.0, 122.0))] {
        let p = DVec2::new(-96.0, 150.0) + turn.rotate(local);
        doc.add(rect(w, 34.0, 17.0, Color::rgba(0x0F, 0x11, 0x13, 0x38), (p.x, p.y), -9.0));
    }

    // The portal: zoom in here.
    let at = (168.0, -150.0);
    let mut portal = disc(500.0, yellow, at);
    portal.shadow = shadow(70.0, 30.0);
    doc.add(portal);
    doc.add(disc(250.0, graphite, at));
    doc.add(disc(96.0, yellow, at));

    let mut r = HeadlessRenderer::new()?;
    let camera = Camera { center: DVec2::ZERO, zoom: 1.0, rotation: 0.0 };
    let mut rgba = r.render_rgba(&doc, &camera, [SIZE, SIZE], false)?;

    // Cut the rounded-square silhouette with an anti-aliased mask.
    let half = SIZE as f64 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = DVec2::new(x as f64 + 0.5 - half, y as f64 + 0.5 - half);
            let q = p.abs() - DVec2::splat(half - RADIUS);
            let d = q.max(DVec2::ZERO).length() + q.x.max(q.y).min(0.0) - RADIUS;
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * SIZE + x) * 4 + 3) as usize;
            rgba[i] = (rgba[i] as f64 * a).round() as u8;
        }
    }
    let full = image::RgbaImage::from_raw(SIZE, SIZE, rgba).expect("size");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon");
    std::fs::create_dir_all(&out)?;
    let mut frames = Vec::new();
    for s in [16u32, 24, 32, 48, 64, 128, 256, 512, 1024] {
        let img = image::imageops::resize(&full, s, s, image::imageops::FilterType::Lanczos3);
        img.save(out.join(format!("glidedeck-{s}.png")))?;
        if s <= 256 {
            frames.push(image::codecs::ico::IcoFrame::as_png(img.as_raw(), s, s, image::ExtendedColorType::Rgba8)?);
        }
    }
    let ico = std::fs::File::create(out.join("glidedeck.ico"))?;
    image::codecs::ico::IcoEncoder::new(ico).encode_images(&frames)?;
    println!("wrote {}", out.canonicalize()?.display());
    Ok(())
}
