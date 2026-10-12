//! SVG support: files are parsed with usvg (which resolves CSS, `use`,
//! transforms and converts text to outlines) and tessellated into vector
//! meshes, so SVGs stay as crisp as native shapes at any zoom.

use crate::tessellate::{Mesh, Vertex};
use lyon::math::point;
use lyon::path::Path as LyonPath;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex,
};
use resvg::usvg;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// A 2×3 affine: x' = a*x + c*y + e, y' = b*x + d*y + f.
#[derive(Clone, Copy, Debug)]
struct Affine([f32; 6]);

impl Affine {
    fn from_usvg(t: usvg::Transform) -> Self {
        Affine([t.sx, t.ky, t.kx, t.sy, t.tx, t.ty])
    }
    fn apply(&self, x: f32, y: f32) -> [f32; 2] {
        let [a, b, c, d, e, f] = self.0;
        [a * x + c * y + e, b * x + d * y + f]
    }
    fn inverse(&self) -> Option<Self> {
        let [a, b, c, d, e, f] = self.0;
        let det = a * d - b * c;
        if det.abs() < 1e-12 {
            return None;
        }
        let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
        Some(Affine([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)]))
    }
    fn scale(&self) -> f32 {
        let [a, b, c, d, ..] = self.0;
        (a * d - b * c).abs().sqrt().max(1e-12)
    }
}

#[derive(Clone, Debug)]
enum Paint {
    Solid([u8; 4]),
    Linear { from: [f32; 2], to: [f32; 2], inv: Affine, stops: Vec<(f32, [u8; 4])> },
    Radial { center: [f32; 2], r: f32, inv: Affine, stops: Vec<(f32, [u8; 4])> },
}

impl Paint {
    fn from_usvg(p: &usvg::Paint, opacity: f32) -> Option<Paint> {
        let alpha = |o: f32| (o.clamp(0.0, 1.0) * 255.0).round() as u8;
        let stops = |s: &[usvg::Stop]| -> Vec<(f32, [u8; 4])> {
            s.iter()
                .map(|st| {
                    let c = st.color();
                    (st.offset().get(), [c.red, c.green, c.blue, alpha(st.opacity().get() * opacity)])
                })
                .collect()
        };
        Some(match p {
            usvg::Paint::Color(c) => Paint::Solid([c.red, c.green, c.blue, alpha(opacity)]),
            usvg::Paint::LinearGradient(g) => Paint::Linear {
                from: [g.x1(), g.y1()],
                to: [g.x2(), g.y2()],
                inv: Affine::from_usvg(g.transform()).inverse()?,
                stops: stops(g.stops()),
            },
            usvg::Paint::RadialGradient(g) => Paint::Radial {
                center: [g.cx(), g.cy()],
                r: g.r().get(),
                inv: Affine::from_usvg(g.transform()).inverse()?,
                stops: stops(g.stops()),
            },
            // Patterns: approximate with a neutral mid-grey.
            usvg::Paint::Pattern(_) => Paint::Solid([128, 128, 128, alpha(opacity)]),
        })
    }

    /// Color at a point in the path's own coordinate space.
    fn at(&self, x: f32, y: f32) -> [u8; 4] {
        let sample = |stops: &[(f32, [u8; 4])], t: f32| -> [u8; 4] {
            let t = t.clamp(0.0, 1.0);
            match stops {
                [] => [0, 0, 0, 0],
                [one] => one.1,
                _ => {
                    if t <= stops[0].0 {
                        return stops[0].1;
                    }
                    for w in stops.windows(2) {
                        let ((a, ca), (b, cb)) = (w[0], w[1]);
                        if t <= b {
                            let k = if b > a { (t - a) / (b - a) } else { 1.0 };
                            let mix = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * k).round() as u8;
                            return [mix(ca[0], cb[0]), mix(ca[1], cb[1]), mix(ca[2], cb[2]), mix(ca[3], cb[3])];
                        }
                    }
                    stops[stops.len() - 1].1
                }
            }
        };
        match self {
            Paint::Solid(c) => *c,
            Paint::Linear { from, to, inv, stops } => {
                let [px, py] = inv.apply(x, y);
                let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
                let len2 = (dx * dx + dy * dy).max(1e-12);
                sample(stops, ((px - from[0]) * dx + (py - from[1]) * dy) / len2)
            }
            Paint::Radial { center, r, inv, stops } => {
                let [px, py] = inv.apply(x, y);
                let d = ((px - center[0]).powi(2) + (py - center[1]).powi(2)).sqrt();
                sample(stops, d / r.max(1e-12))
            }
        }
    }
}

struct Shape {
    path: LyonPath,
    transform: Affine,
    fill: Option<(Paint, FillRule)>,
    stroke: Option<(Paint, f32, LineJoin, LineCap)>,
    stroke_first: bool,
}

/// A parsed SVG ready for tessellation.
pub struct SvgDoc {
    pub size: (f32, f32),
    shapes: Vec<Shape>,
}

fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

/// Parses an SVG file into a usvg tree (text converted to outlines).
pub fn parse_tree(path: &Path) -> Result<usvg::Tree, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let opt = usvg::Options { resources_dir: path.parent().map(Path::to_path_buf), fontdb: fontdb(), ..Default::default() };
    usvg::Tree::from_data(&data, &opt).map_err(|e| e.to_string())
}

/// Intrinsic size of an SVG file in CSS pixels.
pub fn svg_dimensions(path: &Path) -> Option<(f32, f32)> {
    let t = parse_tree(path).ok()?;
    Some((t.size().width(), t.size().height()))
}

fn to_lyon(p: &usvg::tiny_skia_path::Path) -> LyonPath {
    let mut b = LyonPath::builder();
    let mut open = false;
    for seg in p.segments() {
        use usvg::tiny_skia_path::PathSegment as S;
        match seg {
            S::MoveTo(a) => {
                if open {
                    b.end(false);
                }
                b.begin(point(a.x, a.y));
                open = true;
            }
            S::LineTo(a) if open => {
                b.line_to(point(a.x, a.y));
            }
            S::QuadTo(c, a) if open => {
                b.quadratic_bezier_to(point(c.x, c.y), point(a.x, a.y));
            }
            S::CubicTo(c1, c2, a) if open => {
                b.cubic_bezier_to(point(c1.x, c1.y), point(c2.x, c2.y), point(a.x, a.y));
            }
            S::Close if open => {
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

fn collect(group: &usvg::Group, opacity: f32, out: &mut Vec<Shape>) {
    let opacity = opacity * group.opacity().get();
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect(g, opacity, out),
            usvg::Node::Text(t) => collect(t.flattened(), opacity, out),
            usvg::Node::Path(p) if p.is_visible() => {
                let fill = p.fill().and_then(|f| {
                    let rule = match f.rule() {
                        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
                        usvg::FillRule::NonZero => FillRule::NonZero,
                    };
                    Paint::from_usvg(f.paint(), f.opacity().get() * opacity).map(|paint| (paint, rule))
                });
                let stroke = p.stroke().and_then(|s| {
                    let join = match s.linejoin() {
                        usvg::LineJoin::Round => LineJoin::Round,
                        usvg::LineJoin::Bevel => LineJoin::Bevel,
                        _ => LineJoin::Miter,
                    };
                    let cap = match s.linecap() {
                        usvg::LineCap::Round => LineCap::Round,
                        usvg::LineCap::Square => LineCap::Square,
                        usvg::LineCap::Butt => LineCap::Butt,
                    };
                    Paint::from_usvg(s.paint(), s.opacity().get() * opacity).map(|paint| (paint, s.width().get(), join, cap))
                });
                out.push(Shape {
                    path: to_lyon(p.data()),
                    transform: Affine::from_usvg(p.abs_transform()),
                    fill,
                    stroke,
                    stroke_first: matches!(p.paint_order(), usvg::PaintOrder::StrokeAndFill),
                });
            }
            _ => {}
        }
    }
}

pub fn load_svg(path: &Path) -> Result<SvgDoc, String> {
    let tree = parse_tree(path)?;
    let mut shapes = Vec::new();
    collect(tree.root(), 1.0, &mut shapes);
    Ok(SvgDoc { size: (tree.size().width(), tree.size().height()), shapes })
}

/// Tessellates an SVG into a `width` × `height` box centred on the origin.
pub fn tessellate_svg(doc: &SvgDoc, width: f64, height: f64, tolerance: f32) -> Mesh {
    let mut mesh = Mesh::new();
    let (sw, sh) = (doc.size.0.max(1e-6), doc.size.1.max(1e-6));
    let (kx, ky) = (width as f32 / sw, height as f32 / sh);
    let (hw, hh) = (width as f32 / 2.0, height as f32 / 2.0);
    for shape in &doc.shapes {
        let t = shape.transform;
        let place = move |x: f32, y: f32| {
            let [px, py] = t.apply(x, y);
            [px * kx - hw, py * ky - hh]
        };
        let tol = (tolerance / (t.scale() * kx.min(ky).max(1e-12))).max(1e-4);
        let do_fill = |mesh: &mut Mesh| {
            if let Some((paint, rule)) = &shape.fill {
                let opts = FillOptions::tolerance(tol).with_fill_rule(*rule);
                let mut b = BuffersBuilder::new(mesh, |v: FillVertex| {
                    let p = v.position();
                    Vertex { pos: place(p.x, p.y), color: paint.at(p.x, p.y) }
                });
                if let Err(e) = FillTessellator::new().tessellate_path(&shape.path, &opts, &mut b) {
                    log::warn!("svg fill failed: {e:?}");
                }
            }
        };
        let do_stroke = |mesh: &mut Mesh| {
            if let Some((paint, w, join, cap)) = &shape.stroke {
                let opts = StrokeOptions::tolerance(tol)
                    .with_line_width(*w)
                    .with_line_join(*join)
                    .with_start_cap(*cap)
                    .with_end_cap(*cap);
                let mut b = BuffersBuilder::new(mesh, |v: StrokeVertex| {
                    let p = v.position();
                    Vertex { pos: place(p.x, p.y), color: paint.at(p.x, p.y) }
                });
                if let Err(e) = StrokeTessellator::new().tessellate_path(&shape.path, &opts, &mut b) {
                    log::warn!("svg stroke failed: {e:?}");
                }
            }
        };
        if shape.stroke_first {
            do_stroke(&mut mesh);
            do_fill(&mut mesh);
        } else {
            do_fill(&mut mesh);
            do_stroke(&mut mesh);
        }
    }
    mesh
}

/// Rasterizes an SVG to PNG bytes (used for export fallbacks).
pub fn rasterize_png(path: &Path, max_side: u32) -> Option<Vec<u8>> {
    let tree = parse_tree(path).ok()?;
    let size = tree.size();
    let k = (max_side as f32 / size.width().max(size.height())).min(8.0);
    let (w, h) = ((size.width() * k).ceil().max(1.0) as u32, (size.height() * k).ceil().max(1.0) as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pixmap.as_mut());
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_tessellates_gradients_and_strokes() {
        let dir = std::env::temp_dir().join(format!("glide-svg-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("t.svg");
        std::fs::write(
            &file,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">
                <defs><linearGradient id="g" x1="0" x2="1"><stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/></linearGradient></defs>
                <rect x="10" y="10" width="180" height="80" rx="12" fill="url(#g)"/>
                <circle cx="100" cy="50" r="30" fill="none" stroke="#00ff00" stroke-width="6"/>
            </svg>"##,
        )
        .unwrap();
        assert_eq!(svg_dimensions(&file), Some((200.0, 100.0)));
        let doc = load_svg(&file).unwrap();
        let mesh = tessellate_svg(&doc, 400.0, 200.0, 0.5);
        assert!(!mesh.indices.is_empty());
        // Gradient: leftmost vertices are red-ish, rightmost blue-ish.
        let left = mesh.vertices.iter().min_by(|a, b| a.pos[0].total_cmp(&b.pos[0])).unwrap();
        let right = mesh.vertices.iter().filter(|v| v.color[1] < 128).max_by(|a, b| a.pos[0].total_cmp(&b.pos[0])).unwrap();
        assert!(left.color[0] > 200 && right.color[2] > 200, "{:?} {:?}", left.color, right.color);
        // Everything lands inside the 400 × 200 box centred on the origin.
        assert!(mesh.vertices.iter().all(|v| v.pos[0].abs() <= 200.5 && v.pos[1].abs() <= 100.5));
        assert!(rasterize_png(&file, 256).is_some());
        std::fs::remove_dir_all(&dir).ok();
    }
}
