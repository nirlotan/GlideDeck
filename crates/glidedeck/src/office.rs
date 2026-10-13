//! Pasting from Microsoft Office (PowerPoint, Word, Excel shapes).
//!
//! When shapes are copied in Office, the clipboard carries a picture of them
//! and, on Windows, an "Art::GVML ClipFormat" package: a small zip holding the
//! shapes as DrawingML (a `lockedCanvas` of `sp`, `grpSp` and `pic` elements),
//! the theme, and any embedded pictures. Parsing it lets a paste arrive as
//! editable text boxes and shapes instead of a flat image.
//!
//! The format has no public specification, so the parser is deliberately
//! tolerant: elements are matched by local name, anything unknown is skipped,
//! and the caller falls back to pasting the picture when nothing is found.

use glam::DVec2;
use glide_core::{Color, Element, ElementKind, EllipseShape, ListStyle, Media, RectShape, Stroke, TextAlign, TextBlock, Transform};
use roxmltree::Node;
use std::collections::HashMap;
use std::io::Read;

/// DrawingML units per pixel at 96 dpi.
const EMU_PER_PX: f64 = 9525.0;
/// Prefix of the placeholder path given to pictures until the caller stores them.
pub const MEDIA_PREFIX: &str = "office-media:";

/// Shapes parsed from an Office clipboard package, in pixels at 96 dpi.
pub struct OfficeClip {
    pub elements: Vec<Element>,
    /// Embedded pictures: bytes and file extension. Image elements refer to
    /// them by `office-media:<index>` until the caller imports them.
    pub media: Vec<(Vec<u8>, String)>,
}

/// Reads the Office shapes package from the system clipboard, if there is one.
#[cfg(windows)]
pub fn clipboard_package() -> Option<Vec<u8>> {
    if cfg!(test) {
        return None;
    }
    let id = clipboard_win::register_format("Art::GVML ClipFormat")?.get();
    let _open = clipboard_win::Clipboard::new_attempts(10).ok()?;
    if !clipboard_win::raw::is_format_avail(id) {
        return None;
    }
    let mut data = Vec::new();
    clipboard_win::raw::get_vec(id, &mut data).ok()?;
    Some(data)
}

/// Office for Mac does not expose its shapes package through the standard
/// pasteboard types we read, so other platforms paste the picture (or text
/// with Ctrl/⌘+Shift+V).
#[cfg(not(windows))]
pub fn clipboard_package() -> Option<Vec<u8>> {
    None
}

// --- theme -----------------------------------------------------------------------

pub(crate) struct OfficeTheme {
    colors: HashMap<String, Color>,
    major_font: Option<String>,
    minor_font: Option<String>,
}

impl OfficeTheme {
    pub(crate) fn default_office() -> Self {
        let colors = [
            ("dk1", 0x000000),
            ("lt1", 0xFFFFFF),
            ("dk2", 0x44546A),
            ("lt2", 0xE7E6E6),
            ("accent1", 0x4472C4),
            ("accent2", 0xED7D31),
            ("accent3", 0xA5A5A5),
            ("accent4", 0xFFC000),
            ("accent5", 0x5B9BD5),
            ("accent6", 0x70AD47),
            ("hlink", 0x0563C1),
            ("folHlink", 0x954F72),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), hex(v)))
        .collect();
        OfficeTheme { colors, major_font: None, minor_font: None }
    }

    pub(crate) fn parse(xml: &str) -> Self {
        let mut theme = Self::default_office();
        let Ok(doc) = roxmltree::Document::parse(xml) else { return theme };
        if let Some(scheme) = find(doc.root(), "clrScheme") {
            for slot in scheme.children().filter(Node::is_element) {
                let value = child(slot, "srgbClr")
                    .and_then(|c| c.attribute("val"))
                    .or_else(|| child(slot, "sysClr").and_then(|c| c.attribute("lastClr")))
                    .and_then(parse_hex);
                if let Some(c) = value {
                    theme.colors.insert(slot.tag_name().name().to_owned(), c);
                }
            }
        }
        let typeface = |which: &str| {
            find(doc.root(), which).and_then(|f| child(f, "latin")).and_then(|l| l.attribute("typeface")).filter(|t| !t.is_empty()).map(str::to_owned)
        };
        theme.major_font = typeface("majorFont");
        theme.minor_font = typeface("minorFont");
        theme
    }

    pub(crate) fn scheme(&self, name: &str, placeholder: Option<Color>) -> Option<Color> {
        let key = match name {
            "tx1" => "dk1",
            "bg1" => "lt1",
            "tx2" => "dk2",
            "bg2" => "lt2",
            "phClr" => return placeholder,
            other => other,
        };
        self.colors.get(key).copied()
    }
}

// --- xml helpers ---------------------------------------------------------------------

pub(crate) fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

pub(crate) fn find<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.descendants().find(|c| c.is_element() && c.tag_name().name() == name)
}

pub(crate) fn num(node: Option<Node>, attr: &str) -> Option<f64> {
    node?.attribute(attr)?.parse().ok()
}

fn hex(v: u32) -> Color {
    Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn parse_hex(s: &str) -> Option<Color> {
    u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6).map(hex)
}

// --- colors --------------------------------------------------------------------------

fn rgb_to_hsl(c: Color) -> (f64, f64, f64) {
    let (r, g, b) = (c.r() as f64 / 255.0, c.g() as f64 / 255.0, c.b() as f64 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64, a: u8) -> Color {
    let f = |p: f64, q: f64, mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        (f(p, q, h + 1.0 / 3.0), f(p, q, h), f(p, q, h - 1.0 / 3.0))
    };
    let u = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgba(u(r), u(g), u(b), a)
}

/// A color element (`srgbClr`, `schemeClr`, …) with its modifiers applied.
fn color_value(node: Node, theme: &OfficeTheme, placeholder: Option<Color>) -> Option<Color> {
    let base = match node.tag_name().name() {
        "srgbClr" => node.attribute("val").and_then(parse_hex),
        "schemeClr" => node.attribute("val").and_then(|v| theme.scheme(v, placeholder)),
        "sysClr" => node.attribute("lastClr").and_then(parse_hex),
        "prstClr" => match node.attribute("val")? {
            "black" => Some(Color::BLACK),
            "white" => Some(Color::WHITE),
            "red" => Some(hex(0xFF0000)),
            "green" => Some(hex(0x008000)),
            "blue" => Some(hex(0x0000FF)),
            "yellow" => Some(hex(0xFFFF00)),
            "gray" | "grey" => Some(hex(0x808080)),
            _ => None,
        },
        _ => None,
    }?;
    let mut c = base;
    let pct = |m: Node| m.attribute("val").and_then(|v| v.parse::<f64>().ok()).map(|v| v / 100_000.0);
    for m in node.children().filter(Node::is_element) {
        let Some(v) = pct(m) else { continue };
        match m.tag_name().name() {
            "lumMod" | "lumOff" => {
                let (h, s, l) = rgb_to_hsl(c);
                let l = if m.tag_name().name() == "lumMod" { l * v } else { l + v };
                c = hsl_to_rgb(h, s, l, c.a());
            }
            "shade" => {
                let k = |x: u8| (x as f64 * v).round() as u8;
                c = Color::rgba(k(c.r()), k(c.g()), k(c.b()), c.a());
            }
            "tint" => {
                let k = |x: u8| (x as f64 + (255.0 - x as f64) * (1.0 - v)).round() as u8;
                c = Color::rgba(k(c.r()), k(c.g()), k(c.b()), c.a());
            }
            "alpha" => c = Color::rgba(c.r(), c.g(), c.b(), (v.clamp(0.0, 1.0) * 255.0).round() as u8),
            _ => {}
        }
    }
    Some(c)
}

/// The first color child of `node` (e.g. inside `solidFill` or a style ref).
pub(crate) fn first_color(node: Node, theme: &OfficeTheme, placeholder: Option<Color>) -> Option<Color> {
    node.children().filter(Node::is_element).find_map(|c| color_value(c, theme, placeholder))
}

pub(crate) enum Paint {
    None,
    Color(Color),
    Unset,
}

/// A fill declared directly on `props` (spPr, ln or rPr).
pub(crate) fn paint(props: Option<Node>, theme: &OfficeTheme) -> Paint {
    let Some(props) = props else { return Paint::Unset };
    for c in props.children().filter(Node::is_element) {
        match c.tag_name().name() {
            "noFill" => return Paint::None,
            "solidFill" => return first_color(c, theme, None).map_or(Paint::Unset, Paint::Color),
            // Gradients come across as their first stop.
            "gradFill" => return find(c, "gs").and_then(|gs| first_color(gs, theme, None)).map_or(Paint::Unset, Paint::Color),
            _ => {}
        }
    }
    Paint::Unset
}

/// A style reference (`fillRef`, `lnRef`, `fontRef`) with a non-zero index.
fn style_color(shape: Node, reference: &str, theme: &OfficeTheme) -> Option<Color> {
    let r = child(shape, "style").and_then(|s| child(s, reference))?;
    if r.attribute("idx").is_some_and(|i| i == "0") {
        return None;
    }
    first_color(r, theme, None)
}

// --- geometry ------------------------------------------------------------------------

/// Maps group-child coordinates (EMU) into the clip's coordinates (EMU).
#[derive(Clone, Copy)]
pub(crate) struct GroupMap {
    scale: DVec2,
    offset: DVec2,
}

impl GroupMap {
    pub(crate) const IDENTITY: GroupMap = GroupMap { scale: DVec2::ONE, offset: DVec2::ZERO };

    fn apply(&self, p: DVec2) -> DVec2 {
        p * self.scale + self.offset
    }

    fn nested(&self, xfrm: Option<Node>) -> GroupMap {
        let pair = |name: &str, a: &str, b: &str| {
            let n = xfrm.and_then(|x| child(x, name));
            Some(DVec2::new(num(n, a)?, num(n, b)?))
        };
        let (Some(off), Some(ext), Some(ch_off), Some(ch_ext)) =
            (pair("off", "x", "y"), pair("ext", "cx", "cy"), pair("chOff", "x", "y"), pair("chExt", "cx", "cy"))
        else {
            return *self;
        };
        let k = DVec2::new(if ch_ext.x > 0.0 { ext.x / ch_ext.x } else { 1.0 }, if ch_ext.y > 0.0 { ext.y / ch_ext.y } else { 1.0 });
        // child → group: off + (p - chOff) * k, then through the parent.
        GroupMap { scale: self.scale * k, offset: self.apply(off - ch_off * k) }
    }
}

/// A shape's box in pixels: center, size and rotation (degrees).
struct Placed {
    center: DVec2,
    size: DVec2,
    rotation: f64,
}

fn placement(props: Option<Node>, map: &GroupMap) -> Option<Placed> {
    let xfrm = props.and_then(|p| child(p, "xfrm"))?;
    let off = Some(child(xfrm, "off")?);
    let ext = Some(child(xfrm, "ext")?);
    let top_left = map.apply(DVec2::new(num(off, "x")?, num(off, "y")?));
    let size = DVec2::new(num(ext, "cx")?, num(ext, "cy")?) * map.scale;
    let rotation = num(Some(xfrm), "rot").unwrap_or(0.0) / 60_000.0;
    Some(Placed { center: (top_left + size / 2.0) / EMU_PER_PX, size: size / EMU_PER_PX, rotation })
}

// --- parsing -------------------------------------------------------------------------

/// Defaults a slide placeholder takes from its slide layout and master.
#[derive(Clone, Debug, Default)]
pub(crate) struct Inherited {
    /// Offset and extent (EMU) when the placeholder has no position of its own.
    pub xfrm: Option<(DVec2, DVec2)>,
    /// Font size in hundredths of a point.
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub bullets: Option<bool>,
    pub align: Option<String>,
    pub anchor: Option<String>,
    pub color: Option<Color>,
}

/// Looks up what a placeholder shape inherits.
pub(crate) type InheritFn<'m> = &'m dyn Fn(Node) -> Option<Inherited>;

/// Walks a DrawingML shape tree into GlideDeck elements (pixels at 96 dpi).
pub(crate) struct Parser<'m> {
    pub theme: &'m OfficeTheme,
    /// The part's relationships: id → target part.
    pub rels: HashMap<String, String>,
    pub files: &'m HashMap<String, Vec<u8>>,
    pub measure: &'m mut dyn FnMut(&ElementKind) -> DVec2,
    pub out: &'m mut OfficeClip,
    /// Skip placeholders (on slide layouts and masters they are only prompts).
    pub skip_placeholders: bool,
    /// Looks up what a slide placeholder inherits.
    pub inherit: Option<InheritFn<'m>>,
    current: Inherited,
}

impl<'m> Parser<'m> {
    pub(crate) fn new(
        theme: &'m OfficeTheme,
        rels: HashMap<String, String>,
        files: &'m HashMap<String, Vec<u8>>,
        measure: &'m mut dyn FnMut(&ElementKind) -> DVec2,
        out: &'m mut OfficeClip,
    ) -> Self {
        Parser { theme, rels, files, measure, out, skip_placeholders: false, inherit: None, current: Inherited::default() }
    }
}

/// Reads every part of an Office zip package (skipping very large files).
pub(crate) fn unzip(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut files = HashMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() || f.size() > 256 << 20 {
            continue;
        }
        let name = f.name().map_err(|e| e.to_string())?.trim_start_matches('/').to_owned();
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| e.to_string())?;
        files.insert(name, data);
    }
    Ok(files)
}

/// A part's relationships: id → (resolved target part, relationship type).
pub(crate) fn relationships(files: &HashMap<String, Vec<u8>>, part: &str) -> HashMap<String, (String, String)> {
    let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
    let rels_name = if dir.is_empty() { format!("_rels/{file}.rels") } else { format!("{dir}/_rels/{file}.rels") };
    let mut rels = HashMap::new();
    let Some(data) = files.get(&rels_name) else { return rels };
    let xml = String::from_utf8_lossy(data);
    if let Ok(doc) = roxmltree::Document::parse(&xml) {
        for r in doc.descendants().filter(|n| n.tag_name().name() == "Relationship") {
            if let (Some(id), Some(target)) = (r.attribute("Id"), r.attribute("Target")) {
                let external = r.attribute("TargetMode") == Some("External");
                let target = if external { target.to_owned() } else { resolve(dir, target) };
                rels.insert(id.to_owned(), (target, r.attribute("Type").unwrap_or_default().to_owned()));
            }
        }
    }
    rels
}

/// Parses an Office shapes package. `measure` returns the local size of a
/// text element, used to anchor text vertically inside its shape.
pub fn parse_package(bytes: &[u8], measure: &mut dyn FnMut(&ElementKind) -> DVec2) -> Result<OfficeClip, String> {
    let files = unzip(bytes)?;
    let text = |data: &Vec<u8>| String::from_utf8_lossy(data).into_owned();
    let drawing_name = files
        .keys()
        .filter(|n| n.ends_with(".xml") && !n.contains("_rels") && !n.contains("theme") && !n.starts_with("[Content_Types]"))
        .find(|n| files.get(*n).is_some_and(|d| text(d).contains("lockedCanvas") || text(d).contains(":sp>")))
        .cloned()
        .ok_or("no drawing part in the package")?;
    let theme = files
        .iter()
        .find(|(n, _)| n.contains("theme") && n.ends_with(".xml"))
        .map(|(_, d)| OfficeTheme::parse(&text(d)))
        .unwrap_or_else(OfficeTheme::default_office);
    let rels = relationships(&files, &drawing_name).into_iter().map(|(id, (target, _))| (id, target)).collect();
    let xml = text(&files[&drawing_name]);
    let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
    let mut out = OfficeClip { elements: Vec::new(), media: Vec::new() };
    let mut parser = Parser::new(&theme, rels, &files, measure, &mut out);
    let root = find(doc.root(), "lockedCanvas").unwrap_or(doc.root_element());
    parser.walk(root, GroupMap::IDENTITY);
    Ok(out)
}

/// Resolves a relationship target relative to the part's folder.
fn resolve(dir: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_owned();
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn is_placeholder(node: Node) -> bool {
    find(node, "ph").is_some()
}

impl Parser<'_> {
    pub(crate) fn walk(&mut self, node: Node, map: GroupMap) {
        for c in node.children().filter(Node::is_element) {
            match c.tag_name().name() {
                "sp" => self.shape(c, &map),
                "pic" => self.picture(c, &map),
                "grpSp" => {
                    let inner = map.nested(child(c, "grpSpPr").and_then(|p| child(p, "xfrm")));
                    self.walk(c, inner);
                }
                // Content wrappers PowerPoint may put around the canvas.
                "graphic" | "graphicData" | "AlternateContent" | "Choice" | "spTree" | "cSld" => self.walk(c, map),
                _ => {}
            }
        }
    }

    /// What a placeholder inherits (nothing for ordinary shapes).
    fn inherited(&self, node: Node) -> Inherited {
        match self.inherit {
            Some(f) if is_placeholder(node) => f(node).unwrap_or_default(),
            _ => Inherited::default(),
        }
    }

    fn shape(&mut self, sp: Node, map: &GroupMap) {
        if self.skip_placeholders && is_placeholder(sp) {
            return;
        }
        self.current = self.inherited(sp);
        let props = child(sp, "spPr");
        let Some(place) = placement(props, map).or_else(|| inherited_place(&self.current, map)) else { return };
        let theme = self.theme;
        // Geometry: rectangles and ellipses become real shapes.
        let geometry = props.and_then(|p| child(p, "prstGeom")).and_then(|g| g.attribute("prst"));
        let fill = match paint(props, theme) {
            Paint::Color(c) => Some(c),
            Paint::None => None,
            Paint::Unset => style_color(sp, "fillRef", theme),
        };
        let line = props.and_then(|p| child(p, "ln"));
        let stroke = match paint(line, theme) {
            Paint::Color(c) => Some(c),
            Paint::None => None,
            Paint::Unset => style_color(sp, "lnRef", theme),
        }
        .map(|color| Stroke { color, width: num(line, "w").unwrap_or(12_700.0) * map.scale.x.abs() / EMU_PER_PX });
        let (w, h) = (place.size.x.abs(), place.size.y.abs());
        let kind = match geometry {
            Some("rect") | Some("snipRoundRect") | Some("flowChartProcess") => {
                Some(ElementKind::Rect(RectShape { width: w, height: h, fill, stroke: stroke.clone(), ..Default::default() }))
            }
            Some("roundRect") | Some("flowChartAlternateProcess") => Some(ElementKind::Rect(RectShape {
                width: w,
                height: h,
                corner_radius: w.min(h) * 0.1667,
                fill,
                stroke: stroke.clone(),
                ..Default::default()
            })),
            Some("ellipse") | Some("flowChartConnector") => {
                Some(ElementKind::Ellipse(EllipseShape { width: w, height: h, fill, stroke: stroke.clone(), ..Default::default() }))
            }
            _ => None,
        };
        let is_text_box = find(sp, "cNvSpPr").is_some_and(|n| n.attribute("txBox") == Some("1"));
        if let Some(kind) = kind.filter(|_| !is_text_box && (fill.is_some() || stroke.is_some())) {
            self.out.elements.push(element(kind, place.center, place.rotation, "Shape"));
        }
        if let Some(body) = find(sp, "txBody") {
            let font_color = style_color(sp, "fontRef", self.theme);
            self.text(body, &place, font_color, map);
        }
    }

    fn text(&mut self, body: Node, place: &Placed, font_color: Option<Color>, map: &GroupMap) {
        let theme = self.theme;
        let inherited = self.current.clone();
        let bp = child(body, "bodyPr");
        let inset = |attr: &str, default: f64| num(bp, attr).unwrap_or(default) * map.scale.x.abs() / EMU_PER_PX;
        let (l, t, r, b) = (inset("lIns", 91_440.0), inset("tIns", 45_720.0), inset("rIns", 91_440.0), inset("bIns", 45_720.0));
        let mut paragraphs = Vec::new();
        let mut first_run: Option<Node> = None;
        // Every run's byte range in the final content, with its properties.
        let mut runs: Vec<(usize, usize, Option<Node>)> = Vec::new();
        let mut offset = 0usize;
        let mut align = None;
        let mut list = ListStyle::None;
        for p in body.children().filter(|n| n.is_element() && n.tag_name().name() == "p") {
            let mut line = String::new();
            for part in p.children().filter(Node::is_element) {
                match part.tag_name().name() {
                    "r" | "fld" => {
                        if let Some(t) = child(part, "t").and_then(|t| t.text()) {
                            let start = offset + line.len();
                            line.push_str(t);
                            runs.push((start, offset + line.len(), child(part, "rPr")));
                        }
                        if first_run.is_none() {
                            first_run = child(part, "rPr");
                        }
                    }
                    "br" => line.push('\n'),
                    _ => {}
                }
            }
            let ppr = child(p, "pPr");
            if align.is_none() && !line.trim().is_empty() {
                align = ppr.and_then(|pp| pp.attribute("algn"));
            }
            if let Some(pp) = ppr {
                if child(pp, "buChar").is_some() && list == ListStyle::None {
                    list = ListStyle::Bullet;
                } else if child(pp, "buAutoNum").is_some() && list == ListStyle::None {
                    list = ListStyle::Numbered;
                }
            }
            // A body placeholder's paragraphs are bulleted unless they say otherwise.
            let no_bullet = ppr.is_some_and(|pp| child(pp, "buNone").is_some());
            if inherited.bullets == Some(true) && !no_bullet && list == ListStyle::None && !line.trim().is_empty() {
                list = ListStyle::Bullet;
            }
            if first_run.is_none() {
                first_run = child(p, "endParaRPr");
            }
            offset += line.len() + 1;
            paragraphs.push(line);
        }
        while paragraphs.last().is_some_and(|p| p.trim().is_empty()) {
            paragraphs.pop();
        }
        let content = paragraphs.join("\n");
        if content.trim().is_empty() {
            return;
        }
        let rpr = first_run;
        let default_size = inherited.size.unwrap_or(1800.0);
        let default_bold = inherited.bold.unwrap_or(false);
        let size_pt = num(rpr, "sz").unwrap_or(default_size) / 100.0;
        let default_color = font_color.or(inherited.color).or_else(|| theme.scheme("tx1", None)).unwrap_or(Color::BLACK);
        let align = align.or(inherited.align.as_deref());
        let color = match paint(rpr, theme) {
            Paint::Color(c) => c,
            _ => default_color,
        };
        let family = rpr
            .and_then(|r| child(r, "latin"))
            .and_then(|l| l.attribute("typeface"))
            .and_then(|f| match f {
                "+mj-lt" => theme.major_font.clone(),
                "+mn-lt" => theme.minor_font.clone(),
                f if f.starts_with('+') => None,
                f => Some(f.to_owned()),
            })
            .or_else(|| theme.minor_font.clone())
            .unwrap_or_else(|| "sans-serif".into());
        let width = (place.size.x.abs() - l - r).max(1.0);
        let wrap = bp.and_then(|b| b.attribute("wrap")) != Some("none");
        let mut block = TextBlock::new(content);
        block.font_size = size_pt * map.scale.y.abs() * 96.0 / 72.0;
        block.font_family = family;
        block.weight = if rpr.and_then(|r| r.attribute("b")).map_or(default_bold, |b| b == "1") { 700 } else { 400 };
        block.italic = rpr.and_then(|r| r.attribute("i")) == Some("1");
        block.color = color;
        block.align = match align {
            Some("ctr") => TextAlign::Center,
            Some("r") => TextAlign::Right,
            _ => TextAlign::Left,
        };
        block.max_width = wrap.then_some(width);
        block.list = list;
        // Runs that differ from the first one become formatted spans.
        let px_per_pt = map.scale.y.abs() * 96.0 / 72.0;
        for (start, end, props) in runs {
            if start >= end || end > block.content.len() {
                continue;
            }
            let flag = |name: &str| props.and_then(|r| r.attribute(name));
            // Office defaults for a run: not bold, not italic, 18 pt, the shape's text color.
            let style = glide_core::SpanStyle {
                bold: Some(flag("b").map_or(default_bold, |v| v == "1")),
                italic: Some(flag("i") == Some("1")),
                underline: flag("u").is_some_and(|v| v != "none"),
                strike: flag("strike").is_some_and(|v| v != "noStrike"),
                color: Some(match paint(props, theme) {
                    Paint::Color(c) => c,
                    _ => default_color,
                }),
                size: Some(num(props, "sz").unwrap_or(default_size) / 100.0 * px_per_pt),
            };
            block.format_range(start..end, |s| *s = style.clone());
        }
        let kind = ElementKind::Text(block);
        // Anchor vertically inside the shape, then rotate with it.
        let text_h = (self.measure)(&kind).y;
        let half = place.size.y.abs() / 2.0;
        let dy = match bp.and_then(|b| b.attribute("anchor")).or(inherited.anchor.as_deref()) {
            Some("ctr") => (t - b) / 2.0,
            Some("b") => half - b - text_h / 2.0,
            _ => -half + t + text_h / 2.0,
        };
        let dx = (l - r) / 2.0;
        let offset = DVec2::from_angle(place.rotation.to_radians()).rotate(DVec2::new(dx, dy));
        self.out.elements.push(element(kind, place.center + offset, place.rotation, "Text"));
    }

    fn picture(&mut self, pic: Node, map: &GroupMap) {
        if self.skip_placeholders && is_placeholder(pic) {
            return;
        }
        let inherited = self.inherited(pic);
        let Some(place) = placement(child(pic, "spPr"), map).or_else(|| inherited_place(&inherited, map)) else { return };
        let Some(id) = find(pic, "blip").and_then(|b| b.attributes().find(|a| a.name() == "embed").map(|a| a.value().to_owned())) else {
            return;
        };
        let Some(target) = self.rels.get(&id) else { return };
        let ext = target.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if !["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp"].contains(&ext.as_str()) {
            return;
        }
        let Some(bytes) = self.files.get(target) else { return };
        let index = self.out.media.len();
        self.out.media.push((bytes.clone(), ext));
        let media = Media::new(format!("{MEDIA_PREFIX}{index}"), place.size.x.abs(), place.size.y.abs());
        self.out.elements.push(element(ElementKind::Image(media), place.center, place.rotation, "Picture"));
    }
}

/// A placeholder's inherited position as a placed box.
fn inherited_place(inherited: &Inherited, map: &GroupMap) -> Option<Placed> {
    let (off, ext) = inherited.xfrm?;
    let top_left = map.apply(off);
    let size = ext * map.scale;
    Some(Placed { center: (top_left + size / 2.0) / EMU_PER_PX, size: size / EMU_PER_PX, rotation: 0.0 })
}

fn element(kind: ElementKind, center: DVec2, rotation: f64, name: &str) -> Element {
    let mut el = Element::new(kind).with_transform(Transform { x: center.x, y: center.y, scale: 1.0, rotation });
    el.name = name.into();
    el
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A package shaped like the one PowerPoint puts on the clipboard.
    fn package(drawing: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        let theme = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements>
            <a:clrScheme name="t"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>
            <a:accent1><a:srgbClr val="156082"/></a:accent1></a:clrScheme>
            <a:fontScheme name="f"><a:majorFont><a:latin typeface="Aptos Display"/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/></a:minorFont></a:fontScheme>
            </a:themeElements></a:theme>"#;
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
            <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/></Relationships>"#;
        let mut files: Vec<(&str, &[u8])> = vec![
            ("[Content_Types].xml", b"<Types/>"),
            ("clipboard/drawings/drawing1.xml", drawing.as_bytes()),
            ("clipboard/drawings/_rels/drawing1.xml.rels", rels.as_bytes()),
            ("clipboard/theme/theme1.xml", theme.as_bytes()),
        ];
        files.extend_from_slice(extra);
        for (name, data) in files {
            zip.start_file(name, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:lc="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

    fn measure(kind: &ElementKind) -> DVec2 {
        match kind {
            ElementKind::Text(t) => DVec2::new(t.max_width.unwrap_or(100.0), t.font_size * 1.2 * (t.content.lines().count() as f64)),
            _ => DVec2::ZERO,
        }
    }

    #[test]
    fn text_box_and_filled_shape_become_editable_elements() {
        let drawing = format!(
            r#"<a:graphic {NS}><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"><lc:lockedCanvas>
            <a:nvGrpSpPr><a:cNvPr id="0" name=""/><a:cNvGrpSpPr/></a:nvGrpSpPr>
            <a:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></a:grpSpPr>
            <a:sp><a:nvSpPr><a:cNvPr id="2" name="TextBox 1"/><a:cNvSpPr txBox="1"/></a:nvSpPr>
              <a:spPr><a:xfrm><a:off x="952500" y="952500"/><a:ext cx="2857500" cy="476250"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></a:spPr>
              <a:txSp><a:txBody><a:bodyPr wrap="square" rtlCol="0"><a:spAutoFit/></a:bodyPr><a:lstStyle/>
                <a:p><a:r><a:rPr lang="en-US" sz="2400" b="1"><a:solidFill><a:srgbClr val="C00000"/></a:solidFill><a:latin typeface="+mj-lt"/></a:rPr><a:t>Hello </a:t></a:r><a:r><a:rPr lang="en-US" sz="2400"/><a:t>world</a:t></a:r></a:p>
                <a:p><a:pPr marL="285750" indent="-285750"><a:buFont typeface="Arial"/><a:buChar char="•"/></a:pPr><a:r><a:rPr lang="en-US" sz="2400"/><a:t>Second line</a:t></a:r></a:p>
              </a:txBody></a:txSp></a:sp>
            <a:sp><a:nvSpPr><a:cNvPr id="3" name="Oval 2"/><a:cNvSpPr/></a:nvSpPr>
              <a:spPr><a:xfrm rot="5400000"><a:off x="0" y="1905000"/><a:ext cx="1905000" cy="952500"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom></a:spPr>
              <a:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef>
                <a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></a:style>
              <a:txSp><a:txBody><a:bodyPr rtlCol="0" anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="en-US"/><a:t>Inside</a:t></a:r></a:p></a:txBody></a:txSp></a:sp>
            <a:pic><a:nvPicPr><a:cNvPr id="4" name="Picture 3"/><a:cNvPicPr/></a:nvPicPr>
              <a:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></a:blipFill>
              <a:spPr><a:xfrm><a:off x="3810000" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"/></a:spPr></a:pic>
            </lc:lockedCanvas></a:graphicData></a:graphic>"#
        );
        let png: &[u8] = &[0x89, b'P', b'N', b'G'];
        let bytes = package(&drawing, &[("clipboard/media/image1.png", png)]);
        let clip = parse_package(&bytes, &mut measure).expect("parses");
        let kinds: Vec<&str> = clip.elements.iter().map(|e| e.kind.type_name()).collect();
        assert_eq!(kinds, ["text", "ellipse", "text", "image"], "{kinds:?}");

        // The text box: two paragraphs, first-run styling, theme heading font, bullets.
        let tb = &clip.elements[0];
        let ElementKind::Text(t) = &tb.kind else { unreachable!() };
        assert_eq!(t.content, "Hello world\nSecond line");
        assert!((t.font_size - 32.0).abs() < 1e-9, "24 pt is 32 px");
        assert_eq!((t.weight, t.color, t.font_family.as_str()), (700, hex(0xC00000), "Aptos Display"));
        assert_eq!(t.list, ListStyle::Bullet);
        assert!((t.max_width.unwrap() - (300.0 - 19.2)).abs() < 1e-9, "wraps inside the insets");
        // "Hello " is a bold red run; "world" is a plain run in the theme's text
        // color, so it becomes a regular-weight, black span.
        let world = t.spans.iter().find(|s| &t.content[s.start..s.end] == "world").expect("a span for the plain run");
        assert_eq!((world.style.bold, world.style.color), (Some(false), Some(Color::BLACK)), "{:?}", t.spans);
        assert!((tb.transform.x - 250.0).abs() < 1e-9, "box spans 100..400 px");

        // The oval: theme fill and outline from the shape style, rotated 90°.
        let ov = &clip.elements[1];
        let ElementKind::Ellipse(e) = &ov.kind else { unreachable!() };
        assert_eq!(e.fill, Some(hex(0x156082)));
        assert_eq!(e.stroke.as_ref().map(|s| s.color), Some(Color::rgb(0x0B, 0x30, 0x41)));
        assert_eq!((ov.transform.x, ov.transform.y, ov.transform.rotation), (100.0, 250.0, 90.0));
        // Its text is centered, white (fontRef lt1) and in the theme body font.
        let ElementKind::Text(inside) = &clip.elements[2].kind else { unreachable!() };
        assert_eq!((inside.content.as_str(), inside.color, inside.align, inside.font_family.as_str()), ("Inside", Color::WHITE, TextAlign::Center, "Aptos"));
        assert_eq!((clip.elements[2].transform.x, clip.elements[2].transform.y), (100.0, 250.0));

        // The picture refers to its embedded bytes.
        let ElementKind::Image(m) = &clip.elements[3].kind else { unreachable!() };
        assert_eq!(m.path, "office-media:0");
        assert_eq!((m.width, m.height), (100.0, 100.0));
        assert_eq!(clip.media, vec![(png.to_vec(), "png".to_owned())]);
    }

    #[test]
    fn groups_map_child_coordinates() {
        let drawing = format!(
            r#"<lc:lockedCanvas {NS}>
            <a:grpSp><a:nvGrpSpPr><a:cNvPr id="5" name="Group"/><a:cNvGrpSpPr/></a:nvGrpSpPr>
              <a:grpSpPr><a:xfrm><a:off x="952500" y="0"/><a:ext cx="1905000" cy="1905000"/><a:chOff x="0" y="0"/><a:chExt cx="952500" cy="952500"/></a:xfrm></a:grpSpPr>
              <a:sp><a:nvSpPr><a:cNvPr id="6" name="R"/><a:cNvSpPr/></a:nvSpPr>
                <a:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="476250"/></a:xfrm><a:prstGeom prst="rect"/><a:solidFill><a:schemeClr val="accent1"><a:lumMod val="75000"/></a:schemeClr></a:solidFill></a:spPr></a:sp>
            </a:grpSp></lc:lockedCanvas>"#
        );
        let clip = parse_package(&package(&drawing, &[]), &mut measure).unwrap();
        assert_eq!(clip.elements.len(), 1);
        let el = &clip.elements[0];
        let ElementKind::Rect(r) = &el.kind else { unreachable!() };
        // The group doubles its children and moves them 100 px right.
        assert_eq!((r.width, r.height), (200.0, 100.0));
        assert_eq!((el.transform.x, el.transform.y), (200.0, 50.0));
        let f = r.fill.unwrap();
        assert!(f.r() < 0x15 && f.b() < 0x82, "lumMod darkens: {f:?}");
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_package(b"not a zip", &mut measure).is_err());
        assert!(parse_package(&package("<a:p/>", &[]), &mut measure).is_err());
    }
}
