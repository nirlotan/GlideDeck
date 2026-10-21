//! The speaker notes panel under the canvas, as in PowerPoint: notes for the
//! current slide, shown to the presenter in presenter view.

use crate::app::GlideDeckApp;
use crate::theme::{self, icon};
use eframe::egui::{self, RichText};

/// The notes panel (or, when collapsed, a slim bar that opens it).
pub fn notes_panel(app: &mut GlideDeckApp, ui: &mut egui::Ui) {
    let p = theme::palette(ui.ctx());
    let frame = egui::Frame::new().fill(p.surface).inner_margin(egui::Margin { left: 12, right: 12, top: 6, bottom: 8 });
    if !app.show_notes {
        egui::Panel::bottom("notes-bar").frame(frame).show_separator_line(false).show(ui, |ui| {
            let r = ui.add(egui::Button::new(RichText::new(format!("{}  Speaker notes  {}", icon::NOTE, icon::CARET_UP)).size(12.5).color(p.text_muted)).frame(false));
            if r.on_hover_text("Show the notes for the current slide").clicked() {
                app.show_notes = true;
            }
        });
        return;
    }
    let step = app.current_step.filter(|i| *i < app.doc.path.len());
    egui::Panel::bottom("notes")
        .frame(frame)
        .show_separator_line(false)
        .resizable(true)
        .default_size(132.0)
        .size_range(84.0..=420.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}  Speaker notes", icon::NOTE)).font(theme::semibold(13.0)));
                if let Some(i) = step {
                    let name = app.doc.step_frame(i).map(|f| f.display_name()).unwrap_or_default();
                    ui.label(RichText::new(format!("·  Slide {}  {name}", i + 1)).size(12.5).color(p.text_muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new(icon::CARET_DOWN).color(p.text_muted)).frame(false)).on_hover_text("Hide speaker notes").clicked() {
                        app.show_notes = false;
                    }
                    ui.label(RichText::new("Only you see these, in presenter view (Alt+F5) · # heading, - bullet, **bold**").size(11.5).color(p.text_muted));
                });
            });
            ui.add_space(2.0);
            match step {
                Some(i) => {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.add_sized(
                            ui.available_size(),
                            egui::TextEdit::multiline(&mut app.doc.path[i].notes)
                                .id_salt("speaker-notes")
                                .hint_text("Click to add notes for this slide: what to say, numbers to mention, a reminder to pause…")
                                .desired_width(f32::INFINITY),
                        );
                    });
                }
                None => {
                    ui.label(RichText::new("Pick a slide in the Slides list to write its notes.").color(p.text_muted));
                }
            }
        });
}
