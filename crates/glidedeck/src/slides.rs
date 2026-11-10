//! The path strip: the presentation path shown under the canvas as a row of
//! live thumbnails, with the camera move between each pair of stops.
//! Thumbnails are rendered by the same GPU canvas renderer and refreshed when
//! the document changes.

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

/// Thumbnail size on the path strip.
const THUMB: egui::Vec2 = egui::vec2(128.0, 72.0);
/// Width of the camera-move chip between two stops.
const CHIP_W: f32 = 78.0;

/// The path strip under the canvas: every stop as a thumbnail, with the
/// camera move into it shown between stops.
pub fn path_strip(app: &mut GlideDeckApp, ui: &mut egui::Ui, frame: &eframe::Frame) -> Vec<Action> {
    let mut out = Vec::new();
    let p = theme::palette(ui.ctx());
    let ppp = ui.ctx().pixels_per_point();
    let n = app.doc.path.len();

    // Free thumbnails of steps that no longer exist.
    if let Some(rs) = frame.wgpu_render_state() {
        let stale: Vec<usize> = app.thumbs.items.keys().copied().filter(|k| *k >= n).collect();
        for k in stale {
            if let Some(t) = app.thumbs.items.remove(&k) {
                rs.renderer.write().free_texture(&t.texture);
            }
        }
    }

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        // Heading column.
        ui.allocate_ui_with_layout(egui::vec2(92.0, THUMB.y + 20.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
            ui.label(RichText::new("Path").font(theme::semibold(14.0)));
            let stops = match (n, app.current_step) {
                (0, _) => "No stops yet".to_owned(),
                (_, Some(i)) => format!("Stop {} of {n}", i + 1),
                (1, None) => "1 stop".to_owned(),
                (_, None) => format!("{n} stops"),
            };
            ui.label(RichText::new(stops).size(11.5).color(p.text_muted));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if crate::shell::icon_button(ui, icon::PLUS, "New slide", false, true, 30.0).on_hover_text("New slide from the current view (Ctrl+Shift+F)").clicked() {
                    out.push(Action::FrameFromView);
                }
                if crate::shell::icon_button(ui, icon::PATH, "Arrange", app.arrange_dialog.is_some(), n > 0, 30.0)
                    .on_hover_text("Arrange slides: spiral, orbits, tree, zoom tunnel, carousel…")
                    .clicked()
                {
                    out.push(Action::ShowArrange);
                }
            });
        });
        let (line, _) = ui.allocate_exact_size(egui::vec2(13.0, THUMB.y + 20.0), Sense::hover());
        ui.painter().vline(line.center().x, line.y_range(), egui::Stroke::new(1.0, p.border));

        if n == 0 {
            empty_state(ui, &mut out);
            return;
        }

        let mut budget = RENDERS_PER_FRAME;
        let mut cards: Vec<egui::Rect> = Vec::new();
        let pointer = ui.ctx().pointer_interact_pos();
        egui::ScrollArea::horizontal().auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for i in 0..n {
                    let frame_el = app.doc.step_frame(i).cloned();
                    let current = app.current_step == Some(i);

                    // The camera move into this stop.
                    if i > 0 {
                        let t = app.doc.path[i].transition;
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(CHIP_W, THUMB.y), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Move into slide {}: {}", i + 1, t.style.label())));
                        let c = rect.center();
                        ui.painter().hline(rect.x_range().shrink(3.0), c.y - 15.0, egui::Stroke::new(1.0, p.border));
                        let fg = if resp.hovered() || current { p.accent } else { p.text_muted };
                        let dot = egui::Rect::from_center_size(egui::pos2(c.x, c.y - 15.0), egui::vec2(22.0, 22.0));
                        ui.painter().rect_filled(dot, 11, if resp.hovered() { p.accent_soft } else { p.surface });
                        ui.painter().rect_stroke(dot, 11, egui::Stroke::new(1.0, if resp.hovered() { p.accent } else { p.border }), StrokeKind::Inside);
                        ui.painter().text(dot.center(), egui::Align2::CENTER_CENTER, theme::transition_icon(t.style), egui::FontId::proportional(12.0), fg);
                        ui.painter().text(egui::pos2(c.x, c.y + 6.0), egui::Align2::CENTER_CENTER, t.style.label(), egui::FontId::proportional(10.5), fg);
                        ui.painter().text(egui::pos2(c.x, c.y + 20.0), egui::Align2::CENTER_CENTER, format!("{:.1} s", t.duration), egui::FontId::proportional(10.0), p.text_muted);
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(format!("{} · change it in the Slide tab", t.style.description())).clicked() {
                            out.push(Action::GoToStep(i));
                            out.push(Action::SetSideTab(crate::shell::SideTab::Slide));
                        }
                    }

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        let (rect, resp) = ui.allocate_exact_size(THUMB, Sense::click_and_drag());
                        let name = frame_el.as_ref().map(|f| f.display_name()).unwrap_or_else(|| "Missing frame".into());
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Slide {}: {name}", i + 1)));
                        cards.push(rect);

                        // Thumbnail.
                        let size_px = [(THUMB.x * ppp).round() as u32, (THUMB.y * ppp).round() as u32];
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
                                        ui.painter().rect_filled(rect, 7, p.surface_alt);
                                        ui.ctx().request_repaint();
                                        None
                                    }
                                    Fetch::Render => {
                                        render_thumb(app, frame, i, THUMB, ppp);
                                        app.thumbs.items.get(&i).map(|t| t.texture)
                                    }
                                };
                                if let Some(tex) = tex {
                                    egui::Image::new(egui::load::SizedTexture::new(tex, rect.size())).corner_radius(7).paint_at(ui, rect);
                                }
                            }
                            None => {
                                ui.painter().rect_filled(rect, 7, p.surface_alt);
                                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Missing frame", egui::FontId::proportional(11.5), p.danger);
                            }
                        }
                        let (stroke_w, stroke_c) = if current {
                            (2.0, p.accent)
                        } else if resp.hovered() {
                            (1.5, p.accent.gamma_multiply(0.5))
                        } else {
                            (1.0, p.border)
                        };
                        ui.painter().rect_stroke(rect, 7, egui::Stroke::new(stroke_w, stroke_c), StrokeKind::Outside);

                        // Animation badge: how many objects animate on the slide.
                        let builds = app.doc.path[i].builds.len();
                        if builds > 0 {
                            let text = format!("{} {builds}", icon::STAR);
                            let w = 20.0 + 7.0 * builds.to_string().len() as f32;
                            let badge = egui::Rect::from_min_size(rect.right_bottom() + egui::vec2(-w - 5.0, -22.0), egui::vec2(w, 17.0));
                            ui.painter().rect_filled(badge, 5, Color32::from_black_alpha(150));
                            ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(10.5), Color32::WHITE);
                        }

                        // Number and name under the thumbnail.
                        let (caption, _) = ui.allocate_exact_size(egui::vec2(THUMB.x, 15.0), Sense::hover());
                        let number = (i + 1).to_string();
                        let color = if current { p.accent } else { p.text_muted };
                        let galley = ui.painter().layout_no_wrap(number.clone(), theme::semibold(11.5), color);
                        let number_w = galley.size().x;
                        ui.painter().galley(caption.left_top(), galley, color);
                        let mut short: String = name.chars().take(19).collect();
                        if name.chars().count() > 19 {
                            short.push('…');
                        }
                        ui.painter().text(caption.left_top() + egui::vec2(number_w + 6.0, 0.0), egui::Align2::LEFT_TOP, short, egui::FontId::proportional(11.5), if current { p.text } else { p.text_muted });

                        if resp.clicked() {
                            out.push(Action::GoToStep(i));
                        }
                        if resp.double_clicked() {
                            out.push(Action::Present { from_start: false });
                        }
                        if resp.drag_started() {
                            app.thumbs.dragging = Some(i);
                        }
                        resp.on_hover_text(name).context_menu(|ui| {
                            if ui.button(format!("{}  Present from here", icon::PLAY)).clicked() {
                                out.push(Action::GoToStep(i));
                                out.push(Action::Present { from_start: false });
                                ui.close();
                            }
                            if ui.button(format!("{}  Duplicate slide", icon::COPY)).clicked() {
                                out.push(Action::DuplicateStep(i));
                                ui.close();
                            }
                            ui.separator();
                            if ui.add_enabled(i > 0, egui::Button::new(format!("{}  Move earlier", icon::ARROW_LEFT))).clicked() {
                                out.push(Action::MoveStep { from: i, to: i - 1 });
                                ui.close();
                            }
                            if ui.add_enabled(i + 1 < n, egui::Button::new(format!("{}  Move later", icon::ARROW_RIGHT))).clicked() {
                                out.push(Action::MoveStep { from: i, to: i + 1 });
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(format!("{}  Remove from path", icon::X)).on_hover_text("The frame stays on the canvas").clicked() {
                                out.push(Action::RemoveStep(i));
                                ui.close();
                            }
                        });
                    });
                }
                ui.add_space(10.0);

                // Drag to reorder: show an insertion line, move on release.
                if let (Some(from), Some(ptr)) = (app.thumbs.dragging, pointer) {
                    let to = cards.iter().position(|r| ptr.x < r.center().x).unwrap_or(cards.len());
                    let x = if to < cards.len() { cards[to].left() - CHIP_W * 0.5 } else { cards.last().map(|r| r.right() + 6.0).unwrap_or(0.0) };
                    if let Some(r) = cards.first() {
                        ui.painter().vline(x.max(r.left() - 4.0), r.y_range(), egui::Stroke::new(3.0, p.accent));
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
        });
    });
    if !ui.input(|i| i.pointer.any_down()) {
        app.thumbs.dragging = None;
    }
    out
}

fn empty_state(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    ui.add_space(8.0);
    ui.label(RichText::new(icon::FRAME_CORNERS).size(34.0).color(p.accent.gamma_multiply(0.7)));
    ui.add_space(10.0);
    ui.vertical(|ui| {
        ui.label(RichText::new("No slides yet").font(theme::semibold(14.0)));
        ui.label(RichText::new("Draw frames on the canvas to build your path. Frames are invisible in the show unless you give them a border.").size(12.0).color(p.text_muted));
        ui.add_space(4.0);
        if ui
            .add(egui::Button::new(RichText::new(format!("{}  Frame current view", icon::PLUS)).color(p.on_accent)).fill(p.accent).corner_radius(8))
            .clicked()
        {
            out.push(Action::FrameFromView);
        }
    });
}
