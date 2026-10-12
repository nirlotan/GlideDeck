//! The interactive canvas: navigation, selection, direct manipulation,
//! drawing new objects, and screen-space overlays on top of the GPU scene.

use crate::app::{format_zoom, Action, Tool, GlideDeckApp};
use crate::theme::{self, icon};
use eframe::egui::{self, Color32, CursorIcon, PointerButton, Pos2, Sense, Shape, StrokeKind};
use glam::DVec2;
use glide_core::{
    Aabb, Camera, CameraTween, Element, ElementKind, EllipseShape, Frame, RectShape, Transform,
};

/// Natural-log zoom per scrolled point; one wheel notch (~40 pt) ≈ 1.17x.
const ZOOM_PER_POINT: f64 = 0.004;
/// Radians of view rotation per scrolled point.
const ROTATE_PER_POINT: f64 = 0.003;
/// Exponential smoothing rate for wheel zoom/rotation (1/s).
const SMOOTHING: f64 = 20.0;
/// Drags shorter than this create a default-sized object instead.
const MIN_DRAW: f32 = 6.0;
/// Smart-guide snapping distance in screen points.
const SNAP: f64 = 6.0;

/// An in-progress move with smart-guide snapping.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveState {
    pub ids: Vec<String>,
    origins: Vec<DVec2>,
    /// Accumulated pointer travel in screen points.
    total: DVec2,
    /// Screen bounds of the moving set when the drag began.
    bbox: Aabb,
    /// Snap targets (screen x / y of other objects' edges and centres).
    xs: Vec<f64>,
    ys: Vec<f64>,
}

pub fn dvec(v: egui::Vec2) -> DVec2 {
    DVec2::new(v.x as f64, v.y as f64)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Drag {
    #[default]
    None,
    Pan,
    /// Moving the selection plus the contents of selected frames.
    Move(MoveState),
    Marquee {
        start: Pos2,
        additive: bool,
    },
    /// Drawing a new object with a creation tool.
    Create {
        start: Pos2,
        tool: Tool,
    },
    /// Uniform resize from a corner handle; the opposite corner stays put.
    Resize {
        id: String,
        anchor: DVec2,
        diag: DVec2,
        scale: f64,
    },
    /// Rotating with the knob above the element.
    Rotate {
        id: String,
        center: Pos2,
        start_angle: f64,
        start_rotation: f64,
    },
}

/// Distance (screen points) within which a handle is grabbed.
const HANDLE_REACH: f32 = 9.0;
/// How far the rotation knob sits above the top edge.
const KNOB_OFFSET: f32 = 26.0;

/// Smoothed camera motion: wheel input accumulates into pending zoom/rotation
/// that is applied over a few frames about a fixed anchor, and scripted moves
/// run as tweens.
#[derive(Default)]
pub struct Navigation {
    pending_log_zoom: f64,
    pending_rotation: f64,
    anchor: DVec2,
    pub tween: Option<CameraTween>,
}

impl Navigation {
    pub fn zoom(&mut self, anchor: DVec2, log_amount: f64) {
        self.tween = None;
        self.anchor = anchor;
        self.pending_log_zoom += log_amount;
    }

    pub fn rotate(&mut self, anchor: DVec2, radians: f64) {
        self.tween = None;
        self.anchor = anchor;
        self.pending_rotation += radians;
    }

    pub fn clear_pending(&mut self) {
        self.pending_log_zoom = 0.0;
        self.pending_rotation = 0.0;
    }

    pub fn stop(&mut self) {
        self.tween = None;
        self.clear_pending();
    }

    pub fn is_animating(&self) -> bool {
        self.tween.is_some() || self.pending_log_zoom != 0.0 || self.pending_rotation != 0.0
    }

    /// Advances animations. Returns true while anything is still moving.
    pub fn tick(&mut self, cam: &mut Camera, dt: f64, viewport: DVec2) -> bool {
        if let Some(tw) = &mut self.tween {
            *cam = tw.step(dt);
            if tw.finished() {
                self.tween = None;
            }
            return true;
        }
        let k = 1.0 - (-dt * SMOOTHING).exp();
        let mut moving = false;
        if self.pending_log_zoom != 0.0 {
            let step = if self.pending_log_zoom.abs() < 1e-3 { self.pending_log_zoom } else { self.pending_log_zoom * k };
            cam.zoom_about(self.anchor, step.exp(), viewport);
            self.pending_log_zoom -= step;
            moving = true;
        }
        if self.pending_rotation != 0.0 {
            let step =
                if self.pending_rotation.abs() < 1e-4 { self.pending_rotation } else { self.pending_rotation * k };
            cam.rotate_about(self.anchor, step, viewport);
            self.pending_rotation -= step;
            moving = true;
        }
        moving
    }
}

impl GlideDeckApp {
    pub fn canvas_ui(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) -> Vec<Action> {
        let mut actions = Vec::new();
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.canvas_rect = rect;
        let viewport = self.viewport();
        if self.pending_fit && rect.width() > 1.0 && rect.height() > 1.0 {
            self.fit_all(false);
            self.pending_fit = false;
        }

        self.handle_input(ui, &response, rect, &mut actions);
        let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1) as f64;
        if self.nav.tick(&mut self.camera, dt, viewport) {
            ui.ctx().request_repaint();
        }

        let p = theme::palette(ui.ctx());
        if let Some(tex) = self.render_canvas(ui, frame, rect, true) {
            egui::Image::new(egui::load::SizedTexture::new(tex, rect.size()))
                .corner_radius(10)
                .paint_at(ui, rect);
        }
        ui.painter().rect_stroke(rect, 10, egui::Stroke::new(1.0, p.border), StrokeKind::Inside);
        let painter = ui.painter_at(rect);
        self.paint_overlays(&painter, rect, response.hover_pos());
        self.floating_controls(ui, rect, &mut actions);
        actions
    }

    fn to_local(rect: egui::Rect, p: Pos2) -> DVec2 {
        dvec(p - rect.min)
    }

    /// Topmost element under a world-space point (frames only along their edge).
    pub fn hit(&mut self, world: DVec2) -> Option<String> {
        let tolerance = 3.0 / self.camera.zoom;
        let text = &mut self.renderer.text;
        self.doc
            .elements
            .iter()
            .rev()
            .filter(|e| !e.hidden)
            .find(|e| e.hit_test(world, glide_render::local_size(text, &e.kind), tolerance))
            .map(|e| e.id.clone())
    }

    /// Hit test including the clickable frame name tags.
    fn hit_at(&mut self, rect: egui::Rect, screen: Pos2) -> Option<String> {
        if let Some((_, id)) = self.frame_tags.iter().rev().find(|(r, _)| r.contains(screen)) {
            return Some(id.clone());
        }
        let world = self.camera.screen_to_world(Self::to_local(rect, screen), self.viewport());
        self.hit(world)
    }

    fn handle_input(&mut self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect, actions: &mut Vec<Action>) {
        let ctx = ui.ctx().clone();
        let viewport = self.viewport();
        let pointer = response.hover_pos();
        let typing = ctx.egui_wants_keyboard_input();
        let (events, modifiers, space_down, press_origin) =
            ctx.input(|i| (i.events.clone(), i.modifiers, i.key_down(egui::Key::Space), i.pointer.press_origin()));
        let space = space_down && !typing;
        let multi = modifiers.shift || modifiers.command;

        self.cursor_world = pointer.map(|p| self.camera.screen_to_world(Self::to_local(rect, p), viewport));

        // Wheel and pinch navigation.
        if let Some(p) = pointer {
            let anchor = Self::to_local(rect, p);
            for ev in &events {
                match ev {
                    egui::Event::MouseWheel { unit, delta, modifiers, .. } => {
                        let scale = match unit {
                            egui::MouseWheelUnit::Point => 1.0,
                            egui::MouseWheelUnit::Line => 40.0,
                            egui::MouseWheelUnit::Page => viewport.y,
                        };
                        let d = dvec(*delta) * scale;
                        let primary = if d.y != 0.0 { d.y } else { d.x };
                        if modifiers.alt {
                            self.nav.rotate(anchor, -primary * ROTATE_PER_POINT);
                        } else if modifiers.shift && !modifiers.command {
                            self.nav.stop();
                            self.camera.pan_by_screen(DVec2::new(primary, 0.0));
                        } else if self.scroll_pans && !modifiers.command && !modifiers.ctrl {
                            self.nav.stop();
                            self.camera.pan_by_screen(d);
                        } else {
                            self.nav.zoom(anchor, primary * ZOOM_PER_POINT);
                        }
                    }
                    egui::Event::Zoom(factor) => {
                        self.nav.stop();
                        self.camera.zoom_about(anchor, *factor as f64, viewport);
                    }
                    _ => {}
                }
            }
        }

        if self.text_edit_input(ui, response, rect) {
            self.hovered = None;
            self.drag = Drag::None;
            return;
        }

        // Drags.
        if response.drag_started() {
            let start = press_origin.or(pointer).unwrap_or(rect.center());
            let primary = response.dragged_by(PointerButton::Primary);
            self.drag = if response.dragged_by(PointerButton::Middle) || space || self.tool == Tool::Hand {
                Drag::Pan
            } else if primary && matches!(self.tool, Tool::Frame | Tool::Rect | Tool::Ellipse | Tool::Text) {
                Drag::Create { start, tool: self.tool }
            } else if primary && self.tool == Tool::Select && let Some(handle) = self.handle_at(rect, start) {
                handle
            } else if primary && self.tool == Tool::Select {
                match self.hit_at(rect, start) {
                    Some(id) => {
                        if !self.selection.contains(&id) {
                            if !multi {
                                self.selection.clear();
                            }
                            self.selection.push(id);
                        }
                        Drag::Move(self.begin_move(rect))
                    }
                    None => {
                        if !multi {
                            self.selection.clear();
                        }
                        Drag::Marquee { start, additive: multi }
                    }
                }
            } else {
                Drag::None
            };
        }
        if response.dragged() {
            let delta = dvec(response.drag_delta());
            match &self.drag {
                Drag::Pan => {
                    self.nav.stop();
                    self.camera.pan_by_screen(delta);
                }
                Drag::Move(_) => {
                    let snap = !modifiers.alt;
                    self.update_move(delta, snap);
                }
                Drag::Resize { id, anchor, diag, scale } => {
                    if let Some(p) = pointer.or(ctx.pointer_latest_pos()) {
                        let world = self.camera.screen_to_world(Self::to_local(rect, p), viewport);
                        let t = ((world - *anchor).dot(*diag) / diag.length_squared().max(1e-300)).max(0.05);
                        let (anchor, diag, scale, id) = (*anchor, *diag, *scale, id.clone());
                        if let Some(el) = self.doc.get_mut(&id) {
                            el.transform.scale = scale * t;
                            el.transform.set_position(anchor + diag * t * 0.5);
                        }
                    }
                }
                Drag::Rotate { id, center, start_angle, start_rotation } => {
                    if let Some(p) = pointer.or(ctx.pointer_latest_pos()) {
                        let v = p - *center;
                        let angle = (v.y as f64).atan2(v.x as f64);
                        let mut deg = *start_rotation + (angle - *start_angle).to_degrees();
                        if modifiers.shift {
                            deg = (deg / 15.0).round() * 15.0;
                        }
                        let deg = (deg + 180.0).rem_euclid(360.0) - 180.0;
                        let id = id.clone();
                        if let Some(el) = self.doc.get_mut(&id) {
                            el.transform.rotation = deg;
                        }
                    }
                }
                _ => {}
            }
        }
        if response.drag_stopped() {
            let end = pointer.or(ctx.pointer_latest_pos());
            match (self.drag.clone(), end) {
                (Drag::Marquee { start, additive }, Some(end)) => {
                    self.marquee_select(rect, egui::Rect::from_two_pos(start, end), additive);
                }
                (Drag::Create { start, tool }, Some(end)) => {
                    let r = egui::Rect::from_two_pos(start, end);
                    let drawn = (r.width() >= MIN_DRAW || r.height() >= MIN_DRAW).then_some(r);
                    let alt = modifiers.alt;
                    self.create(tool, rect, drawn, end, alt);
                }
                _ => {}
            }
            self.drag = Drag::None;
            self.guides.clear();
        }

        // Video play/pause buttons take clicks first.
        if response.clicked()
            && let Some(p) = pointer
            && let Some((_, id)) = self.video_buttons.iter().find(|(r, _)| r.contains(p)).cloned()
        {
            self.toggle_video(&id);
            return;
        }

        // Clicks.
        if response.clicked()
            && let Some(p) = pointer
        {
            match self.tool {
                Tool::Select => match self.hit_at(rect, p) {
                    Some(id) if multi => {
                        if let Some(i) = self.selection.iter().position(|s| *s == id) {
                            self.selection.remove(i);
                        } else {
                            self.selection.push(id);
                        }
                    }
                    Some(id) => {
                        if let Some(step) = self.steps_of(&id).first() {
                            self.current_step = Some(*step);
                        }
                        self.selection = vec![id];
                    }
                    None if !multi => self.selection.clear(),
                    None => {}
                },
                Tool::Hand => {}
                tool => self.create(tool, rect, None, p, modifiers.alt),
            }
        }
        if response.double_clicked()
            && self.tool == Tool::Select
            && let Some(p) = pointer
            && let Some(id) = self.hit_at(rect, p)
        {
            let el = self.doc.get(&id).cloned();
            self.selection = vec![id.clone()];
            match el.map(|e| e.kind) {
                Some(ElementKind::Text(_)) => {
                    let world = self.camera.screen_to_world(Self::to_local(rect, p), viewport);
                    self.start_text_edit(&id, Some(world));
                }
                Some(ElementKind::Frame(_)) => match self.steps_of(&id).first() {
                    Some(step) => actions.push(Action::GoToStep(*step)),
                    None => self.frame_element(&id),
                },
                _ => self.frame_element(&id),
            }
        }

        // Hover state and cursor.
        self.hovered = match (pointer, &self.drag) {
            (Some(p), Drag::None) if self.tool == Tool::Select => self.hit_at(rect, p),
            _ => None,
        };
        let handle_hover = match (pointer, &self.drag) {
            (Some(p), Drag::None) if self.tool == Tool::Select => self.handle_at(rect, p),
            _ => None,
        };
        if response.hovered() || self.drag != Drag::None {
            let icon = match (&self.drag, self.tool) {
                (Drag::Resize { .. }, _) => CursorIcon::Crosshair,
                (Drag::Rotate { .. }, _) => CursorIcon::Grabbing,
                _ if matches!(handle_hover, Some(Drag::Resize { .. })) => CursorIcon::ResizeNwSe,
                _ if matches!(handle_hover, Some(Drag::Rotate { .. })) => CursorIcon::Grab,
                (Drag::Pan, _) => CursorIcon::Grabbing,
                (Drag::Move(_), _) => CursorIcon::Grabbing,
                _ if space || self.tool == Tool::Hand => CursorIcon::Grab,
                (_, Tool::Rect | Tool::Ellipse | Tool::Frame) => CursorIcon::Crosshair,
                (_, Tool::Text) => CursorIcon::Text,
                _ if self.hovered.is_some() => CursorIcon::PointingHand,
                _ => CursorIcon::Default,
            };
            ctx.set_cursor_icon(icon);
        }
    }

    /// Screen positions (absolute) of the single selected element's corners
    /// and its rotation knob, when handles apply.
    fn handle_points(&mut self, rect: egui::Rect) -> Option<(String, [Pos2; 4], Pos2)> {
        if self.selection.len() != 1 || self.text_edit.is_some() {
            return None;
        }
        let el = self.doc.get(&self.selection[0])?.clone();
        if el.locked || el.hidden {
            return None;
        }
        let size = self.renderer.local_size(&el);
        let vp = self.viewport();
        let corners = el.world_corners(size).map(|c| {
            let s = self.camera.world_to_screen(c, vp);
            rect.min + egui::vec2(s.x as f32, s.y as f32)
        });
        let center = egui::pos2((corners[0].x + corners[2].x) / 2.0, (corners[0].y + corners[2].y) / 2.0);
        let top_mid = egui::pos2((corners[0].x + corners[1].x) / 2.0, (corners[0].y + corners[1].y) / 2.0);
        let up = (top_mid - center).normalized();
        let up = if up.is_finite() { up } else { egui::vec2(0.0, -1.0) };
        Some((el.id, corners, top_mid + up * KNOB_OFFSET))
    }

    /// A resize or rotate drag if `p` is on a handle of the selected element.
    fn handle_at(&mut self, rect: egui::Rect, p: Pos2) -> Option<Drag> {
        let (id, corners, knob) = self.handle_points(rect)?;
        let el = self.doc.get(&id)?.clone();
        if knob.distance(p) <= HANDLE_REACH + 2.0 {
            let center = egui::pos2((corners[0].x + corners[2].x) / 2.0, (corners[0].y + corners[2].y) / 2.0);
            let v = p - center;
            return Some(Drag::Rotate { id, center, start_angle: (v.y as f64).atan2(v.x as f64), start_rotation: el.transform.rotation });
        }
        let i = corners.iter().position(|c| c.distance(p) <= HANDLE_REACH)?;
        let size = self.renderer.local_size(&el);
        let world = el.world_corners(size);
        let anchor = world[(i + 2) % 4];
        Some(Drag::Resize { id, anchor, diag: world[i] - anchor, scale: el.transform.scale })
    }

    /// Screen-space (canvas-local) bounds of an element.
    fn screen_box(&mut self, el: &Element) -> Aabb {
        let vp = self.viewport();
        let size = glide_render::local_size(&mut self.renderer.text, &el.kind);
        Aabb::from_points(el.world_corners(size).map(|c| self.camera.world_to_screen(c, vp)))
    }

    fn begin_move(&mut self, rect: egui::Rect) -> MoveState {
        let ids: Vec<String> = self
            .with_frame_contents(&self.selection.clone())
            .into_iter()
            .filter(|id| self.doc.get(id).is_some_and(|e| !e.locked))
            .collect();
        let origins = ids.iter().filter_map(|id| self.doc.get(id)).map(|e| e.transform.position()).collect();
        let moving: Vec<Element> = ids.iter().filter_map(|id| self.doc.get(id)).cloned().collect();
        let bbox = moving.iter().fold(Aabb::EMPTY, |b, e| b.union(self.screen_box(e)));
        let view = Aabb { min: DVec2::splat(-200.0), max: dvec(rect.size()) + DVec2::splat(200.0) };
        let others: Vec<Element> = self.doc.elements.iter().filter(|e| !e.hidden && !ids.contains(&e.id)).cloned().collect();
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        for e in &others {
            let b = self.screen_box(e);
            // Skip huge (e.g. overview frames) and off-screen objects.
            if !b.intersects(&view) || b.size().max_element() > view.size().max_element() * 3.0 {
                continue;
            }
            xs.extend([b.min.x, b.center().x, b.max.x]);
            ys.extend([b.min.y, b.center().y, b.max.y]);
        }
        MoveState { ids, origins, total: DVec2::ZERO, bbox, xs, ys }
    }

    fn update_move(&mut self, delta: DVec2, snap: bool) {
        let Drag::Move(state) = &mut self.drag else { return };
        state.total += delta;
        let moved = Aabb { min: state.bbox.min + state.total, max: state.bbox.max + state.total };
        let best = |edges: [f64; 3], targets: &[f64]| -> Option<(f64, f64)> {
            let mut best: Option<(f64, f64)> = None;
            for e in edges {
                for t in targets {
                    let d = t - e;
                    if d.abs() <= SNAP && best.is_none_or(|(bd, _)| d.abs() < bd.abs()) {
                        best = Some((d, *t));
                    }
                }
            }
            best
        };
        let mut correction = DVec2::ZERO;
        self.guides.clear();
        if snap && !moved.is_empty() {
            if let Some((dx, x)) = best([moved.min.x, moved.center().x, moved.max.x], &state.xs) {
                correction.x = dx;
                self.guides.push((true, x as f32));
            }
            if let Some((dy, y)) = best([moved.min.y, moved.center().y, moved.max.y], &state.ys) {
                correction.y = dy;
                self.guides.push((false, y as f32));
            }
        }
        let offset = self.camera.screen_delta_to_world(state.total + correction);
        let pairs: Vec<(String, DVec2)> = state.ids.iter().cloned().zip(state.origins.iter().copied()).collect();
        for (id, origin) in pairs {
            if let Some(el) = self.doc.get_mut(&id) {
                el.transform.set_position(origin + offset);
            }
        }
    }

    fn marquee_select(&mut self, rect: egui::Rect, marquee: egui::Rect, additive: bool) {
        let viewport = self.viewport();
        let m = Aabb { min: Self::to_local(rect, marquee.min), max: Self::to_local(rect, marquee.max) };
        if !additive {
            self.selection.clear();
        }
        let text = &mut self.renderer.text;
        for el in self.doc.elements.iter().filter(|e| !e.hidden) {
            let size = glide_render::local_size(text, &el.kind);
            let screen = Aabb::from_points(el.world_corners(size).map(|c| self.camera.world_to_screen(c, viewport)));
            // Frames are only marquee-selected when fully enclosed, so sweeping
            // over a frame's contents doesn't grab the frame too.
            let hit = if el.kind.is_frame() { m.contains_aabb(&screen) } else { m.intersects(&screen) };
            if hit && !self.selection.contains(&el.id) {
                self.selection.push(el.id.clone());
            }
        }
    }

    /// Creates an object with a creation tool, either from a drawn screen
    /// rectangle or (on click) at a default size centered on `at`.
    ///
    /// New objects are sized in screen units and scaled by 1/zoom, and aligned
    /// with the view rotation, so they appear exactly as drawn wherever the
    /// user is on the infinite canvas.
    fn create(&mut self, tool: Tool, canvas: egui::Rect, drawn: Option<egui::Rect>, at: Pos2, free_aspect: bool) {
        let vp = self.viewport();
        let (center, mut size) = match drawn {
            Some(r) => (r.center(), dvec(r.size())),
            None => {
                let default = match tool {
                    Tool::Frame => {
                        let w = (vp.x * 0.5).min(vp.y * 0.5 * 16.0 / 9.0);
                        DVec2::new(w, w * 9.0 / 16.0)
                    }
                    Tool::Ellipse => DVec2::new(180.0, 180.0),
                    Tool::Text => DVec2::ZERO,
                    _ => DVec2::new(240.0, 150.0),
                };
                (at, default)
            }
        };
        if tool == Tool::Frame && !free_aspect {
            let w = size.x.max(size.y * 16.0 / 9.0);
            size = DVec2::new(w, w * 9.0 / 16.0);
        }
        let world = self.camera.screen_to_world(Self::to_local(canvas, center), vp);
        let transform = Transform {
            x: world.x,
            y: world.y,
            scale: 1.0 / self.camera.zoom,
            rotation: self.camera.rotation.to_degrees(),
        };
        let theme = self.theme();
        let kind = match tool {
            Tool::Frame => ElementKind::Frame(Frame { width: size.x, height: size.y, corner_radius: 0.0, fill: None, border: None }),
            Tool::Rect => ElementKind::Rect(RectShape {
                width: size.x,
                height: size.y,
                corner_radius: 14.0_f64.min(size.min_element() / 2.0),
                fill: Some(theme.accents[0]),
                gradient: None,
                stroke: None,
            }),
            Tool::Ellipse => ElementKind::Ellipse(EllipseShape {
                width: size.x,
                height: size.y,
                fill: Some(theme.accents[1]),
                gradient: None,
                stroke: None,
            }),
            Tool::Text => {
                let mut t = theme.text_style(glide_core::TextRole::Body);
                t.content = "Text".into();
                t.font_size = 40.0;
                if drawn.is_some() && size.x > 40.0 {
                    t.max_width = Some(size.x);
                }
                ElementKind::Text(t)
            }
            Tool::Select | Tool::Hand => return,
        };
        let is_text = matches!(kind, ElementKind::Text(_));
        let is_frame = kind.is_frame();
        let mut el = Element::new(kind).with_transform(transform);
        if is_frame {
            el.name = format!("Frame {}", self.doc.elements.iter().filter(|e| e.kind.is_frame()).count() + 1);
        }
        let id = self.doc.add(el);
        if is_frame {
            self.insert_step(&id);
        }
        self.selection = vec![id.clone()];
        self.tool = Tool::Select;
        if is_text {
            self.start_text_edit(&id, None);
        }
    }

    fn paint_overlays(&mut self, painter: &egui::Painter, rect: egui::Rect, pointer: Option<Pos2>) {
        let p = theme::palette(painter.ctx());
        let viewport = self.viewport();
        let cam = self.camera;
        let to_screen = |w: DVec2| {
            let s = cam.world_to_screen(w, viewport);
            rect.min + egui::vec2(s.x as f32, s.y as f32)
        };
        let outline = |el: &Element, size: DVec2| el.world_corners(size).map(to_screen);

        // Frames: invisible unless hovered/selected (or outlines are on), with a name tag.
        self.frame_tags.clear();
        let frames: Vec<Element> = self.doc.elements.iter().filter(|e| e.kind.is_frame() && !e.hidden).cloned().collect();
        for el in &frames {
            let selected = self.selection.contains(&el.id);
            let hovered = self.hovered.as_deref() == Some(el.id.as_str());
            if !(selected || hovered || self.show_frames) {
                continue;
            }
            let size = el.kind.intrinsic_size().unwrap_or_default();
            let pts = outline(el, size);
            let color = if selected { p.accent } else { p.accent.gamma_multiply(if hovered { 0.75 } else { 0.4 }) };
            if !selected {
                let mut closed = pts.to_vec();
                closed.push(pts[0]);
                painter.add(Shape::dashed_line(&closed, egui::Stroke::new(1.2, color), 6.0, 4.0));
            }
            // Name tag above the frame's top-left corner.
            let steps = self.steps_of(&el.id);
            let number = steps.first().map(|s| format!("{}  ", s + 1)).unwrap_or_default();
            let label = format!("{number}{}", el.display_name());
            let galley = painter.layout_no_wrap(label, theme::semibold(11.5), if selected { p.on_accent } else { p.text });
            let tag = egui::Rect::from_min_size(pts[0] + egui::vec2(0.0, -22.0), galley.size() + egui::vec2(14.0, 6.0));
            if rect.intersects(tag) {
                let fill = if selected { p.accent } else { p.surface.gamma_multiply(0.92) };
                painter.rect(tag, 6, fill, egui::Stroke::new(1.0, color), StrokeKind::Inside);
                painter.galley(tag.min + egui::vec2(7.0, 3.0), galley, p.text);
                self.frame_tags.push((tag, el.id.clone()));
            }
        }

        if let Some(id) = &self.hovered
            && !self.selection.contains(id)
            && let Some(el) = self.doc.get(id)
            && !el.kind.is_frame()
        {
            let size = self.renderer.local_size(el);
            painter.add(Shape::closed_line(outline(el, size).to_vec(), egui::Stroke::new(1.0, p.accent.gamma_multiply(0.7))));
        }

        let editing = self.text_edit.as_ref().map(|s| s.id.clone());
        for id in &self.selection {
            let Some(el) = self.doc.get(id) else { continue };
            let size = self.renderer.local_size(el);
            let pts = outline(el, size);
            let color = if el.locked { Color32::GRAY } else { p.accent };
            if editing.as_deref() == Some(id.as_str()) {
                let mut closed = pts.to_vec();
                closed.push(pts[0]);
                painter.add(Shape::dashed_line(&closed, egui::Stroke::new(1.2, color), 4.0, 3.0));
                continue;
            }
            painter.add(Shape::closed_line(pts.to_vec(), egui::Stroke::new(1.5, color)));
            if self.selection.len() == 1 {
                for c in pts {
                    let r = egui::Rect::from_center_size(c, egui::vec2(9.0, 9.0));
                    painter.rect(r, 2, Color32::WHITE, egui::Stroke::new(1.5, color), StrokeKind::Middle);
                }
            }
        }
        if let Some((_, corners, knob)) = self.handle_points(rect) {
            let top_mid = egui::pos2((corners[0].x + corners[1].x) / 2.0, (corners[0].y + corners[1].y) / 2.0);
            painter.line_segment([top_mid, knob], egui::Stroke::new(1.2, p.accent));
            painter.circle(knob, 5.5, Color32::WHITE, egui::Stroke::new(1.5, p.accent));
        }
        self.paint_text_edit(painter, rect);
        self.paint_video_controls(painter, rect, pointer);

        // Smart guides.
        let guide = egui::Stroke::new(1.0, egui::Color32::from_rgb(0xF4, 0x3F, 0x5E));
        for (vertical, at) in &self.guides {
            if *vertical {
                let x = rect.min.x + at;
                painter.vline(x, rect.y_range(), guide);
            } else {
                let y = rect.min.y + at;
                painter.hline(rect.x_range(), y, guide);
            }
        }

        match (&self.drag, pointer) {
            (Drag::Marquee { start, .. }, Some(end)) => {
                let r = egui::Rect::from_two_pos(*start, end);
                painter.rect(r, 2, p.accent.gamma_multiply(0.08), egui::Stroke::new(1.0, p.accent), StrokeKind::Inside);
            }
            (Drag::Create { start, tool }, Some(end)) => {
                let mut r = egui::Rect::from_two_pos(*start, end);
                if *tool == Tool::Frame && !painter.ctx().input(|i| i.modifiers.alt) {
                    let w = r.width().max(r.height() * 16.0 / 9.0);
                    r = egui::Rect::from_center_size(r.center(), egui::vec2(w, w * 9.0 / 16.0));
                }
                let stroke = egui::Stroke::new(1.5, p.accent);
                match tool {
                    Tool::Ellipse => {
                        painter.add(egui::epaint::EllipseShape {
                            center: r.center(),
                            radius: r.size() / 2.0,
                            fill: p.accent.gamma_multiply(0.12),
                            stroke,
                            angle: 0.0,
                        });
                    }
                    Tool::Frame => {
                        let pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                        painter.add(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
                    }
                    _ => {
                        painter.rect(r, 4, p.accent.gamma_multiply(0.12), stroke, StrokeKind::Inside);
                    }
                }
                let label = format!("{:.0} × {:.0}", r.width(), r.height());
                painter.text(r.right_bottom() + egui::vec2(6.0, 6.0), egui::Align2::LEFT_TOP, label, theme::semibold(11.0), p.accent);
            }
            _ => {}
        }
    }

    /// A play/pause button over each visible video that is hovered, selected or playing.
    fn paint_video_controls(&mut self, painter: &egui::Painter, rect: egui::Rect, pointer: Option<Pos2>) {
        self.video_buttons.clear();
        let vp = self.viewport();
        let videos: Vec<Element> = self.doc.elements.iter().filter(|e| !e.hidden && matches!(e.kind, ElementKind::Video(_))).cloned().collect();
        for el in videos {
            let ElementKind::Video(m) = &el.kind else { continue };
            let corners = el.world_corners(DVec2::new(m.width, m.height)).map(|c| {
                let s = self.camera.world_to_screen(c, vp);
                rect.min + egui::vec2(s.x as f32, s.y as f32)
            });
            let bounds = egui::Rect::from_points(&corners);
            if !rect.intersects(bounds) || bounds.width().min(bounds.height()) < 60.0 {
                continue;
            }
            let status = self.renderer.video_status(&m.path);
            let playing = matches!(status, glide_render::VideoStatus::Ready { playing: true, .. });
            let hovered = pointer.is_some_and(|p| bounds.contains(p));
            let center = bounds.center();
            let button = egui::Rect::from_center_size(center, egui::vec2(56.0, 56.0));
            let show = hovered || self.selection.contains(&el.id) || !playing;
            if let glide_render::VideoStatus::Unavailable(reason) = &status {
                if hovered {
                    painter.text(center, egui::Align2::CENTER_CENTER, format!("{}  {reason}", icon::WARNING), theme::semibold(12.0), Color32::WHITE);
                }
                continue;
            }
            if show {
                let over = pointer.is_some_and(|p| button.contains(p));
                painter.circle_filled(center, 28.0, Color32::from_black_alpha(if over { 190 } else { 140 }));
                let glyph = if playing { icon::PAUSE } else { icon::PLAY };
                painter.text(center + egui::vec2(if playing { 0.0 } else { 2.0 }, 0.0), egui::Align2::CENTER_CENTER, glyph, egui::FontId::new(26.0, egui::FontFamily::Name("phosphor-fill".into())), Color32::WHITE);
                if let glide_render::VideoStatus::Ready { position, duration, .. } = status
                    && duration > 0.0
                {
                    let bar = egui::Rect::from_min_max(egui::pos2(bounds.left() + 12.0, bounds.bottom() - 10.0), egui::pos2(bounds.right() - 12.0, bounds.bottom() - 7.0));
                    painter.rect_filled(bar, 2, Color32::from_white_alpha(70));
                    let w = bar.width() * (position / duration).clamp(0.0, 1.0) as f32;
                    painter.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(w, bar.height())), 2, Color32::WHITE);
                }
                self.video_buttons.push((button, el.id.clone()));
            }
        }
    }

    /// Floating zoom pill (bottom right) and tool hint (bottom center).
    fn floating_controls(&mut self, ui: &mut egui::Ui, rect: egui::Rect, actions: &mut Vec<Action>) {
        let p = theme::palette(ui.ctx());
        let pill = egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0, p.border))
            .corner_radius(10)
            .shadow(egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(30) })
            .inner_margin(egui::Margin::symmetric(4, 3));

        egui::Area::new(egui::Id::new("zoom-pill"))
            .fixed_pos(rect.right_bottom() - egui::vec2(12.0, 12.0))
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                pill.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let small = |ui: &mut egui::Ui, glyph: &str, tip: &str| {
                            ui.add(egui::Button::new(egui::RichText::new(glyph).size(15.0)).frame(false).min_size(egui::vec2(28.0, 26.0)))
                                .on_hover_text(tip)
                                .clicked()
                        };
                        if small(ui, icon::MINUS, "Zoom out") {
                            actions.push(Action::ZoomOut);
                        }
                        ui.add_sized([58.0, 26.0], egui::Label::new(egui::RichText::new(format_zoom(self.camera.zoom)).font(theme::semibold(12.5))));
                        if small(ui, icon::PLUS, "Zoom in") {
                            actions.push(Action::ZoomIn);
                        }
                        ui.separator();
                        if small(ui, icon::CORNERS_OUT, "Fit everything (Home)") {
                            actions.push(Action::FitAll);
                        }
                        if self.camera.rotation.abs() > 1e-3 && small(ui, icon::ARROW_COUNTER_CLOCKWISE, "Straighten view") {
                            let mut c = self.camera;
                            c.rotation = 0.0;
                            self.animate_to(c);
                        }
                    });
                });
            });

        let hint = match self.tool {
            Tool::Frame => Some("Drag to draw a frame (16:9 · hold Alt for free aspect) — or click for a default frame"),
            Tool::Rect | Tool::Ellipse => Some("Drag to draw · click for default size"),
            Tool::Text => Some("Click to add text · drag to set a wrap width"),
            Tool::Hand => Some("Drag to pan"),
            Tool::Select => None,
        };
        if let Some(hint) = hint {
            egui::Area::new(egui::Id::new("tool-hint"))
                .fixed_pos(rect.center_bottom() - egui::vec2(0.0, 14.0))
                .pivot(egui::Align2::CENTER_BOTTOM)
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(20, 20, 26, 220))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(12, 6))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(hint).color(Color32::WHITE).size(12.5));
                        });
                });
        }
    }
}
