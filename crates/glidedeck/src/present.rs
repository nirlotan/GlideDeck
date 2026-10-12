//! Presentation mode: fullscreen, chrome-free, the camera travelling along the
//! path with each step's transition. Invisible frames never render here.

use crate::app::GlideDeckApp;
use crate::theme::{self, icon};
use eframe::egui::{self, Color32, Key, RichText, Sense};
use glam::DVec2;
use glide_core::{Camera, CameraTween};

pub struct Presenting {
    pub step: usize,
    editor_camera: Camera,
    last_viewport: DVec2,
    last_activity: f64,
    was_fullscreen: bool,
}

impl GlideDeckApp {
    pub fn start_presenting(&mut self, ctx: &egui::Context, start: usize) {
        if self.doc.path.is_empty() {
            self.notify(ctx, "Add a frame first — every frame on the path is a slide");
            return;
        }
        self.stop_text_edit();
        let step = start.min(self.doc.path.len() - 1);
        let was_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        self.presenting = Some(Presenting {
            step,
            editor_camera: self.camera,
            last_viewport: DVec2::ZERO,
            last_activity: ctx.input(|i| i.time),
            was_fullscreen,
        });
        self.nav.stop();
        self.drag = crate::canvas::Drag::None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
    }

    pub fn stop_presenting(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.presenting.take() {
            self.current_step = Some(p.step);
            self.camera = p.editor_camera;
            self.nav.stop();
            if !p.was_fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
            if let Some(f) = self.doc.step_frame(p.step) {
                self.selection = vec![f.id.clone()];
            }
        }
    }

    /// Plays the autoplay videos inside the current step's frame and pauses the rest.
    fn autoplay_step_videos(&mut self) {
        let Some(step) = self.presenting.as_ref().map(|p| p.step) else { return };
        let Some(frame) = self.doc.step_frame(step).cloned() else { return };
        let size = frame.kind.intrinsic_size().unwrap_or_default();
        let inv = frame.transform.affine().inverse();
        let videos: Vec<(glide_core::DVec2, glide_core::Media)> = self
            .doc
            .elements
            .iter()
            .filter_map(|e| match &e.kind {
                glide_core::ElementKind::Video(m) if !e.hidden => Some((e.transform.position(), m.clone())),
                _ => None,
            })
            .collect();
        for (pos, m) in videos {
            let l = inv.transform_point2(pos);
            let inside = l.x.abs() <= size.x / 2.0 && l.y.abs() <= size.y / 2.0;
            self.renderer.video_set_playing(&m.path, inside && m.autoplay, m.looping, m.muted);
        }
    }

    fn step_camera(&self, index: usize) -> Option<Camera> {
        Camera::for_frame(self.doc.step_frame(index)?, self.viewport(), 0.0)
    }

    /// Moves the show to `index`, animating with the transition of the later
    /// of the two steps (so going back replays the same move in reverse).
    fn present_go(&mut self, index: usize) {
        let Some(p) = &self.presenting else { return };
        let from = p.step;
        let index = index.min(self.doc.path.len().saturating_sub(1));
        let Some(target) = self.step_camera(index) else { return };
        let t = self.doc.path[index.max(from)].transition;
        self.nav.tween = Some(CameraTween::new(self.camera, target, t.duration, t.easing).with_style(t.style, self.viewport()));
        self.nav.clear_pending();
        if let Some(p) = &mut self.presenting {
            p.step = index;
        }
    }
}

pub fn present_ui(app: &mut GlideDeckApp, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
    let ctx = ui.ctx().clone();
    let bg = app.doc.background;
    let fill = Color32::from_rgb(bg.r(), bg.g(), bg.b());
    egui::CentralPanel::no_frame().frame(egui::Frame::new().fill(fill)).show(ui, |ui| {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
        app.canvas_rect = rect;
        let viewport = app.viewport();
        let now = ctx.input(|i| i.time);
        let Some(state) = &mut app.presenting else { return };
        let step = state.step;

        // Snap to the current frame on entry and whenever the window resizes.
        if (state.last_viewport - viewport).length() > 0.5 {
            let first = state.last_viewport == DVec2::ZERO;
            state.last_viewport = viewport;
            if !app.nav.is_animating() || first {
                if let Some(cam) = app.step_camera(step) {
                    if first {
                        // Glide in from the editor view.
                        let t = app.doc.path[step].transition;
                        app.nav.tween = Some(
                            CameraTween::new(app.camera, cam, t.duration.min(1.2), t.easing).with_style(t.style, viewport),
                        );
                    } else {
                        app.camera = cam;
                    }
                }
            }
        }

        // Navigation keys (also what presentation clickers send).
        let (mut next, mut prev, mut first, mut last, mut exit) = (false, false, false, false, false);
        ctx.input(|i| {
            for k in [Key::ArrowRight, Key::ArrowDown, Key::Space, Key::PageDown, Key::Enter, Key::N] {
                next |= i.key_pressed(k);
            }
            for k in [Key::ArrowLeft, Key::ArrowUp, Key::PageUp, Key::Backspace, Key::P] {
                prev |= i.key_pressed(k);
            }
            first = i.key_pressed(Key::Home);
            last = i.key_pressed(Key::End);
            exit = i.key_pressed(Key::Escape);
        });
        next |= response.clicked();
        prev |= response.secondary_clicked();

        // Free exploration: the wheel still zooms; the next step returns to the path.
        if let Some(p) = response.hover_pos() {
            let anchor = crate::canvas::dvec(p - rect.min);
            for ev in ctx.input(|i| i.events.clone()) {
                match ev {
                    egui::Event::MouseWheel { delta, unit, .. } => {
                        let scale = if unit == egui::MouseWheelUnit::Line { 40.0 } else { 1.0 };
                        app.nav.zoom(anchor, delta.y as f64 * scale * 0.004);
                    }
                    egui::Event::Zoom(f) => app.camera.zoom_about(anchor, f as f64, viewport),
                    _ => {}
                }
            }
        }
        if ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO || i.pointer.any_pressed()) {
            if let Some(s) = &mut app.presenting {
                s.last_activity = now;
            }
        }

        // Clicking a video plays/pauses it instead of advancing.
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let world = app.camera.screen_to_world(crate::canvas::dvec(p - rect.min), viewport);
            let hit = app.doc.elements.iter().rev().find(|e| {
                matches!(e.kind, glide_core::ElementKind::Video(_)) && !e.hidden && e.hit_test(world, e.kind.intrinsic_size().unwrap_or_default(), 0.0)
            });
            if let Some(v) = hit.map(|e| e.id.clone()) {
                app.toggle_video(&v);
                next = false;
            }
        }

        let n = app.doc.path.len();
        if exit {
            app.renderer.pause_all_videos();
            app.stop_presenting(&ctx);
            return;
        }
        if next && step + 1 < n {
            app.present_go(step + 1);
        } else if prev && step > 0 {
            app.present_go(step - 1);
        } else if first {
            app.present_go(0);
        } else if last {
            app.present_go(n - 1);
        } else if next && step + 1 >= n && app.step_camera(step).is_some_and(|c| c != app.camera) && !app.nav.is_animating() {
            // At the end, "next" returns to the last frame after free roaming.
            app.present_go(step);
        }

        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1) as f64;
        let was_moving = app.nav.is_animating();
        if app.nav.tick(&mut app.camera, dt, viewport) {
            ctx.request_repaint();
        }
        // Arriving at a slide: start its autoplay videos, pause everything else.
        if was_moving && !app.nav.is_animating() {
            app.autoplay_step_videos();
        }
        if let Some(tex) = app.render_canvas(ui, frame, rect, false) {
            ui.painter().image(tex, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }

        hud(app, ui, rect, now);
    });
}

/// Bottom-center control pill that fades away when the mouse is idle.
fn hud(app: &mut GlideDeckApp, ui: &mut egui::Ui, rect: egui::Rect, now: f64) {
    let Some(state) = &app.presenting else { return };
    let idle = now - state.last_activity;
    let alpha = ((2.5 - idle) / 0.4).clamp(0.0, 1.0) as f32;
    if alpha <= 0.0 {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        return;
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    let step = state.step;
    let n = app.doc.path.len();
    let mut go: Option<isize> = None;
    let mut exit = false;
    egui::Area::new(egui::Id::new("present-hud"))
        .fixed_pos(rect.center_bottom() - egui::vec2(0.0, 28.0))
        .pivot(egui::Align2::CENTER_BOTTOM)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            ui.set_opacity(alpha);
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(18, 18, 22, 210))
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let b = |ui: &mut egui::Ui, g: &str| {
                            ui.add(egui::Button::new(RichText::new(g).size(16.0).color(Color32::WHITE)).frame(false).min_size(egui::vec2(30.0, 28.0)))
                        };
                        if b(ui, icon::CARET_LEFT).clicked() {
                            go = Some(-1);
                        }
                        ui.label(RichText::new(format!("{} / {}", step + 1, n)).font(theme::semibold(13.0)).color(Color32::WHITE));
                        if b(ui, icon::CARET_RIGHT).clicked() {
                            go = Some(1);
                        }
                        ui.add_space(6.0);
                        if b(ui, icon::X).on_hover_text("End show (Esc)").clicked() {
                            exit = true;
                        }
                    });
                });
        });
    if exit {
        let ctx = ui.ctx().clone();
        app.stop_presenting(&ctx);
    } else if let Some(d) = go {
        let target = (step as isize + d).clamp(0, n as isize - 1) as usize;
        if target != step {
            app.present_go(target);
        }
    }
}
