//! Getting content in and out: image import, drag & drop, and the system
//! clipboard (images, files, text, and GlideDeck's own elements).
//!
//! Imported media is copied into the document's `assets/` folder and
//! referenced by relative path; the JSON never contains media bytes.

use crate::app::{Action, Tool, GlideDeckApp};
use eframe::egui;
use glam::DVec2;
use serde_json::json;
use std::path::{Path, PathBuf};
use glide_core::{assets, Element, ElementKind, Media, TextBlock, Transform};

const CLIPBOARD_TAG: &str = "glidedeck/elements";
pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "svg"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "avi", "wmv"];

fn ext_in(p: &Path, list: &[&str]) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| list.contains(&e.to_ascii_lowercase().as_str()))
}

fn is_image_path(p: &Path) -> bool {
    ext_in(p, IMAGE_EXTS)
}

fn is_video_path(p: &Path) -> bool {
    ext_in(p, VIDEO_EXTS)
}

fn is_svg(p: &Path) -> bool {
    ext_in(p, &["svg"])
}

impl GlideDeckApp {
    /// World position for new content: the given screen point or the view center.
    fn placement(&self, at_screen: Option<DVec2>) -> DVec2 {
        let vp = self.viewport();
        self.camera.screen_to_world(at_screen.unwrap_or(vp * 0.5), vp)
    }

    /// Adds an image element for an already-stored asset, sized to read well
    /// at the current zoom (at most ~60% of the view, never upscaled).
    fn add_image_element(&mut self, reference: String, px: (u32, u32), name: String, at_screen: Option<DVec2>, offset: usize) -> String {
        self.add_media_element(ElementKind::Image, reference, (px.0 as f64, px.1 as f64), name, at_screen, offset, 1.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_media_element(
        &mut self,
        kind: fn(Media) -> ElementKind,
        reference: String,
        size: (f64, f64),
        name: String,
        at_screen: Option<DVec2>,
        offset: usize,
        max_upscale: f64,
    ) -> String {
        let vp = self.viewport();
        let (w, h) = (size.0.max(1.0), size.1.max(1.0));
        let fit = ((vp.x * 0.6) / w).min((vp.y * 0.6) / h).min(max_upscale);
        let pos = self.placement(at_screen) + self.camera.screen_delta_to_world(DVec2::splat(24.0 * offset as f64));
        let mut media = Media::new(reference, w, h);
        let is_video = matches!(kind(media.clone()), ElementKind::Video(_));
        if is_video {
            // Videos sit in a rounded "screen" with a soft shadow.
            media.corner_radius = w.min(h) * 0.03;
        }
        let mut el = Element::new(kind(media)).with_transform(
            Transform { x: pos.x, y: pos.y, scale: fit / self.camera.zoom, rotation: self.camera.rotation.to_degrees() },
        );
        if is_video {
            el.shadow = Some(glide_core::Shadow::lifted());
        }
        el.name = name;
        self.doc.add(el)
    }

    /// Imports video files (copied into assets/, played with FFmpeg).
    pub fn import_video_files(&mut self, ctx: &egui::Context, files: &[PathBuf], at_screen: Option<DVec2>) {
        let base = self.base_dir();
        let mut added = Vec::new();
        for (i, f) in files.iter().enumerate() {
            match assets::import_file(&base, f) {
                Ok(reference) => {
                    let size = glide_render::video::probe(f).map(|v| (v.width as f64, v.height as f64)).unwrap_or((1920.0, 1080.0));
                    let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    added.push(self.add_media_element(ElementKind::Video, reference, size, name, at_screen, i, 1.0));
                }
                Err(e) => self.notify(ctx, format!("Could not import {}: {e}", f.display())),
            }
        }
        if !added.is_empty() {
            self.selection = added;
            self.tool = Tool::Select;
            if glide_render::video::ffmpeg_path().is_none() {
                self.notify(ctx, "Video added. Install FFmpeg to play it (see Properties).");
            }
        }
    }

    /// Imports image files from disk.
    pub fn import_image_files(&mut self, ctx: &egui::Context, files: &[PathBuf], at_screen: Option<DVec2>) {
        let base = self.base_dir();
        let mut added = Vec::new();
        let mut failed = Vec::new();
        for (i, f) in files.iter().enumerate() {
            if is_svg(f) {
                match (glide_render::svg::svg_dimensions(f), assets::import_file(&base, f)) {
                    (Some((w, h)), Ok(reference)) => {
                        let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        // Vectors can scale up freely.
                        added.push(self.add_media_element(ElementKind::Svg, reference, (w as f64, h as f64), name, at_screen, i, 4.0));
                    }
                    _ => failed.push(f.display().to_string()),
                }
                continue;
            }
            let dims = glide_render::image_dimensions(f);
            match (dims, assets::import_file(&base, f)) {
                (Some(px), Ok(reference)) => {
                    let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    added.push(self.add_image_element(reference, px, name, at_screen, i));
                }
                _ => failed.push(f.display().to_string()),
            }
        }
        if !added.is_empty() {
            self.selection = added;
            self.tool = Tool::Select;
        }
        if !failed.is_empty() {
            self.notify(ctx, format!("Could not import: {}", failed.join(", ")));
        }
    }

    /// Imports raw RGBA pixels (e.g. a screenshot on the clipboard) as a PNG asset.
    fn import_rgba(&mut self, width: u32, height: u32, rgba: Vec<u8>, at_screen: Option<DVec2>) -> Result<String, String> {
        let img = image::RgbaImage::from_raw(width, height, rgba).ok_or("clipboard image has an unexpected size")?;
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        let reference = assets::import_bytes(&self.base_dir(), png.get_ref(), "png").map_err(|e| e.to_string())?;
        Ok(self.add_image_element(reference, (width, height), "Pasted image".into(), at_screen, 0))
    }

    pub fn copy_selection(&mut self, ctx: &egui::Context) {
        let elements: Vec<&Element> = self.doc.elements.iter().filter(|e| self.selection.contains(&e.id)).collect();
        if elements.is_empty() {
            return;
        }
        // Media references are made absolute so pasting into another document
        // can import the files into its own asset folder.
        let base = self.base_dir();
        let mut payload = Vec::new();
        for el in elements {
            let mut el = el.clone();
            if let ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) = &mut el.kind {
                m.path = assets::resolve(Some(&base), &m.path).display().to_string();
            }
            payload.push(el);
        }
        let text = json!({ "type": CLIPBOARD_TAG, "elements": payload }).to_string();
        ctx.copy_text(text);
    }

    /// Handles text that arrived via a paste event or the clipboard.
    fn paste_text(&mut self, ctx: &egui::Context, text: &str, at_screen: Option<DVec2>) {
        // Our own elements?
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text)
            && v.get("type").and_then(|t| t.as_str()) == Some(CLIPBOARD_TAG)
            && let Ok(mut elements) = serde_json::from_value::<Vec<Element>>(v["elements"].clone())
        {
            let base = self.base_dir();
            for el in &mut elements {
                if let ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) = &mut el.kind {
                    let src = Path::new(&m.path);
                    if src.is_absolute()
                        && let Ok(reference) = assets::import_file(&base, src)
                    {
                        m.path = reference;
                    }
                }
            }
            // Paste centered on the view (or cursor), keeping relative layout.
            let bounds = glide_core::Aabb::from_points(elements.iter().map(|e| e.transform.position()));
            let target = self.placement(at_screen);
            let offset = if bounds.is_empty() { DVec2::ZERO } else { target - bounds.center() };
            self.paste_elements(elements, offset);
            return;
        }
        // A path to an image file?
        let trimmed = text.trim().trim_matches('"');
        let as_path = Path::new(trimmed);
        if !trimmed.contains('\n') && is_image_path(as_path) && as_path.exists() {
            self.import_image_files(ctx, &[as_path.to_path_buf()], at_screen);
            return;
        }
        self.add_text_box(text, at_screen);
    }

    /// Plain text becomes a text element.
    fn add_text_box(&mut self, text: &str, at_screen: Option<DVec2>) {
        let pos = self.placement(at_screen);
        let mut block = TextBlock { font_size: 40.0, ..TextBlock::new(text.trim_end()) };
        if text.chars().count() > 80 {
            block.max_width = Some(900.0);
        }
        let el = Element::new(ElementKind::Text(block)).with_transform(Transform {
            x: pos.x,
            y: pos.y,
            scale: 1.0 / self.camera.zoom,
            rotation: self.camera.rotation.to_degrees(),
        });
        let id = self.doc.add(el);
        self.selection = vec![id];
    }

    /// Pastes shapes copied in PowerPoint (or another Office app) as editable
    /// text boxes, shapes and pictures. Returns false when the clipboard holds
    /// no Office shapes, or none could be read, so the caller can fall back.
    fn paste_office_shapes(&mut self, ctx: &egui::Context, at_screen: Option<DVec2>) -> bool {
        let Some(package) = crate::office::clipboard_package() else { return false };
        let text = &mut self.renderer.text;
        let clip = match crate::office::parse_package(&package, &mut |k| glide_render::local_size(text, k)) {
            Ok(clip) if !clip.elements.is_empty() => clip,
            Ok(_) => return false,
            Err(e) => {
                log::warn!("could not read Office shapes from the clipboard: {e}");
                return false;
            }
        };
        // Store embedded pictures next to the document.
        let base = self.base_dir();
        let stored: Vec<Option<String>> = clip.media.iter().map(|(bytes, ext)| assets::import_bytes(&base, bytes, ext).ok()).collect();
        let mut elements: Vec<Element> = clip
            .elements
            .into_iter()
            .filter_map(|mut el| {
                if let ElementKind::Image(m) = &mut el.kind
                    && let Some(i) = m.path.strip_prefix(crate::office::MEDIA_PREFIX)
                {
                    m.path = stored.get(i.parse::<usize>().ok()?)?.clone()?;
                }
                Some(el)
            })
            .collect();
        // Keep the Office layout, sized like PowerPoint at 100% (never more
        // than ~70% of the view) and turned to match the view.
        let bounds = elements.iter().fold(glide_core::Aabb::EMPTY, |b, e| b.union(e.world_aabb(glide_render::local_size(&mut self.renderer.text, &e.kind))));
        let vp = self.viewport();
        let size = bounds.size().max(DVec2::splat(1.0));
        let k = ((vp.x * 0.7) / size.x).min((vp.y * 0.7) / size.y).min(1.0) / self.camera.zoom;
        let target = self.placement(at_screen);
        let turn = DVec2::from_angle(self.camera.rotation);
        for el in &mut elements {
            let p = target + turn.rotate((el.transform.position() - bounds.center()) * k);
            el.transform = Transform { x: p.x, y: p.y, scale: el.transform.scale * k, rotation: el.transform.rotation + self.camera.rotation.to_degrees() };
        }
        let n = elements.len();
        self.paste_elements(elements, DVec2::ZERO);
        self.tool = Tool::Select;
        self.notify(ctx, format!("Pasted {n} object{} from Office", if n == 1 { "" } else { "s" }));
        true
    }

    /// Ctrl/⌘+Shift+V: the clipboard's text as a plain text box, ignoring any
    /// picture that came with it.
    fn paste_plain_text(&mut self, ctx: &egui::Context, text: Option<String>, at_screen: Option<DVec2>) {
        let text = text.or_else(|| arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()));
        match text.filter(|t| !t.trim().is_empty()) {
            Some(t) => self.add_text_box(&t, at_screen),
            None => self.notify(ctx, "There is no text on the clipboard"),
        }
    }

    /// Reads the OS clipboard directly: Office shapes, images, copied files, then text.
    pub fn paste_from_clipboard(&mut self, ctx: &egui::Context, at_screen: Option<DVec2>) {
        if self.paste_office_shapes(ctx, at_screen) {
            return;
        }
        let Ok(mut cb) = arboard::Clipboard::new() else {
            self.notify(ctx, "The clipboard is not available");
            return;
        };
        if let Ok(img) = cb.get_image() {
            let (w, h) = (img.width as u32, img.height as u32);
            match self.import_rgba(w, h, img.bytes.into_owned(), at_screen) {
                Ok(id) => self.selection = vec![id],
                Err(e) => self.notify(ctx, format!("Could not paste image: {e}")),
            }
            return;
        }
        if let Ok(files) = cb.get().file_list() {
            let videos: Vec<PathBuf> = files.iter().filter(|f| is_video_path(f)).cloned().collect();
            let images: Vec<PathBuf> = files.into_iter().filter(|f| is_image_path(f)).collect();
            if !images.is_empty() || !videos.is_empty() {
                self.import_image_files(ctx, &images, at_screen);
                self.import_video_files(ctx, &videos, at_screen);
                return;
            }
        }
        if let Ok(text) = cb.get_text()
            && !text.is_empty()
        {
            self.paste_text(ctx, &text, at_screen);
            return;
        }
        self.notify(ctx, "Nothing to paste");
    }

    /// egui reports Ctrl/⌘+V as a `Paste` event only when the clipboard holds
    /// text. Images and copied files arrive as a bare key release, which is
    /// picked up here and routed through the native clipboard.
    pub fn handle_paste_events(&mut self, ctx: &egui::Context) -> Vec<Action> {
        if ctx.text_edit_focused() {
            return Vec::new();
        }
        let pointer = ctx.pointer_hover_pos().filter(|p| self.canvas_rect.contains(*p)).map(|p| crate::canvas::dvec(p - self.canvas_rect.min));
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Paste(text) => {
                    self.paste_consumed = true;
                    if ctx.input(|i| i.modifiers.command && i.modifiers.shift) {
                        self.paste_plain_text(ctx, Some(text), pointer);
                        continue;
                    }
                    if self.paste_office_shapes(ctx, pointer) {
                        continue;
                    }
                    // Prefer a real image if the clipboard has one (e.g. copied from a browser).
                    // Tests never consult the machine's real clipboard.
                    let has_image = !cfg!(test) && arboard::Clipboard::new().is_ok_and(|mut c| c.get_image().is_ok());
                    if has_image {
                        self.paste_from_clipboard(ctx, pointer);
                    } else {
                        self.paste_text(ctx, &text, pointer);
                    }
                }
                egui::Event::Key { key: egui::Key::V, pressed: false, modifiers, .. } => {
                    if std::mem::take(&mut self.paste_consumed) {
                        continue;
                    }
                    if modifiers.command && modifiers.shift {
                        self.paste_plain_text(ctx, None, pointer);
                    } else if modifiers.command {
                        self.paste_from_clipboard(ctx, pointer);
                    }
                }
                _ => {}
            }
        }
        Vec::new()
    }

    /// Files dropped onto the window: images are imported at the drop point,
    /// a GlideDeck document is opened.
    pub fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let paths: Vec<PathBuf> =
            dropped.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect();
        if let Some(doc) = paths.iter().find(|p| {
            let name = p.to_string_lossy().to_lowercase();
            name.ends_with(".json") || name.ends_with(".pptx")
        }) {
            let doc = doc.clone();
            if self.confirm_discard() {
                self.open_any(ctx, &doc);
            }
            return;
        }
        let videos: Vec<PathBuf> = paths.iter().filter(|p| is_video_path(p)).cloned().collect();
        let images: Vec<PathBuf> = paths.into_iter().filter(|p| is_image_path(p)).collect();
        if images.is_empty() && videos.is_empty() {
            return;
        }
        let at = ctx
            .pointer_latest_pos()
            .filter(|p| self.canvas_rect.contains(*p))
            .map(|p| crate::canvas::dvec(p - self.canvas_rect.min));
        self.import_image_files(ctx, &images, at);
        self.import_video_files(ctx, &videos, at);
    }
}
