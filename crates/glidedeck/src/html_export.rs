//! Exports a presentation as a single, self-contained HTML file that plays
//! exactly like the app's show: the same camera paths and transitions, the
//! same in-slide animations, parallax and navigation.
//!
//! Everything is vector data drawn on a `<canvas>` by a small player
//! (`html_player.js`) that re-implements the show's maths. Text is exported
//! as the very glyph outlines the app draws, so it looks the same without any
//! fonts installed. Web videos (YouTube, Vimeo, Loom, video links) play in
//! the site's own player, laid over the canvas and moving with the camera.
//! Images and SVGs are embedded; videos are copied next to
//! the HTML file (they are usually too big to inline).

use crate::anim::Measure;
use glide_core::{assets, BuildKind, Color, Document, Effect, Element, ElementKind, TextBlock};
use glide_render::{path_to_svg, TextEngine};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

const PLAYER_JS: &str = include_str!("html_player.js");

/// Count-up animations are pre-laid-out at this many points.
const COUNT_FRAMES: usize = 32;

#[derive(Debug, Default, PartialEq)]
pub struct HtmlReport {
    pub slides: usize,
    pub elements: usize,
    pub bytes: usize,
    /// Videos copied next to the HTML file.
    pub media_files: usize,
    pub missing_media: Vec<String>,
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
}

fn r4(v: f64) -> f64 {
    if v.is_finite() { (v * 1e4).round() / 1e4 } else { 0.0 }
}

/// Precise enough for world coordinates at any zoom depth (f64 in JSON).
fn num(v: f64) -> Value {
    if v.is_finite() { json!(v) } else { json!(0.0) }
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
}

fn data_uri(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{};base64,{}", mime_for(path), crate::mcp::base64(&bytes)))
}

/// Text outlines grouped by colour (`c` null = the text box's own colour).
fn text_runs(text: &mut TextEngine, block: &TextBlock) -> (Vec<Value>, [f64; 2]) {
    let layout = text.layout(block);
    let runs = text.colored_outlines(&layout).into_iter().map(|(c, p)| json!({ "d": path_to_svg(&p, 2), "c": c.map(hex) })).collect();
    (runs, [r4(layout.size.x), r4(layout.size.y)])
}

/// Byte ranges of the words, as the Words effect splits them.
fn word_ranges(content: &str) -> Vec<[usize; 2]> {
    let mut at = 0;
    content
        .split_inclusive(char::is_whitespace)
        .map(|w| {
            let r = [at, at + w.len()];
            at += w.len();
            r
        })
        .collect()
}

struct Ctx<'a> {
    text: &'a mut TextEngine,
    base: Option<PathBuf>,
    media_dir: Option<(PathBuf, String)>,
    report: HtmlReport,
    /// Embedded media by path, so a picture used twice is stored once.
    media: Map<String, Value>,
}

impl Ctx<'_> {
    fn embed(&mut self, rel: &str) -> Option<String> {
        let path = assets::resolve(self.base.as_deref(), rel);
        let key = path.to_string_lossy().into_owned();
        if !self.media.contains_key(&key) {
            match data_uri(&path) {
                Some(uri) => {
                    self.media.insert(key.clone(), Value::String(uri));
                }
                None => {
                    self.report.missing_media.push(rel.to_owned());
                    return None;
                }
            }
        }
        Some(key)
    }

    /// Copies a video next to the HTML file; returns its relative URL.
    fn copy_video(&mut self, rel: &str) -> Option<String> {
        let src = assets::resolve(self.base.as_deref(), rel);
        let (dir, url_prefix) = self.media_dir.clone()?;
        let name = src.file_name()?.to_string_lossy().into_owned();
        if std::fs::create_dir_all(&dir).is_err() || std::fs::copy(&src, dir.join(&name)).is_err() {
            self.report.missing_media.push(rel.to_owned());
            return None;
        }
        self.report.media_files += 1;
        Some(format!("{url_prefix}/{name}"))
    }
}

/// Which effects target each element (by id), to export only what's needed.
fn effects_by_element(doc: &Document) -> std::collections::HashMap<&str, Vec<Effect>> {
    let mut m: std::collections::HashMap<&str, Vec<Effect>> = Default::default();
    for s in &doc.path {
        for b in &s.builds {
            m.entry(b.element.as_str()).or_default().push(b.effect);
        }
    }
    m
}

fn element_json(cx: &mut Ctx, el: &Element, effects: &[Effect]) -> Value {
    let size = cx.text.size(el);
    let t = el.transform;
    let mut o = json!({
        "id": el.id,
        "x": num(t.x), "y": num(t.y), "s": num(t.scale), "r": num(t.rotation),
        "w": r4(size.x), "h": r4(size.y),
    });
    let m = o.as_object_mut().expect("object");
    if el.opacity != 1.0 {
        m.insert("o".into(), json!(r4(el.opacity)));
    }
    if el.hidden {
        m.insert("hidden".into(), json!(true));
    }
    if el.parallax != 0.0 {
        m.insert("px".into(), json!(r4(el.parallax)));
    }
    if let Some(sh) = el.shadow {
        m.insert("sh".into(), json!({ "c": hex(sh.color), "b": r4(sh.blur), "x": r4(sh.x), "y": r4(sh.y) }));
    }
    let gradient = |g: &Option<glide_core::Gradient>| g.map(|g| json!({ "to": hex(g.to), "a": r4(g.angle) }));
    let stroke = |s: &Option<glide_core::Stroke>| s.as_ref().map(|s| json!({ "c": hex(s.color), "w": r4(s.width) }));
    match &el.kind {
        ElementKind::Rect(r) => {
            m.insert("k".into(), json!("rect"));
            m.insert("rad".into(), json!(r4(r.corner_radius)));
            m.insert("fill".into(), json!(r.fill.map(hex)));
            m.insert("grad".into(), json!(gradient(&r.gradient)));
            m.insert("stroke".into(), json!(stroke(&r.stroke)));
        }
        ElementKind::Ellipse(e) => {
            m.insert("k".into(), json!("ellipse"));
            m.insert("fill".into(), json!(e.fill.map(hex)));
            m.insert("grad".into(), json!(gradient(&e.gradient)));
            m.insert("stroke".into(), json!(stroke(&e.stroke)));
        }
        ElementKind::Frame(f) => {
            m.insert("k".into(), json!("frame"));
            m.insert("rad".into(), json!(r4(f.corner_radius)));
            m.insert("fill".into(), json!(f.fill.map(hex)));
            m.insert("stroke".into(), json!(stroke(&f.border)));
            if f.is_invisible() {
                m.insert("invisible".into(), json!(true));
            }
        }
        ElementKind::Icon(icon) => {
            m.insert("k".into(), json!("icon"));
            m.insert("color".into(), json!(hex(icon.color)));
            let d = cx.text.icon_outline(&icon.name, icon.style, icon.size as f32).map(|c| path_to_svg(&glide_render::commands_to_path(&c), 3));
            m.insert("d".into(), json!(d.unwrap_or_default()));
        }
        ElementKind::Text(block) => {
            m.insert("k".into(), json!("text"));
            m.insert("color".into(), json!(hex(block.color)));
            let (runs, _) = text_runs(cx.text, block);
            m.insert("runs".into(), json!(runs));
            let layout = cx.text.layout(block);
            m.insert("lines".into(), json!(layout.line_boxes().iter().map(|l| l.map(r4)).collect::<Vec<_>>()));
            // Typewriter and Words fade single characters: export each glyph.
            if effects.iter().any(|e| matches!(e, Effect::Typewriter | Effect::Words)) {
                let glyphs: Vec<Value> = cx
                    .text
                    .glyph_outlines(&layout)
                    .into_iter()
                    .filter(|g| g.path.iter().next().is_some())
                    .map(|g| json!({ "d": path_to_svg(&g.path, 2), "s": g.start, "c": g.color.map(hex) }))
                    .collect();
                m.insert("glyphs".into(), json!(glyphs));
                m.insert("chars".into(), json!(block.content.char_indices().map(|(i, _)| i).collect::<Vec<_>>()));
                m.insert("len".into(), json!(block.content.len()));
                m.insert("words".into(), json!(word_ranges(&block.content)));
            }
            // Count up changes the digits, so lay the text out at each step.
            if effects.contains(&Effect::CountUp) {
                let frames: Vec<Value> = (0..COUNT_FRAMES)
                    .map(|i| {
                        let p = i as f64 / COUNT_FRAMES as f64;
                        let mut b = block.clone();
                        b.set_content(crate::anim::count_up_text(&block.content, p));
                        let (runs, size) = text_runs(cx.text, &b);
                        json!({ "runs": runs, "w": size[0], "h": size[1] })
                    })
                    .collect();
                m.insert("count".into(), json!(frames));
            }
        }
        ElementKind::Image(media) => {
            m.insert("k".into(), json!("image"));
            m.insert("rad".into(), json!(r4(media.corner_radius)));
            m.insert("src".into(), json!(cx.embed(&media.path)));
        }
        ElementKind::Svg(media) => {
            m.insert("k".into(), json!("svg"));
            m.insert("src".into(), json!(cx.embed(&media.path)));
        }
        ElementKind::Video(media) if media.url.as_deref().is_some_and(glide_core::weblink::is_web_url) => {
            let url = media.url.as_deref().unwrap_or_default().trim();
            m.insert("k".into(), json!("embed"));
            m.insert("rad".into(), json!(r4(media.corner_radius)));
            // The poster draws like an image until the video plays.
            m.insert("src".into(), json!(cx.embed(&media.path)));
            m.insert("url".into(), json!(url));
            m.insert("title".into(), json!(el.display_name()));
            match glide_core::weblink::site_video(url) {
                Some(v) => {
                    m.insert("player".into(), json!(v.player_url(true, media.muted, media.looping)));
                    m.insert("site".into(), json!(v.site_name().to_lowercase()));
                }
                None => {
                    m.insert("media".into(), json!(url));
                }
            }
            m.insert("loop".into(), json!(media.looping));
            m.insert("muted".into(), json!(media.muted));
            m.insert("autoplay".into(), json!(media.autoplay));
        }
        ElementKind::Video(media) => {
            m.insert("k".into(), json!("video"));
            m.insert("rad".into(), json!(r4(media.corner_radius)));
            m.insert("src".into(), json!(cx.copy_video(&media.path)));
            m.insert("loop".into(), json!(media.looping));
            m.insert("muted".into(), json!(media.muted));
            m.insert("autoplay".into(), json!(media.autoplay));
        }
    }
    o
}

/// The deck as the player's JSON.
pub fn deck_json(doc: &Document, text: &mut TextEngine, base: Option<PathBuf>, media_dir: Option<(PathBuf, String)>) -> (Value, HtmlReport) {
    let mut cx = Ctx { text, base, media_dir, report: HtmlReport::default(), media: Map::new() };
    let effects = effects_by_element(doc);
    let index: std::collections::HashMap<&str, usize> = doc.elements.iter().enumerate().map(|(i, e)| (e.id.as_str(), i)).collect();
    let elements: Vec<Value> = doc.elements.iter().map(|e| element_json(&mut cx, e, effects.get(e.id.as_str()).map(Vec::as_slice).unwrap_or(&[]))).collect();
    let path: Vec<Value> = doc
        .path
        .iter()
        .filter_map(|s| {
            let frame = *index.get(s.frame.as_str())?;
            let builds: Vec<Value> = s
                .builds
                .iter()
                .filter_map(|b| {
                    let el = *index.get(b.element.as_str())?;
                    let kind = match b.effect.kind() {
                        BuildKind::Appear => "appear",
                        BuildKind::Emphasis => "emphasis",
                        BuildKind::Disappear => "disappear",
                    };
                    Some(json!({
                        "el": el, "fx": b.effect.id(), "kind": kind,
                        "trigger": serde_json::to_value(b.trigger).unwrap_or_default(),
                        "dur": r4(b.duration), "delay": r4(b.delay),
                        "dir": serde_json::to_value(b.direction).unwrap_or_default(),
                    }))
                })
                .collect();
            Some(json!({
                "frame": frame,
                "style": s.transition.style.id(),
                "dur": r4(s.transition.duration),
                "ease": serde_json::to_value(s.transition.easing).unwrap_or_default(),
                "builds": builds,
            }))
        })
        .collect();
    let accent = doc.theme.as_ref().map(|t| t.accents[0]).unwrap_or(Color::rgb(0x8B, 0x7C, 0xFF));
    let deck = json!({
        "title": doc.title,
        "bg": hex(doc.background),
        "bgTo": doc.background_to.map(hex),
        "accent": hex(accent),
        "elements": elements,
        "path": path,
        "media": cx.media,
    });
    cx.report.slides = doc.path.len();
    cx.report.elements = doc.elements.len();
    (deck, cx.report)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The complete HTML page for a deck.
pub fn html_page(deck: &Value, title: &str) -> String {
    // `</script>` inside JSON strings would end the script element early.
    let data = deck.to_string().replace("</", "<\\/");
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="GlideDeck {version}">
<title>{title}</title>
<style>
  html, body {{ margin: 0; height: 100%; overflow: hidden; background: #000; }}
  canvas {{ display: block; width: 100vw; height: 100vh; touch-action: none; cursor: default; }}
  #hud {{ position: fixed; left: 50%; bottom: 24px; transform: translateX(-50%); display: flex; gap: 6px; align-items: center;
         padding: 6px 10px; border-radius: 14px; background: rgba(18,18,22,.82); color: #fff; font: 600 13px system-ui, sans-serif;
         transition: opacity .35s; user-select: none; }}
  #hud button {{ all: unset; cursor: pointer; padding: 4px 9px; border-radius: 8px; font-size: 15px; line-height: 1; }}
  #hud button:hover {{ background: rgba(255,255,255,.12); }}
  #hud .count {{ min-width: 56px; text-align: center; font-variant-numeric: tabular-nums; }}
  #hud.hidden {{ opacity: 0; pointer-events: none; }}
  #help {{ position: fixed; inset: 0; display: none; place-items: center; background: rgba(0,0,0,.55); color: #fff; font: 15px/1.7 system-ui, sans-serif; }}
  #help.show {{ display: grid; }}
  #embeds {{ position: fixed; inset: 0; pointer-events: none; overflow: hidden; }}
  #embeds .embed {{ position: absolute; left: 0; top: 0; transform-origin: 0 0; overflow: hidden; display: none; }}
  #embeds iframe, #embeds video {{ position: absolute; inset: 0; width: 100%; height: 100%; border: 0; background: #000; pointer-events: auto; }}
  #embeds .play {{ position: absolute; left: 50%; top: 50%; width: 11%; transform: translate(-50%, -50%); }}
  #embeds .play svg {{ display: block; width: 100%; height: auto; }}
  #help div {{ background: #16161c; padding: 22px 28px; border-radius: 14px; }}
  #help b {{ display: inline-block; min-width: 128px; }}
</style>
</head>
<body>
<canvas id="stage" aria-label="{title}"></canvas>
<div id="hud" class="hidden"><button id="prev" title="Previous (←)">&#8249;</button><span class="count" id="count"></span><button id="next" title="Next (→)">&#8250;</button><button id="full" title="Fullscreen (F)">&#x26F6;</button><button id="helpbtn" title="Keys (?)">?</button></div>
<div id="help"><div>
<div><b>Next</b>→ · Space · click · PageDown</div>
<div><b>Back</b>← · right-click · PageUp</div>
<div><b>First / last</b>Home · End</div>
<div><b>Zoom freely</b>mouse wheel / pinch</div>
<div><b>Black / white screen</b>B · W</div>
<div><b>Fullscreen</b>F</div>
</div></div>
<script id="deck" type="application/json">{data}</script>
<script>
{player}
</script>
</body>
</html>
"#,
        version = env!("CARGO_PKG_VERSION"),
        title = html_escape(title),
        data = data,
        player = PLAYER_JS,
    )
}

/// Loads the font files a document carries into a text engine (for headless
/// exports; the app's renderer loads them itself).
pub fn load_document_fonts(text: &mut TextEngine, doc: &Document, base: Option<&Path>) {
    for rel in &doc.fonts {
        if let Ok(bytes) = std::fs::read(assets::resolve(base, rel)) {
            text.load_font_data(bytes);
        }
    }
}

/// Writes the HTML file (and a `<name>_media` folder for videos, if any).
pub fn export(doc: &Document, text: &mut TextEngine, base: Option<&Path>, out: &Path) -> Result<HtmlReport, String> {
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "presentation".into());
    let media_name = format!("{stem}_media");
    let media_dir = out.parent().map(|p| p.join(&media_name)).unwrap_or_else(|| PathBuf::from(&media_name));
    let (deck, mut report) = deck_json(doc, text, base.map(Path::to_path_buf), Some((media_dir, media_name)));
    let title = if doc.title.trim().is_empty() { "GlideDeck presentation".to_owned() } else { doc.title.clone() };
    let page = html_page(&deck, &title);
    std::fs::write(out, &page).map_err(|e| format!("writing {}: {e}", out.display()))?;
    report.bytes = page.len();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glide_core::{Build, Frame, Step, Transform};

    #[test]
    fn deck_carries_vectors_path_and_builds() {
        let mut doc = crate::design::build_template(crate::design::Template::PitchDeck);
        let title_id = doc.elements.iter().find(|e| matches!(&e.kind, ElementKind::Text(t) if t.content == "Your big idea")).unwrap().id.clone();
        let step = doc.path.iter().position(|s| doc.get(&s.frame).is_some()).unwrap();
        doc.path[1].builds = vec![Build::new(title_id.clone(), Effect::Typewriter), Build::new(title_id, Effect::Highlight)];
        let mut text = TextEngine::new();
        let (deck, report) = deck_json(&doc, &mut text, None, None);
        assert_eq!(report.slides, doc.path.len());
        assert_eq!(deck["path"].as_array().unwrap().len(), doc.path.len());
        assert!(step < doc.path.len());
        let els = deck["elements"].as_array().unwrap();
        let title = els.iter().find(|e| e["k"] == "text" && e["glyphs"].is_array()).expect("typewriter text exports glyphs");
        let glyphs = title["glyphs"].as_array().unwrap();
        assert_eq!(glyphs.len(), "Your big idea".chars().filter(|c| !c.is_whitespace()).count(), "one outline per visible character");
        assert!(glyphs[0]["d"].as_str().unwrap().starts_with('M'));
        assert_eq!(glyphs[1]["s"], 1, "byte offsets of the characters");
        assert!(title["lines"].as_array().unwrap().len() == 1, "line boxes for the highlight marker");
        assert_eq!(deck["path"][1]["builds"][0]["fx"], "typewriter");
        // Every text has outlines, so the browser needs no fonts.
        assert!(els.iter().filter(|e| e["k"] == "text").all(|e| !e["runs"].as_array().unwrap().is_empty()));
    }

    #[test]
    fn web_videos_play_in_the_site_player_and_link_from_powerpoint() {
        let dir = std::env::temp_dir().join(format!("gd-webvideo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(16, 9, image::Rgba([200, 30, 30, 255])).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let poster = assets::import_bytes(&dir, png.get_ref(), "png").unwrap();

        let mut doc = Document::new("Web");
        let frame = doc.add(Element::new(ElementKind::Frame(Frame::widescreen(1600.0))));
        doc.path.push(Step::new(frame));
        let mut yt = glide_core::Media::new(poster.clone(), 1280.0, 720.0);
        yt.url = Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=30".into());
        yt.muted = true;
        let mut clip = glide_core::Media::new(poster, 1280.0, 720.0);
        clip.url = Some("https://cdn.example.com/clip.mp4".into());
        doc.add(Element::new(ElementKind::Video(yt)).with_transform(Transform { x: -300.0, y: 0.0, scale: 0.3, rotation: 0.0 }));
        doc.add(Element::new(ElementKind::Video(clip)).with_transform(Transform { x: 300.0, y: 0.0, scale: 0.3, rotation: 0.0 }));

        let mut text = TextEngine::new();
        let (deck, report) = deck_json(&doc, &mut text, Some(dir.clone()), None);
        assert_eq!(report.media_files, 0, "web videos are not copied");
        let embeds: Vec<&Value> = deck["elements"].as_array().unwrap().iter().filter(|e| e["k"] == "embed").collect();
        assert_eq!(embeds.len(), 2);
        assert_eq!(
            embeds[0]["player"],
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?rel=0&playsinline=1&autoplay=1&mute=1&start=30"
        );
        assert_eq!(embeds[0]["site"], "youtube");
        assert_eq!(embeds[1]["media"], "https://cdn.example.com/clip.mp4");
        // The poster is embedded so it draws before (and instead of) the player.
        let key = embeds[0]["src"].as_str().unwrap();
        assert!(deck["media"][key].as_str().unwrap().starts_with("data:image/png;base64,"));
        assert!(html_page(&deck, "Web").contains("#embeds"));

        // PowerPoint: the poster, linked to the video.
        let out = dir.join("web.pptx");
        glide_pptx::export(&doc, Some(&dir), &mut text, &out).unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut rels = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("ppt/slides/_rels/slide1.xml.rels").unwrap(), &mut rels).unwrap();
        assert!(rels.contains(r#"Target="https://www.youtube.com/watch?v=dQw4w9WgXcQ&amp;t=30" TargetMode="External""#), "{rels}");
        let mut slide = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("ppt/slides/slide1.xml").unwrap(), &mut slide).unwrap();
        assert_eq!(slide.matches("<a:hlinkClick r:id=").count(), 2);
        assert!(!slide.contains("videoFile"), "no embedded video file for web videos");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn count_up_is_pre_laid_out_and_page_is_self_contained() {
        let mut doc = Document::new("Numbers");
        let mut f = Element::new(ElementKind::Frame(Frame::widescreen(1600.0)));
        f.id = "f".into();
        doc.add(f);
        doc.path.push(Step::new("f"));
        let mut t = Element::new(ElementKind::Text(TextBlock::new("Revenue $12,400"))).with_transform(Transform::default());
        t.id = "n".into();
        doc.add(t);
        doc.path[0].builds = vec![Build::new("n", Effect::CountUp)];
        let mut text = TextEngine::new();
        let (deck, _) = deck_json(&doc, &mut text, None, None);
        let count = deck["elements"][1]["count"].as_array().unwrap();
        assert_eq!(count.len(), COUNT_FRAMES);
        assert_ne!(count[5]["runs"], deck["elements"][1]["runs"], "digits differ mid-count");
        let page = html_page(&deck, "Numbers <test>");
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.contains("Numbers &lt;test&gt;"));
        assert!(page.contains("function sampleTween") && page.contains("id=\"deck\""));
        assert!(!page.contains("http://") && !page.contains("https://"), "no external requests");
    }

    #[test]
    fn words_are_split_like_the_app() {
        assert_eq!(word_ranges("Hi there you"), vec![[0, 3], [3, 9], [9, 12]]);
    }
}

/// Parity checks against the app: renders states exactly as the show draws
/// them, for comparing with browser screenshots of the exported page.
/// `GLIDEDECK_PARITY=<dir> GLIDEDECK_PARITY_DOC=<file> cargo test -p glidedeck parity -- --ignored`
#[cfg(test)]
mod parity {
    use crate::anim::{apply_parallax, scene, BuildClock};
    use glide_core::{Camera, CameraTween, Document};
    use glide_render::HeadlessRenderer;
    use glam::DVec2;

    fn save(dir: &std::path::Path, name: &str, rgba: &[u8], w: u32, h: u32) {
        image::save_buffer(dir.join(name), rgba, w, h, image::ExtendedColorType::Rgba8).unwrap();
    }

    #[test]
    #[ignore = "writes images for comparing with the HTML export"]
    fn parity_renders() {
        let (Some(dir), Some(file)) = (std::env::var_os("GLIDEDECK_PARITY"), std::env::var_os("GLIDEDECK_PARITY_DOC")) else { return };
        let dir = std::path::PathBuf::from(dir);
        let file = std::path::PathBuf::from(file);
        let doc = Document::load(&file).unwrap();
        let mut r = HeadlessRenderer::new().unwrap();
        r.canvas.set_base_dir(file.parent().map(|p| p.to_path_buf()));
        let (w, h) = (1280u32, 720u32);
        let vp = DVec2::new(w as f64, h as f64);
        let cam_of = |i: usize| Camera::for_frame(doc.step_frame(i).unwrap(), vp, 0.0).unwrap();
        // Settled states: "#step=N&elapsed=E" (and clicks).
        let states: Vec<(usize, usize, f64)> = std::env::var("GLIDEDECK_PARITY_STATES")
            .unwrap_or_else(|_| "2:0:8,2:1:0.35,4:0:0.3".into())
            .split(',')
            .map(|s| {
                let v: Vec<&str> = s.split(':').collect();
                (v[0].parse().unwrap(), v[1].parse().unwrap(), v[2].parse().unwrap())
            })
            .collect();
        for (step, clicks, elapsed) in states {
            let i = step - 1;
            let mut clock = BuildClock::fresh(&doc, i);
            let t0 = 1000.0;
            if clock.next_is_automatic(&doc)
                && let Some(g) = clock.next_group(&doc)
            {
                clock.start(&doc, g, t0);
            }
            for _ in 0..clicks {
                clock.finish_running();
                if let Some(g) = clock.next_group(&doc) {
                    clock.start(&doc, g, t0);
                }
            }
            let cam = cam_of(i);
            let mut s = scene(&doc, &clock, t0 + elapsed, &mut r.canvas);
            apply_parallax(&mut s, &cam, vp);
            let rgba = r.canvas.snapshot(&s, &cam, [w, h], false).unwrap();
            save(&dir, &format!("app-s{step}-c{clicks}-e{elapsed}.png"), &rgba, w, h);
        }
        // Transitions: "#step=T&from=F&t=X".
        for (from, to, t) in std::env::var("GLIDEDECK_PARITY_TWEENS").unwrap_or_else(|_| "1:2:0.5,2:3:0.4,3:4:0.5,8:9:0.5".into()).split(',').map(|s| { let v: Vec<&str> = s.split(':').collect(); (v[0].parse::<usize>().unwrap(), v[1].parse::<usize>().unwrap(), v[2].parse::<f64>().unwrap()) }) {
            if to > doc.path.len() {
                continue;
            }
            let step = &doc.path[(to - 1).max(from - 1)];
            let tr = step.transition;
            let tw = CameraTween::new(cam_of(from - 1), cam_of(to - 1), tr.duration, tr.easing).with_style(tr.style, vp);
            let cam = tw.sample(t);
            let clock = if to < from { BuildClock::finished(&doc, to - 1) } else { BuildClock::fresh(&doc, to - 1) };
            let mut s = scene(&doc, &clock, 0.0, &mut r.canvas);
            apply_parallax(&mut s, &cam, vp);
            let mut rgba = r.canvas.snapshot(&s, &cam, [w, h], false).unwrap();
            // The Fade dip, drawn over the finished image like present.rs does.
            let mut tw = tw;
            tw.elapsed = t * tw.duration;
            let fade = tw.fade_amount();
            if fade > 0.0 {
                let bg = doc.background;
                for px in rgba.chunks_mut(4) {
                    for (c, b) in px.iter_mut().zip([bg.r(), bg.g(), bg.b()]) {
                        *c = (*c as f64 + (b as f64 - *c as f64) * fade).round() as u8;
                    }
                }
            }
            save(&dir, &format!("app-t{from}-{to}-{t}.png"), &rgba, w, h);
        }
    }
}
