//! Choosing fonts: every typeface installed on this computer (plus fonts the
//! document carries), each name drawn in its own face, with search.

use crate::theme::{self, icon};
use eframe::egui::{self, Color32, RichText};
use glide_core::{Color, ElementKind, TextBlock};
use glide_render::TextEngine;
use std::collections::HashMap;

/// What the picker asked for, besides changing the family in place.
#[derive(Default)]
pub struct Picked {
    pub changed: bool,
    /// "Add font file…" was clicked.
    pub add_file: bool,
}

/// A tessellated sample of a family's name, drawn in that family.
struct Preview {
    vertices: Vec<[f32; 2]>,
    indices: Vec<u32>,
    size: egui::Vec2,
}

thread_local! {
    static PREVIEWS: std::cell::RefCell<HashMap<String, Option<Preview>>> = Default::default();
    static FAMILIES: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Forgets cached family lists and previews (after loading a font file).
pub fn invalidate() {
    FAMILIES.with(|f| *f.borrow_mut() = None);
    PREVIEWS.with(|p| p.borrow_mut().clear());
}

fn families(text: &TextEngine) -> Vec<String> {
    FAMILIES.with(|f| f.borrow_mut().get_or_insert_with(|| text.families()).clone())
}

/// Families used by text in the document, most used first.
pub fn used_families(doc: &glide_core::Document) -> Vec<String> {
    let mut count: HashMap<String, usize> = HashMap::new();
    for e in &doc.elements {
        if let ElementKind::Text(t) = &e.kind {
            *count.entry(t.font_family.clone()).or_default() += 1;
        }
    }
    let mut v: Vec<(String, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.into_iter().map(|(f, _)| f).collect()
}

const PREVIEW_PX: f64 = 16.0;

fn preview(text: &mut TextEngine, family: &str) -> Option<Preview> {
    let block = TextBlock { content: family.to_owned(), font_family: family.to_owned(), font_size: PREVIEW_PX, color: Color::WHITE, ..TextBlock::new("") };
    let kind = ElementKind::Text(block);
    let size = glide_render::local_size(text, &kind);
    if size.x <= 0.0 {
        return None;
    }
    let mesh = glide_render::tessellate(text, &kind, 0.05);
    if mesh.indices.is_empty() {
        return None;
    }
    // Local space is centred on the origin; shift to a top-left origin.
    let vertices = mesh.vertices.iter().map(|v| [v.pos[0] + size.x as f32 / 2.0, v.pos[1] + size.y as f32 / 2.0]).collect();
    Some(Preview { vertices, indices: mesh.indices, size: egui::vec2(size.x as f32, size.y as f32) })
}

/// Draws `family` in its own typeface at `pos` (left-centre), clipped to `width`.
fn paint_sample(ui: &egui::Ui, text: &mut TextEngine, family: &str, rect: egui::Rect, color: Color32) {
    let painted = PREVIEWS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.entry(family.to_owned()).or_insert_with(|| preview(text, family));
        let Some(p) = entry else { return false };
        let origin = egui::pos2(rect.left(), rect.center().y - p.size.y / 2.0);
        let mut mesh = egui::Mesh::default();
        for v in &p.vertices {
            mesh.colored_vertex(origin + egui::vec2(v[0], v[1]), color);
        }
        mesh.indices = p.indices.clone();
        ui.painter().with_clip_rect(rect).add(egui::Shape::mesh(mesh));
        true
    });
    if !painted {
        ui.painter().text(rect.left_center(), egui::Align2::LEFT_CENTER, family, egui::FontId::proportional(13.0), color);
    }
}

/// A button showing the current family that opens a searchable list of every
/// available font. Changes `family` in place.
pub fn font_picker(ui: &mut egui::Ui, text: &mut TextEngine, id: &str, family: &mut String, used: &[String], width: f32) -> Picked {
    let p = theme::palette(ui.ctx());
    let mut out = Picked::default();
    let button = egui::Button::new(RichText::new(format!("{}  {}", family, icon::CARET_DOWN)).size(12.5)).min_size(egui::vec2(width, 0.0)).truncate();
    let response = ui.add(button).on_hover_text("Choose from every font on this computer");
    let query_id = egui::Id::new((id, "font-query"));
    egui::Popup::menu(&response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_width(300.0);
        let mut query: String = ui.data_mut(|d| d.get_temp(query_id).unwrap_or_default());
        let search = ui.add(egui::TextEdit::singleline(&mut query).hint_text(format!("{}  Search fonts", icon::MAGNIFYING_GLASS)).desired_width(ui.available_width()));
        if !search.has_focus() && query.is_empty() {
            search.request_focus();
        }
        ui.data_mut(|d| d.insert_temp(query_id, query.clone()));
        let q = query.trim().to_lowercase();
        let all = families(text);
        let mut rows: Vec<(Option<&str>, String)> = Vec::new();
        let matches = |f: &str| q.is_empty() || f.to_lowercase().contains(&q);
        let section = |title: &'static str, items: Vec<String>, rows: &mut Vec<(Option<&str>, String)>| {
            let items: Vec<String> = items.into_iter().filter(|f| matches(f)).collect();
            if !items.is_empty() {
                rows.push((Some(title), String::new()));
                rows.extend(items.into_iter().map(|f| (None, f)));
            }
        };
        section("Theme", vec!["sans-serif".into(), "serif".into(), "monospace".into()], &mut rows);
        section("In this presentation", used.iter().filter(|f| !matches!(f.as_str(), "sans-serif" | "serif" | "monospace")).cloned().collect(), &mut rows);
        let count = all.len();
        section("On this computer", all, &mut rows);
        ui.add_space(2.0);
        let row_h = 26.0;
        egui::ScrollArea::vertical().max_height(340.0).auto_shrink([false, true]).show_rows(ui, row_h, rows.len(), |ui, range| {
            for (title, f) in &rows[range] {
                if let Some(t) = title {
                    let label = if *t == "On this computer" { format!("{t} ({count})") } else { t.to_string() };
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), row_h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(RichText::new(label.to_uppercase()).size(10.5).color(p.text_muted));
                    });
                    continue;
                }
                let selected = family == f;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, format!("Font {f}")));
                if selected || resp.hovered() {
                    ui.painter().rect_filled(rect, 5, if selected { p.accent_soft } else { ui.visuals().widgets.hovered.bg_fill });
                }
                let color = if selected { p.accent } else { p.text };
                if matches!(f.as_str(), "sans-serif" | "serif" | "monospace") {
                    // Generic families: say which installed font they resolve to.
                    let desc = format!("{f}  ·  {}", text.resolve_family(f));
                    ui.painter().text(rect.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, desc, egui::FontId::proportional(12.5), color);
                } else {
                    paint_sample(ui, text, f, rect.shrink2(egui::vec2(8.0, 0.0)), color);
                }
                if resp.clicked() {
                    *family = f.clone();
                    out.changed = true;
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui.button(format!("{}  Add font file…", icon::PLUS)).on_hover_text("Use a .ttf or .otf font that isn't installed. It's copied into the presentation, so it travels with it.").clicked() {
            out.add_file = true;
            ui.close();
        }
    });
    out
}

/// Asks for a font file, copies it into the document's assets and loads it.
/// Returns the first family it provides.
pub fn add_font_file(app: &mut crate::app::GlideDeckApp, ctx: &egui::Context) -> Option<String> {
    let file = rfd::FileDialog::new().add_filter("Fonts", &["ttf", "otf", "ttc", "otc"]).pick_file()?;
    let reference = match glide_core::assets::import_file(&app.base_dir(), &file) {
        Ok(r) => r,
        Err(e) => {
            app.notify(ctx, format!("Could not copy the font: {e}"));
            return None;
        }
    };
    match app.renderer.load_document_font(&reference) {
        Ok(names) => {
            if !app.doc.fonts.contains(&reference) {
                app.doc.fonts.push(reference);
            }
            invalidate();
            let first = names.into_iter().next();
            if let Some(f) = &first {
                app.notify(ctx, format!("Added the font {f}; it travels with this presentation"));
            }
            first
        }
        Err(e) => {
            app.notify(ctx, format!("Could not use that font: {e}"));
            None
        }
    }
}
