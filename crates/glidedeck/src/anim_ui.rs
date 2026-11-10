//! Editing in-slide animations: the Animate tab of the side panel, its cards,
//! numbered badges on the canvas and the on-canvas preview.

use crate::anim::{self, BuildClock};
use crate::app::{Action, GlideDeckApp};
use crate::inspector::card;
use crate::theme::{self, icon};
use eframe::egui::{self, Color32, RichText};
use glide_core::{Build, BuildKind, Direction, Effect, Trigger};

/// An animation preview playing on the editor canvas.
pub struct AnimPreview {
    pub clock: BuildClock,
    pub until: f64,
}

pub fn kind_label(kind: BuildKind) -> &'static str {
    match kind {
        BuildKind::Appear => "Appear",
        BuildKind::Emphasis => "Emphasis",
        BuildKind::Disappear => "Disappear",
    }
}

pub fn kind_icon(kind: BuildKind) -> &'static str {
    match kind {
        BuildKind::Appear => icon::ARROW_SQUARE_IN,
        BuildKind::Emphasis => icon::STAR,
        BuildKind::Disappear => icon::ARROW_SQUARE_OUT,
    }
}

/// Green-ish for entrances, amber for emphasis, red for exits (as in PowerPoint).
pub fn kind_color(kind: BuildKind) -> Color32 {
    match kind {
        BuildKind::Appear => Color32::from_rgb(0x16, 0xA3, 0x4A),
        BuildKind::Emphasis => Color32::from_rgb(0xD9, 0x77, 0x06),
        BuildKind::Disappear => Color32::from_rgb(0xDC, 0x26, 0x26),
    }
}

fn effects_of(kind: BuildKind) -> &'static [Effect] {
    match kind {
        BuildKind::Appear => &Effect::APPEAR,
        BuildKind::Emphasis => &Effect::EMPHASIS,
        BuildKind::Disappear => &Effect::DISAPPEAR,
    }
}

const KINDS: [BuildKind; 3] = [BuildKind::Appear, BuildKind::Emphasis, BuildKind::Disappear];

/// The effect list of one kind, for a menu. Returns the picked effect.
fn effect_menu(ui: &mut egui::Ui, kind: BuildKind) -> Option<Effect> {
    let mut picked = None;
    ui.set_min_width(230.0);
    for &e in effects_of(kind) {
        let text = RichText::new(format!("{}  {}", kind_icon(kind), e.label()));
        if ui.button(text).on_hover_text(e.description()).clicked() {
            picked = Some(e);
            ui.close();
        }
    }
    picked
}

/// Selected objects that can be animated (not frames).
fn targets(app: &GlideDeckApp) -> Vec<String> {
    app.selection.iter().filter(|id| app.doc.get(id).is_some_and(|e| !e.kind.is_frame())).cloned().collect()
}

// --- side panel -------------------------------------------------------------------------

/// The Animate tab of the side panel.
pub fn animate_panel(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    let ids = targets(app);
    let any = !ids.is_empty();
    card(ui, "Add to the selected objects", |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let w = (ui.available_width() - 8.0) / 3.0;
            for kind in KINDS {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 52.0), if any { egui::Sense::click() } else { egui::Sense::hover() });
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, any, kind_label(kind)));
                let hovered = any && resp.hovered();
                ui.painter().rect_filled(rect, 8, if hovered { ui.visuals().widgets.hovered.bg_fill } else { p.surface });
                ui.painter().rect_stroke(rect, 8, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
                let fade = if any { 1.0 } else { 0.45 };
                ui.painter().text(rect.center_top() + egui::vec2(0.0, 18.0), egui::Align2::CENTER_CENTER, kind_icon(kind), egui::FontId::proportional(18.0), kind_color(kind).gamma_multiply(fade));
                ui.painter().text(rect.center_bottom() - egui::vec2(0.0, 13.0), egui::Align2::CENTER_CENTER, kind_label(kind), egui::FontId::proportional(12.0), p.text.gamma_multiply(fade));
                let resp = resp.on_hover_text(match kind {
                    BuildKind::Appear => "How the selected objects come in",
                    BuildKind::Emphasis => "Draw attention to the selected objects: spotlight, highlight, pulse…",
                    BuildKind::Disappear => "How the selected objects leave",
                });
                if any
                    && let Some(Some(e)) = egui::Popup::menu(&resp).show(|ui| effect_menu(ui, kind)).map(|r| r.inner)
                {
                    out.push(Action::AddAnimation(e));
                }
            }
        });
        if !any {
            ui.add_space(2.0);
            ui.label(RichText::new("Select objects on a slide to animate them.").size(11.5).color(p.text_muted));
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Start").size(11.5).color(p.text_muted));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for t in Trigger::ALL {
                if ui.add(egui::Button::selectable(app.anim_trigger == t, RichText::new(format!("{}  {}", trigger_glyph(t), t.label())).size(12.0))).clicked() {
                    app.anim_trigger = t;
                }
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Several objects").size(11.5).color(p.text_muted));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui.add(egui::Button::selectable(!app.anim_together, RichText::new(format!("{}  One by one", icon::LIST_NUMBERS)).size(12.0))).on_hover_text("Each object gets its own turn").clicked() {
                app.anim_together = false;
            }
            if ui.add(egui::Button::selectable(app.anim_together, RichText::new(format!("{}  All together", icon::SQUARES_FOUR)).size(12.0))).on_hover_text("The objects animate at the same time").clicked() {
                app.anim_together = true;
            }
        });
    });

    let step = app.current_step.filter(|i| *i < app.doc.path.len());
    let slide_builds = step.map_or(0, |i| app.doc.path[i].builds.len());
    card(ui, "This slide", |ui| {
        if crate::shell::wide_button(ui, icon::MAGIC_WAND, "Auto-animate", true, step.is_some())
            .on_hover_text("Bring this slide's content in with a tasteful cascade: titles focus, text rises, icons pop, pictures zoom, numbers count up")
            .clicked()
        {
            out.push(Action::AutoAnimate { all: false });
        }
        ui.add_space(4.0);
        if crate::shell::wide_button(ui, icon::PLAY_CIRCLE, "Preview", false, slide_builds > 0).on_hover_text("Play this slide's animations on the canvas").clicked() {
            out.push(Action::PreviewAnimations);
        }
        ui.add_space(4.0);
        let text = match slide_builds {
            0 => "No animations on this slide".to_owned(),
            1 => "1 animation on this slide".to_owned(),
            n => format!("{n} animations on this slide"),
        };
        ui.label(RichText::new(format!("{text}. Numbers on the canvas show the click order.")).size(11.5).color(p.text_muted));
    });

    // The animations of each selected object, then the slide's whole list.
    for id in ids.iter().take(3) {
        element_card(app, ui, id);
    }
    if let Some(i) = step
        && slide_builds > 0
    {
        slide_card(app, ui, i, out);
    }

    let any_builds = app.doc.path.iter().any(|s| !s.builds.is_empty());
    card(ui, "Whole deck", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            if ui.add_enabled(!app.doc.path.is_empty(), egui::Button::new(format!("{}  Auto-animate every slide", icon::SPARKLE))).clicked() {
                out.push(Action::AutoAnimate { all: true });
            }
            if ui.add_enabled(any, egui::Button::new(format!("{}  Remove from selection", icon::X))).clicked() {
                out.push(Action::RemoveSelectionAnimations);
            }
            if ui.add_enabled(slide_builds > 0, egui::Button::new(format!("{}  Clear this slide", icon::TRASH))).clicked() {
                out.push(Action::ClearAnimations { all: false });
            }
            if ui.add_enabled(any_builds, egui::Button::new(format!("{}  Clear every slide", icon::TRASH))).clicked() {
                out.push(Action::ClearAnimations { all: true });
            }
        });
    });
}

// --- actions ------------------------------------------------------------------------------

impl GlideDeckApp {
    /// Adds `effect` to the selected objects, on the slide each belongs to.
    pub fn add_animation(&mut self, ctx: &egui::Context, effect: Effect) {
        let ids = targets(self);
        if ids.is_empty() {
            self.notify(ctx, "Select objects on a slide first");
            return;
        }
        let mut first_new: Option<(usize, usize)> = None;
        for (k, id) in ids.iter().enumerate() {
            let Some(step) = anim::owner_step(&self.doc, id, self.current_step) else { continue };
            let trigger = if k > 0 && self.anim_together { Trigger::WithPrevious } else { self.anim_trigger };
            let mut b = Build::new(id.clone(), effect).with_trigger(trigger);
            if effect.has_direction() {
                b.direction = if effect == Effect::FlyOut { Direction::Right } else { Direction::Left };
            }
            let builds = &mut self.doc.path[step].builds;
            first_new.get_or_insert((step, builds.len()));
            builds.push(b);
        }
        match first_new {
            None => self.notify(ctx, "Those objects aren't on any slide of the path, so they never show in the presentation"),
            Some((step, from)) => self.preview_animations(ctx, step, from),
        }
    }

    pub fn auto_animate(&mut self, ctx: &egui::Context, all: bool) {
        let steps: Vec<usize> = if all {
            (0..self.doc.path.len()).collect()
        } else {
            match self.current_step.filter(|i| *i < self.doc.path.len()) {
                Some(i) => vec![i],
                None => {
                    self.notify(ctx, "Pick a slide first");
                    return;
                }
            }
        };
        let mut count = 0;
        for &s in &steps {
            let builds = anim::auto_builds(&self.doc, s, &mut self.renderer);
            count += builds.len();
            self.doc.path[s].builds = builds;
        }
        if count == 0 {
            self.notify(ctx, "Nothing to animate: add objects inside the slide's frame");
        } else if let [s] = steps[..] {
            self.preview_animations(ctx, s, 0);
        } else {
            self.notify(ctx, format!("Animated {count} objects on {} slides. Press F5 to see them", steps.len()));
        }
    }

    pub fn clear_animations(&mut self, all: bool) {
        for (i, s) in self.doc.path.iter_mut().enumerate() {
            if all || Some(i) == self.current_step {
                s.builds.clear();
            }
        }
        self.anim_preview = None;
    }

    pub fn remove_selection_animations(&mut self) {
        let ids = targets(self);
        for s in &mut self.doc.path {
            s.builds.retain(|b| !ids.contains(&b.element));
        }
        self.anim_preview = None;
    }

    /// Plays a slide's animations from build `from` on the editor canvas.
    pub fn preview_animations(&mut self, ctx: &egui::Context, step: usize, from: usize) {
        let Some(s) = self.doc.path.get(step) else { return };
        if s.builds.is_empty() {
            return;
        }
        if self.current_step != Some(step) {
            self.go_to_step(step);
        }
        let now = ctx.input(|i| i.time);
        let mut clock = BuildClock::fresh(&self.doc, step);
        // Builds before `from` show as already played.
        for t in clock.starts.iter_mut().take(from) {
            *t = Some(anim::DONE);
        }
        let until = clock.play_all(&self.doc, now + 0.25);
        self.anim_preview = Some(AnimPreview { clock, until });
        ctx.request_repaint();
    }

    /// The animated document while a preview plays (None otherwise).
    pub fn preview_scene(&mut self, ctx: &egui::Context) -> Option<glide_core::Document> {
        let now = ctx.input(|i| i.time);
        let p = self.anim_preview.as_ref()?;
        if now > p.until || p.clock.step >= self.doc.path.len() {
            self.anim_preview = None;
            return None;
        }
        ctx.request_repaint();
        Some(anim::scene(&self.doc, &p.clock, now, &mut self.renderer))
    }
}

// --- properties ---------------------------------------------------------------------------

fn trigger_glyph(t: Trigger) -> &'static str {
    match t {
        Trigger::OnClick => icon::CURSOR_CLICK,
        Trigger::WithPrevious => icon::STACK,
        Trigger::AfterPrevious => icon::TIMER,
    }
}

/// Editable settings of one build. Returns true if it should be removed.
fn build_editor(ui: &mut egui::Ui, b: &mut Build, salt: (usize, usize)) -> bool {
    let p = theme::palette(ui.ctx());
    let mut remove = false;
    let kind = b.effect.kind();
    ui.horizontal(|ui| {
        ui.label(RichText::new(kind_icon(kind)).size(15.0).color(kind_color(kind)));
        egui::ComboBox::from_id_salt(("build-effect", salt)).selected_text(b.effect.label()).width(130.0).show_ui(ui, |ui| {
            for k in KINDS {
                ui.label(RichText::new(kind_label(k)).size(11.0).color(p.text_muted));
                for &e in effects_of(k) {
                    if ui.selectable_label(b.effect == e, format!("{}  {}", kind_icon(k), e.label())).on_hover_text(e.description()).clicked() {
                        if b.duration == b.effect.default_duration() {
                            b.duration = e.default_duration();
                        }
                        b.effect = e;
                    }
                }
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            remove = ui.add(egui::Button::new(RichText::new(icon::X).color(p.text_muted)).frame(false)).on_hover_text("Remove this animation").clicked();
        });
    });
    egui::Grid::new(("build-grid", salt)).num_columns(2).spacing([10.0, 5.0]).min_col_width(64.0).show(ui, |ui| {
        ui.label(RichText::new("Start").color(p.text_muted));
        egui::ComboBox::from_id_salt(("build-trigger", salt)).selected_text(format!("{}  {}", trigger_glyph(b.trigger), b.trigger.label())).show_ui(ui, |ui| {
            for t in Trigger::ALL {
                ui.selectable_value(&mut b.trigger, t, format!("{}  {}", trigger_glyph(t), t.label()));
            }
        });
        ui.end_row();
        ui.label(RichText::new("Timing").color(p.text_muted));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut b.duration).speed(0.02).range(0.1..=10.0).suffix(" s").max_decimals(2)).on_hover_text("Duration");
            ui.add(egui::DragValue::new(&mut b.delay).speed(0.02).range(0.0..=30.0).prefix("delay ").suffix(" s").max_decimals(2)).on_hover_text("Wait before starting");
        });
        ui.end_row();
        if b.effect.has_direction() {
            ui.label(RichText::new(if b.effect == Effect::FlyOut { "Toward" } else { "From" }).color(p.text_muted));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for d in Direction::ALL {
                    ui.selectable_value(&mut b.direction, d, d.label());
                }
            });
            ui.end_row();
        }
    });
    remove
}

/// The "Animation" card for one selected object.
pub fn element_card(app: &mut GlideDeckApp, ui: &mut egui::Ui, id: &str) {
    let p = theme::palette(ui.ctx());
    let mut preview: Option<(usize, usize)> = None;
    let name = app.doc.get(id).map(|e| e.display_name()).unwrap_or_default();
    let title: String = name.chars().take(26).collect();
    card(ui, &title, |ui| {
        let mut remove: Option<(usize, usize)> = None;
        let mut any = false;
        let n_steps = app.doc.path.len();
        for si in 0..n_steps {
            for bi in 0..app.doc.path[si].builds.len() {
                if app.doc.path[si].builds[bi].element != id {
                    continue;
                }
                any = true;
                let order = glide_core::build::click_groups(&app.doc.path[si].builds).iter().position(|g| g.contains(&bi)).unwrap_or(0) + 1;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Slide {} · click {order}", si + 1)).size(11.0).color(p.text_muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new(format!("{}  Preview", icon::PLAY)).size(11.0)).frame(false)).clicked() {
                            preview = Some((si, bi));
                        }
                    });
                });
                if build_editor(ui, &mut app.doc.path[si].builds[bi], (si, bi)) {
                    remove = Some((si, bi));
                }
                ui.separator();
            }
        }
        if let Some((si, bi)) = remove {
            app.doc.path[si].builds.remove(bi);
        }
        if !any {
            ui.label(RichText::new("No animation yet. Add one above: make it appear, stand out or leave.").color(p.text_muted));
        }
    });
    if let Some((si, bi)) = preview {
        let ctx = ui.ctx().clone();
        app.preview_animations(&ctx, si, bi);
    }
}

/// The animations of a slide, in order, for the slide's frame.
pub fn slide_card(app: &mut GlideDeckApp, ui: &mut egui::Ui, step: usize, actions: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    let Some(s) = app.doc.path.get(step) else { return };
    let count = s.builds.len();
    let mut select: Option<String> = None;
    let mut swap: Option<(usize, usize)> = None;
    let mut remove: Option<usize> = None;
    card(ui, &format!("Slide {} animations", step + 1), |ui| {
        if count == 0 {
            ui.label(RichText::new("Nothing animates on this slide yet.").color(p.text_muted));
        }
        let builds = app.doc.path[step].builds.clone();
        let groups = glide_core::build::click_groups(&builds);
        for (bi, b) in builds.iter().enumerate() {
            let order = groups.iter().position(|g| g.contains(&bi)).unwrap_or(0) + 1;
            let name = app.doc.get(&b.element).map(|e| e.display_name()).unwrap_or_else(|| "(deleted)".into());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let first_of_group = groups.iter().any(|g| g.start == bi);
                ui.label(RichText::new(if first_of_group { format!("{order}") } else { "  ".into() }).font(theme::semibold(12.0)).color(p.accent));
                ui.label(RichText::new(trigger_glyph(b.trigger)).size(12.0).color(p.text_muted)).on_hover_text(b.trigger.label());
                ui.label(RichText::new(kind_icon(b.effect.kind())).size(13.0).color(kind_color(b.effect.kind())));
                let mut short: String = name.chars().take(18).collect();
                if name.chars().count() > 18 {
                    short.push('…');
                }
                if ui.add(egui::Button::new(format!("{} · {short}", b.effect.label())).frame(false)).on_hover_text(format!("{} · select {name}", b.effect.description())).clicked() {
                    select = Some(b.element.clone());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let tiny = |ui: &mut egui::Ui, g: &str, tip: &str, on: bool| ui.add_enabled(on, egui::Button::new(RichText::new(g).size(12.0)).frame(false)).on_hover_text(tip).clicked();
                    if tiny(ui, icon::X, "Remove", true) {
                        remove = Some(bi);
                    }
                    if tiny(ui, icon::ARROW_DOWN, "Later", bi + 1 < count) {
                        swap = Some((bi, bi + 1));
                    }
                    if tiny(ui, icon::ARROW_UP, "Earlier", bi > 0) {
                        swap = Some((bi - 1, bi));
                    }
                });
            });
        }
        if count == 0 && ui.button(format!("{}  Open Animate", icon::MAGIC_WAND)).on_hover_text("Add animations in the Animate tab").clicked() {
            actions.push(Action::SetSideTab(crate::shell::SideTab::Animate));
        }
    });
    let builds = &mut app.doc.path[step].builds;
    if let Some((a, b)) = swap {
        builds.swap(a, b);
    }
    if let Some(i) = remove {
        builds.remove(i);
    }
    if let Some(id) = select {
        app.selection = vec![id];
    }
}

// --- canvas ---------------------------------------------------------------------------------

/// Numbered badges (click order) on the current slide's animated objects,
/// shown while the Animate tab is open.
pub fn paint_badges(app: &mut GlideDeckApp, painter: &egui::Painter, rect: egui::Rect) {
    if app.side_tab != crate::shell::SideTab::Animate || !app.show_inspector || app.anim_preview.is_some() {
        return;
    }
    let Some(step) = app.current_step.filter(|i| *i < app.doc.path.len()) else { return };
    let builds = app.doc.path[step].builds.clone();
    let groups = glide_core::build::click_groups(&builds);
    let viewport = app.viewport();
    let cam = app.camera;
    let mut per_element: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (bi, b) in builds.iter().enumerate() {
        let Some(el) = app.doc.get(&b.element).cloned() else { continue };
        let size = app.renderer.local_size(&el);
        let corners = el.world_corners(size).map(|w| {
            let s = cam.world_to_screen(w, viewport);
            rect.min + egui::vec2(s.x as f32, s.y as f32)
        });
        let min = corners.iter().fold(egui::pos2(f32::INFINITY, f32::INFINITY), |m, c| m.min(*c));
        let slot = per_element.entry(b.element.clone()).or_insert(0);
        let order = groups.iter().position(|g| g.contains(&bi)).unwrap_or(0) + 1;
        let center = min + egui::vec2(-4.0 + *slot as f32 * 20.0, -4.0);
        *slot += 1;
        if !rect.expand(10.0).contains(center) {
            continue;
        }
        let color = kind_color(b.effect.kind());
        painter.rect_filled(egui::Rect::from_center_size(center, egui::vec2(18.0, 18.0)), 5, color);
        painter.text(center, egui::Align2::CENTER_CENTER, order.to_string(), theme::semibold(11.0), Color32::WHITE);
    }
}
