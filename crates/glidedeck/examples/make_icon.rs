//! Renders the GlideDeck app icon with GlideDeck's own renderer and writes
//! PNGs (16–1024 px) plus a multi-size Windows .ico into `assets/icon/`.
//!
//! cargo run -p glidedeck --example make_icon

use glide_core::{Camera, Color, Document, Element, ElementKind, Gradient, RectShape, Shadow, Stroke, Transform};
use glide_render::HeadlessRenderer;
use glam::DVec2;

const SIZE: u32 = 1024;
const RADIUS: f64 = 228.0;

fn rect(w: f64, h: f64, r: f64, fill: Option<Color>, stroke: Option<Stroke>, at: (f64, f64)) -> Element {
    Element::new(ElementKind::Rect(RectShape { width: w, height: h, corner_radius: r, fill, stroke, ..Default::default() }))
        .with_transform(Transform { x: at.0, y: at.1, scale: 1.0, rotation: 0.0 })
}

fn main() -> anyhow::Result<()> {
    let mut doc = Document::new("icon");
    doc.background = Color::WHITE;
    let mut bg = rect(SIZE as f64, SIZE as f64, RADIUS, Some(Color::rgb(0x6D, 0x4D, 0xFF)), None, (0.0, 0.0));
    if let ElementKind::Rect(r) = &mut bg.kind {
        r.gradient = Some(Gradient { to: Color::rgb(0x1E, 0xC8, 0xC8), angle: 45.0 });
    }
    doc.add(bg);
    let white = |a: u8| Color::rgba(0xFF, 0xFF, 0xFF, a);
    // Three nested slide frames, each tucked into its parent's top-right
    // corner: zooming into the canvas.
    doc.add(rect(640.0, 360.0, 46.0, None, Some(Stroke { color: white(0x5C), width: 28.0 }), (-20.0, 60.0)));
    doc.add(rect(380.0, 214.0, 32.0, None, Some(Stroke { color: white(0xB0), width: 26.0 }), (70.0, 27.0)));
    let mut focus = rect(176.0, 99.0, 20.0, Some(white(0xFF)), None, (138.0, 3.5));
    focus.shadow = Some(Shadow { color: Color::rgba(0x1B, 0x10, 0x5C, 0x60), blur: 24.0, x: 0.0, y: 12.0 });
    doc.add(focus);

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
