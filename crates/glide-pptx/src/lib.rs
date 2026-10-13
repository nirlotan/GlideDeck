//! PowerPoint export with Morph transitions.
//!
//! Every step of the presentation path becomes one 16:9 slide. The frame of
//! the step defines the slide's "camera": each element is projected through it
//! (position, size and rotation relative to the frame), so neighbouring slides
//! contain the *same* objects at different positions and scales. Each shape is
//! named `!!<element id>` — the `!!` prefix forces PowerPoint's Morph to match
//! objects by name — and slides are linked with the Morph transition, so
//! PowerPoint itself animates the zooming, panning and rotation.
//!
//! The package is plain OpenXML (ECMA-376) written by hand: no Office install
//! or network access is involved.

use glam::DVec2;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use glide_core::{assets, Color, Document, Element, ElementKind, Gradient, Shadow, Stroke, TextAlign, TransitionStyle};
use glide_render::{local_size, PathCmd, TextEngine};

/// 16:9 slide in EMU (English Metric Units, 914400 per inch).
const SLIDE_W: f64 = 12_192_000.0;
const SLIDE_H: f64 = 6_858_000.0;
const EMU_PER_PT: f64 = 12_700.0;
/// OpenXML coordinates are 32-bit; stay well inside.
const COORD_LIMIT: f64 = 2.0e9;

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[derive(Debug, thiserror::Error)]
pub enum PptxError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("nothing to export: the document is empty")]
    Empty,
}

#[derive(Debug, Default, Clone)]
pub struct ExportReport {
    pub slides: usize,
    pub shapes: usize,
    /// Media that could not be read or is not supported yet (video, svg).
    pub skipped: Vec<String>,
}

/// How a slide sees the canvas: world center, rotation and scale.
#[derive(Clone, Copy)]
struct SlideView {
    center: DVec2,
    rotation_deg: f64,
    emu_per_world: f64,
}

impl SlideView {
    fn for_frame(frame: &Element) -> Option<SlideView> {
        let size = frame.kind.intrinsic_size()? * frame.transform.scale.abs();
        if size.x <= 0.0 || size.y <= 0.0 {
            return None;
        }
        Some(SlideView {
            center: frame.transform.position(),
            rotation_deg: frame.transform.rotation,
            emu_per_world: (SLIDE_W / size.x).min(SLIDE_H / size.y),
        })
    }

    /// World point → slide EMU.
    fn project(&self, p: DVec2) -> DVec2 {
        let d = DVec2::from_angle(-self.rotation_deg.to_radians()).rotate(p - self.center) * self.emu_per_world;
        DVec2::new(SLIDE_W / 2.0, SLIDE_H / 2.0) + d
    }
}

enum Source {
    Bytes(Vec<u8>),
    File(std::path::PathBuf),
}

/// Media parts for one referenced file.
struct Media {
    /// Bitmap (or poster / SVG fallback) part name, e.g. `image3.png`.
    image: String,
    svg: Option<String>,
    video: Option<String>,
    files: Vec<(String, Source)>,
}

struct SlideOut {
    xml: String,
    /// (rId, relationship type, target) beyond the layout relationship.
    media_rels: Vec<(String, &'static str, String)>,
}

const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_VIDEO: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";
const REL_MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";

fn rel(rels: &mut Vec<(String, &'static str, String)>, typ: &'static str, name: &str) -> String {
    let target = format!("../media/{name}");
    if let Some((r, _, _)) = rels.iter().find(|(_, t, g)| *t == typ && *g == target) {
        return r.clone();
    }
    let r = format!("rId{}", rels.len() + 2);
    rels.push((r.clone(), typ, target));
    r
}

/// Exports the presentation path of `doc` to `out`.
///
/// `base_dir` is the document's folder (for resolving media); `text` provides
/// the same text layout the canvas uses, so text boxes are sized identically.
pub fn export(doc: &Document, base_dir: Option<&Path>, text: &mut TextEngine, out: &Path) -> Result<ExportReport, PptxError> {
    let mut report = ExportReport::default();
    let mut views: Vec<(SlideView, Option<&glide_core::Step>)> = (0..doc.path.len())
        .filter_map(|i| doc.step_frame(i).and_then(SlideView::for_frame).map(|v| (v, Some(&doc.path[i]))))
        .collect();
    if views.is_empty() {
        // No path yet: one overview slide of everything.
        let bounds = doc
            .elements
            .iter()
            .filter(|e| !e.hidden)
            .fold(glide_core::Aabb::EMPTY, |b, e| b.union(e.world_aabb(local_size(text, &e.kind))));
        if bounds.is_empty() {
            return Err(PptxError::Empty);
        }
        let size = bounds.size().max(DVec2::splat(1e-9));
        views.push((
            SlideView {
                center: bounds.center(),
                rotation_deg: 0.0,
                emu_per_world: (SLIDE_W / size.x).min(SLIDE_H / size.y) * 0.92,
            },
            None,
        ));
    }

    let mut media: HashMap<String, Option<Media>> = HashMap::new();
    let mut media_order: Vec<String> = Vec::new();
    let mut slides = Vec::new();
    for (index, (view, step)) in views.iter().enumerate() {
        let slide = build_slide(doc, base_dir, text, view, *step, index, &mut media, &mut media_order, &mut report);
        slides.push(slide);
    }
    report.slides = slides.len();

    let file = std::fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let put = |zip: &mut zip::ZipWriter<_>, name: &str, data: &[u8]| -> Result<(), PptxError> {
        zip.start_file(name, opts)?;
        zip.write_all(data)?;
        Ok(())
    };

    put(&mut zip, "[Content_Types].xml", content_types(slides.len()).as_bytes())?;
    put(&mut zip, "_rels/.rels", ROOT_RELS.as_bytes())?;
    put(&mut zip, "docProps/core.xml", core_props(&doc.title).as_bytes())?;
    put(&mut zip, "docProps/app.xml", app_props(slides.len()).as_bytes())?;
    put(&mut zip, "ppt/presentation.xml", presentation(slides.len()).as_bytes())?;
    put(&mut zip, "ppt/_rels/presentation.xml.rels", presentation_rels(slides.len()).as_bytes())?;
    put(&mut zip, "ppt/presProps.xml", PRES_PROPS.as_bytes())?;
    put(&mut zip, "ppt/viewProps.xml", VIEW_PROPS.as_bytes())?;
    put(&mut zip, "ppt/tableStyles.xml", TABLE_STYLES.as_bytes())?;
    put(&mut zip, "ppt/theme/theme1.xml", THEME.as_bytes())?;
    put(&mut zip, "ppt/slideMasters/slideMaster1.xml", SLIDE_MASTER.as_bytes())?;
    put(&mut zip, "ppt/slideMasters/_rels/slideMaster1.xml.rels", SLIDE_MASTER_RELS.as_bytes())?;
    put(&mut zip, "ppt/slideLayouts/slideLayout1.xml", SLIDE_LAYOUT.as_bytes())?;
    put(&mut zip, "ppt/slideLayouts/_rels/slideLayout1.xml.rels", SLIDE_LAYOUT_RELS.as_bytes())?;
    for (i, slide) in slides.iter().enumerate() {
        let n = i + 1;
        put(&mut zip, &format!("ppt/slides/slide{n}.xml"), slide.xml.as_bytes())?;
        put(&mut zip, &format!("ppt/slides/_rels/slide{n}.xml.rels"), slide_rels(&slide.media_rels).as_bytes())?;
    }
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
    for reference in &media_order {
        if let Some(Some(m)) = media.get(reference) {
            for (name, source) in &m.files {
                zip.start_file(format!("ppt/media/{name}"), stored)?;
                match source {
                    Source::Bytes(b) => zip.write_all(b)?,
                    Source::File(p) => {
                        std::io::copy(&mut std::fs::File::open(p)?, &mut zip)?;
                    }
                }
            }
        }
    }
    zip.finish()?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn build_slide(
    doc: &Document,
    base_dir: Option<&Path>,
    text: &mut TextEngine,
    view: &SlideView,
    step: Option<&glide_core::Step>,
    index: usize,
    media: &mut HashMap<String, Option<Media>>,
    media_order: &mut Vec<String>,
    report: &mut ExportReport,
) -> SlideOut {
    let mut shapes = String::new();
    let mut media_rels: Vec<(String, &'static str, String)> = Vec::new();
    let mut next_id = 2u32;

    for el in doc.elements.iter().filter(|e| !e.hidden && e.opacity > 0.0) {
        if let ElementKind::Frame(f) = &el.kind
            && f.is_invisible()
        {
            continue;
        }
        let size_local = local_size(text, &el.kind);
        let k = el.transform.scale.abs() * view.emu_per_world;
        let ext = size_local * k;
        if ext.max_element() < SLIDE_W / 4000.0 || ext.max_element() > SLIDE_W * 60.0 {
            continue;
        }
        let center = view.project(el.transform.position());
        let off = center - ext * 0.5;
        // Keep objects near the slide (Morph fades the rest in/out).
        let reach = ext.length() * 0.5;
        if center.x + reach < -SLIDE_W
            || center.x - reach > 2.0 * SLIDE_W
            || center.y + reach < -SLIDE_H
            || center.y - reach > 2.0 * SLIDE_H
            || off.abs().max_element() > COORD_LIMIT
            || (off + ext).abs().max_element() > COORD_LIMIT
        {
            continue;
        }
        let rot = ((el.transform.rotation - view.rotation_deg).rem_euclid(360.0) * 60_000.0).round() as i64 % 21_600_000;
        let xfrm = Xfrm { off, ext, rot };
        let id = next_id;
        let name = format!("!!{}", el.id);
        let descr = el.display_name();
        let opacity = el.opacity.clamp(0.0, 1.0);

        let fx = effects(el.shadow, k, opacity);
        let xml = match &el.kind {
            ElementKind::Rect(r) => {
                let geom = rect_geom(r.width, r.height, r.corner_radius);
                let paint = fill_or_gradient(r.fill, r.gradient, opacity);
                Some(shape(id, &name, &descr, &xfrm, &geom, paint, line(r.stroke.as_ref(), k, opacity) + &fx, None))
            }
            ElementKind::Frame(f) => {
                let geom = rect_geom(f.width, f.height, f.corner_radius);
                Some(shape(id, &name, &descr, &xfrm, &geom, fill(f.fill, opacity), line(f.border.as_ref(), k, opacity) + &fx, None))
            }
            ElementKind::Ellipse(e) => {
                let geom = r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#.to_owned();
                let paint = fill_or_gradient(e.fill, e.gradient, opacity);
                Some(shape(id, &name, &descr, &xfrm, &geom, paint, line(e.stroke.as_ref(), k, opacity) + &fx, None))
            }
            ElementKind::Icon(icon) => text.icon_outline(&icon.name, icon.style, icon.size as f32).map(|cmds| {
                let geom = icon_geom(&cmds, icon.size);
                shape(id, &name, &descr, &xfrm, &geom, solid(icon.color, opacity), "<a:ln><a:noFill/></a:ln>".into(), None)
            }),
            ElementKind::Text(t) => {
                let family = text.resolve_family(&t.font_family);
                let body = text_body(t, k, opacity, &family);
                let geom = r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.to_owned();
                Some(shape(id, &name, &descr, &xfrm, &geom, "<a:noFill/>".into(), "<a:ln><a:noFill/></a:ln>".into(), Some(body)))
            }
            ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) => {
                let entry = media.entry(m.path.clone()).or_insert_with(|| {
                    let loaded = load_media(base_dir, &m.path, media_order.len() + 1, &el.kind);
                    if loaded.is_some() {
                        media_order.push(m.path.clone());
                    }
                    loaded
                });
                match entry {
                    Some(med) => {
                        let image_rid = rel(&mut media_rels, REL_IMAGE, &med.image);
                        let svg_rid = med.svg.as_ref().map(|s| rel(&mut media_rels, REL_IMAGE, s));
                        let video_rids = med.video.as_ref().map(|v| (rel(&mut media_rels, REL_VIDEO, v), rel(&mut media_rels, REL_MEDIA, v)));
                        let geom = image_geom(m.width, m.height, m.corner_radius);
                        Some(match video_rids {
                            Some((link, embed)) => video_picture(id, &name, &descr, &xfrm, &image_rid, &link, &embed, &geom, &fx),
                            None => picture(id, &name, &descr, &xfrm, &image_rid, svg_rid.as_deref(), opacity, &geom, &fx),
                        })
                    }
                    None => {
                        if !report.skipped.contains(&m.path) {
                            report.skipped.push(m.path.clone());
                        }
                        None
                    }
                }
            }
        };
        if let Some(xml) = xml {
            shapes.push_str(&xml);
            next_id += 1;
            report.shapes += 1;
        }
    }

    let bg = match doc.background_to {
        Some(to) => format!(
            r#"<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:srgbClr val="{}"/></a:gs><a:gs pos="100000"><a:srgbClr val="{}"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill>"#,
            hex(doc.background),
            hex(to)
        ),
        None => format!(r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(doc.background)),
    };
    let transition = match step {
        Some(s) if index > 0 && s.transition.style != TransitionStyle::Cut => {
            let ms = (s.transition.duration * 1000.0).round().clamp(100.0, 60_000.0) as u32;
            format!(
                r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:p159="http://schemas.microsoft.com/office/powerpoint/2015/09/main" Requires="p159"><p:transition spd="slow" p14:dur="{ms}"><p159:morph option="byObject"/></p:transition></mc:Choice><mc:Fallback><p:transition spd="slow"><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"#
            )
        }
        _ => String::new(),
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" mc:Ignorable="p14"><p:cSld><p:bg><p:bgPr>{bg}<a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>{transition}</p:sld>"#
    );
    SlideOut { xml, media_rels }
}

struct Xfrm {
    off: DVec2,
    ext: DVec2,
    rot: i64,
}

impl Xfrm {
    fn xml(&self) -> String {
        let rot = if self.rot != 0 { format!(r#" rot="{}""#, self.rot) } else { String::new() };
        format!(
            r#"<a:xfrm{rot}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
            self.off.x.round() as i64,
            self.off.y.round() as i64,
            (self.ext.x.round() as i64).max(1),
            (self.ext.y.round() as i64).max(1)
        )
    }
}

fn rect_geom(w: f64, h: f64, radius: f64) -> String {
    let min = w.min(h);
    if radius > 0.0 && min > 0.0 {
        let adj = ((radius / min) * 100_000.0).round().clamp(0.0, 50_000.0) as i64;
        format!(r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val {adj}"/></a:avLst></a:prstGeom>"#)
    } else {
        r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.to_owned()
    }
}

fn image_geom(w: f64, h: f64, radius: f64) -> String {
    let min = w.min(h);
    if radius > 0.0 && (w - h).abs() < 1e-6 * w.max(1.0) && radius >= min / 2.0 - 1e-9 {
        r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#.to_owned()
    } else {
        rect_geom(w, h, radius)
    }
}

/// An icon as an editable DrawingML custom geometry.
fn icon_geom(cmds: &[PathCmd], size: f64) -> String {
    const N: f64 = 100_000.0;
    let s = size.max(1e-12);
    let pt = |p: [f32; 2]| {
        let x = ((p[0] as f64 / s + 0.5) * N).round() as i64;
        let y = ((p[1] as f64 / s + 0.5) * N).round() as i64;
        format!(r#"<a:pt x="{x}" y="{y}"/>"#)
    };
    let mut path = String::new();
    for c in cmds {
        match *c {
            PathCmd::Move(p) => path.push_str(&format!("<a:moveTo>{}</a:moveTo>", pt(p))),
            PathCmd::Line(p) => path.push_str(&format!("<a:lnTo>{}</a:lnTo>", pt(p))),
            PathCmd::Quad(a, p) => path.push_str(&format!("<a:quadBezTo>{}{}</a:quadBezTo>", pt(a), pt(p))),
            PathCmd::Cubic(a, b, p) => path.push_str(&format!("<a:cubicBezTo>{}{}{}</a:cubicBezTo>", pt(a), pt(b), pt(p))),
            PathCmd::Close => path.push_str("<a:close/>"),
        }
    }
    format!(
        r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/><a:pathLst><a:path w="{n}" h="{n}">{path}</a:path></a:pathLst></a:custGeom>"#,
        n = N as i64
    )
}

fn clr(c: Color, opacity: f64) -> String {
    let alpha = (c.a() as f64 / 255.0 * opacity * 100_000.0).round() as i64;
    if alpha >= 100_000 {
        format!(r#"<a:srgbClr val="{}"/>"#, hex(c))
    } else {
        format!(r#"<a:srgbClr val="{}"><a:alpha val="{alpha}"/></a:srgbClr>"#, hex(c))
    }
}

fn fill_or_gradient(fill_color: Option<Color>, gradient: Option<Gradient>, opacity: f64) -> String {
    match (fill_color, gradient) {
        (Some(from), Some(g)) => {
            let ang = ((g.angle.rem_euclid(360.0)) * 60_000.0).round() as i64;
            format!(
                r#"<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0">{}</a:gs><a:gs pos="100000">{}</a:gs></a:gsLst><a:lin ang="{ang}" scaled="0"/></a:gradFill>"#,
                clr(from, opacity),
                clr(g.to, opacity)
            )
        }
        _ => fill(fill_color, opacity),
    }
}

/// A soft outer shadow matching the canvas look.
fn effects(shadow: Option<Shadow>, emu_per_local: f64, opacity: f64) -> String {
    let Some(s) = shadow else { return String::new() };
    let blur = (s.blur * 2.0 * emu_per_local).round().clamp(0.0, 20_000_000.0) as i64;
    let dist = ((s.x * s.x + s.y * s.y).sqrt() * emu_per_local).round().clamp(0.0, 20_000_000.0) as i64;
    let dir = ((s.y.atan2(s.x).to_degrees().rem_euclid(360.0)) * 60_000.0).round() as i64 % 21_600_000;
    format!(
        r#"<a:effectLst><a:outerShdw blurRad="{blur}" dist="{dist}" dir="{dir}" algn="ctr" rotWithShape="1">{}</a:outerShdw></a:effectLst>"#,
        clr(s.color, opacity)
    )
}

fn hex(c: Color) -> String {
    format!("{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

fn solid(c: Color, opacity: f64) -> String {
    let alpha = (c.a() as f64 / 255.0 * opacity * 100_000.0).round() as i64;
    if alpha >= 100_000 {
        format!(r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(c))
    } else {
        format!(r#"<a:solidFill><a:srgbClr val="{}"><a:alpha val="{alpha}"/></a:srgbClr></a:solidFill>"#, hex(c))
    }
}

fn fill(c: Option<Color>, opacity: f64) -> String {
    match c {
        Some(c) if c.a() > 0 => solid(c, opacity),
        _ => "<a:noFill/>".into(),
    }
}

fn line(s: Option<&Stroke>, emu_per_local: f64, opacity: f64) -> String {
    match s {
        Some(s) if s.width > 0.0 && s.color.a() > 0 => {
            let w = (s.width * emu_per_local).round().clamp(1.0, 20_116_800.0) as i64;
            format!(r#"<a:ln w="{w}">{}</a:ln>"#, solid(s.color, opacity))
        }
        _ => "<a:ln><a:noFill/></a:ln>".into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn shape(id: u32, name: &str, descr: &str, xfrm: &Xfrm, geom: &str, fill: String, line: String, body: Option<String>) -> String {
    let txbox = if body.is_some() { r#" txBox="1""# } else { "" };
    let body = body.unwrap_or_default();
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}" descr="{}"/><p:cNvSpPr{txbox}/><p:nvPr/></p:nvSpPr><p:spPr>{}{geom}{fill}{line}</p:spPr>{body}</p:sp>"#,
        esc(name),
        esc(descr),
        xfrm.xml()
    )
}

#[allow(clippy::too_many_arguments)]
fn picture(id: u32, name: &str, descr: &str, xfrm: &Xfrm, rid: &str, svg_rid: Option<&str>, opacity: f64, geom: &str, fx: &str) -> String {
    let mut alpha = if opacity < 1.0 {
        format!(r#"<a:alphaModFix amt="{}"/>"#, (opacity * 100_000.0).round() as i64)
    } else {
        String::new()
    };
    // Native SVG (Office 2016+) with the PNG as fallback for older readers.
    if let Some(svg) = svg_rid {
        alpha.push_str(&format!(
            r#"<a:extLst><a:ext uri="{{96DAC541-7B7A-43D3-8B79-37D633B846F1}}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="{svg}"/></a:ext></a:extLst>"#
        ));
    }
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{}" descr="{}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{rid}">{alpha}</a:blip><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}{geom}{fx}</p:spPr></p:pic>"#,
        esc(name),
        esc(descr),
        xfrm.xml()
    )
}

/// An embedded video (plays on click in PowerPoint), with its poster frame.
#[allow(clippy::too_many_arguments)]
fn video_picture(id: u32, name: &str, descr: &str, xfrm: &Xfrm, poster: &str, link: &str, embed: &str, geom: &str, fx: &str) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{}" descr="{}"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:videoFile r:link="{link}"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DA1AE8F5C1F}}"><p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" r:embed="{embed}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="{poster}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}{geom}{fx}</p:spPr></p:pic>"#,
        esc(name),
        esc(descr),
        xfrm.xml()
    )
}

fn is_rtl(s: &str) -> bool {
    s.chars().find(|c| c.is_alphabetic()).is_some_and(|c| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF))
}

fn text_body(t: &glide_core::TextBlock, emu_per_local: f64, opacity: f64, family: &str) -> String {
    let sz = (t.font_size * emu_per_local / EMU_PER_PT * 100.0).round().clamp(100.0, 400_000.0) as i64;
    let algn = match t.align {
        TextAlign::Left => "l",
        TextAlign::Center => "ctr",
        TextAlign::Right => "r",
    };
    let wrap = if t.max_width.is_some() { "square" } else { "none" };
    // PowerPoint's "single" spacing is ~1.2× the font size.
    let spacing = ((t.line_height / 1.2) * 100_000.0).round().clamp(10_000.0, 1_000_000.0) as i64;
    let face = esc(family);
    // Run properties for a formatted span (the box's own style when plain).
    let rpr_for = |extra: &str, style: &glide_core::SpanStyle| {
        let size = style.size.map_or(sz, |z| (z * emu_per_local / EMU_PER_PT * 100.0).round().clamp(100.0, 400_000.0) as i64);
        let b = if t.is_bold(style) { "1" } else { "0" };
        let i = if t.is_italic(style) { "1" } else { "0" };
        let u = if style.underline { r#" u="sng""# } else { "" };
        let strike = if style.strike { r#" strike="sngStrike""# } else { "" };
        format!(
            r#"<a:{extra} lang="en-US" sz="{size}" b="{b}" i="{i}"{u}{strike} dirty="0">{}<a:latin typeface="{face}"/><a:ea typeface="{face}"/><a:cs typeface="{face}"/></a:{extra}>"#,
            solid(style.color.unwrap_or(t.color), opacity)
        )
    };
    let rpr = |extra: &str| rpr_for(extra, &glide_core::SpanStyle::default());
    let segments = t.segments();
    let mut paras = String::new();
    // Hanging indent matching the canvas (0.95em bullets, 1.55em numbers).
    let em = t.font_size * emu_per_local;
    let list_ppr = |line: &str| -> (String, String) {
        if line.trim().is_empty() {
            return (String::new(), "<a:buNone/>".into());
        }
        match t.list {
            glide_core::ListStyle::None => (String::new(), String::new()),
            glide_core::ListStyle::Bullet => {
                let m = (em * 0.95).round() as i64;
                (format!(r#" marL="{m}" indent="-{m}""#), r#"<a:buFont typeface="Arial"/><a:buChar char="•"/>"#.into())
            }
            glide_core::ListStyle::Numbered => {
                let m = (em * 1.55).round() as i64;
                (format!(r#" marL="{m}" indent="-{m}""#), r#"<a:buFont typeface="+mj-lt"/><a:buAutoNum type="arabicPeriod"/>"#.into())
            }
        }
    };
    let mut line_start = 0;
    for raw in t.content.split('\n') {
        let start = line_start;
        line_start += raw.len() + 1;
        let line = raw.trim_end_matches('\r');
        let rtl = if is_rtl(line) { r#" rtl="1""# } else { "" };
        let (mar, bullet) = list_ppr(line);
        let ppr = format!(r#"<a:pPr{mar} algn="{algn}"{rtl}><a:lnSpc><a:spcPct val="{spacing}"/></a:lnSpc>{bullet}</a:pPr>"#);
        if line.is_empty() {
            let _ = write!(paras, "<a:p>{ppr}{}</a:p>", rpr("endParaRPr"));
        } else {
            // One run per formatted segment of this paragraph.
            let mut runs = String::new();
            for (a, b, style) in &segments {
                let (a, b) = ((*a).max(start), (*b).min(start + line.len()));
                if a < b {
                    let _ = write!(runs, "<a:r>{}<a:t>{}</a:t></a:r>", rpr_for("rPr", style), esc(&t.content[a..b]));
                }
            }
            let _ = write!(paras, "<a:p>{ppr}{runs}</a:p>");
        }
    }
    format!(
        r#"<p:txBody><a:bodyPr wrap="{wrap}" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t" rtlCol="0"><a:noAutofit/></a:bodyPr><a:lstStyle/>{paras}</p:txBody>"#
    )
}

/// A neutral 16:9 poster for videos when no frame can be decoded.
fn placeholder_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(320, 180, image::Rgba([0x1f, 0x23, 0x2b, 0xff]));
    let mut png = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut png, image::ImageFormat::Png);
    png.into_inner()
}

/// Reads a media file, converting formats PowerPoint can't embed to PNG.
fn load_media(base_dir: Option<&Path>, reference: &str, n: usize, kind: &ElementKind) -> Option<Media> {
    let path = assets::resolve(base_dir, reference);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match kind {
        ElementKind::Video(_) => {
            if !path.exists() {
                return None;
            }
            let video = format!("media{n}.{}", if ext.is_empty() { "mp4" } else { &ext });
            let poster_bytes = glide_render::video::poster_png(&path).unwrap_or_else(placeholder_png);
            let poster = format!("image{n}.png");
            Some(Media {
                image: poster.clone(),
                svg: None,
                video: Some(video.clone()),
                files: vec![(poster, Source::Bytes(poster_bytes)), (video, Source::File(path))],
            })
        }
        ElementKind::Svg(_) => {
            let svg_bytes = std::fs::read(&path).ok()?;
            let png = glide_render::svg::rasterize_png(&path, 1600)?;
            let (image, svg) = (format!("image{n}.png"), format!("image{n}.svg"));
            Some(Media {
                image: image.clone(),
                svg: Some(svg.clone()),
                video: None,
                files: vec![(image, Source::Bytes(png)), (svg, Source::Bytes(svg_bytes))],
            })
        }
        _ => {
            let bytes = std::fs::read(&path).ok()?;
            let (name, bytes) = match ext.as_str() {
                "png" | "gif" | "bmp" => (format!("image{n}.{ext}"), bytes),
                "jpg" | "jpeg" => (format!("image{n}.jpeg"), bytes),
                _ => {
                    let img = image::load_from_memory(&bytes).ok()?;
                    let mut png = std::io::Cursor::new(Vec::new());
                    img.write_to(&mut png, image::ImageFormat::Png).ok()?;
                    (format!("image{n}.png"), png.into_inner())
                }
            };
            Some(Media { image: name.clone(), svg: None, video: None, files: vec![(name, Source::Bytes(bytes))] })
        }
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

// --- package boilerplate ---------------------------------------------------------

fn content_types(slides: usize) -> String {
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpeg" ContentType="image/jpeg"/><Default Extension="gif" ContentType="image/gif"/><Default Extension="bmp" ContentType="image/bmp"/><Default Extension="svg" ContentType="image/svg+xml"/><Default Extension="mp4" ContentType="video/mp4"/><Default Extension="m4v" ContentType="video/mp4"/><Default Extension="mov" ContentType="video/quicktime"/><Default Extension="webm" ContentType="video/webm"/><Default Extension="wmv" ContentType="video/x-ms-wmv"/><Default Extension="avi" ContentType="video/x-msvideo"/><Default Extension="mkv" ContentType="video/x-matroska"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/><Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/><Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
    );
    for n in 1..=slides {
        let _ = write!(
            s,
            r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        );
    }
    s.push_str("</Types>");
    s
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;

fn core_props(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>GlideDeck</dc:creator></cp:coreProperties>"#,
        esc(title)
    )
}

fn app_props(slides: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>GlideDeck</Application><Slides>{slides}</Slides></Properties>"#
    )
}

fn presentation(slides: usize) -> String {
    let mut ids = String::new();
    for i in 0..slides {
        let _ = write!(ids, r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + i, 10 + i);
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{}" cy="{}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#,
        SLIDE_W as i64, SLIDE_H as i64
    )
}

fn presentation_rels(slides: usize) -> String {
    let mut s = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL_BASE}/slideMaster" Target="slideMasters/slideMaster1.xml"/><Relationship Id="rId2" Type="{REL_BASE}/theme" Target="theme/theme1.xml"/><Relationship Id="rId3" Type="{REL_BASE}/presProps" Target="presProps.xml"/><Relationship Id="rId4" Type="{REL_BASE}/viewProps" Target="viewProps.xml"/><Relationship Id="rId5" Type="{REL_BASE}/tableStyles" Target="tableStyles.xml"/>"#
    );
    for i in 0..slides {
        let _ = write!(s, r#"<Relationship Id="rId{}" Type="{REL_BASE}/slide" Target="slides/slide{}.xml"/>"#, 10 + i, i + 1);
    }
    s.push_str("</Relationships>");
    s
}

fn slide_rels(media: &[(String, &'static str, String)]) -> String {
    let mut s = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL_BASE}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#
    );
    for (rid, typ, target) in media {
        let _ = write!(s, r#"<Relationship Id="{rid}" Type="{typ}" Target="{target}"/>"#);
    }
    s.push_str("</Relationships>");
    s
}

const PRES_PROPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentationPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#;

const VIEW_PROPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:viewPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:gridSpacing cx="76200" cy="76200"/></p:viewPr>"#;

const TABLE_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>"#;

const SLIDE_MASTER: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>"#,
    r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#,
    r#"</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="4400"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/></a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr><a:defRPr sz="2800"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz="1800"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#
);

const SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;

const SLIDE_LAYOUT: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>"#,
    r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#,
    r#"</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
);

const SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="GlideDeck"><a:themeElements><a:clrScheme name="GlideDeck"><a:dk1><a:srgbClr val="1F232B"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="3D5A80"/></a:dk2><a:lt2><a:srgbClr val="F7F6F2"/></a:lt2><a:accent1><a:srgbClr val="4F46E5"/></a:accent1><a:accent2><a:srgbClr val="17A398"/></a:accent2><a:accent3><a:srgbClr val="E4572E"/></a:accent3><a:accent4><a:srgbClr val="F2A541"/></a:accent4><a:accent5><a:srgbClr val="7C3AED"/></a:accent5><a:accent6><a:srgbClr val="0EA5E9"/></a:accent6><a:hlink><a:srgbClr val="4F46E5"/></a:hlink><a:folHlink><a:srgbClr val="7C3AED"/></a:folHlink></a:clrScheme><a:fontScheme name="GlideDeck"><a:majorFont><a:latin typeface="Segoe UI"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Segoe UI"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="GlideDeck"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use glide_core::{Element, Frame, Step, TextBlock, Transform};

    #[test]
    fn exports_one_slide_per_step_with_morph_names() {
        let mut doc = Document::new("t");
        let mut f1 = Element::new(ElementKind::Frame(Frame::widescreen(1600.0)));
        f1.id = "f1".into();
        let mut f2 = Element::new(ElementKind::Frame(Frame::widescreen(160.0)))
            .with_transform(Transform { x: 300.0, y: 100.0, scale: 1.0, rotation: 30.0 });
        f2.id = "f2".into();
        let mut tb = TextBlock::new("Hello & <world>");
        tb.format_range(0..5, |st| st.bold = Some(true));
        tb.format_range(8..15, |st| {
            st.underline = true;
            st.strike = true;
            st.color = Some(Color::rgb(0x12, 0x34, 0x56));
        });
        let mut t = Element::new(ElementKind::Text(tb))
            .with_transform(Transform { x: 300.0, y: 100.0, scale: 0.5, rotation: 30.0 });
        t.id = "hello".into();
        doc.add(f1);
        doc.add(f2);
        doc.add(t);
        let mut icon = Element::new(ElementKind::Icon(glide_core::Icon {
            name: "rocket-launch".into(),
            style: glide_core::IconStyle::Fill,
            size: 40.0,
            color: Color::rgb(1, 2, 3),
        }));
        icon.id = "rocket".into();
        icon.shadow = Some(glide_core::Shadow::soft());
        doc.add(icon);
        let mut card = Element::new(ElementKind::Rect(glide_core::RectShape {
            width: 100.0,
            height: 60.0,
            fill: Some(Color::rgb(255, 0, 0)),
            gradient: Some(Gradient { to: Color::rgb(0, 0, 255), angle: 90.0 }),
            ..Default::default()
        }));
        card.id = "card".into();
        card.shadow = Some(glide_core::Shadow::soft());
        doc.add(card);
        doc.background_to = Some(Color::rgb(0, 0, 0));
        doc.path = vec![Step::new("f1"), Step::new("f2")];

        let out = std::env::temp_dir().join(format!("glide-pptx-test-{}.pptx", std::process::id()));
        let mut text = TextEngine::new();
        let report = export(&doc, None, &mut text, &out).unwrap();
        assert_eq!(report.slides, 2);
        assert_eq!(report.shapes, 4, "invisible frames are not exported");

        let mut zip = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut read = |name: &str| {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut zip.by_name(name).unwrap(), &mut s).unwrap();
            s
        };
        let s1 = read("ppt/slides/slide1.xml");
        let s2 = read("ppt/slides/slide2.xml");
        assert!(s1.contains(r#"name="!!hello""#) && s2.contains(r#"name="!!hello""#));
        // Formatted spans become separate runs.
        assert!(s1.contains(r#"b="1""#), "a bold run");
        assert!(s1.contains("<a:t>Hello</a:t>") && s1.contains("<a:t> &amp; </a:t>") && s1.contains("<a:t>&lt;world&gt;</a:t>"), "{s1}");
        assert!(s1.contains(r#"u="sng" strike="sngStrike""#));
        assert!(s1.contains(r#"val="123456""#));
        assert!(s1.contains("<a:custGeom>") && (s1.contains("<a:cubicBezTo>") || s1.contains("<a:quadBezTo>")), "icons export as vector geometry");
        assert!(s1.contains(r#"<a:lin ang="5400000""#), "gradient fill + gradient background");
        assert!(s1.contains("<a:outerShdw"), "shadows export");
        assert!(!s1.contains("p159:morph"), "first slide has no incoming transition");
        assert!(s2.contains("p159:morph"));
        // On slide 2 the text sits at the frame center and is upright.
        assert!(!s2.contains(r#"rot=""#));
        std::fs::remove_file(&out).ok();
    }
}
