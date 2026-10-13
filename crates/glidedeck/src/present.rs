//! Presentation mode: fullscreen, chrome-free, the camera travelling along the
//! path with each step's transition. Invisible frames never render here.
//!
//! Two layouts:
//! - **Single screen**: the show fills the editor window.
//! - **Presenter view**: the show runs in a separate audience window (fullscreen
//!   on a second display when there is one), and the editor window becomes the
//!   presenter console: current and next slide, speaker notes, timer and clock.

use crate::app::GlideDeckApp;
use crate::theme::{self, icon};
use eframe::egui::{self, Color32, Key, RichText, Sense};
use glam::DVec2;
use glide_core::{Camera, CameraTween};

const AUDIENCE: &str = "glidedeck-audience";

pub struct Presenting {
    pub step: usize,
    editor_camera: Camera,
    last_viewport: DVec2,
    last_activity: f64,
    was_fullscreen: bool,
    /// A black or white screen hiding the show (B / W).
    pub blank: Option<Color32>,
    /// The presenter console, when presenting with presenter view.
    pub console: Option<Console>,
}

pub struct Console {
    timer_start: f64,
    paused_at: Option<f64>,
    /// How the audience window is created; kept constant so egui doesn't
    /// re-apply it every frame (later changes go through viewport commands).
    audience: egui::ViewportBuilder,
    pub audience_fullscreen: bool,
    /// The audience view is on its own display.
    pub second_display: bool,
    /// Pending re-fullscreen after moving the audience window to another display.
    refullscreen_in: u8,
    /// Frames until the audience window is brought to the front (single display).
    focus_audience_in: u8,
    notes_size: f32,
}

/// Navigation requested by keys, clicks or buttons, from any window.
#[derive(Default)]
struct Nav {
    next: bool,
    prev: bool,
    first: bool,
    last: bool,
    exit: bool,
    black: bool,
    white: bool,
}

impl Nav {
    fn from_keys(ctx: &egui::Context) -> Nav {
        let mut n = Nav::default();
        ctx.input(|i| {
            for k in [Key::ArrowRight, Key::ArrowDown, Key::Space, Key::PageDown, Key::Enter, Key::N] {
                n.next |= i.key_pressed(k);
            }
            for k in [Key::ArrowLeft, Key::ArrowUp, Key::PageUp, Key::Backspace, Key::P] {
                n.prev |= i.key_pressed(k);
            }
            n.first = i.key_pressed(Key::Home);
            n.last = i.key_pressed(Key::End);
            n.exit = i.key_pressed(Key::Escape);
            // B / period black out the screen (what clickers send); W whites it out.
            n.black = i.key_pressed(Key::B) || i.key_pressed(Key::Period);
            n.white = i.key_pressed(Key::W) || i.key_pressed(Key::Comma);
        });
        n
    }
}

impl GlideDeckApp {
    /// Starts the show at `start`. With `presenter` the presenter console is
    /// used; `None` picks it automatically when a second display is connected.
    pub fn start_presenting(&mut self, ctx: &egui::Context, start: usize, presenter: Option<bool>) {
        if self.doc.path.is_empty() {
            self.notify(ctx, "Add a frame first — every frame on the path is a slide");
            return;
        }
        self.stop_text_edit();
        let step = start.min(self.doc.path.len() - 1);
        let was_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let displays = crate::displays::list();
        let presenter = presenter.unwrap_or(displays.len() > 1);
        let now = ctx.input(|i| i.time);
        let console = presenter.then(|| {
            let (outer, native_ppp) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().native_pixels_per_point.unwrap_or(1.0)));
            let k = if cfg!(windows) { native_ppp as f64 } else { 1.0 };
            let center = outer.map(|r| (r.center().x as f64 * k, r.center().y as f64 * k)).unwrap_or_default();
            let target = crate::displays::audience_display(&displays, center);
            // Full HD-ish on a projector; smaller when it shares the screen with the console.
            let size = if target.is_some() { egui::vec2(1280.0, 720.0) } else { egui::vec2(960.0, 540.0) };
            let mut audience = egui::ViewportBuilder::default()
                .with_title("GlideDeck — Audience")
                .with_inner_size(size)
                .with_min_inner_size([320.0, 180.0])
                .with_icon(crate::app_icon());
            match target {
                Some(d) => {
                    let (x, y) = d.center_in_points(native_ppp);
                    audience = audience.with_position([x - size.x / 2.0, y - size.y / 2.0]).with_fullscreen(true);
                }
                None => {
                    let at = outer.map(|r| r.min + egui::vec2(60.0, 60.0)).unwrap_or(egui::pos2(80.0, 80.0));
                    audience = audience.with_position(at);
                }
            }
            Console {
                timer_start: now,
                paused_at: None,
                audience,
                audience_fullscreen: target.is_some(),
                second_display: target.is_some(),
                refullscreen_in: 0,
                // On a single display the new window would open behind the console.
                focus_audience_in: if target.is_some() { 0 } else { 3 },
                notes_size: 20.0,
            }
        });
        let single = console.is_none();
        self.presenting = Some(Presenting {
            step,
            editor_camera: self.camera,
            last_viewport: DVec2::ZERO,
            last_activity: now,
            was_fullscreen,
            blank: None,
            console,
        });
        self.nav.stop();
        self.drag = crate::canvas::Drag::None;
        if single {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
    }

    pub fn stop_presenting(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.presenting.take() {
            self.renderer.pause_all_videos();
            self.current_step = Some(p.step);
            self.camera = p.editor_camera;
            self.nav.stop();
            if p.console.is_none() && !p.was_fullscreen {
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
            p.blank = None;
        }
    }

    /// Applies navigation from any input source. Returns true to end the show.
    fn apply_nav(&mut self, nav: &Nav) -> bool {
        let Some(p) = &mut self.presenting else { return false };
        if nav.exit {
            return true;
        }
        if nav.black || nav.white {
            let color = if nav.black { Color32::BLACK } else { Color32::WHITE };
            p.blank = if p.blank == Some(color) { None } else { Some(color) };
            return false;
        }
        if p.blank.is_some() && (nav.next || nav.prev) {
            // The first key after a black screen just reveals the show again.
            p.blank = None;
            return false;
        }
        let step = p.step;
        let n = self.doc.path.len();
        if nav.next && step + 1 < n {
            self.present_go(step + 1);
        } else if nav.prev && step > 0 {
            self.present_go(step - 1);
        } else if nav.first {
            self.present_go(0);
        } else if nav.last {
            self.present_go(n - 1);
        } else if nav.next && step + 1 >= n && self.step_camera(step).is_some_and(|c| c != self.camera) && !self.nav.is_animating() {
            // At the end, "next" returns to the last frame after free roaming.
            self.present_go(step);
        }
        false
    }
}

/// The show itself: renders the camera's view into `ui`, handling keys
/// (when `read_keys`), clicks and the wheel from this window. Returns true to
/// end the show.
fn show_view(app: &mut GlideDeckApp, ui: &mut egui::Ui, frame: &eframe::Frame, read_keys: bool) -> bool {
    let ctx = ui.ctx().clone();
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
    app.canvas_rect = rect;
    let viewport = app.viewport();
    let now = ctx.input(|i| i.time);
    let Some(state) = &mut app.presenting else { return true };
    let step = state.step;

    // Snap to the current frame on entry and whenever the window resizes.
    if (state.last_viewport - viewport).length() > 0.5 {
        let first = state.last_viewport == DVec2::ZERO;
        state.last_viewport = viewport;
        if (!app.nav.is_animating() || first)
            && let Some(cam) = app.step_camera(step)
        {
            if first {
                // Glide in from the editor view.
                let t = app.doc.path[step].transition;
                app.nav.tween = Some(CameraTween::new(app.camera, cam, t.duration.min(1.2), t.easing).with_style(t.style, viewport));
            } else {
                app.camera = cam;
            }
        }
    }

    let mut nav = if read_keys { Nav::from_keys(&ctx) } else { Nav::default() };
    nav.next |= response.clicked();
    nav.prev |= response.secondary_clicked();

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
    if ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO || i.pointer.any_pressed())
        && let Some(s) = &mut app.presenting
    {
        s.last_activity = now;
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
            nav.next = false;
        }
    }
    if app.apply_nav(&nav) {
        return true;
    }

    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1) as f64;
    let was_moving = app.nav.is_animating();
    if app.nav.tick(&mut app.camera, dt, viewport) {
        ctx.request_repaint();
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
    // Arriving at a slide: start its autoplay videos, pause everything else.
    if was_moving && !app.nav.is_animating() {
        app.autoplay_step_videos();
    }
    if let Some(tex) = app.render_canvas(ui, frame, rect, false) {
        ui.painter().image(tex, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }
    if let Some(color) = app.presenting.as_ref().and_then(|p| p.blank) {
        ui.painter().rect_filled(rect, 0, color);
    }
    false
}

pub fn present_ui(app: &mut GlideDeckApp, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
    if app.presenting.as_ref().is_some_and(|p| p.console.is_some()) {
        presenter_ui(app, ui, frame);
        return;
    }
    let ctx = ui.ctx().clone();
    let bg = app.doc.background;
    let fill = Color32::from_rgb(bg.r(), bg.g(), bg.b());
    let mut exit = false;
    egui::CentralPanel::no_frame().frame(egui::Frame::new().fill(fill)).show(ui, |ui| {
        let rect = ui.max_rect();
        exit = show_view(app, ui, frame, true);
        if !exit {
            let now = ctx.input(|i| i.time);
            hud(app, ui, rect, now);
        }
    });
    if exit {
        app.stop_presenting(&ctx);
    }
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

// --- presenter view -------------------------------------------------------------------

/// Console colors: always dark, so the laptop screen doesn't light up the room.
mod ink {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x10, 0x12, 0x17);
    pub const PANEL: Color32 = Color32::from_rgb(0x19, 0x1C, 0x24);
    pub const LINE: Color32 = Color32::from_rgb(0x2A, 0x2E, 0x3A);
    pub const TEXT: Color32 = Color32::from_rgb(0xEC, 0xEE, 0xF4);
    pub const MUTED: Color32 = Color32::from_rgb(0x8E, 0x95, 0xA8);
    pub const ACCENT: Color32 = Color32::from_rgb(0x8B, 0x7C, 0xFF);
    pub const WARN: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
}

fn presenter_ui(app: &mut GlideDeckApp, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);

    // Keys pressed while the console has focus drive the show too.
    let console_nav = Nav::from_keys(&ctx);
    if app.apply_nav(&console_nav) {
        app.stop_presenting(&ctx);
        return;
    }

    // The audience window: rendered first, so the console can show the same image.
    let Some(console) = app.presenting.as_mut().and_then(|p| p.console.as_mut()) else { return };
    let builder = console.audience.clone();
    let audience_id = egui::ViewportId::from_hash_of(AUDIENCE);
    if console.focus_audience_in > 0 {
        console.focus_audience_in -= 1;
        if console.focus_audience_in == 0 {
            ctx.send_viewport_cmd_to(audience_id, egui::ViewportCommand::Focus);
        }
    }
    if console.refullscreen_in > 0 {
        console.refullscreen_in -= 1;
        if console.refullscreen_in == 0 {
            ctx.send_viewport_cmd_to(audience_id, egui::ViewportCommand::Fullscreen(true));
            console.audience_fullscreen = true;
        }
    }
    let bg = app.doc.background;
    let fill = Color32::from_rgb(bg.r(), bg.g(), bg.b());
    let mut exit = false;
    ctx.show_viewport_immediate(audience_id, builder, |ui, class| {
        if ui.input(|i| i.viewport().close_requested()) {
            exit = true;
        }
        // An embedded audience view shares the console's input, already read above.
        let own_window = class != egui::ViewportClass::EmbeddedWindow;
        egui::CentralPanel::no_frame().frame(egui::Frame::new().fill(fill)).show(ui, |ui| {
            exit |= show_view(app, ui, frame, own_window);
        });
    });
    if exit {
        app.stop_presenting(&ctx);
        return;
    }

    let Some(state) = &app.presenting else { return };
    let step = state.step;
    let n = app.doc.path.len();
    let blank = state.blank;
    let Some(console) = &state.console else { return };
    let elapsed = console.paused_at.unwrap_or(now) - console.timer_start;
    let paused = console.paused_at.is_some();
    let notes_size = console.notes_size;
    let (second_display, audience_fullscreen) = (console.second_display, console.audience_fullscreen);
    let audience_aspect = {
        let v = app.viewport();
        if v.x > 1.0 && v.y > 1.0 { (v.x / v.y) as f32 } else { 16.0 / 9.0 }
    };
    let current_tex = app.texture_id;
    let notes = app.doc.path.get(step).map(|s| s.notes.clone()).unwrap_or_default();
    let name = |app: &GlideDeckApp, i: usize| app.doc.step_frame(i).map(|f| f.display_name()).unwrap_or_default();
    let current_name = name(app, step);
    let next_name = (step + 1 < n).then(|| name(app, step + 1));

    #[derive(PartialEq)]
    enum Click {
        Prev,
        Next,
        Black,
        White,
        Pause,
        Reset,
        Smaller,
        Larger,
        Swap,
        Fullscreen,
        End,
    }
    let mut clicked: Option<Click> = None;

    egui::CentralPanel::no_frame().frame(egui::Frame::new().fill(ink::BG).inner_margin(egui::Margin::same(18))).show(ui, |ui| {
        let full = ui.available_rect_before_wrap();
        // Top bar: title, timer, clock.
        let top = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 54.0));
        let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(top).layout(egui::Layout::left_to_right(egui::Align::Center)));
        bar.label(RichText::new(format!("{}  Presenter view", icon::PRESENTATION_CHART)).font(theme::semibold(15.0)).color(ink::MUTED));
        bar.add_space(18.0);
        bar.label(RichText::new(format!("Slide {} of {n}", step + 1)).font(theme::semibold(15.0)).color(ink::TEXT));
        bar.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((h, m)) = local_clock() {
                ui.label(RichText::new(format!("{h:02}:{m:02}")).font(theme::semibold(22.0)).color(ink::MUTED));
                ui.add_space(20.0);
            }
            if console_button(ui, icon::ARROW_COUNTER_CLOCKWISE, "Reset timer").clicked() {
                clicked = Some(Click::Reset);
            }
            let (glyph, tip) = if paused { (icon::PLAY, "Resume timer") } else { (icon::PAUSE, "Pause timer") };
            if console_button(ui, glyph, tip).clicked() {
                clicked = Some(Click::Pause);
            }
            let t = elapsed.max(0.0) as u64;
            let timer = format!("{}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60);
            ui.label(RichText::new(timer).font(theme::semibold(30.0)).color(if paused { ink::WARN } else { ink::TEXT }));
        });

        // Bottom bar: controls.
        let bottom = egui::Rect::from_min_max(egui::pos2(full.min.x, full.max.y - 50.0), full.max);
        let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(bottom).layout(egui::Layout::left_to_right(egui::Align::Center)));
        bar.spacing_mut().item_spacing.x = 8.0;
        if wide_button(&mut bar, &format!("{}  Previous", icon::CARET_LEFT), step > 0).clicked() {
            clicked = Some(Click::Prev);
        }
        if wide_button(&mut bar, &format!("Next  {}", icon::CARET_RIGHT), step + 1 < n).clicked() {
            clicked = Some(Click::Next);
        }
        bar.add_space(16.0);
        let black_on = blank == Some(Color32::BLACK);
        if toggle_button(&mut bar, &format!("{}  Black (B)", icon::MOON), black_on).clicked() {
            clicked = Some(Click::Black);
        }
        let white_on = blank == Some(Color32::WHITE);
        if toggle_button(&mut bar, &format!("{}  White (W)", icon::SUN), white_on).clicked() {
            clicked = Some(Click::White);
        }
        bar.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if wide_button(ui, &format!("{}  End show (Esc)", icon::X), true).clicked() {
                clicked = Some(Click::End);
            }
            if second_display {
                if wide_button(ui, &format!("{}  Swap displays", icon::ARROWS_LEFT_RIGHT), true)
                    .on_hover_text("Move the audience view to the next display")
                    .clicked()
                {
                    clicked = Some(Click::Swap);
                }
            } else {
                let label = if audience_fullscreen { "Exit fullscreen" } else { "Audience fullscreen" };
                if toggle_button(ui, &format!("{}  {label}", icon::CORNERS_OUT), audience_fullscreen)
                    .on_hover_text("Drag the audience window to the projector first, then make it fullscreen")
                    .clicked()
                {
                    clicked = Some(Click::Fullscreen);
                }
            }
        });

        // Body: current slide on the left; next slide and notes on the right.
        let body = egui::Rect::from_min_max(egui::pos2(full.min.x, top.max.y + 8.0), egui::pos2(full.max.x, bottom.min.y - 12.0));
        let gap = 18.0;
        let left_w = (body.width() * 0.6).max(200.0);
        let left = egui::Rect::from_min_size(body.min, egui::vec2(left_w, body.height()));
        let right = egui::Rect::from_min_max(egui::pos2(left.max.x + gap, body.min.y), body.max);

        let painter = ui.painter().clone();
        let caption = |at: egui::Pos2, text: String| {
            painter.text(at, egui::Align2::LEFT_TOP, text, theme::semibold(12.5), ink::MUTED);
        };
        caption(left.min, format!("NOW  ·  {current_name}"));
        let area = egui::Rect::from_min_max(left.min + egui::vec2(0.0, 22.0), left.max);
        let shown = fit_aspect(area, audience_aspect);
        painter.rect_filled(shown, 8, fill);
        if let Some(tex) = current_tex {
            egui::Image::new(egui::load::SizedTexture::new(tex, shown.size())).corner_radius(8).paint_at(ui, shown);
        }
        if let Some(c) = blank {
            painter.rect_filled(shown, 8, c.gamma_multiply(0.92));
            let label = if c == Color32::BLACK { "Screen is black · press B to show it" } else { "Screen is white · press W to show it" };
            painter.text(shown.center(), egui::Align2::CENTER_CENTER, label, theme::semibold(16.0), if c == Color32::BLACK { ink::MUTED } else { Color32::DARK_GRAY });
        }
        painter.rect_stroke(shown, 8, egui::Stroke::new(1.0, ink::LINE), egui::StrokeKind::Outside);
        // Progress along the path.
        let progress = egui::Rect::from_min_size(egui::pos2(shown.min.x, shown.max.y + 8.0), egui::vec2(shown.width(), 4.0));
        painter.rect_filled(progress, 2, ink::LINE);
        let done = progress.width() * (step + 1) as f32 / n.max(1) as f32;
        painter.rect_filled(egui::Rect::from_min_size(progress.min, egui::vec2(done, 4.0)), 2, ink::ACCENT);

        // Next slide.
        caption(right.min, match &next_name {
            Some(name) => format!("NEXT  ·  {name}"),
            None => "NEXT".into(),
        });
        let next_area = egui::Rect::from_min_size(right.min + egui::vec2(0.0, 22.0), egui::vec2(right.width(), (right.height() * 0.42).min(right.width() * 9.0 / 16.0)));
        let next_rect = fit_aspect(next_area, 16.0 / 9.0);
        match next_name {
            Some(_) => {
                painter.rect_filled(next_rect, 8, fill);
                if let Some(cam) = app.doc.step_frame(step + 1).and_then(|f| Camera::for_frame(f, crate::canvas::dvec(next_rect.size()), 0.0))
                    && let Some(tex) = app.render_snapshot(ui, frame, "presenter-next", next_rect.size(), &cam)
                {
                    egui::Image::new(egui::load::SizedTexture::new(tex, next_rect.size())).corner_radius(8).paint_at(ui, next_rect);
                }
            }
            None => {
                painter.rect_filled(next_rect, 8, ink::PANEL);
                painter.text(next_rect.center(), egui::Align2::CENTER_CENTER, "End of presentation", theme::semibold(15.0), ink::MUTED);
            }
        }
        painter.rect_stroke(next_rect, 8, egui::Stroke::new(1.0, ink::LINE), egui::StrokeKind::Outside);

        // Notes.
        let notes_top = next_rect.max.y + 18.0;
        let notes_rect = egui::Rect::from_min_max(egui::pos2(right.min.x, notes_top), right.max);
        let mut notes_ui = ui.new_child(egui::UiBuilder::new().max_rect(notes_rect).layout(egui::Layout::top_down(egui::Align::Min)));
        notes_ui.horizontal(|ui| {
            ui.label(RichText::new("NOTES").font(theme::semibold(12.5)).color(ink::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if console_button(ui, "A+", "Larger notes").clicked() {
                    clicked = Some(Click::Larger);
                }
                if console_button(ui, "A−", "Smaller notes").clicked() {
                    clicked = Some(Click::Smaller);
                }
            });
        });
        egui::Frame::new().fill(ink::PANEL).corner_radius(8).inner_margin(egui::Margin::same(14)).show(&mut notes_ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if notes.trim().is_empty() {
                    ui.label(RichText::new("No notes for this slide. Add them in the editor: Properties → Speaker notes.").size(notes_size * 0.75).color(ink::MUTED));
                } else {
                    notes_markdown(ui, &notes, notes_size);
                }
            });
        });
    });

    // Apply console buttons.
    let audience_id = egui::ViewportId::from_hash_of(AUDIENCE);
    match clicked {
        Some(Click::Prev) => {
            app.apply_nav(&Nav { prev: true, ..Default::default() });
        }
        Some(Click::Next) => {
            app.apply_nav(&Nav { next: true, ..Default::default() });
        }
        Some(Click::Black) => {
            app.apply_nav(&Nav { black: true, ..Default::default() });
        }
        Some(Click::White) => {
            app.apply_nav(&Nav { white: true, ..Default::default() });
        }
        Some(Click::End) => {
            app.stop_presenting(&ctx);
            return;
        }
        Some(click) => {
            let native_ppp = ctx.input(|i| i.viewport().native_pixels_per_point.unwrap_or(1.0));
            let audience_rect = ctx.input_for(audience_id, |i| i.viewport().outer_rect);
            if let Some(console) = app.presenting.as_mut().and_then(|p| p.console.as_mut()) {
                match click {
                    Click::Pause => match console.paused_at.take() {
                        Some(at) => console.timer_start += now - at,
                        None => console.paused_at = Some(now),
                    },
                    Click::Reset => {
                        console.timer_start = now;
                        console.paused_at = console.paused_at.map(|_| now);
                    }
                    Click::Smaller => console.notes_size = (console.notes_size - 2.0).max(12.0),
                    Click::Larger => console.notes_size = (console.notes_size + 2.0).min(48.0),
                    Click::Fullscreen => {
                        console.audience_fullscreen = !console.audience_fullscreen;
                        ctx.send_viewport_cmd_to(audience_id, egui::ViewportCommand::Fullscreen(console.audience_fullscreen));
                    }
                    Click::Swap => {
                        // Move the audience window to the next display, then fullscreen it there.
                        let displays = crate::displays::list();
                        let k = if cfg!(windows) { native_ppp as f64 } else { 1.0 };
                        let here = audience_rect.map(|r| (r.center().x as f64 * k, r.center().y as f64 * k)).unwrap_or_default();
                        let i = displays.iter().position(|d| d.contains(here.0, here.1)).unwrap_or(0);
                        if let Some(d) = displays.get((i + 1) % displays.len().max(1)) {
                            let (x, y) = d.center_in_points(native_ppp);
                            ctx.send_viewport_cmd_to(audience_id, egui::ViewportCommand::Fullscreen(false));
                            ctx.send_viewport_cmd_to(audience_id, egui::ViewportCommand::OuterPosition(egui::pos2(x - 640.0, y - 360.0)));
                            console.refullscreen_in = 3;
                        }
                    }
                    _ => {}
                }
            }
        }
        None => {}
    }
    // The timer and clock tick.
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
}

/// The largest rect of `aspect` (width / height) centered in `area`.
fn fit_aspect(area: egui::Rect, aspect: f32) -> egui::Rect {
    let w = area.width().min(area.height() * aspect);
    egui::Rect::from_center_size(egui::pos2(area.center().x, area.min.y + w / aspect / 2.0), egui::vec2(w, w / aspect))
}

fn console_button(ui: &mut egui::Ui, glyph: &str, tip: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(glyph).size(17.0).color(ink::TEXT)).fill(ink::PANEL).corner_radius(8).min_size(egui::vec2(36.0, 32.0)))
        .on_hover_text(tip)
}

fn wide_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).font(theme::semibold(14.0)).color(ink::TEXT)).fill(ink::PANEL).corner_radius(8).min_size(egui::vec2(0.0, 38.0)),
    )
}

fn toggle_button(ui: &mut egui::Ui, label: &str, on: bool) -> egui::Response {
    let (fill, text) = if on { (ink::ACCENT, Color32::WHITE) } else { (ink::PANEL, ink::TEXT) };
    ui.add(egui::Button::new(RichText::new(label).font(theme::semibold(14.0)).color(text)).fill(fill).corner_radius(8).min_size(egui::vec2(0.0, 38.0)))
}

/// Renders speaker notes with a little Markdown: `#` headings, `-`/`*`
/// bullets, `1.` numbers and `**bold**`.
fn notes_markdown(ui: &mut egui::Ui, notes: &str, size: f32) {
    ui.spacing_mut().item_spacing.y = size * 0.35;
    for line in notes.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            ui.add_space(size * 0.3);
            continue;
        }
        let (text, scale, bold, prefix) = if let Some(h) = trimmed.strip_prefix("## ") {
            (h, 1.1, true, String::new())
        } else if let Some(h) = trimmed.strip_prefix("# ") {
            (h, 1.25, true, String::new())
        } else if let Some(b) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            (b, 1.0, false, "•  ".to_owned())
        } else if let Some((num, rest)) = trimmed.split_once(". ").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            (rest, 1.0, false, format!("{num}.  "))
        } else {
            (trimmed, 1.0, false, String::new())
        };
        let mut job = egui::text::LayoutJob::default();
        let font = |b: bool| if b { theme::semibold(size * scale) } else { egui::FontId::proportional(size * scale) };
        let fmt = |b: bool| egui::TextFormat { font_id: font(b), color: ink::TEXT, ..Default::default() };
        if !prefix.is_empty() {
            job.append(&prefix, 0.0, egui::TextFormat { color: ink::ACCENT, ..fmt(true) });
        }
        // `**bold**` spans.
        for (i, part) in text.split("**").enumerate() {
            if !part.is_empty() {
                job.append(part, 0.0, fmt(bold || i % 2 == 1));
            }
        }
        job.wrap.max_width = ui.available_width();
        ui.label(job);
    }
}

/// The local wall-clock time as (hours, minutes).
#[cfg(windows)]
fn local_clock() -> Option<(u32, u32)> {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(t: *mut SystemTime);
    }
    let mut t = SystemTime::default();
    // SAFETY: GetLocalTime fills the SYSTEMTIME we pass.
    unsafe { GetLocalTime(&mut t) };
    Some((t.hour as u32, t.minute as u32))
}

#[cfg(unix)]
fn local_clock() -> Option<(u32, u32)> {
    // `struct tm` up to the fields every Unix has, padded for platform extras.
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        _rest: [i32; 6],
        _extra: [u64; 4],
    }
    unsafe extern "C" {
        fn time(t: *mut i64) -> i64;
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let mut tm = Tm { sec: 0, min: 0, hour: 0, _rest: [0; 6], _extra: [0; 4] };
    // SAFETY: `time` accepts null; `localtime_r` writes into our buffer, which is
    // larger than any platform's `struct tm`.
    let ok = unsafe {
        let now = time(std::ptr::null_mut());
        !localtime_r(&now, &mut tm).is_null()
    };
    ok.then_some((tm.hour as u32, tm.min as u32))
}

#[cfg(not(any(windows, unix)))]
fn local_clock() -> Option<(u32, u32)> {
    None
}
