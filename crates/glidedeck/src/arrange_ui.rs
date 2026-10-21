//! The "Arrange slides" dialog: pick a layout and path from live diagrams
//! drawn for the document's own number of slides, tune it, apply.

use crate::app::GlideDeckApp;
use crate::arrange::{self, ArrangeOptions, Arrangement};
use crate::theme::{self, icon};
use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use glam::DVec2;

pub struct ArrangeDialog {
    pub kind: Arrangement,
    pub opts: ArrangeOptions,
}

impl Default for ArrangeDialog {
    fn default() -> Self {
        let kind = Arrangement::Spiral;
        ArrangeDialog { kind, opts: ArrangeOptions { tilt: kind.default_tilt(), ..Default::default() } }
    }
}

/// Draws an arrangement's slides and path, fitted into `rect`.
fn diagram(painter: &egui::Painter, rect: egui::Rect, kind: Arrangement, n: usize, cell: DVec2, opts: &ArrangeOptions, (accent, ink): (Color32, Color32)) {
    let places = arrange::placements(kind, n, cell, opts);
    if places.is_empty() {
        return;
    }
    let corners = |p: &arrange::Placement| {
        let r = DVec2::from_angle(p.rotation.to_radians());
        let h = cell * p.scale / 2.0;
        [DVec2::new(-h.x, -h.y), DVec2::new(h.x, -h.y), DVec2::new(h.x, h.y), DVec2::new(-h.x, h.y)].map(|c| p.center + r.rotate(c))
    };
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for c in places.iter().flat_map(corners) {
        lo = lo.min(c);
        hi = hi.max(c);
    }
    let area = rect.shrink(8.0);
    let span = (hi - lo).max(DVec2::splat(1e-9));
    let k = (area.width() as f64 / span.x).min(area.height() as f64 / span.y);
    let mid = (lo + hi) / 2.0;
    let to_screen = |p: DVec2| {
        let v = (p - mid) * k;
        area.center() + egui::vec2(v.x as f32, v.y as f32)
    };
    // The path first, so slides sit on top of it.
    if kind != Arrangement::Tunnel {
        let pts: Vec<egui::Pos2> = if kind == Arrangement::Tree {
            Vec::new()
        } else {
            places.iter().map(|p| to_screen(p.center)).collect()
        };
        if pts.len() > 1 {
            painter.add(egui::Shape::line(pts, Stroke::new(1.0, accent.gamma_multiply(0.55))));
        }
        if kind == Arrangement::Tree {
            for (a, b) in arrange::tree_edges(n) {
                painter.line_segment([to_screen(places[a].center), to_screen(places[b].center)], Stroke::new(1.0, accent.gamma_multiply(0.55)));
            }
        }
    }
    for (i, p) in places.iter().enumerate() {
        let pts: Vec<egui::Pos2> = corners(p).iter().map(|c| to_screen(*c)).collect();
        let first = i == 0;
        // In the tunnel, slides nest inside the first one: keep it see-through.
        let fill = match (kind, first) {
            (Arrangement::Tunnel, true) => accent.gamma_multiply(0.08),
            (Arrangement::Tunnel, false) => accent.gamma_multiply(0.5),
            (_, true) => accent,
            _ => accent.gamma_multiply(0.22),
        };
        let stroke = if first { Stroke::new(1.5, accent) } else { Stroke::new(0.8, ink.gamma_multiply(0.55)) };
        painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
    }
}

/// Shows the dialog while it is open. Applying rearranges the document.
pub fn arrange_window(app: &mut GlideDeckApp, ctx: &egui::Context) {
    if app.arrange_dialog.is_none() {
        return;
    }
    let p = theme::palette(ctx);
    let (slides, _) = arrange::path_frames(&app.doc);
    let n = slides.len();
    // The diagram uses the deck's own slide shape (16:9 when there is none).
    let cell = slides
        .iter()
        .filter_map(|id| app.doc.get(id))
        .map(|f| f.kind.intrinsic_size().unwrap_or_default() * f.transform.scale.abs())
        .fold(DVec2::ZERO, DVec2::max);
    let cell = if cell.x > 0.0 && cell.y > 0.0 { cell } else { DVec2::new(16.0, 9.0) };
    let preview_n = n.clamp(3, 80);
    let mut open = true;
    let mut apply = false;
    let mut cancel = false;
    let Some(dialog) = &mut app.arrange_dialog else { return };
    egui::Window::new(RichText::new(format!("{}  Arrange slides", icon::PATH)).font(theme::semibold(16.0)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!(
                    "Moves all {n} slides on the path into a new layout. Each slide's content moves with it, and the path keeps its order. Undo with {}.",
                    if cfg!(target_os = "macos") { "⌘Z" } else { "Ctrl+Z" }
                ))
                .color(p.text_muted),
            );
            ui.add_space(8.0);
            let size = egui::vec2(176.0, 104.0);
            egui::Grid::new("arrangements").num_columns(4).spacing([12.0, 10.0]).show(ui, |ui| {
                for (i, kind) in Arrangement::ALL.into_iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, dialog.kind == kind, format!("Arrange as {}", kind.label())));
                        let selected = dialog.kind == kind;
                        let bg = if selected { p.accent.gamma_multiply(0.08) } else { p.surface_alt };
                        ui.painter().rect_filled(rect, 10, bg);
                        // Each diagram shows the options as they would apply to it.
                        let opts = ArrangeOptions { tilt: if selected { dialog.opts.tilt } else { kind.default_tilt() }, ..dialog.opts };
                        diagram(ui.painter(), rect, kind, preview_n, cell, &opts, (p.accent, p.text));
                        let stroke = if selected { Stroke::new(2.0, p.accent) } else if resp.hovered() { Stroke::new(1.5, p.accent.gamma_multiply(0.6)) } else { Stroke::new(1.0, p.border) };
                        ui.painter().rect_stroke(rect, 10, stroke, egui::StrokeKind::Outside);
                        ui.label(RichText::new(kind.label()).font(theme::semibold(13.0)));
                        if resp.on_hover_text(kind.blurb()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !selected {
                            dialog.kind = kind;
                            dialog.opts.tilt = kind.default_tilt();
                        }
                    });
                    if i % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
            ui.add_space(6.0);
            ui.separator();
            ui.label(RichText::new(dialog.kind.blurb()).color(p.text_muted));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Spacing");
                ui.add(egui::Slider::new(&mut dialog.opts.spacing, 0.05..=1.0).show_value(false));
                ui.add_space(12.0);
                ui.add_enabled(dialog.kind.can_tilt(), egui::Checkbox::new(&mut dialog.opts.tilt, "Turn slides along the path"))
                    .on_hover_text("The show's camera turns with each slide, for a more cinematic ride");
                let (label, tip) = if dialog.kind == Arrangement::Carousel {
                    ("Parallax backdrop", "Glowing layers behind the slides that drift at different speeds in the show")
                } else {
                    ("Draw the path", "A dotted trail between slides (lines for Tree); replaced on the next arrangement")
                };
                ui.add_enabled(dialog.kind.has_trail(), egui::Checkbox::new(&mut dialog.opts.trail, label)).on_hover_text(tip);
                ui.checkbox(&mut dialog.opts.overview, "Overview at start and end");
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = RichText::new(format!("{}  Arrange {n} slides", icon::CHECK)).font(theme::semibold(14.0)).color(p.on_accent);
                if ui.add_enabled(n > 0, egui::Button::new(label).fill(p.accent).corner_radius(8).min_size(egui::vec2(0.0, 32.0))).clicked() {
                    apply = true;
                }
                if ui.add(egui::Button::new("Cancel").corner_radius(8).min_size(egui::vec2(0.0, 32.0))).clicked() {
                    cancel = true;
                }
            });
        });
    if apply {
        let dialog = app.arrange_dialog.take().expect("open");
        match arrange::arrange(&mut app.doc, dialog.kind, &dialog.opts) {
            Ok(report) => {
                app.current_step = (!app.doc.path.is_empty()).then_some(0);
                app.selection.clear();
                app.fit_all(true);
                app.notify(ctx, format!("Arranged {} slides as {}", report.slides, dialog.kind.label()));
            }
            Err(e) => app.notify(ctx, format!("Could not arrange: {e}")),
        }
    } else if cancel || !open {
        app.arrange_dialog = None;
    }
}
