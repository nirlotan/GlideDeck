//! Web videos: a YouTube, Vimeo or Loom link (or a direct link to a video
//! file) placed on the canvas as a video with its poster and title.
//!
//! Inserting one fetches only the title and the poster image (oEmbed, or the
//! site's thumbnail), which is stored in `assets/` so the deck looks right
//! offline. The video itself streams only when it is played.

use crate::app::{GlideDeckApp, Tool};
use eframe::egui;
use glam::DVec2;
use glide_core::{assets, weblink, ElementKind};
use serde_json::Value;
use std::path::Path;
use std::sync::mpsc;

/// What inserting a web video needs: a title, a poster and the picture size.
#[derive(Debug, Clone)]
pub struct WebVideoMeta {
    pub url: String,
    pub title: String,
    /// Poster image bytes and their file extension.
    pub poster: Option<(Vec<u8>, &'static str)>,
    pub size: (f64, f64),
}

/// A web video being looked up in the background.
pub struct Pending {
    rx: mpsc::Receiver<Result<WebVideoMeta, String>>,
    at_screen: Option<DVec2>,
}

/// The Insert → Web video dialog.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    pub url: String,
}

fn curl(url: &str) -> Result<Vec<u8>, String> {
    crate::update::curl(&["-sSfL", "--max-time", "15", "--proto", "=https,http", url])
}

fn json(url: &str) -> Option<Value> {
    serde_json::from_slice(&curl(url).ok()?).ok()
}

/// Percent-encodes a value for a query string.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    match image::guess_format(bytes).ok()? {
        image::ImageFormat::Jpeg => Some("jpg"),
        image::ImageFormat::Png => Some("png"),
        image::ImageFormat::WebP => Some("webp"),
        _ => None,
    }
}

/// Downloads an image, if it is one.
fn fetch_image(url: &str) -> Option<(Vec<u8>, &'static str)> {
    if !weblink::is_web_url(url) {
        return None;
    }
    let bytes = curl(url).ok()?;
    let ext = image_ext(&bytes)?;
    Some((bytes, ext))
}

fn size_of(v: &Value) -> Option<(f64, f64)> {
    let (w, h) = (v["width"].as_f64()?, v["height"].as_f64()?);
    (w > 0.0 && h > 0.0).then_some((w, h))
}

/// Looks up a web video's title and poster. Blocks on the network.
pub fn fetch(url: &str) -> Result<WebVideoMeta, String> {
    let url = url.trim().to_owned();
    if !weblink::is_web_url(&url) {
        return Err("That isn't a web address. Paste a link that starts with https://".into());
    }
    let hd = (1280.0, 720.0);
    if let Some(v) = weblink::site_video(&url) {
        let (oembed, size) = match v.site {
            weblink::Site::YouTube => {
                let vertical = url.contains("/shorts/");
                (format!("https://www.youtube.com/oembed?format=json&url={}", encode(&url)), if vertical { (720.0, 1280.0) } else { hd })
            }
            weblink::Site::Vimeo => (format!("https://vimeo.com/api/oembed.json?width=1280&url={}", encode(&url)), hd),
            weblink::Site::Loom => (format!("https://www.loom.com/v1/oembed?url={}", encode(&url)), hd),
        };
        let info = json(&oembed);
        let title = info.as_ref().and_then(|i| i["title"].as_str()).map(str::to_owned).unwrap_or_else(|| format!("{} video", v.site_name()));
        let size = match v.site {
            weblink::Site::YouTube => size,
            _ => info.as_ref().and_then(size_of).unwrap_or(size),
        };
        let poster = match v.site {
            // The largest thumbnail when there is one; hqdefault always exists.
            weblink::Site::YouTube => ["maxresdefault", "sddefault", "hqdefault"]
                .iter()
                .find_map(|q| fetch_image(&format!("https://i.ytimg.com/vi/{}/{q}.jpg", v.id)).filter(|(b, _)| b.len() > 2000)),
            _ => info.as_ref().and_then(|i| i["thumbnail_url"].as_str()).and_then(fetch_image),
        };
        return Ok(WebVideoMeta { url, title, poster, size });
    }
    let name = url.rsplit('/').find(|s| !s.is_empty()).map(|s| s.split(['?', '#']).next().unwrap_or(s).to_owned()).unwrap_or_default();
    if weblink::is_direct_video(&url) {
        let path = Path::new(&url);
        let info = glide_render::video::probe(path).ok();
        let size = info.as_ref().map(|i| (i.width as f64, i.height as f64)).unwrap_or(hd);
        let poster = glide_render::video::poster_png(path).map(|b| (b, "png"));
        return Ok(WebVideoMeta { url, title: name, poster, size });
    }
    // Any other site yt-dlp knows.
    if let Some(ytdlp) = glide_render::video::ytdlp_path() {
        let mut cmd = std::process::Command::new(ytdlp);
        cmd.args(["-J", "--no-playlist", "--no-warnings", "--"]).arg(&url);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        if let Ok(out) = cmd.output()
            && let Ok(v) = serde_json::from_slice::<Value>(&out.stdout)
        {
            let title = v["title"].as_str().unwrap_or(&name).to_owned();
            let size = size_of(&v).unwrap_or(hd);
            let poster = v["thumbnail"].as_str().and_then(fetch_image);
            return Ok(WebVideoMeta { url, title, poster, size });
        }
    }
    Err("GlideDeck recognises YouTube, Vimeo and Loom links, and direct links to video files (.mp4, .webm, .m3u8…)".into())
}

/// A plain dark poster for videos without a thumbnail.
fn placeholder_poster((w, h): (f64, f64)) -> Vec<u8> {
    let k = (640.0 / w.max(h)).min(1.0);
    let (w, h) = (((w * k).round() as u32).max(16), ((h * k).round() as u32).max(16));
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([17, 24, 39, 255]));
    let mut png = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut png, image::ImageFormat::Png);
    png.into_inner()
}

/// Stores a web video's poster (or a plain one) in `assets/`; returns its reference.
pub fn store_poster(base: &Path, meta: &WebVideoMeta) -> String {
    let (bytes, ext) = meta.poster.clone().unwrap_or_else(|| (placeholder_poster(meta.size), "png"));
    assets::import_bytes(base, &bytes, ext).unwrap_or_default()
}

/// Opens a web address in the default browser. Only http(s) links are opened.
pub fn open_in_browser(url: &str) {
    if weblink::is_web_url(url) {
        crate::update::open_external(url.trim());
    }
}

impl GlideDeckApp {
    /// Starts inserting a web video; it appears once its details arrive.
    pub fn insert_web_video(&mut self, ctx: &egui::Context, url: &str, at_screen: Option<DVec2>) {
        let url = url.trim().to_owned();
        if !weblink::is_web_url(&url) {
            self.notify(ctx, "That isn't a web address. Paste a link that starts with https://");
            return;
        }
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(fetch(&url));
            repaint.request_repaint();
        });
        self.web_videos.push(Pending { rx, at_screen });
        self.notify(ctx, "Getting the video's details…");
    }

    /// Adds web videos whose details have arrived.
    pub fn poll_web_videos(&mut self, ctx: &egui::Context) {
        let mut done = Vec::new();
        self.web_videos.retain(|p| match p.rx.try_recv() {
            Ok(r) => {
                done.push((r, p.at_screen));
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        if !self.web_videos.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        for (result, at) in done {
            match result {
                Ok(meta) => {
                    let id = self.add_web_video(meta, at);
                    self.selection = vec![id];
                    self.tool = Tool::Select;
                    let hint = if glide_render::video::ffmpeg_path().is_none() {
                        "Web video added. Install FFmpeg to play it here; it always plays in the web page export"
                    } else if weblink::site_video(self.selected_video_url().as_deref().unwrap_or("")).is_some() && glide_render::video::ytdlp_path().is_none() {
                        "Web video added. It plays in the web page export; to play it here too, install yt-dlp"
                    } else {
                        "Web video added. It streams when played, so keep a connection for the show"
                    };
                    self.notify(ctx, hint);
                }
                Err(e) => self.notify(ctx, e),
            }
        }
    }

    fn selected_video_url(&self) -> Option<String> {
        let el = self.doc.get(self.selection.first()?)?;
        match &el.kind {
            ElementKind::Video(m) => m.url.clone(),
            _ => None,
        }
    }

    /// Places a web video on the canvas. Returns its id.
    pub fn add_web_video(&mut self, meta: WebVideoMeta, at_screen: Option<DVec2>) -> String {
        let reference = store_poster(&self.base_dir(), &meta);
        let id = self.add_media_element(ElementKind::Video, reference, meta.size, meta.title.clone(), at_screen, 0, 1.0);
        if let Some(ElementKind::Video(m)) = self.doc.get_mut(&id).map(|e| &mut e.kind) {
            m.url = Some(meta.url);
            // Sites start with sound; the show waits for a click unless set to autoplay.
            m.autoplay = false;
        }
        id
    }
}

/// The Insert → Web video dialog.
pub fn dialog(app: &mut GlideDeckApp, ctx: &egui::Context) {
    if !app.web_video_dialog.open {
        return;
    }
    let p = crate::theme::palette(ctx);
    let mut open = true;
    let mut insert = false;
    egui::Window::new("Insert web video")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -60.0))
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label("Paste a link to a video:");
            let r = ui.add(
                egui::TextEdit::singleline(&mut app.web_video_dialog.url)
                    .hint_text("https://www.youtube.com/watch?v=…")
                    .desired_width(f32::INFINITY),
            );
            if !r.has_focus() && app.web_video_dialog.url.is_empty() {
                r.request_focus();
            }
            ui.label(
                egui::RichText::new("YouTube, Vimeo and Loom links, or a direct link to a video file (.mp4, .webm, .m3u8). The poster is saved with the presentation; the video streams when it plays.")
                    .size(11.5)
                    .color(p.text_muted),
            );
            if glide_render::video::ytdlp_path().is_none() {
                ui.label(
                    egui::RichText::new("YouTube, Vimeo and Loom videos play in the web page export. To play them in GlideDeck too, install yt-dlp (Windows: winget install yt-dlp · macOS: brew install yt-dlp).")
                        .size(11.5)
                        .color(p.text_muted),
                );
            }
            ui.add_space(6.0);
            let valid = weblink::is_web_url(&app.web_video_dialog.url);
            ui.horizontal(|ui| {
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.add_enabled(valid, egui::Button::new("Insert")).clicked() || (enter && valid) {
                    insert = true;
                }
                if ui.button("Cancel").clicked() {
                    app.web_video_dialog.open = false;
                }
            });
        });
    if insert {
        let url = std::mem::take(&mut app.web_video_dialog.url);
        app.web_video_dialog.open = false;
        app.insert_web_video(ctx, &url, None);
    }
    if !open {
        app.web_video_dialog.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_values_are_encoded() {
        assert_eq!(encode("https://youtu.be/a?t=1&x=y z"), "https%3A%2F%2Fyoutu.be%2Fa%3Ft%3D1%26x%3Dy%20z");
    }

    #[test]
    fn non_links_are_refused_without_network() {
        assert!(fetch("not a link").is_err());
        assert!(fetch("file:///C:/video.mp4").is_err());
    }
}
