//! The window around the canvas: the top bar with its menus and command
//! search, the tool rail on the left, the Slide / Design / Animate panel on
//! the right, and the shortcut sheet. The path strip lives in `slides.rs`.

use crate::app::*;
use crate::design::AlignOp;
use crate::inspector::card;
use crate::theme::{self, icon};
use eframe::egui::{self, Align, Color32, Key, Layout, RichText, Sense, ThemePreference};
use glide_core::{Easing, ElementKind, TextRole, TransitionStyle};

/// The three tabs of the right-hand panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideTab {
    /// The current stop on the path: camera move, animations and notes.
    Slide,
    /// Properties of the selection, and the deck's theme, fonts and background.
    Design,
    /// In-slide animations.
    Animate,
}

impl SideTab {
    pub const ALL: [SideTab; 3] = [SideTab::Slide, SideTab::Design, SideTab::Animate];

    pub fn label(self) -> &'static str {
        match self {
            SideTab::Slide => "Slide",
            SideTab::Design => "Design",
            SideTab::Animate => "Animate",
        }
    }
}

pub fn easing_name(e: Easing) -> &'static str {
    match e {
        Easing::Linear => "Linear",
        Easing::EaseInOutSine => "Gentle",
        Easing::EaseInOutCubic => "Smooth",
        Easing::EaseInOutQuint => "Dramatic",
        Easing::EaseOutExpo => "Snappy",
    }
}

// --- small widgets ----------------------------------------------------------------------

/// A square icon-only button, named for screen readers and tests.
pub(crate) fn icon_button(ui: &mut egui::Ui, glyph: &str, label: &str, active: bool, enabled: bool, side: f32) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(side, side), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let hovered = enabled && resp.hovered();
    let fill = if active {
        p.accent_soft
    } else if hovered {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 8, fill);
    let fg = if !enabled {
        p.text_muted.gamma_multiply(0.5)
    } else if active {
        p.accent
    } else {
        p.text
    };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(side * 0.46), fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A full-width button with an icon, named exactly `label`.
pub(crate) fn wide_button(ui: &mut egui::Ui, glyph: &str, label: &str, primary: bool, enabled: bool) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let hovered = enabled && resp.hovered();
    let (fill, fg) = match (primary, enabled) {
        (true, true) => (if hovered { p.accent.gamma_multiply(0.9) } else { p.accent }, p.on_accent),
        (false, true) => (if hovered { ui.visuals().widgets.hovered.bg_fill } else { p.surface }, p.text),
        (_, false) => (p.surface, p.text_muted.gamma_multiply(0.55)),
    };
    ui.painter().rect_filled(rect, 7, fill);
    if !primary {
        ui.painter().rect_stroke(rect, 7, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    }
    let font = theme::semibold(12.5);
    let text = format!("{glyph}  {label}");
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, font, fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A menu row: icon, text and an optional shortcut. Returns true when chosen.
fn menu_item(ui: &mut egui::Ui, glyph: &str, text: &str, shortcut: Option<String>, enabled: bool) -> bool {
    let mut b = egui::Button::new(format!("{glyph}   {text}"));
    if let Some(sc) = shortcut {
        b = b.shortcut_text(sc);
    }
    let clicked = ui.add_enabled(enabled, b).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// A top-bar menu title: flat until hovered.
fn bar_menu(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.weak_bg_fill = Color32::TRANSPARENT;
        w.inactive.bg_fill = Color32::TRANSPARENT;
        ui.menu_button(RichText::new(title).size(13.0), |ui| {
            ui.set_min_width(230.0);
            add(ui)
        });
    });
}

// --- top bar ----------------------------------------------------------------------------

pub fn top_bar(app: &mut GlideDeckApp, ui: &mut egui::Ui) -> Vec<Action> {
    let mut out = Vec::new();
    let p = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.set_height(30.0);
        ui.spacing_mut().item_spacing.x = 2.0;

        // Logo mark and name.
        let (mark, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
        let (graphite, yellow) = (Color32::from_rgb(0x0F, 0x11, 0x13), Color32::from_rgb(0xFF, 0xD2, 0x3F));
        ui.painter().rect_filled(mark, 7, graphite);
        ui.painter().circle_filled(mark.center(), 7.5, yellow);
        ui.painter().circle_filled(mark.center(), 3.8, graphite);
        ui.painter().circle_filled(mark.center(), 1.5, yellow);
        ui.add_space(6.0);
        ui.label(RichText::new("GlideDeck").font(theme::semibold(14.0)));
        ui.add_space(12.0);

        file_menu(app, ui, &mut out);
        edit_menu(app, ui, &mut out);
        insert_menu(ui, &mut out);
        view_menu(app, ui, &mut out);
        ui.add_space(12.0);

        let mut state = String::new();
        if app.autosave.enabled && app.path.is_some() {
            state = if app.is_dirty() { "  ·  Saving…".into() } else { "  ·  Saved".into() };
        } else if app.is_dirty() {
            state = "  ·  Edited".into();
        }
        ui.add(egui::Label::new(RichText::new(format!("{}{state}", app.display_name())).size(12.5).color(p.text_muted)).truncate());

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let has_path = !app.doc.path.is_empty();

            // Present, with the other ways to start the show behind the caret.
            ui.scope(|ui| {
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                ui.menu_button(RichText::new(icon::CARET_DOWN).size(12.0), |ui| {
                    ui.set_min_width(230.0);
                    let ctx = ui.ctx().clone();
                    if menu_item(ui, icon::PLAY, "From the start", Some(ctx.format_shortcut(&SC_PRESENT)), has_path) {
                        out.push(Action::Present { from_start: true });
                    }
                    if menu_item(ui, icon::MONITOR_PLAY, "From this slide", Some(ctx.format_shortcut(&SC_PRESENT_HERE)), has_path) {
                        out.push(Action::Present { from_start: false });
                    }
                    if menu_item(ui, icon::PRESENTATION_CHART, "Presenter view", Some(ctx.format_shortcut(&SC_PRESENTER)), has_path) {
                        out.push(Action::PresenterView { from_start: false });
                    }
                })
                .response
                .on_hover_text("More ways to present");
            });
            let present = egui::Button::new(RichText::new(format!("{}  Present", icon::PLAY)).color(p.on_accent).font(theme::semibold(13.0)))
                .fill(p.accent)
                .corner_radius(8)
                .min_size(egui::vec2(100.0, 30.0));
            if ui
                .add_enabled(has_path, present)
                .on_hover_text("Present from the beginning (F5)")
                .on_disabled_hover_text("Add a frame to create your first slide")
                .clicked()
            {
                out.push(Action::Present { from_start: true });
            }

            // Share: every export.
            ui.menu_button(RichText::new(format!("{}  Share", icon::EXPORT)).size(13.0), |ui| {
                ui.set_min_width(250.0);
                if menu_item(ui, icon::FILE_HTML, "Web page (HTML)…", None, has_path) {
                    out.push(Action::ExportHtml);
                }
                if menu_item(ui, icon::FILE_PPT, "PowerPoint (Morph)…", None, true) {
                    out.push(Action::ExportPptx);
                }
                if menu_item(ui, icon::FILE_PDF, "PDF…", None, has_path) {
                    out.push(Action::ExportPdf);
                }
                if menu_item(ui, icon::FILE_IMAGE, "This view as PNG…", None, true) {
                    out.push(Action::ExportPng);
                }
            })
            .response
            .on_hover_text("Export a web page, PowerPoint, PDF or PNG");

            let dark = ui.ctx().theme() == egui::Theme::Dark;
            if icon_button(ui, icon::CIRCLE_HALF, if dark { "Switch to light" } else { "Switch to dark" }, false, true, 30.0)
                .on_hover_text(if dark { "Switch to the light theme" } else { "Switch to the dark theme" })
                .clicked()
            {
                out.push(Action::SetTheme(if dark { ThemePreference::Light } else { ThemePreference::Dark }));
            }
            if icon_button(ui, icon::ARROW_CLOCKWISE, "Redo", false, app.history.can_redo(), 30.0).on_hover_text("Redo").clicked() {
                out.push(Action::Redo);
            }
            if icon_button(ui, icon::ARROW_COUNTER_CLOCKWISE, "Undo", false, app.history.can_undo() || app.is_dirty(), 30.0).on_hover_text("Undo").clicked() {
                out.push(Action::Undo);
            }

            // Command search, centered in what is left.
            let avail = ui.available_width();
            let w = (avail - 24.0).clamp(0.0, 340.0);
            if w > 120.0 {
                ui.add_space(((avail - w) * 0.5).max(0.0));
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 30.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Search or run a command"));
                ui.painter().rect_filled(rect, 8, p.surface_alt);
                let stroke = if resp.hovered() { p.accent.gamma_multiply(0.7) } else { p.border };
                ui.painter().rect_stroke(rect, 8, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
                ui.painter().text(rect.left_center() + egui::vec2(11.0, 0.0), egui::Align2::LEFT_CENTER, icon::MAGNIFYING_GLASS, egui::FontId::proportional(14.0), p.text_muted);
                ui.painter().text(rect.left_center() + egui::vec2(33.0, 0.0), egui::Align2::LEFT_CENTER, "Search or run a command", egui::FontId::proportional(12.5), p.text_muted);
                let key = ui.ctx().format_shortcut(&SC_PALETTE);
                ui.painter().text(rect.right_center() - egui::vec2(10.0, 0.0), egui::Align2::RIGHT_CENTER, key, egui::FontId::proportional(11.0), p.text_muted);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    out.push(Action::OpenPalette);
                }
            }
        });
    });
    out
}

fn file_menu(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let ctx = ui.ctx().clone();
    let sc = |s: &egui::KeyboardShortcut| Some(ctx.format_shortcut(s));
    bar_menu(ui, "File", |ui| {
        if menu_item(ui, icon::FILE_PLUS, "New", sc(&SC_NEW), true) {
            out.push(Action::New);
        }
        if menu_item(ui, icon::SQUARES_FOUR, "New from template…", None, true) {
            out.push(Action::ShowTemplates);
        }
        if menu_item(ui, icon::FOLDER_OPEN, "Open…", sc(&SC_OPEN), true) {
            out.push(Action::Open);
        }
        ui.menu_button(format!("{}   Open recent", icon::CLOCK_COUNTER_CLOCKWISE), |ui| {
            ui.set_min_width(300.0);
            if app.recent.paths.is_empty() {
                ui.add_enabled(false, egui::Button::new("No recent files"));
            }
            for path in app.recent.paths.clone() {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let glyph = if name.to_lowercase().ends_with(".pptx") { icon::FILE_PPT } else { icon::PRESENTATION };
                let folder = path.parent().map(|d| d.display().to_string()).unwrap_or_default();
                if ui.button(format!("{glyph}   {name}")).on_hover_text(folder).clicked() {
                    out.push(Action::OpenRecent(path));
                    ui.close();
                }
            }
            if !app.recent.paths.is_empty() {
                ui.separator();
                if ui.button(format!("{}   Clear recent", icon::TRASH)).clicked() {
                    out.push(Action::ClearRecent);
                    ui.close();
                }
            }
        });
        if menu_item(ui, icon::FILE_PPT, "Import PowerPoint…", None, true) {
            out.push(Action::ImportPptx);
        }
        ui.separator();
        if menu_item(ui, icon::FLOPPY_DISK, "Save", sc(&SC_SAVE), true) {
            out.push(Action::Save);
        }
        if menu_item(ui, icon::FLOPPY_DISK, "Save As…", sc(&SC_SAVE_AS), true) {
            out.push(Action::SaveAs);
        }
        let mut on = app.autosave.enabled;
        if ui
            .checkbox(&mut on, "AutoSave")
            .on_hover_text("Save a presentation that has a file a moment after each change. Unsaved work is always kept for recovery after a crash.")
            .changed()
        {
            app.autosave.set_enabled(on);
        }
        ui.separator();
        if menu_item(ui, icon::FILE_PPT, "Export PowerPoint (Morph)…", None, true) {
            out.push(Action::ExportPptx);
        }
        if menu_item(ui, icon::FILE_PDF, "Export PDF…", None, true) {
            out.push(Action::ExportPdf);
        }
        if menu_item(ui, icon::FILE_HTML, "Export web page (HTML)…", None, true) {
            out.push(Action::ExportHtml);
        }
        if menu_item(ui, icon::FILE_IMAGE, "Export view as PNG…", None, true) {
            out.push(Action::ExportPng);
        }
        ui.separator();
        if menu_item(ui, icon::QUESTION, "Keyboard shortcuts", Some("F1".into()), true) {
            out.push(Action::ToggleShortcuts);
        }
        if menu_item(ui, icon::ARROWS_CLOCKWISE, "Check for updates…", None, true) {
            out.push(Action::CheckForUpdates);
        }
        if menu_item(ui, icon::INFO, "About GlideDeck", None, true) {
            out.push(Action::ShowAbout);
        }
        if ui.button(format!("{}   Quit", icon::X)).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
}

const ALIGN_OPS: [(&str, &str, AlignOp); 8] = [
    (icon::ALIGN_LEFT, "Align left", AlignOp::Left),
    (icon::ALIGN_CENTER_HORIZONTAL, "Align centers", AlignOp::HCenter),
    (icon::ALIGN_RIGHT, "Align right", AlignOp::Right),
    (icon::COLUMNS, "Distribute horizontally", AlignOp::DistributeH),
    (icon::ALIGN_TOP, "Align top", AlignOp::Top),
    (icon::ALIGN_CENTER_VERTICAL, "Align middles", AlignOp::VMiddle),
    (icon::ALIGN_BOTTOM, "Align bottom", AlignOp::Bottom),
    (icon::ROWS, "Distribute vertically", AlignOp::DistributeV),
];

fn align_enabled(op: AlignOp, n: usize) -> bool {
    if matches!(op, AlignOp::DistributeH | AlignOp::DistributeV) { n >= 3 } else { n >= 1 }
}

fn edit_menu(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let ctx = ui.ctx().clone();
    let sc = |s: &egui::KeyboardShortcut| Some(ctx.format_shortcut(s));
    let n = app.selection.len();
    let has_sel = n > 0;
    bar_menu(ui, "Edit", |ui| {
        if menu_item(ui, icon::ARROW_COUNTER_CLOCKWISE, "Undo", sc(&SC_UNDO), app.history.can_undo() || app.is_dirty()) {
            out.push(Action::Undo);
        }
        if menu_item(ui, icon::ARROW_CLOCKWISE, "Redo", sc(&SC_REDO), app.history.can_redo()) {
            out.push(Action::Redo);
        }
        ui.separator();
        if menu_item(ui, icon::SCISSORS, "Cut", sc(&SC_CUT), has_sel) {
            out.push(Action::Cut);
        }
        if menu_item(ui, icon::COPY, "Copy", sc(&SC_COPY), has_sel) {
            out.push(Action::Copy);
        }
        if menu_item(ui, icon::CLIPBOARD, "Paste", sc(&SC_PASTE), true) {
            out.push(Action::Paste);
        }
        if menu_item(ui, icon::COPY_SIMPLE, "Duplicate", sc(&SC_DUPLICATE), has_sel) {
            out.push(Action::Duplicate);
        }
        if menu_item(ui, icon::TRASH, "Delete", Some("Del".into()), has_sel) {
            out.push(Action::Delete);
        }
        if menu_item(ui, icon::SELECTION_ALL, "Select all", sc(&SC_SELECT_ALL), true) {
            out.push(Action::SelectAll);
        }
        ui.separator();
        if menu_item(ui, icon::ARROW_LINE_UP, "Bring to front", sc(&SC_FRONT), has_sel) {
            out.push(Action::BringToFront);
        }
        if menu_item(ui, icon::ARROW_LINE_DOWN, "Send to back", sc(&SC_BACK), has_sel) {
            out.push(Action::SendToBack);
        }
        ui.menu_button(format!("{}   Align and distribute", icon::ALIGN_LEFT), |ui| {
            for (glyph, name, op) in ALIGN_OPS {
                if menu_item(ui, glyph, name, None, align_enabled(op, n)) {
                    out.push(Action::Align(op));
                }
            }
        });
        ui.separator();
        if menu_item(ui, icon::PATH, "Arrange slides…", None, !app.doc.path.is_empty()) {
            out.push(Action::ShowArrange);
        }
        if menu_item(ui, icon::SEAL_CHECK, "Design check", None, true) {
            out.push(Action::ToggleDesignCheck);
        }
    });
}

fn insert_menu(ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let ctx = ui.ctx().clone();
    let sc = |s: &egui::KeyboardShortcut| Some(ctx.format_shortcut(s));
    bar_menu(ui, "Insert", |ui| {
        if menu_item(ui, icon::FRAME_CORNERS, "Slide frame (draw)", Some("F".into()), true) {
            out.push(Action::SetTool(Tool::Frame));
        }
        if menu_item(ui, icon::SELECTION, "Slide from this view", sc(&SC_FRAME_VIEW), true) {
            out.push(Action::FrameFromView);
        }
        if menu_item(ui, icon::LAYOUT, "Slide layout…", sc(&SC_LAYOUTS), true) {
            out.push(Action::ShowLayouts);
        }
        ui.separator();
        for (glyph, role) in [(icon::TEXT_H_ONE, TextRole::Title), (icon::TEXT_H_TWO, TextRole::Heading), (icon::PARAGRAPH, TextRole::Body)] {
            if menu_item(ui, glyph, role.label(), None, true) {
                out.push(Action::InsertText(role));
            }
        }
        if menu_item(ui, icon::TEXT_T, "Text box", Some("T".into()), true) {
            out.push(Action::SetTool(Tool::Text));
        }
        ui.separator();
        if menu_item(ui, icon::SQUARE, "Rectangle shape", Some("R".into()), true) {
            out.push(Action::SetTool(Tool::Rect));
        }
        if menu_item(ui, icon::CIRCLE, "Ellipse shape", Some("O".into()), true) {
            out.push(Action::SetTool(Tool::Ellipse));
        }
        if menu_item(ui, icon::SHAPES, "Icon…", sc(&SC_ICONS), true) {
            out.push(Action::ShowIcons);
        }
        ui.separator();
        if menu_item(ui, icon::IMAGE, "Picture or SVG…", sc(&SC_INSERT_IMAGE), true) {
            out.push(Action::InsertImage);
        }
        if menu_item(ui, icon::FILM_STRIP, "Video file…", None, true) {
            out.push(Action::InsertVideo);
        }
        if menu_item(ui, icon::YOUTUBE_LOGO, "Web video (YouTube, Vimeo, Loom)…", None, true) {
            out.push(Action::InsertWebVideo);
        }
    });
}

fn view_menu(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let ctx = ui.ctx().clone();
    let sc = |s: &egui::KeyboardShortcut| Some(ctx.format_shortcut(s));
    let check = |on: bool| if on { icon::CHECK_SQUARE } else { icon::SQUARE };
    bar_menu(ui, "View", |ui| {
        if menu_item(ui, icon::CORNERS_OUT, "Fit everything", sc(&SC_FIT_ALL), true) {
            out.push(Action::FitAll);
        }
        if menu_item(ui, icon::MAGNIFYING_GLASS_PLUS, "Zoom to selection", sc(&SC_FIT_SELECTION), !app.selection.is_empty()) {
            out.push(Action::FitSelection);
        }
        if menu_item(ui, icon::ARROWS_CLOCKWISE, "Reset view", sc(&SC_RESET_VIEW), true) {
            out.push(Action::ResetView);
        }
        ui.separator();
        if menu_item(ui, check(app.show_grid), "Grid", Some("G".into()), true) {
            out.push(Action::ToggleGrid);
        }
        if menu_item(ui, check(app.show_frames), "Frame outlines", None, true) {
            out.push(Action::ToggleFrameOutlines);
        }
        if menu_item(ui, check(app.scroll_pans), "Trackpad mode", None, true) {
            app.scroll_pans = !app.scroll_pans;
        }
        ui.separator();
        if menu_item(ui, check(app.show_slides), "Path strip", None, true) {
            out.push(Action::ToggleSlides);
        }
        if menu_item(ui, check(app.show_inspector), "Side panel", None, true) {
            out.push(Action::ToggleInspector);
        }
        ui.separator();
        let pref = ui.ctx().options(|o| o.theme_preference);
        for (t, g, l) in [(ThemePreference::Light, icon::SUN, "Light theme"), (ThemePreference::Dark, icon::MOON, "Dark theme"), (ThemePreference::System, icon::CIRCLE_HALF, "Match the system")] {
            let text = if pref == t { format!("{l}  {}", icon::CHECK) } else { l.to_owned() };
            if menu_item(ui, g, &text, None, true) {
                out.push(Action::SetTheme(t));
            }
        }
        ui.separator();
        if menu_item(ui, icon::QUESTION, "Keyboard shortcuts", Some("F1".into()), true) {
            out.push(Action::ToggleShortcuts);
        }
    });
}

// --- tool rail --------------------------------------------------------------------------

pub fn tool_rail(app: &mut GlideDeckApp, ui: &mut egui::Ui) -> Vec<Action> {
    let mut out = Vec::new();
    let p = theme::palette(ui.ctx());
    const SIDE: f32 = 40.0;
    let divider = |ui: &mut egui::Ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(SIDE, 9.0), Sense::hover());
        ui.painter().hline(rect.x_range().shrink(8.0), rect.center().y, egui::Stroke::new(1.0, p.border));
    };
    egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for t in [Tool::Select, Tool::Hand] {
            if icon_button(ui, t.icon(), t.label(), app.tool == t, true, SIDE).on_hover_text(format!("{} ({:?})", t.label(), t.key())).clicked() {
                out.push(Action::SetTool(t));
            }
        }
        divider(ui);
        if icon_button(ui, icon::FRAME_CORNERS, "Frame", app.tool == Tool::Frame, true, SIDE)
            .on_hover_text("Slide: draw a frame. Each frame on the path is a slide (F)")
            .clicked()
        {
            out.push(Action::SetTool(Tool::Frame));
        }
        if icon_button(ui, icon::LAYOUT, "Layouts", app.show_layouts, true, SIDE).on_hover_text("Add a ready-made slide (Ctrl+Shift+L)").clicked() {
            out.push(Action::ShowLayouts);
        }
        if icon_button(ui, icon::TEXT_T, "Text", app.tool == Tool::Text, true, SIDE).on_hover_text("Text: click to type, drag for a wrapping box (T)").clicked() {
            out.push(Action::SetTool(Tool::Text));
        }
        for t in [Tool::Rect, Tool::Ellipse] {
            if icon_button(ui, t.icon(), t.label(), app.tool == t, true, SIDE).on_hover_text(format!("{}: drag to draw ({:?})", t.label(), t.key())).clicked() {
                out.push(Action::SetTool(t));
            }
        }
        if icon_button(ui, icon::IMAGE, "Picture", false, true, SIDE).on_hover_text("Picture or SVG from this computer (Ctrl+Shift+I)").clicked() {
            out.push(Action::InsertImage);
        }
        if icon_button(ui, icon::SHAPES, "Icons", app.icon_picker.open, true, SIDE).on_hover_text("1,500 vector icons (Ctrl+Shift+K)").clicked() {
            out.push(Action::ShowIcons);
        }
        let video = icon_button(ui, icon::FILM_STRIP, "Video", app.web_video_dialog.open, true, SIDE).on_hover_text("Video: a file, or a YouTube, Vimeo or Loom link");
        egui::Popup::menu(&video).show(|ui| {
            ui.set_min_width(220.0);
            if menu_item(ui, icon::FILM_STRIP, "Video file…", None, true) {
                out.push(Action::InsertVideo);
            }
            if menu_item(ui, icon::YOUTUBE_LOGO, "Web video link…", None, true) {
                out.push(Action::InsertWebVideo);
            }
        });
        divider(ui);
        if icon_button(ui, icon::SQUARES_FOUR, "Templates", app.show_templates, true, SIDE).on_hover_text("Start a new presentation from a template").clicked() {
            out.push(Action::ShowTemplates);
        }
    });
    out
}

// --- side panel -------------------------------------------------------------------------

pub fn side_panel(app: &mut GlideDeckApp, ui: &mut egui::Ui) -> Vec<Action> {
    let mut out = Vec::new();
    let p = theme::palette(ui.ctx());

    // The three tabs, as one segmented control.
    egui::Frame::new().fill(p.surface_alt).corner_radius(9).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let w = (ui.available_width() - 4.0) / 3.0;
            for tab in SideTab::ALL {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tab.label()));
                let on = app.side_tab == tab;
                if on {
                    ui.painter().rect_filled(rect, 7, p.surface);
                    ui.painter().rect_stroke(rect, 7, egui::Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
                }
                let (font, color) = if on {
                    (theme::semibold(12.5), p.accent)
                } else {
                    (egui::FontId::proportional(12.5), if resp.hovered() { p.text } else { p.text_muted })
                };
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, tab.label(), font, color);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    app.side_tab = tab;
                }
            }
        });
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.side_tab {
        SideTab::Slide => slide_tab(app, ui, &mut out),
        SideTab::Design => design_tab(app, ui, &mut out),
        SideTab::Animate => crate::anim_ui::animate_panel(app, ui, &mut out),
    });
    out
}

fn slide_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    let Some(index) = app.current_step.filter(|i| *i < app.doc.path.len()) else {
        card(ui, "Slide", |ui| {
            ui.label(RichText::new("Each frame on the path is a slide. Pick a stop on the path below, or make the first one from what you see now.").color(p.text_muted));
            ui.add_space(6.0);
            if wide_button(ui, icon::PLUS, "Frame current view", true, true).clicked() {
                out.push(Action::FrameFromView);
            }
        });
        return;
    };
    let name = app.doc.step_frame(index).map(|f| f.display_name()).unwrap_or_else(|| "Missing frame".into());
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", index + 1)).font(theme::semibold(15.0)).color(p.accent));
        ui.add(egui::Label::new(RichText::new(name).font(theme::semibold(15.0))).truncate());
    });
    ui.label(RichText::new(format!("Stop {} of {} on the path", index + 1, app.doc.path.len())).size(11.5).color(p.text_muted));
    ui.add_space(8.0);

    let mut t = app.doc.path[index].transition;
    let mut apply_all = false;
    card(ui, "Camera move", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for style in TransitionStyle::ALL {
                let text = RichText::new(format!("{}  {}", theme::transition_icon(style), style.label())).size(12.5);
                if ui.add(egui::Button::selectable(t.style == style, text)).on_hover_text(style.description()).clicked() {
                    t.style = style;
                }
            }
        });
        ui.add_space(6.0);
        egui::Grid::new("camera-move").num_columns(2).spacing([10.0, 7.0]).min_col_width(64.0).show(ui, |ui| {
            ui.label(RichText::new("Duration").color(p.text_muted));
            ui.add(egui::Slider::new(&mut t.duration, 0.2..=5.0).suffix(" s").fixed_decimals(1));
            ui.end_row();
            ui.label(RichText::new("Easing").color(p.text_muted));
            egui::ComboBox::from_id_salt("easing").selected_text(easing_name(t.easing)).show_ui(ui, |ui| {
                for e in [Easing::EaseInOutCubic, Easing::EaseInOutSine, Easing::EaseInOutQuint, Easing::EaseOutExpo, Easing::Linear] {
                    ui.selectable_value(&mut t.easing, e, easing_name(e));
                }
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        apply_all = ui.button(format!("{}  Use for every slide", icon::SQUARES_FOUR)).on_hover_text("Give all slides this camera move, duration and easing").clicked();
    });
    if apply_all {
        for s in &mut app.doc.path {
            s.transition = t;
        }
    } else {
        app.doc.path[index].transition = t;
    }

    crate::anim_ui::slide_card(app, ui, index, out);

    card(ui, "Speaker notes", |ui| {
        crate::bidi::multiline(ui, "speaker-notes", &mut app.doc.path[index].notes, |e| {
            e.hint_text("What to say, numbers to mention, a reminder to pause…").desired_rows(5).desired_width(f32::INFINITY)
        });
        ui.label(RichText::new("Speaker notes show only to you, in presenter view (Alt+F5). # heading, - bullet, **bold**").size(11.0).color(p.text_muted));
    });
}

fn design_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    if app.selection.is_empty() {
        deck_design(app, ui, out);
    } else {
        crate::inspector::selection_cards(app, ui, out);
        egui::CollapsingHeader::new(RichText::new(format!("{}  Deck theme and fonts", icon::PALETTE)).font(theme::semibold(13.0)))
            .default_open(false)
            .show(ui, |ui| deck_design(app, ui, out));
    }
    crate::inspector::layers(app, ui);
}

/// Deck-wide design: title, theme, fonts, background and the design check.
fn deck_design(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    card(ui, "Presentation", |ui| {
        crate::bidi::singleline(ui, "doc-title", &mut app.doc.title, |e| e.hint_text("Untitled").desired_width(f32::INFINITY));
    });

    card(ui, "Theme", |ui| {
        let themes = glide_core::theme::builtin_themes();
        let current = app.doc.theme.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| "Paper".into());
        // The house style first, then the rest of the gallery.
        let house = [glide_core::theme::HOUSE_DARK, glide_core::theme::HOUSE_LIGHT];
        let mut order: Vec<usize> = (0..themes.len()).collect();
        order.sort_by_key(|i| !house.contains(&themes[*i].name.as_str()));
        let cols = 3;
        let w = ((ui.available_width() - 8.0 * (cols as f32 - 1.0)) / cols as f32).floor();
        egui::Grid::new("themes").num_columns(cols).spacing([8.0, 8.0]).show(ui, |ui| {
            for (k, i) in order.into_iter().enumerate() {
                let t = &themes[i];
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    if crate::galleries::theme_card(ui, t, t.name == current, egui::vec2(w, (w * 0.6).round())).clicked() {
                        out.push(Action::ApplyTheme(i));
                    }
                    ui.label(RichText::new(&t.name).size(11.0).color(if t.name == current { p.accent } else { p.text_muted }));
                });
                if k % cols == cols - 1 {
                    ui.end_row();
                }
            }
        });
    });

    card(ui, "Fonts", |ui| {
        let mut theme = app.theme();
        let used = crate::fontpick::used_families(&app.doc);
        let (mut heading, mut body) = (theme.heading_font.clone(), theme.body_font.clone());
        let mut add = false;
        egui::Grid::new("deck-fonts").num_columns(2).spacing([10.0, 7.0]).min_col_width(64.0).show(ui, |ui| {
            ui.label(RichText::new("Headings").color(p.text_muted));
            add |= crate::fontpick::font_picker(ui, &mut app.renderer.text, "heading-font", &mut heading, &used, 150.0).add_file;
            ui.end_row();
            ui.label(RichText::new("Body").color(p.text_muted));
            add |= crate::fontpick::font_picker(ui, &mut app.renderer.text, "body-font", &mut body, &used, 150.0).add_file;
            ui.end_row();
        });
        if add {
            let ctx = ui.ctx().clone();
            if let Some(f) = crate::fontpick::add_font_file(app, &ctx) {
                heading = f;
            }
        }
        if heading != theme.heading_font || body != theme.body_font {
            theme.heading_font = heading;
            theme.body_font = body;
            // Restyles text by role (titles and headings use the heading font).
            glide_core::theme::apply_theme(&mut app.doc, &theme);
        }
    });

    card(ui, "Canvas background", |ui| {
        ui.horizontal(|ui| {
            ui.color_edit_button_srgba_unmultiplied(&mut app.doc.background.0);
            let mut gradient = app.doc.background_to.is_some();
            if ui.checkbox(&mut gradient, "Gradient").changed() {
                app.doc.background_to = gradient.then(|| {
                    let b = app.doc.background;
                    let f = |v: u8| v.saturating_sub(18);
                    glide_core::Color::rgb(f(b.r()), f(b.g()), b.b().saturating_add(10))
                });
            }
            if let Some(to) = &mut app.doc.background_to {
                ui.color_edit_button_srgba_unmultiplied(&mut to.0);
            }
        });
    });

    crate::galleries::refresh_design_check(app);
    let count = app.design_check.issues.len();
    card(ui, "Review", |ui| {
        let label = match count {
            0 => "Design check".to_owned(),
            1 => "Design check: 1 issue".to_owned(),
            n => format!("Design check: {n} issues"),
        };
        if ui
            .button(format!("{}  {label}", icon::SEAL_CHECK))
            .on_hover_text("Contrast, readability, empty slides, missing media and font consistency")
            .clicked()
        {
            out.push(Action::ToggleDesignCheck);
        }
    });
}

// --- command search ---------------------------------------------------------------------

/// The command search: type a few letters, pick a command or a slide.
#[derive(Default)]
pub struct CommandPalette {
    pub open: bool,
    pub query: String,
    selected: usize,
    /// Focus the field on the first frame after opening.
    focus: bool,
}

impl CommandPalette {
    pub fn show(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.focus = true;
    }
}

struct Command {
    glyph: &'static str,
    label: String,
    /// Extra words the search also matches.
    keywords: String,
    shortcut: Option<String>,
    action: Action,
}

fn commands(app: &GlideDeckApp, ctx: &egui::Context) -> Vec<Command> {
    let sc = |s: &egui::KeyboardShortcut| Some(ctx.format_shortcut(s));
    let mut v: Vec<Command> = Vec::new();
    let mut add = |glyph: &'static str, label: &str, keywords: &str, shortcut: Option<String>, action: Action| {
        v.push(Command { glyph, label: label.to_owned(), keywords: keywords.to_owned(), shortcut, action });
    };
    add(icon::PLAY, "Present from the start", "show play slideshow", sc(&SC_PRESENT), Action::Present { from_start: true });
    add(icon::MONITOR_PLAY, "Present from this slide", "show play", sc(&SC_PRESENT_HERE), Action::Present { from_start: false });
    add(icon::PRESENTATION_CHART, "Presenter view", "notes timer", sc(&SC_PRESENTER), Action::PresenterView { from_start: false });
    add(icon::SELECTION, "New slide from this view", "frame add", sc(&SC_FRAME_VIEW), Action::FrameFromView);
    add(icon::FRAME_CORNERS, "Draw a slide frame", "frame tool", Some("F".into()), Action::SetTool(Tool::Frame));
    add(icon::LAYOUT, "Add a slide layout", "layouts insert", sc(&SC_LAYOUTS), Action::ShowLayouts);
    add(icon::PATH, "Arrange slides (spiral, orbits, carousel…)", "path rearrange tree tunnel", None, Action::ShowArrange);
    add(icon::SQUARES_FOUR, "New from a template", "templates start", None, Action::ShowTemplates);
    add(icon::TEXT_H_ONE, "Insert a title", "text heading", None, Action::InsertText(TextRole::Title));
    add(icon::TEXT_H_TWO, "Insert a heading", "text", None, Action::InsertText(TextRole::Heading));
    add(icon::PARAGRAPH, "Insert body text", "text paragraph", None, Action::InsertText(TextRole::Body));
    add(icon::TEXT_T, "Text box tool", "type", Some("T".into()), Action::SetTool(Tool::Text));
    add(icon::SQUARE, "Rectangle tool", "shape box", Some("R".into()), Action::SetTool(Tool::Rect));
    add(icon::CIRCLE, "Ellipse tool", "shape circle oval", Some("O".into()), Action::SetTool(Tool::Ellipse));
    add(icon::SHAPES, "Insert an icon", "icons library", sc(&SC_ICONS), Action::ShowIcons);
    add(icon::IMAGE, "Insert a picture or SVG", "image photo", sc(&SC_INSERT_IMAGE), Action::InsertImage);
    add(icon::FILM_STRIP, "Insert a video file", "movie mp4", None, Action::InsertVideo);
    add(icon::YOUTUBE_LOGO, "Insert a web video", "youtube vimeo loom link", None, Action::InsertWebVideo);
    add(icon::MAGIC_WAND, "Auto-animate this slide", "animation builds", None, Action::AutoAnimate { all: false });
    add(icon::SPARKLE, "Auto-animate every slide", "animation builds all", None, Action::AutoAnimate { all: true });
    add(icon::PLAY_CIRCLE, "Preview this slide's animations", "animation", None, Action::PreviewAnimations);
    add(icon::SEAL_CHECK, "Design check", "contrast review readability", None, Action::ToggleDesignCheck);
    for (i, t) in glide_core::theme::builtin_themes().iter().enumerate() {
        add(icon::PALETTE, &format!("Theme: {}", t.name), "colors design look", None, Action::ApplyTheme(i));
    }
    add(icon::FILE_PLUS, "New presentation", "file blank", sc(&SC_NEW), Action::New);
    add(icon::FOLDER_OPEN, "Open…", "file", sc(&SC_OPEN), Action::Open);
    add(icon::FILE_PPT, "Import PowerPoint…", "pptx file", None, Action::ImportPptx);
    add(icon::FLOPPY_DISK, "Save", "file", sc(&SC_SAVE), Action::Save);
    add(icon::FLOPPY_DISK, "Save As…", "file", sc(&SC_SAVE_AS), Action::SaveAs);
    add(icon::FILE_HTML, "Export a web page (HTML)", "share browser", None, Action::ExportHtml);
    add(icon::FILE_PPT, "Export PowerPoint (Morph)", "share pptx", None, Action::ExportPptx);
    add(icon::FILE_PDF, "Export PDF", "share print", None, Action::ExportPdf);
    add(icon::FILE_IMAGE, "Export this view as PNG", "share image screenshot", None, Action::ExportPng);
    add(icon::ARROW_COUNTER_CLOCKWISE, "Undo", "", sc(&SC_UNDO), Action::Undo);
    add(icon::ARROW_CLOCKWISE, "Redo", "", sc(&SC_REDO), Action::Redo);
    add(icon::COPY_SIMPLE, "Duplicate selection", "copy", sc(&SC_DUPLICATE), Action::Duplicate);
    add(icon::TRASH, "Delete selection", "remove", None, Action::Delete);
    add(icon::SELECTION_ALL, "Select all", "", sc(&SC_SELECT_ALL), Action::SelectAll);
    add(icon::ARROW_LINE_UP, "Bring to front", "order layer", sc(&SC_FRONT), Action::BringToFront);
    add(icon::ARROW_LINE_DOWN, "Send to back", "order layer", sc(&SC_BACK), Action::SendToBack);
    for (glyph, name, op) in ALIGN_OPS {
        add(glyph, name, "arrange", None, Action::Align(op));
    }
    add(icon::CORNERS_OUT, "Fit everything in view", "zoom overview", sc(&SC_FIT_ALL), Action::FitAll);
    add(icon::MAGNIFYING_GLASS_PLUS, "Zoom to selection", "", sc(&SC_FIT_SELECTION), Action::FitSelection);
    add(icon::ARROWS_CLOCKWISE, "Reset view", "zoom", sc(&SC_RESET_VIEW), Action::ResetView);
    add(icon::DOTS_NINE, "Toggle grid", "view", Some("G".into()), Action::ToggleGrid);
    add(icon::FRAME_CORNERS, "Toggle frame outlines", "view", None, Action::ToggleFrameOutlines);
    add(icon::SLIDESHOW, "Toggle the path strip", "view panel slides", None, Action::ToggleSlides);
    add(icon::SIDEBAR_SIMPLE, "Toggle the side panel", "view properties", None, Action::ToggleInspector);
    add(icon::NOTE, "Speaker notes", "write notes", None, Action::ToggleNotes);
    add(icon::SUN, "Light theme", "appearance color scheme", None, Action::SetTheme(ThemePreference::Light));
    add(icon::MOON, "Dark theme", "appearance color scheme", None, Action::SetTheme(ThemePreference::Dark));
    add(icon::CIRCLE_HALF, "Match the system theme", "appearance", None, Action::SetTheme(ThemePreference::System));
    add(icon::QUESTION, "Keyboard shortcuts", "help keys", Some("F1".into()), Action::ToggleShortcuts);
    add(icon::INFO, "About GlideDeck", "version help", None, Action::ShowAbout);
    add(icon::ARROWS_CLOCKWISE, "Check for updates", "version", None, Action::CheckForUpdates);

    // Slides, found by their name or by the text on them.
    for i in 0..app.doc.path.len() {
        let Some(frame) = app.doc.step_frame(i) else { continue };
        let label = format!("Go to slide {}: {}", i + 1, frame.display_name());
        v.push(Command { glyph: icon::SLIDESHOW, label, keywords: slide_text(&app.doc, frame), shortcut: None, action: Action::GoToStep(i) });
    }
    v
}

/// The text of everything that sits inside a frame.
fn slide_text(doc: &glide_core::Document, frame: &glide_core::Element) -> String {
    let size = frame.kind.intrinsic_size().unwrap_or_default();
    let inv = frame.transform.affine().inverse();
    let mut out = String::new();
    for el in &doc.elements {
        let ElementKind::Text(t) = &el.kind else { continue };
        let local = inv.transform_point2(el.transform.position());
        if local.x.abs() <= size.x * 0.5 && local.y.abs() <= size.y * 0.5 {
            out.push_str(&t.content);
            out.push(' ');
        }
    }
    out
}

/// Every word of the query must appear in the label or keywords. Label
/// matches rank first, then matches at the start of a word.
fn score(cmd: &Command, words: &[String]) -> Option<u32> {
    let label = cmd.label.to_lowercase();
    let keys = cmd.keywords.to_lowercase();
    let mut total = 0;
    for w in words {
        let s = if label.starts_with(w.as_str()) {
            0
        } else if label.split(|c: char| !c.is_alphanumeric()).any(|part| part.starts_with(w.as_str())) {
            1
        } else if label.contains(w.as_str()) {
            2
        } else if keys.contains(w.as_str()) {
            3
        } else {
            return None;
        };
        total += s;
    }
    Some(total)
}

pub fn command_palette(app: &mut GlideDeckApp, ctx: &egui::Context) -> Vec<Action> {
    let mut out = Vec::new();
    if !app.palette.open {
        return out;
    }
    let p = theme::palette(ctx);
    let words: Vec<String> = app.palette.query.to_lowercase().split_whitespace().map(str::to_owned).collect();
    let mut matches: Vec<(u32, Command)> = commands(app, ctx).into_iter().filter_map(|c| score(&c, &words).map(|s| (s, c))).collect();
    // Stable: equal scores keep the curated order.
    matches.sort_by_key(|(s, _)| *s);
    matches.truncate(9);
    app.palette.selected = app.palette.selected.min(matches.len().saturating_sub(1));

    let (up, down, enter, escape) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if down && !matches.is_empty() {
        app.palette.selected = (app.palette.selected + 1) % matches.len();
    }
    if up && !matches.is_empty() {
        app.palette.selected = (app.palette.selected + matches.len() - 1) % matches.len();
    }

    let mut run: Option<usize> = None;
    let width = 540.0;
    let area = egui::Area::new(egui::Id::new("command-search"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 58.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.surface)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(12)
                .shadow(egui::Shadow { offset: [0, 14], blur: 40, spread: 0, color: Color32::from_black_alpha(90) })
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new(icon::MAGNIFYING_GLASS).size(17.0).color(p.text_muted));
                        let field = ui.add(
                            egui::TextEdit::singleline(&mut app.palette.query)
                                .hint_text("Search or run a command")
                                .frame(egui::Frame::NONE)
                                .font(egui::FontId::proportional(15.0))
                                .desired_width(f32::INFINITY),
                        );
                        if std::mem::take(&mut app.palette.focus) || !field.has_focus() {
                            field.request_focus();
                        }
                        if field.changed() {
                            app.palette.selected = 0;
                        }
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    if matches.is_empty() {
                        ui.add_space(6.0);
                        ui.label(RichText::new("Nothing matches. Try another word, such as “export” or “theme”.").color(p.text_muted));
                        ui.add_space(6.0);
                    }
                    for (i, (_, cmd)) in matches.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Run {}", cmd.label)));
                        let on = i == app.palette.selected;
                        if on || resp.hovered() {
                            ui.painter().rect_filled(rect, 7, if on { p.accent_soft } else { p.surface_alt });
                        }
                        let fg = if on { p.accent } else { p.text };
                        ui.painter().text(rect.left_center() + egui::vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, cmd.glyph, egui::FontId::proportional(16.0), fg);
                        ui.painter().text(rect.left_center() + egui::vec2(40.0, 0.0), egui::Align2::LEFT_CENTER, &cmd.label, egui::FontId::proportional(13.5), fg);
                        if let Some(sc) = &cmd.shortcut {
                            ui.painter().text(rect.right_center() - egui::vec2(12.0, 0.0), egui::Align2::RIGHT_CENTER, sc, egui::FontId::proportional(11.5), p.text_muted);
                        }
                        if resp.clicked() {
                            run = Some(i);
                        }
                    }
                    ui.add_space(2.0);
                    ui.label(RichText::new("↑ ↓ to move  ·  Enter to run  ·  Esc to close").size(11.0).color(p.text_muted));
                });
        });
    if enter && !matches.is_empty() {
        run = Some(app.palette.selected);
    }
    let clicked_away = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|pos| !area.response.rect.contains(pos)));
    if let Some(i) = run {
        out.push(matches.swap_remove(i).1.action);
        app.palette.open = false;
    } else if escape || clicked_away {
        app.palette.open = false;
    }
    out
}

// --- shortcut sheet ---------------------------------------------------------------------

pub fn shortcuts_window(app: &mut GlideDeckApp, ctx: &egui::Context) {
    let cmd = |s: &egui::KeyboardShortcut| ctx.format_shortcut(s);
    let rows: Vec<(String, &str)> = vec![
        (cmd(&SC_PALETTE), "Search or run a command"),
        ("Scroll / pinch".into(), "Zoom at cursor"),
        ("Alt + scroll".into(), "Rotate view at cursor"),
        ("Space + drag · middle drag".into(), "Pan"),
        ("[  /  ]".into(), "Rotate view ∓15°"),
        ("Home / ".to_owned() + &cmd(&SC_FIT_ALL), "Fit all"),
        (cmd(&SC_FIT_SELECTION), "Zoom to selection"),
        (cmd(&SC_RESET_VIEW), "Reset view"),
        ("V H F R O T".into(), "Select, Hand, Frame, Rectangle, Ellipse, Text"),
        ("Drag with a tool".into(), "Draw the object (Alt: free frame aspect)"),
        (cmd(&SC_FRAME_VIEW), "Frame from current view"),
        ("PageUp / PageDown".into(), "Previous / next slide"),
        ("F5 / Shift+F5".into(), "Present from start / current slide"),
        ("Alt+F5".into(), "Presenter view: notes, next slide, timer"),
        ("Shift or Ctrl/⌘ + click".into(), "Add to / toggle selection"),
        ("Arrows (Shift ×10)".into(), "Nudge selection"),
        (cmd(&SC_CUT) + " " + &cmd(&SC_COPY) + " " + &cmd(&SC_PASTE), "Cut, copy, paste (images & files too)"),
        (cmd(&SC_INSERT_IMAGE), "Insert image"),
        (cmd(&SC_ICONS), "Icon library"),
        (cmd(&SC_LAYOUTS), "Slide layouts"),
        ("Alt while dragging".into(), "Move without smart-guide snapping"),
        (cmd(&SC_DUPLICATE), "Duplicate"),
        ("Delete / Backspace".into(), "Delete selection"),
        (cmd(&SC_BOLD) + " " + &cmd(&SC_ITALIC) + " " + &cmd(&SC_UNDERLINE), "Bold, italic, underline"),
        (cmd(&SC_SMALLER) + " / " + &cmd(&SC_BIGGER), "Smaller / larger text"),
        (cmd(&SC_CLEAR_FORMAT), "Clear text formatting"),
        ("B / W (in the show)".into(), "Black / white screen"),
        (cmd(&SC_UNDO) + " / " + &cmd(&SC_REDO), "Undo / Redo"),
        ("G".into(), "Toggle grid"),
    ];
    let mut open = app.show_shortcuts;
    egui::Window::new(RichText::new("Keyboard shortcuts").font(theme::semibold(15.0)))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Grid::new("shortcuts").num_columns(2).spacing([24.0, 8.0]).striped(true).show(ui, |ui| {
                for (k, v) in rows {
                    ui.label(RichText::new(k).font(theme::semibold(12.5)));
                    ui.label(v);
                    ui.end_row();
                }
            });
        });
    app.show_shortcuts = open;
}

/// The Arrange card for the selection: order, align, duplicate and delete.
pub(crate) fn arrange_card(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    let n = app.selection.len();
    card(ui, "Arrange", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
            let hint = if n == 1 { " (to its frame)" } else { "" };
            for (glyph, name, op) in ALIGN_OPS {
                let distribute = matches!(op, AlignOp::DistributeH | AlignOp::DistributeV);
                let tip = if distribute { name.to_owned() } else { format!("{name}{hint}") };
                if icon_button(ui, glyph, name, false, align_enabled(op, n), 28.0).on_hover_text(tip).clicked() {
                    out.push(Action::Align(op));
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for (glyph, name, action) in [
                (icon::ARROW_LINE_UP, "Bring to front", Action::BringToFront),
                (icon::ARROW_LINE_DOWN, "Send to back", Action::SendToBack),
                (icon::MAGNIFYING_GLASS_PLUS, "Zoom to", Action::FitSelection),
                (icon::COPY, "Duplicate", Action::Duplicate),
            ] {
                if ui.button(RichText::new(format!("{glyph}  {name}")).size(12.5)).clicked() {
                    out.push(action);
                }
            }
            if ui.button(RichText::new(format!("{}  Delete", icon::TRASH)).size(12.5).color(p.danger)).clicked() {
                out.push(Action::Delete);
            }
        });
    });
}
