//! Importing PowerPoint (.pptx) presentations.
//!
//! Every slide becomes a frame on the canvas, laid out in a grid and placed on
//! the path in slide order, with its speaker notes. A slide's own shapes are
//! read with the same DrawingML parser as pasted Office shapes. On top of that:
//! - placeholders take their position, size and text style from the slide
//!   layout and master;
//! - the slide background (color or picture) and the master's and layout's
//!   decorations (logos, lines) are added behind the content.
//!
//! Tables, charts and SmartArt are reported as skipped.

use crate::office::{child, find, first_color, num, paint, relationships, unzip, GroupMap, Inherited, OfficeClip, OfficeTheme, Paint, Parser, MEDIA_PREFIX};
use glam::DVec2;
use glide_core::{Color, Document, Element, ElementKind, Frame, Media, Step, Transform};
use roxmltree::Node;
use std::collections::HashMap;

const EMU_PER_PX: f64 = 9525.0;
/// The canvas behind imported slides.
const CANVAS: Color = Color::rgb(0xE9, 0xEB, 0xEF);
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub struct Imported {
    pub doc: Document,
    /// Embedded pictures (bytes, extension), referenced as `office-media:<i>`.
    pub media: Vec<(Vec<u8>, String)>,
    /// Content that could not be converted, e.g. "2 tables".
    pub skipped: Vec<String>,
}

enum Background {
    Color(Color),
    Picture(String),
}

/// Imports a .pptx file's bytes. `measure` returns a text element's local size.
pub fn import(bytes: &[u8], fallback_title: &str, measure: &mut dyn FnMut(&ElementKind) -> DVec2) -> Result<Imported, String> {
    let files = unzip(bytes)?;
    let text = |name: &str| files.get(name).map(|d| String::from_utf8_lossy(d).into_owned());
    let root_rels = relationships(&files, "");
    let pres_part = root_rels
        .values()
        .find(|(_, ty)| ty.ends_with("/officeDocument"))
        .map(|(t, _)| t.clone())
        .unwrap_or_else(|| "ppt/presentation.xml".into());
    let pres_xml = text(&pres_part).ok_or("this is not a PowerPoint presentation (no ppt/presentation.xml)")?;
    let pres = roxmltree::Document::parse(&pres_xml).map_err(|e| format!("presentation.xml: {e}"))?;
    let size_node = find(pres.root(), "sldSz");
    let slide_emu = DVec2::new(num(size_node, "cx").unwrap_or(12_192_000.0), num(size_node, "cy").unwrap_or(6_858_000.0));
    let slide_px = slide_emu / EMU_PER_PX;
    let pres_rels = relationships(&files, &pres_part);
    let slide_parts: Vec<String> = find(pres.root(), "sldIdLst")
        .map(|list| {
            list.children()
                .filter(|n| n.is_element())
                .filter_map(|n| n.attributes().find(|a| a.name() == "id" && a.namespace() == Some(REL_NS)).map(|a| a.value().to_owned()))
                .filter_map(|rid| pres_rels.get(&rid).map(|(t, _)| t.clone()))
                .collect()
        })
        .unwrap_or_default();
    if slide_parts.is_empty() {
        return Err("the presentation has no slides".into());
    }

    let title = text("docProps/core.xml")
        .and_then(|x| roxmltree::Document::parse(&x).ok().and_then(|d| find(d.root(), "title").and_then(|t| t.text()).map(str::to_owned)))
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| fallback_title.to_owned());
    let mut doc = Document::new(title);
    let mut out = OfficeClip { elements: Vec::new(), media: Vec::new() };
    let mut themes: HashMap<String, OfficeTheme> = HashMap::new();
    let mut skipped: HashMap<&'static str, usize> = HashMap::new();
    let mut slides = Vec::new();

    for part in &slide_parts {
        let Some(slide_xml) = text(part) else { continue };
        let slide_rels = relationships(&files, part);
        let rel_of = |rels: &HashMap<String, (String, String)>, kind: &str| rels.values().find(|(_, ty)| ty.ends_with(kind)).map(|(t, _)| t.clone());
        let layout_part = rel_of(&slide_rels, "/slideLayout").unwrap_or_default();
        let layout_rels = relationships(&files, &layout_part);
        let master_part = rel_of(&layout_rels, "/slideMaster").unwrap_or_default();
        let master_rels = relationships(&files, &master_part);
        let theme_part = rel_of(&master_rels, "/theme").unwrap_or_default();
        let theme = themes
            .entry(theme_part.clone())
            .or_insert_with(|| text(&theme_part).map(|x| OfficeTheme::parse(&x)).unwrap_or_else(OfficeTheme::default_office));
        let layout_xml = text(&layout_part).unwrap_or_default();
        let master_xml = text(&master_part).unwrap_or_default();
        let slide = roxmltree::Document::parse(&slide_xml).map_err(|e| format!("{part}: {e}"))?;
        let layout = roxmltree::Document::parse(&layout_xml).ok();
        let master = roxmltree::Document::parse(&master_xml).ok();
        let layout_root = layout.as_ref().map(|d| d.root());
        let master_root = master.as_ref().map(|d| d.root());
        let plain = |rels: &HashMap<String, (String, String)>| rels.iter().map(|(k, (t, _))| (k.clone(), t.clone())).collect::<HashMap<_, _>>();

        // Background: the slide's own, else its layout's, else the master's.
        let background = [(Some(slide.root()), &slide_rels), (layout_root, &layout_rels), (master_root, &master_rels)]
            .into_iter()
            .find_map(|(root, rels)| background(root?, theme, rels));

        let start = out.elements.len();
        let shows_master = |root: Option<Node>| root.and_then(|r| find(r, "sld").or_else(|| find(r, "sldLayout"))).and_then(|n| n.attribute("showMasterSp")) != Some("0");
        if let Some(Background::Picture(target)) = &background
            && let Some(bytes) = files.get(target)
        {
            let ext = target.rsplit('.').next().unwrap_or("png").to_ascii_lowercase();
            let index = out.media.len();
            out.media.push((bytes.clone(), ext));
            let mut el = Element::new(ElementKind::Image(Media::new(format!("{MEDIA_PREFIX}{index}"), slide_px.x, slide_px.y)))
                .with_transform(Transform { x: slide_px.x / 2.0, y: slide_px.y / 2.0, scale: 1.0, rotation: 0.0 });
            el.name = "Background".into();
            out.elements.push(el);
        }
        // Master and layout decorations (placeholders there are only prompts).
        let mut decorations = Vec::new();
        if shows_master(Some(slide.root())) {
            if shows_master(layout_root) {
                decorations.push((master_root, &master_rels));
            }
            decorations.push((layout_root, &layout_rels));
        }
        for (root, rels) in decorations {
            let Some(tree) = root.and_then(|r| find(r, "spTree")) else { continue };
            let mut parser = Parser::new(theme, plain(rels), &files, &mut *measure, &mut out);
            parser.skip_placeholders = true;
            parser.walk(tree, GroupMap::IDENTITY);
        }
        // The slide itself, with placeholders inheriting from the layout and master.
        let inherit = |sp: Node| inherited(sp, layout_root, master_root, theme);
        if let Some(tree) = find(slide.root(), "spTree") {
            let mut parser = Parser::new(theme, plain(&slide_rels), &files, &mut *measure, &mut out);
            parser.inherit = Some(&inherit);
            parser.walk(tree, GroupMap::IDENTITY);
            for frame in tree.descendants().filter(|n| n.tag_name().name() == "graphicFrame") {
                let kind = if find(frame, "tbl").is_some() {
                    "table"
                } else if frame.descendants().any(|n| n.tag_name().name() == "chart") {
                    "chart"
                } else if frame.descendants().any(|n| n.tag_name().name() == "relIds") {
                    "SmartArt graphic"
                } else {
                    "embedded object"
                };
                *skipped.entry(kind).or_default() += 1;
            }
        }
        let name = slide_title(slide.root()).unwrap_or_else(|| format!("Slide {}", slides.len() + 1));
        let notes = rel_of(&slide_rels, "/notesSlide").and_then(|p| text(&p)).map(|x| notes_text(&x)).unwrap_or_default();
        let fill = match background {
            Some(Background::Color(c)) => Some(c),
            _ => None,
        };
        slides.push((start..out.elements.len(), name, notes, fill));
    }

    // Slides sit on a neutral desk, each filled with its own background, like
    // PowerPoint's editing view.
    doc.background = CANVAS;

    // Lay the slides out in a grid, in order.
    let cols = (slides.len() as f64).sqrt().ceil().max(1.0) as usize;
    let gap = slide_px.x * 0.12;
    let mut elements = std::mem::take(&mut out.elements).into_iter().map(Some).collect::<Vec<_>>();
    for (i, (range, name, notes, fill)) in slides.into_iter().enumerate() {
        let top_left = DVec2::new((i % cols) as f64 * (slide_px.x + gap), (i / cols) as f64 * (slide_px.y + gap));
        let center = top_left + slide_px / 2.0;
        let mut frame = Element::new(ElementKind::Frame(Frame {
            width: slide_px.x,
            height: slide_px.y,
            corner_radius: 0.0,
            fill: Some(fill.unwrap_or(Color::WHITE)),
            border: None,
        }))
        .with_transform(Transform { x: center.x, y: center.y, scale: 1.0, rotation: 0.0 });
        frame.name = name;
        let id = doc.add(frame);
        for el in elements[range].iter_mut().filter_map(Option::take) {
            let mut el = el;
            el.transform.x += top_left.x;
            el.transform.y += top_left.y;
            doc.add(el);
        }
        let mut step = Step::new(id);
        step.notes = notes;
        doc.path.push(step);
    }

    let mut skipped: Vec<String> = skipped.into_iter().map(|(k, n)| if n == 1 { format!("1 {k}") } else { format!("{n} {k}s") }).collect();
    skipped.sort();
    Ok(Imported { doc, media: out.media, skipped })
}

/// The slide's title placeholder text, for naming its frame.
fn slide_title(root: Node) -> Option<String> {
    let sp = root.descendants().find(|n| {
        n.tag_name().name() == "sp" && find(*n, "ph").and_then(|ph| ph.attribute("type")).is_some_and(|t| t == "title" || t == "ctrTitle")
    })?;
    let text: String = sp.descendants().filter(|n| n.tag_name().name() == "t").filter_map(|n| n.text()).collect::<Vec<_>>().join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then(|| if text.chars().count() > 60 { text.chars().take(57).collect::<String>() + "…" } else { text })
}

/// Speaker notes: the notes page's body placeholder, one paragraph per line.
fn notes_text(xml: &str) -> String {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return String::new() };
    let Some(body) = doc.descendants().find(|n| n.tag_name().name() == "sp" && find(*n, "ph").and_then(|ph| ph.attribute("type")) == Some("body")) else {
        return String::new();
    };
    let paragraphs: Vec<String> = body
        .descendants()
        .filter(|n| n.tag_name().name() == "p")
        .map(|p| p.descendants().filter(|n| n.tag_name().name() == "t").filter_map(|n| n.text()).collect::<String>())
        .collect();
    paragraphs.join("\n").trim().to_owned()
}

fn background(root: Node, theme: &OfficeTheme, rels: &HashMap<String, (String, String)>) -> Option<Background> {
    let bg = find(root, "bg")?;
    if let Some(pr) = child(bg, "bgPr") {
        if let Some(blip) = find(pr, "blip") {
            let id = blip.attributes().find(|a| a.name() == "embed").map(|a| a.value())?;
            return rels.get(id).map(|(t, _)| Background::Picture(t.clone()));
        }
        return match paint(Some(pr), theme) {
            Paint::Color(c) => Some(Background::Color(c)),
            _ => None,
        };
    }
    child(bg, "bgRef").and_then(|r| first_color(r, theme, None)).map(Background::Color)
}

/// Placeholder types as the master knows them.
fn master_type(ty: &str) -> &str {
    match ty {
        "title" | "ctrTitle" => "title",
        "body" | "subTitle" | "obj" => "body",
        other => other,
    }
}

/// The matching placeholder on a layout or master: same index, else same type.
fn find_placeholder<'a, 'i>(root: Node<'a, 'i>, ty: &str, idx: Option<&str>) -> Option<Node<'a, 'i>> {
    let shapes: Vec<(Node, &str, Option<&str>)> = root
        .descendants()
        .filter(|n| matches!(n.tag_name().name(), "sp" | "pic"))
        .filter_map(|n| {
            let ph = find(n, "ph")?;
            Some((n, ph.attribute("type").unwrap_or("body"), ph.attribute("idx")))
        })
        .collect();
    if let Some(idx) = idx
        && let Some((n, ..)) = shapes.iter().find(|(_, _, i)| *i == Some(idx))
    {
        return Some(*n);
    }
    shapes
        .iter()
        .find(|(_, t, _)| *t == ty)
        .or_else(|| shapes.iter().find(|(_, t, _)| master_type(t) == master_type(ty)))
        .map(|(n, ..)| *n)
}

/// What a slide placeholder inherits from its layout, then its master.
fn inherited(sp: Node, layout: Option<Node>, master: Option<Node>, theme: &OfficeTheme) -> Option<Inherited> {
    let ph = find(sp, "ph")?;
    let ty = ph.attribute("type").unwrap_or("body");
    let idx = ph.attribute("idx");
    let layout_ph = layout.and_then(|r| find_placeholder(r, ty, idx));
    let master_ph = master.and_then(|r| find_placeholder(r, master_type(ty), None));
    let is_title = master_type(ty) == "title";
    let master_style = master.and_then(|m| find(m, "txStyles")).and_then(|s| child(s, if is_title { "titleStyle" } else { "bodyStyle" })).and_then(|s| child(s, "lvl1pPr"));
    // First-level paragraph properties, nearest first.
    fn lvl1<'a, 'i>(n: Option<Node<'a, 'i>>) -> Option<Node<'a, 'i>> {
        n.and_then(|n| find(n, "lstStyle")).and_then(|l| child(l, "lvl1pPr"))
    }
    fn def_rpr<'a, 'i>(p: Node<'a, 'i>) -> Option<Node<'a, 'i>> {
        child(p, "defRPr")
    }
    let chain: Vec<Node> = [lvl1(layout_ph), lvl1(master_ph), master_style].into_iter().flatten().collect();
    let xfrm = [layout_ph, master_ph].into_iter().flatten().find_map(|n| {
        let x = child(n, "spPr").and_then(|p| child(p, "xfrm"))?;
        let (off, ext) = (child(x, "off"), child(x, "ext"));
        Some((DVec2::new(num(off, "x")?, num(off, "y")?), DVec2::new(num(ext, "cx")?, num(ext, "cy")?)))
    });
    let bullets = chain.iter().find_map(|p| {
        if child(*p, "buNone").is_some() {
            Some(false)
        } else if child(*p, "buChar").is_some() || child(*p, "buAutoNum").is_some() {
            Some(true)
        } else {
            None
        }
    });
    Some(Inherited {
        xfrm,
        size: chain.iter().find_map(|p| num(def_rpr(*p), "sz")),
        bold: chain.iter().find_map(|p| def_rpr(*p).and_then(|r| r.attribute("b")).map(|b| b == "1")),
        // Titles and subtitles aren't bulleted even when the master's body style is.
        bullets: if is_title || ty == "subTitle" || ty == "ctrTitle" { Some(false) } else { bullets },
        align: chain.iter().find_map(|p| p.attribute("algn")).map(str::to_owned),
        anchor: [layout_ph, master_ph].into_iter().flatten().find_map(|n| find(n, "bodyPr").and_then(|b| b.attribute("anchor"))).map(str::to_owned),
        color: chain.iter().find_map(|p| def_rpr(*p).and_then(|r| child(r, "solidFill")).and_then(|f| first_color(f, theme, None))),
    })
}

/// Stores imported pictures in `base`'s assets folder and points the image
/// elements at them; pictures that can't be stored are dropped.
pub fn store_media(doc: &mut Document, media: &[(Vec<u8>, String)], base: &std::path::Path) {
    let stored: Vec<Option<String>> = media.iter().map(|(bytes, ext)| glide_core::assets::import_bytes(base, bytes, ext).ok()).collect();
    doc.elements.retain_mut(|el| {
        let ElementKind::Image(m) = &mut el.kind else { return true };
        let Some(i) = m.path.strip_prefix(MEDIA_PREFIX) else { return true };
        match i.parse::<usize>().ok().and_then(|i| stored.get(i).cloned().flatten()) {
            Some(reference) => {
                m.path = reference;
                true
            }
            None => false,
        }
    });
}

/// Imports a .pptx file as a GlideDeck project saved at `out`, with its
/// pictures in the `assets/` folder next to it. Returns the document and what
/// was skipped.
pub fn import_to_project(pptx: &std::path::Path, out: &std::path::Path, text: &mut glide_render::TextEngine) -> Result<(Document, Vec<String>), String> {
    let bytes = std::fs::read(pptx).map_err(|e| format!("reading {}: {e}", pptx.display()))?;
    let stem = pptx.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Presentation".into());
    let mut imported = import(&bytes, &stem, &mut |k| glide_render::local_size(text, k))?;
    let base = out.parent().filter(|d| !d.as_os_str().is_empty()).map(std::path::Path::to_path_buf).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    store_media(&mut imported.doc, &imported.media, &base);
    imported.doc.save(out).map_err(|e| format!("saving {}: {e}", out.display()))?;
    Ok((imported.doc, imported.skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glide_core::ListStyle;
    use glide_render::TextEngine;

    fn texts(doc: &Document) -> Vec<(String, glide_core::TextBlock)> {
        doc.elements.iter().filter_map(|e| match &e.kind {
            ElementKind::Text(t) => Some((e.id.clone(), t.clone())),
            _ => None,
        }).collect()
    }

    /// A deck made with python-pptx's default template: placeholders without
    /// positions of their own, bullets from the master, notes, a picture.
    #[test]
    fn imports_a_real_powerpoint_deck() {
        let mut engine = TextEngine::new();
        let bytes = include_bytes!("testdata/sample.pptx");
        let imported = import(bytes, "fallback", &mut |k| glide_render::local_size(&mut engine, k)).unwrap();
        let doc = &imported.doc;
        assert_eq!(doc.title, "Quarterly Business Review", "title from the document properties");
        assert_eq!(doc.path.len(), 5);
        let names: Vec<String> = (0..5).map(|i| doc.step_frame(i).unwrap().name.clone()).collect();
        assert_eq!(names, ["Quarterly Business Review", "Highlights", "Slide 3", "Numbers", "Before and after"]);
        assert_eq!(doc.path[0].notes, "Welcome everyone.\nIntroduce the agenda.");
        assert_eq!(doc.path[1].notes, "Pause after the revenue number.");
        assert_eq!(imported.skipped, ["1 chart", "1 table"]);

        let all = texts(doc);
        // The title placeholder takes the master's 44 pt title size (at 96 dpi).
        let title = &all.iter().find(|(_, t)| t.content == "Quarterly Business Review").unwrap().1;
        assert!((title.font_size - 44.0 * 96.0 / 72.0).abs() < 0.01, "title size {}", title.font_size);
        // The body placeholder is bulleted from the master's body style.
        let body = &all.iter().find(|(_, t)| t.content.starts_with("Revenue up 18%")).unwrap().1;
        assert_eq!(body.content, "Revenue up 18%\nEnterprise grew fastest\nTwo new markets");
        assert_eq!(body.list, ListStyle::Bullet);
        // The subtitle is not.
        let subtitle = &all.iter().find(|(_, t)| t.content.starts_with("Q3 2026")).unwrap().1;
        assert_eq!(subtitle.list, ListStyle::None);
        // Mixed runs keep their formatting.
        let mixed = &all.iter().find(|(_, t)| t.content == "Bold red and plain white").unwrap().1;
        let red = Color::rgb(0xF4, 0x3F, 0x5E);
        let (first, second) = (mixed.style_at(0), mixed.style_at("Bold red ".len()));
        assert!(mixed.is_bold(&first) && first.color.unwrap_or(mixed.color) == red, "{:?}", mixed.spans);
        assert!(!mixed.is_bold(&second) && second.color.unwrap_or(mixed.color) == Color::WHITE, "{:?}", mixed.spans);

        // Slide 3 has a dark background, a picture and a filled rounded shape.
        let f3 = doc.step_frame(2).unwrap();
        let ElementKind::Frame(fr) = &f3.kind else { unreachable!() };
        assert_eq!(fr.fill, Some(Color::rgb(0x0B, 0x10, 0x20)), "each slide keeps its background color");
        let ElementKind::Frame(first) = &doc.step_frame(0).unwrap().kind else { unreachable!() };
        assert_eq!(first.fill, Some(Color::WHITE));
        assert_eq!(doc.background, CANVAS, "slides sit on a neutral canvas");
        assert_eq!(imported.media.len(), 1);
        assert!(doc.elements.iter().any(|e| matches!(&e.kind, ElementKind::Rect(r) if r.corner_radius > 0.0 && r.fill == Some(Color::rgb(0x8B, 0x7C, 0xFF)))));

        // Content sits inside its slide's frame.
        let inside = |el: &Element, frame: &Element| {
            let size = frame.kind.intrinsic_size().unwrap();
            let d = el.transform.position() - frame.transform.position();
            d.x.abs() <= size.x / 2.0 + 1.0 && d.y.abs() <= size.y / 2.0 + 1.0
        };
        let pic = doc.elements.iter().find(|e| matches!(e.kind, ElementKind::Image(_))).unwrap();
        assert!(inside(pic, f3));
    }

    /// GlideDeck → PowerPoint → GlideDeck keeps every slide and its text.
    #[test]
    fn round_trips_through_powerpoint_export() {
        let mut engine = TextEngine::new();
        let original = crate::design::build_template_with(crate::design::Template::PitchDeck, &mut engine);
        let out = std::env::temp_dir().join(format!("glidedeck-roundtrip-{}.pptx", std::process::id()));
        glide_pptx::export(&original, None, &mut engine, &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        let imported = import(&bytes, "Round trip", &mut |k| glide_render::local_size(&mut engine, k)).unwrap();
        assert_eq!(imported.doc.path.len(), original.path.len());
        let before: Vec<String> = texts(&original).into_iter().map(|(_, t)| t.content).collect();
        let after: Vec<String> = texts(&imported.doc).into_iter().map(|(_, t)| t.content).collect();
        for t in ["Your big idea", "Why now", "Roadmap", "Thank you"] {
            assert!(before.iter().any(|b| b == t) && after.iter().any(|a| a == t), "{t} survives");
        }
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn rejects_files_that_are_not_presentations() {
        let mut engine = TextEngine::new();
        assert!(import(b"not a zip", "x", &mut |k| glide_render::local_size(&mut engine, k)).is_err());
    }
}
