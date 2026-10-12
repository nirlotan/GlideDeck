//! In-place text editing on the canvas.
//!
//! Editing goes through cosmic-text — the same engine that shapes and renders
//! the text — so right-to-left scripts (Hebrew, Arabic), mixed bidi text,
//! complex shaping, word motion and selection all behave correctly, and the
//! caret lines up exactly with the rendered glyphs at any zoom or rotation.

use crate::app::GlideDeckApp;
use crate::canvas::dvec;
use crate::theme;
use eframe::egui::{self, Event, Key, Pos2};
use glam::DVec2;
use glide_core::ElementKind;
use glide_render::{EditAction, EditState, TextMotion};

pub struct TextEditSession {
    pub id: String,
    pub state: EditState,
    caret: Option<[DVec2; 2]>,
    selection: Vec<[DVec2; 2]>,
    /// Time of the last edit or caret move (keeps the caret solid while typing).
    activity: f64,
    dragging: bool,
}

impl GlideDeckApp {
    /// Starts editing a text element. `at` is a world point to place the
    /// caret; otherwise everything is selected.
    pub fn start_text_edit(&mut self, id: &str, at: Option<DVec2>) {
        let Some(el) = self.doc.get(id) else { return };
        let ElementKind::Text(t) = &el.kind else { return };
        let state = EditState::select_all(&t.content);
        self.selection = vec![id.to_owned()];
        self.tool = crate::app::Tool::Select;
        self.text_edit = Some(TextEditSession { id: id.to_owned(), state, caret: None, selection: Vec::new(), activity: 0.0, dragging: false });
        match at {
            Some(world) => {
                let local = el.transform.affine().inverse().transform_point2(world);
                self.apply_edit(&[EditAction::Click { at: local, extend: false }], None);
            }
            None => self.apply_edit(&[], None),
        }
    }

    pub fn stop_text_edit(&mut self) {
        if let Some(session) = self.text_edit.take() {
            // Like Keynote: an emptied text box disappears.
            let empty = matches!(self.doc.get(&session.id).map(|e| &e.kind), Some(ElementKind::Text(t)) if t.content.trim().is_empty());
            if empty {
                self.doc.remove(&session.id);
                self.selection.retain(|s| *s != session.id);
            }
        }
    }

    fn apply_edit(&mut self, actions: &[EditAction], ctx: Option<&egui::Context>) {
        let Some(session) = &mut self.text_edit else { return };
        let Some(el) = self.doc.get_mut(&session.id) else {
            self.text_edit = None;
            return;
        };
        let ElementKind::Text(t) = &mut el.kind else { return };
        let result = self.renderer.text.edit(t, session.state, actions);
        if t.content != result.content {
            t.content = result.content;
        }
        session.state = result.state;
        session.caret = result.caret;
        session.selection = result.selection;
        if !actions.is_empty()
            && let Some(ctx) = ctx
        {
            session.activity = ctx.input(|i| i.time);
        }
        if let (Some(copied), Some(ctx)) = (result.copied, ctx) {
            ctx.copy_text(copied);
        }
    }

    /// Handles keyboard and mouse input while editing. Returns true when the
    /// canvas should skip its normal interaction this frame.
    pub fn text_edit_input(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) -> bool {
        let Some(session) = &self.text_edit else { return false };
        let id = session.id.clone();
        let ctx = ui.ctx().clone();
        let Some(el) = self.doc.get(&id).cloned() else {
            self.text_edit = None;
            return false;
        };
        if self.selection != [id.clone()] {
            self.stop_text_edit();
            return false;
        }

        // Keep keyboard focus on the canvas so arrows/tab aren't used for UI navigation.
        response.request_focus();
        ctx.memory_mut(|m| {
            m.set_focus_lock_filter(
                response.id,
                egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true },
            )
        });

        let mut actions = Vec::new();
        let mut exit = false;
        for ev in ctx.input(|i| i.events.clone()) {
            match ev {
                Event::Text(s) if !s.chars().all(char::is_control) => actions.push(EditAction::Insert(s)),
                Event::Ime(egui::ImeEvent::Commit(s)) if !s.is_empty() => actions.push(EditAction::Insert(s)),
                Event::Paste(s) => actions.push(EditAction::Insert(s.replace("\r\n", "\n"))),
                Event::Copy => actions.push(EditAction::Copy),
                Event::Cut => actions.push(EditAction::Cut),
                Event::Key { key, pressed: true, modifiers, .. } => {
                    let select = modifiers.shift;
                    let word = modifiers.command || modifiers.alt;
                    let mv = |motion| EditAction::Move { motion, select };
                    match key {
                        Key::Enter => actions.push(EditAction::Enter),
                        Key::Backspace => actions.push(EditAction::Backspace),
                        Key::Delete => actions.push(EditAction::Delete),
                        Key::Escape => exit = true,
                        Key::ArrowLeft => actions.push(mv(if word { TextMotion::PreviousWord } else { TextMotion::Left })),
                        Key::ArrowRight => actions.push(mv(if word { TextMotion::NextWord } else { TextMotion::Right })),
                        Key::ArrowUp => actions.push(mv(TextMotion::Up)),
                        Key::ArrowDown => actions.push(mv(TextMotion::Down)),
                        Key::Home => actions.push(mv(if modifiers.command { TextMotion::BufferStart } else { TextMotion::Home })),
                        Key::End => actions.push(mv(if modifiers.command { TextMotion::BufferEnd } else { TextMotion::End })),
                        Key::A if modifiers.command => actions.push(EditAction::SelectAll),
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Mouse: click/drag inside the text moves the caret or selects; outside ends editing.
        let (camera, vp) = (self.camera, self.viewport());
        let inverse = el.transform.affine().inverse();
        let to_local = move |p: Pos2| inverse.transform_point2(camera.screen_to_world(dvec(p - rect.min), vp));
        let (pressed, down, released, double, pos, shift) = ctx.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.pointer.button_double_clicked(egui::PointerButton::Primary),
                i.pointer.interact_pos(),
                i.modifiers.shift,
            )
        });
        if let Some(p) = pos
            && rect.contains(p)
        {
            let size = self.renderer.local_size(&el);
            let tolerance = 6.0 / self.camera.zoom;
            let inside = el.hit_test(self.camera.screen_to_world(dvec(p - rect.min), self.viewport()), size, tolerance);
            if pressed && !inside {
                self.stop_text_edit();
                return false;
            }
            if double && inside {
                actions.push(EditAction::DoubleClick { at: to_local(p) });
            } else if pressed {
                actions.push(EditAction::Click { at: to_local(p), extend: shift });
                if let Some(s) = &mut self.text_edit {
                    s.dragging = true;
                }
            } else if down && self.text_edit.as_ref().is_some_and(|s| s.dragging) && ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                actions.push(EditAction::Drag { at: to_local(p) });
            }
        }
        if released && let Some(s) = &mut self.text_edit {
            s.dragging = false;
        }

        if exit {
            self.stop_text_edit();
            return true;
        }
        if !actions.is_empty() {
            self.apply_edit(&actions, Some(&ctx));
        }
        ctx.set_cursor_icon(egui::CursorIcon::Text);
        true
    }

    /// Draws the selection, caret and editing outline, and tells the OS where
    /// the caret is (for IME candidate windows).
    pub fn paint_text_edit(&mut self, painter: &egui::Painter, rect: egui::Rect) {
        let Some(session) = &self.text_edit else { return };
        let Some(el) = self.doc.get(&session.id) else { return };
        let p = theme::palette(painter.ctx());
        let affine = el.transform.affine();
        let vp = self.viewport();
        let to_screen = |local: DVec2| {
            let s = self.camera.world_to_screen(affine.transform_point2(local), vp);
            rect.min + egui::vec2(s.x as f32, s.y as f32)
        };
        for [a, b] in &session.selection {
            let pts = vec![to_screen(*a), to_screen(DVec2::new(b.x, a.y)), to_screen(*b), to_screen(DVec2::new(a.x, b.y))];
            painter.add(egui::Shape::convex_polygon(pts, p.accent.gamma_multiply(0.28), egui::Stroke::NONE));
        }
        let now = painter.ctx().input(|i| i.time);
        let since = now - session.activity;
        let visible = since < 0.6 || (since * 1.6).fract() < 0.6;
        if let Some([top, bottom]) = session.caret {
            let (t, b) = (to_screen(top), to_screen(bottom));
            if visible {
                painter.line_segment([t, b], egui::Stroke::new(2.0, p.accent));
            }
            let caret_rect = egui::Rect::from_two_pos(t, b).expand(1.0);
            painter.ctx().output_mut(|o| {
                o.ime = Some(egui::output::IMEOutput {
                    purpose: egui::IMEPurpose::Normal,
                    rect: caret_rect.expand(40.0),
                    cursor_rect: caret_rect,
                    should_interrupt_composition: false,
                })
            });
        }
        painter.ctx().request_repaint_after(std::time::Duration::from_millis(120));
    }
}
