//! Converts elements into triangle meshes in their local coordinate space.

use crate::text::{PathCmd, TextEngine};
use bytemuck::{Pod, Zeroable};
use glam::DVec2;
use lyon::math::{point, vector, Angle, Box2D, Point};
use lyon::path::builder::BorderRadii;
use lyon::path::{Path, Winding};
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use glide_core::{Color, ElementKind, Gradient, Media, Stroke, TextAlign, TextBlock};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    /// sRGB, straight alpha.
    pub color: [u8; 4],
}

pub type Mesh = VertexBuffers<Vertex, u32>;

/// Hash of everything that affects an element's local geometry.
/// Transforms are deliberately excluded: they never trigger re-tessellation.
pub fn geometry_hash(kind: &ElementKind) -> u64 {
    let mut h = DefaultHasher::new();
    std::mem::discriminant(kind).hash(&mut h);
    let f = |v: f64, h: &mut DefaultHasher| v.to_bits().hash(h);
    let stroke = |s: &Option<Stroke>, h: &mut DefaultHasher| {
        if let Some(s) = s {
            s.color.hash(h);
            s.width.to_bits().hash(h);
        }
    };
    let gradient = |g: &Option<Gradient>, h: &mut DefaultHasher| {
        if let Some(g) = g {
            g.to.hash(h);
            g.angle.to_bits().hash(h);
        }
    };
    match kind {
        ElementKind::Rect(r) => {
            f(r.width, &mut h);
            f(r.height, &mut h);
            f(r.corner_radius, &mut h);
            r.fill.hash(&mut h);
            gradient(&r.gradient, &mut h);
            stroke(&r.stroke, &mut h);
        }
        ElementKind::Ellipse(e) => {
            f(e.width, &mut h);
            f(e.height, &mut h);
            e.fill.hash(&mut h);
            gradient(&e.gradient, &mut h);
            stroke(&e.stroke, &mut h);
        }
        ElementKind::Text(t) => {
            crate::text::layout_hash(t).hash(&mut h);
            t.color.hash(&mut h);
        }
        ElementKind::Frame(fr) => {
            f(fr.width, &mut h);
            f(fr.height, &mut h);
            f(fr.corner_radius, &mut h);
            fr.fill.hash(&mut h);
            stroke(&fr.border, &mut h);
        }
        ElementKind::Icon(i) => {
            i.name.hash(&mut h);
            i.style.hash(&mut h);
            f(i.size, &mut h);
            i.color.hash(&mut h);
        }
        ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) => {
            m.path.hash(&mut h);
            f(m.width, &mut h);
            f(m.height, &mut h);
        }
    }
    h.finish()
}

/// Local size of an element (text requires shaping).
pub fn local_size(text: &mut TextEngine, kind: &ElementKind) -> DVec2 {
    match kind {
        ElementKind::Text(t) => text.layout(t).size,
        k => k.intrinsic_size().unwrap_or(DVec2::ZERO),
    }
}

fn fill_with(mesh: &mut Mesh, path: &Path, tolerance: f32, rule: FillRule, color: impl Fn(Point) -> [u8; 4]) {
    let opts = FillOptions::tolerance(tolerance).with_fill_rule(rule);
    let mut builder = BuffersBuilder::new(mesh, |v: FillVertex| Vertex { pos: v.position().to_array(), color: color(v.position()) });
    if let Err(e) = FillTessellator::new().tessellate_path(path, &opts, &mut builder) {
        log::warn!("fill tessellation failed: {e:?}");
    }
}

fn fill(mesh: &mut Mesh, path: &Path, color: Color, tolerance: f32, rule: FillRule) {
    let c = color.0;
    fill_with(mesh, path, tolerance, rule, |_| c);
}

/// Fills with a solid color or, if given, a linear gradient across the shape box.
fn fill_shape(mesh: &mut Mesh, path: &Path, from: Color, gradient: Option<Gradient>, size: DVec2, tolerance: f32) {
    let Some(g) = gradient else {
        fill(mesh, path, from, tolerance, FillRule::NonZero);
        return;
    };
    let (sin, cos) = (g.angle.to_radians() as f32).sin_cos();
    let half = (size * 0.5).as_vec2();
    let extent = (half.x * cos).abs() + (half.y * sin).abs();
    let (a, b) = (from.0, g.to.0);
    fill_with(mesh, path, tolerance, FillRule::NonZero, move |p| {
        let t = if extent > 0.0 { ((p.x * cos + p.y * sin) / extent * 0.5 + 0.5).clamp(0.0, 1.0) } else { 0.0 };
        let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]), mix(a[3], b[3])]
    });
}

fn stroke(mesh: &mut Mesh, path: &Path, s: &Stroke, tolerance: f32) {
    if s.width <= 0.0 || s.color.a() == 0 {
        return;
    }
    let c = s.color.0;
    let opts = StrokeOptions::tolerance(tolerance)
        .with_line_width(s.width as f32)
        .with_line_join(LineJoin::Round);
    let mut builder =
        BuffersBuilder::new(mesh, |v: StrokeVertex| Vertex { pos: v.position().to_array(), color: c });
    if let Err(e) = StrokeTessellator::new().tessellate_path(path, &opts, &mut builder) {
        log::warn!("stroke tessellation failed: {e:?}");
    }
}

fn rect_path(w: f64, h: f64, radius: f64) -> Path {
    let (hw, hh) = (w as f32 / 2.0, h as f32 / 2.0);
    let r = (radius as f32).clamp(0.0, hw.min(hh));
    let mut b = Path::builder();
    let rect = Box2D::new(point(-hw, -hh), point(hw, hh));
    if r > 0.0 {
        b.add_rounded_rectangle(&rect, &BorderRadii::new(r), Winding::Positive);
    } else {
        b.add_rectangle(&rect, Winding::Positive);
    }
    b.build()
}

fn ellipse_path(w: f64, h: f64) -> Path {
    let mut b = Path::builder();
    b.add_ellipse(point(0.0, 0.0), vector(w as f32 / 2.0, h as f32 / 2.0), Angle::zero(), Winding::Positive);
    b.build()
}

/// Converts path commands into a lyon path.
pub fn commands_to_path(cmds: &[PathCmd]) -> Path {
    let mut b = Path::builder();
    let mut open = false;
    let p = |v: [f32; 2]| point(v[0], v[1]);
    for c in cmds {
        match *c {
            PathCmd::Move(v) => {
                if open {
                    b.end(false);
                }
                b.begin(p(v));
                open = true;
            }
            PathCmd::Line(v) if open => {
                b.line_to(p(v));
            }
            PathCmd::Quad(a, v) if open => {
                b.quadratic_bezier_to(p(a), p(v));
            }
            PathCmd::Cubic(a, c2, v) if open => {
                b.cubic_bezier_to(p(a), p(c2), p(v));
            }
            PathCmd::Close if open => {
                b.end(true);
                open = false;
            }
            _ => {}
        }
    }
    if open {
        b.end(false);
    }
    b.build()
}

/// Tessellates an element's geometry with the given tolerance (local units).
pub fn tessellate(text: &mut TextEngine, kind: &ElementKind, tolerance: f32) -> Mesh {
    let mut mesh = Mesh::new();
    match kind {
        ElementKind::Rect(r) => {
            let path = rect_path(r.width, r.height, r.corner_radius);
            if let Some(c) = r.fill {
                fill_shape(&mut mesh, &path, c, r.gradient, DVec2::new(r.width, r.height), tolerance);
            }
            if let Some(s) = &r.stroke {
                stroke(&mut mesh, &path, s, tolerance);
            }
        }
        ElementKind::Ellipse(e) => {
            let path = ellipse_path(e.width, e.height);
            if let Some(c) = e.fill {
                fill_shape(&mut mesh, &path, c, e.gradient, DVec2::new(e.width, e.height), tolerance);
            }
            if let Some(s) = &e.stroke {
                stroke(&mut mesh, &path, s, tolerance);
            }
        }
        ElementKind::Text(t) => {
            let layout = text.layout(t);
            for (color, path) in text.colored_outlines(&layout) {
                fill(&mut mesh, &path, color.unwrap_or(t.color), tolerance, FillRule::NonZero);
            }
        }
        ElementKind::Frame(fr) => {
            let path = rect_path(fr.width, fr.height, fr.corner_radius);
            if let Some(c) = fr.fill {
                fill(&mut mesh, &path, c, tolerance, FillRule::NonZero);
            }
            if let Some(s) = &fr.border {
                stroke(&mut mesh, &path, s, tolerance);
            }
        }
        ElementKind::Icon(icon) => match text.icon_outline(&icon.name, icon.style, icon.size as f32) {
            Some(cmds) => fill(&mut mesh, &commands_to_path(&cmds), icon.color, tolerance, FillRule::NonZero),
            None => {
                // Unknown icon name: a dashed-looking placeholder square.
                let s = Stroke { color: Color::rgba(0x8a, 0x93, 0xa6, 0xc0), width: icon.size / 24.0 };
                stroke(&mut mesh, &rect_path(icon.size, icon.size, icon.size / 8.0), &s, tolerance);
            }
        },
        ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) => {
            media_placeholder(&mut mesh, text, kind.type_name(), m, tolerance);
        }
    }
    mesh
}

/// Shown while an image decodes, when a file is missing, and for media kinds
/// whose playback is not implemented yet.
fn media_placeholder(mesh: &mut Mesh, text: &mut TextEngine, kind: &str, m: &Media, tolerance: f32) {
    let (w, h) = (m.width.max(1e-9), m.height.max(1e-9));
    let frame = rect_path(w, h, m.corner_radius);
    fill(mesh, &frame, Color::rgba(0x8a, 0x93, 0xa6, 0x30), tolerance, FillRule::NonZero);
    let line = (w.min(h) / 100.0).max(1e-9);
    let s = Stroke { color: Color::rgba(0x5b, 0x64, 0x78, 0xc0), width: line };
    stroke(mesh, &frame, &s, tolerance);

    let file = std::path::Path::new(&m.path).file_name().map(|f| f.to_string_lossy().into_owned());
    let mut label = TextBlock::new(format!("{kind}: {}", file.unwrap_or_default()));
    label.font_size = h.min(w) / 12.0;
    label.align = TextAlign::Center;
    label.color = Color::rgb(0x3b, 0x42, 0x52);
    let layout = text.layout(&label);
    fill(mesh, &text.outline(&layout), label.color, tolerance, FillRule::NonZero);
}
