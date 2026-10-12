//! The slide sorter: the presentation path shown as a vertical strip of live
//! thumbnails, like PowerPoint's slide pane. Thumbnails are rendered by the
//! same GPU canvas renderer and refreshed when the document changes.

use crate::app::{Action, GlideDeckApp};
use crate::theme::{self, icon};
use eframe::egui::{self, Color32, RichText, Sense, StrokeKind};
use eframe::wgpu;
use glam::DVec2;
use std::collections::HashMap;
use glide_core::Camera;
use glide_render::{RenderOptions, RenderTarget};

/// Thumbnails rendered per UI frame at most (the rest follow next frame).
const RENDERS_PER_FRAME: usize = 4;

struct Thumb {
    target: RenderTarget,
    texture: egui::TextureId,
    revision: u64,
    frame_id: String,
}

#[derive(Default)]
pub struct Thumbnails {
    items: HashMap<usize, Thumb>,
    /// Step being dragged in the sorter.
    dragging: Option<usize>,
}

enum Fetch {
    /// Re-render now (within this frame's budget).
    Render,
    /// Show the existing texture; `true` if it is out of date.
    Show(egui::TextureId, bool),
    /// Not rendered yet and over budget this frame.
    Pending,
}

impl Thumbnails {
    fn fetch(&mut self, app_rev: u64, step: usize, frame_id: &str, size_px: [u32; 2], budget: &mut usize) -> Fetch {
        let item = self.items.get(&step);
        let stale = item.is_none_or(|t| t.revision != app_rev || t.frame_id != frame_id || t.target.size != size_px);
        if stale && *budget > 0 {
            *budget -= 1;
            return Fetch::Render;
        }
        match item {
            Some(t) => Fetch::Show(t.texture, stale),
            None => Fetch::Pending,
        }
    }
}

fn render_thumb(app: &mut GlideDeckApp, frame: &eframe::Frame, step: usize, size_pt: egui::Vec2, ppp: f32) {
    let Some(rs) = frame.wgpu_render_state() else { return };
    let Some(frame_el) = app.doc.step_frame(step).cloned() else { return };
    let size_px = [(size_pt.x * ppp).round().max(1.0) as u32, (size_pt.y * ppp).round().max(1.0) as u32];
    let vp = DVec2::new(size_pt.x as f64, size_pt.y as f64);
    let Some(camera) = Camera::for_frame(&frame_el, vp, 0.0) else { return };
    let revision = app.history.revision();

    let entry = app.thumbs.items.remove(&step);
    let (target, texture) = match entry {
        Some(t) if t.target.size == size_px => (t.target, t.texture),
        other => {
            let target = RenderTarget::new(&rs.device, size_px);
            let mut r = rs.renderer.write();
            let texture = match other {
                Some(old) => {
                    r.update_egui_texture_from_wgpu_texture(&rs.device, &target.unorm_view, wgpu::FilterMode::Linear, old.texture);
                    old.texture
                }
                None => r.register_native_texture(&rs.device, &target.unorm_view, wgpu::FilterMode::Linear),
            };
            (target, texture)
        }
    };
    app.renderer.render(&target, &app.doc, &camera, vp, &RenderOptions { show_grid: false, pixels_per_point: ppp as f64 });
    let loading = app.renderer.poll_images();
    // While images are still decoding, keep the thumbnail marked stale.
    let revision = if loading { revision.wrapping_sub(1) } else { revision };
    app.thumbs.items.insert(step, Thumb { target, texture, revision, frame_id: frame_el.id.clone() });
}

pub fn slides_panel(app: &mut GlideDeckApp, ui: &mut egui::Ui, frame: &eframe::Frame) -> Vec<Action> {
    let mut out = Vec::new();
    let p = theme::palette(ui.ctx());
    let ppp = ui.ctx().pixels_per_point();

    ui.horizontal(|ui| {
        ui.label(RichText::new("Slides").font(theme::semibold(15.0)));
        ui.label(RichText::new(app.doc.path.len().to_string()).size(12.0).color(p.text_muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new(RichText::new(icon::PLUS).size(15.0)).min_size(egui::vec2(28.0, 26.0)))
                .on_hover_text("New slide from the current view (Ctrl+Shift+F)")
                .clicked()
            {
                out.push(Action::FrameFromView);
            }
        });
    });
    ui.add_space(6.0);

    // Free thumbnails of steps that no longer exist.
    let n = app.doc.path.len();
    if let Some(rs) = frame.wgpu_render_state() {
        let stale: Vec<usize> = app.thumbs.items.keys().copied().filter(|k| *k >= n).collect();
        for k in stale {
            if let Some(t) = app.thumbs.items.remove(&k) {
                rs.renderer.write().free_texture(&t.texture);
            }
        }
    }

    if n == 0 {
        empty_state(ui, &mut out);
        return out;
    }

    let mut budget = RENDERS_PER_FRAME;
    let mut cards: Vec<egui::Rect> = Vec::new();
    let pointer = ui.ctx().pointer_interact_pos();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let card_w = (ui.available_width() - 26.0).max(60.0);
        let card_h = card_w * 9.0 / 16.0;
        for i in 0..n {
            let frame_el = app.doc.step_frame(i).cloned();
            let current = app.current_step == Some(i);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.allocate_ui_with_layout(egui::vec2(18.0, card_h), egui::Layout::top_down(egui::Align::Max), |ui| {
                    ui.label(RichText::new((i + 1).to_string()).font(theme::semibold(12.0)).color(if current { p.accent } else { p.text_muted }));
                });
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), Sense::click_and_drag());
                cards.push(rect);

                // Thumbnail.
                let size_px = [(card_w * ppp).round() as u32, (card_h * ppp).round() as u32];
                match &frame_el {
                    Some(f) => {
                        let rev = app.history.revision();
                        let tex = match app.thumbs.fetch(rev, i, &f.id, size_px, &mut budget) {
                            Fetch::Show(tex, stale) => {
                                if stale {
                                    ui.ctx().request_repaint();
                                }
                                Some(tex)
                            }
                            Fetch::Pending => {
                                ui.painter().rect_filled(rect, 8, p.surface_alt);
                                ui.ctx().request_repaint();
                                None
                            }
                            Fetch::Render => {
                                render_thumb(app, frame, i, egui::vec2(card_w, card_h), ppp);
                                app.thumbs.items.get(&i).map(|t| t.texture)
                            }
                        };
                        if let Some(tex) = tex {
                            egui::Image::new(egui::load::SizedTexture::new(tex, rect.size())).corner_radius(8).paint_at(ui, rect);
                        }
                    }
                    None => {
                        ui.painter().rect_filled(rect, 8, p.surface_alt);
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Missing frame", egui::FontId::proportional(12.0), p.danger);
                    }
                }
                let (stroke_w, stroke_c) = if current {
                    (2.0, p.accent)
                } else if resp.hovered() {
                    (1.5, p.accent.gamma_multiply(0.5))
                } else {
                    (1.0, p.border)
                };
                ui.painter().rect_stroke(rect, 8, egui::Stroke::new(stroke_w, stroke_c), StrokeKind::Outside);

                // Transition badge.
                if i > 0 {
                    let t = app.doc.path[i].transition;
                    let glyph = match t.style {
                        glide_core::TransitionStyle::Smooth => icon::WAVE_SINE,
                        glide_core::TransitionStyle::Direct => icon::ARROW_RIGHT,
                        glide_core::TransitionStyle::Cut => icon::LIGHTNING,
                    };
                    let badge = egui::Rect::from_min_size(rect.right_top() + egui::vec2(-26.0, 6.0), egui::vec2(20.0, 18.0));
                    ui.painter().rect_filled(badge, 5, Color32::from_black_alpha(140));
                    ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(11.0), Color32::WHITE);
                }

                if resp.clicked() {
                    out.push(Action::GoToStep(i));
                }
                if resp.double_clicked() {
                    out.push(Action::Present { from_start: false });
                }
                if resp.drag_started() {
                    app.thumbs.dragging = Some(i);
                }
                resp.on_hover_text(frame_el.as_ref().map(|f| f.display_name()).unwrap_or_default()).context_menu(|ui| {
                    if ui.button(format!("{}  Present from here", icon::PLAY)).clicked() {
                        out.push(Action::GoToStep(i));
                        out.push(Action::Present { from_start: false });
                    }
                    if ui.button(format!("{}  Duplicate slide", icon::COPY)).clicked() {
                        out.push(Action::DuplicateStep(i));
                    }
                    ui.separator();
                    if ui.add_enabled(i > 0, egui::Button::new(format!("{}  Move up", icon::ARROW_UP))).clicked() {
                        out.push(Action::MoveStep { from: i, to: i - 1 });
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new(format!("{}  Move down", icon::ARROW_DOWN))).clicked() {
                        out.push(Action::MoveStep { from: i, to: i + 1 });
                    }
                    ui.separator();
                    if ui.button(format!("{}  Remove from path", icon::X)).on_hover_text("The frame stays on the canvas").clicked() {
                        out.push(Action::RemoveStep(i));
                    }
                });
            });
            if let Some(f) = &frame_el {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.label(RichText::new(f.display_name()).size(11.5).color(p.text_muted));
                });
            }
            ui.add_space(6.0);
        }

        // Drag to reorder: show an insertion line, move on release.
        if let (Some(from), Some(ptr)) = (app.thumbs.dragging, pointer) {
            let to = cards.iter().position(|r| ptr.y < r.center().y).unwrap_or(cards.len());
            let y = if to < cards.len() { cards[to].top() - 5.0 } else { cards.last().map(|r| r.bottom() + 5.0).unwrap_or(0.0) };
            if let Some(r) = cards.first() {
                ui.painter().hline(r.x_range(), y, egui::Stroke::new(3.0, p.accent));
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if ui.input(|i| i.pointer.any_released()) {
                let target = if to > from { to - 1 } else { to };
                if target != from {
                    out.push(Action::MoveStep { from, to: target });
                }
                app.thumbs.dragging = None;
            }
        }
    });
    if !ui.input(|i| i.pointer.any_down()) {
        app.thumbs.dragging = None;
    }
    out
}

fn empty_state(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon::FRAME_CORNERS).size(40.0).color(p.accent.gamma_multiply(0.7)));
        ui.add_space(6.0);
        ui.label(RichText::new("No slides yet").font(theme::semibold(14.0)));
        ui.add_space(2.0);
        ui.label(
            RichText::new("Draw frames on the canvas to build your path. Frames are invisible in the show unless you give them a border.")
                .size(12.0)
                .color(p.text_muted),
        );
        ui.add_space(10.0);
        if ui
            .add(egui::Button::new(RichText::new(format!("{}  Frame current view", icon::PLUS)).color(p.on_accent)).fill(p.accent).corner_radius(8))
            .clicked()
        {
            out.push(Action::FrameFromView);
        }
    });
}
