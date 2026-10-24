//! The command ribbon (tabs → groups → buttons), status bar and shortcut sheet.

use crate::app::*;
use crate::theme::{self, icon};
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, ThemePreference};
use crate::design::AlignOp;
use glide_core::{Easing, ElementKind, ListStyle, Shadow, TextAlign, TextRole, TransitionStyle};

const BIG_H: f32 = 64.0;
/// Fixed ribbon body height so switching tabs never shifts the layout.
const BODY_H: f32 = BIG_H + 22.0;

/// A large icon-over-label button.
pub(crate) fn big(ui: &mut egui::Ui, glyph: &str, label: &str, active: bool, enabled: bool) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let font = egui::FontId::proportional(12.0);
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE).size().x);
    let size = egui::vec2((text_w + 16.0).max(54.0), BIG_H);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
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
        p.text_muted.gamma_multiply(0.55)
    } else if active {
        p.accent
    } else {
        p.text
    };
    ui.painter().text(rect.center_top() + egui::vec2(0.0, 24.0), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(23.0), if enabled && !active { p.accent } else { fg });
    ui.painter().text(rect.center_bottom() - egui::vec2(0.0, 12.0), egui::Align2::CENTER_CENTER, label, font, fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A compact icon + label button for stacked columns.
pub(crate) fn small(ui: &mut egui::Ui, glyph: &str, label: &str, active: bool, enabled: bool) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let text = RichText::new(format!("{glyph}  {label}")).size(12.5).color(if !enabled {
        p.text_muted.gamma_multiply(0.55)
    } else if active {
        p.accent
    } else {
        p.text
    });
    let mut b = egui::Button::new(text).frame(active).min_size(egui::vec2(0.0, 20.0));
    if active {
        b = b.fill(p.accent_soft).stroke(egui::Stroke::NONE);
    }
    ui.add_enabled(enabled, b)
}

pub(crate) fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    let p = theme::palette(ui.ctx());
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
        let r = ui
            .allocate_ui_with_layout(egui::vec2(f32::INFINITY, BIG_H), Layout::left_to_right(Align::Min), |ui| {
                ui.set_height(BIG_H);
                ui.spacing_mut().item_spacing.x = 4.0;
                add(ui)
            })
            .response;
        ui.allocate_ui_with_layout(egui::vec2(r.rect.width(), 16.0), Layout::top_down(Align::Center), |ui| {
            ui.label(RichText::new(title).size(11.0).color(p.text_muted));
        });
    });
    let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, BIG_H + 12.0), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range().shrink(6.0), egui::Stroke::new(1.0, p.border));
}

pub(crate) fn column(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
        ui.spacing_mut().interact_size.y = 20.0;
        add(ui)
    });
}

pub fn ribbon(app: &mut GlideDeckApp, ui: &mut egui::Ui) -> Vec<Action> {
    let mut out = Vec::new();
    let p = theme::palette(ui.ctx());

    // Tab strip.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(icon::SPARKLE).size(18.0).color(p.accent));
        ui.label(RichText::new("GlideDeck").font(theme::semibold(14.0)));
        ui.add_space(14.0);
        file_menu(app, ui, &mut out);
        ui.add_space(8.0);
        for (tab, label) in [
            (RibbonTab::Home, "Home"),
            (RibbonTab::Insert, "Insert"),
            (RibbonTab::Design, "Design"),
            (RibbonTab::Transitions, "Transitions"),
            (RibbonTab::Animate, "Animate"),
            (RibbonTab::Present, "Present"),
            (RibbonTab::View, "View"),
        ] {
            let selected = app.ribbon_tab == tab;
            let text = RichText::new(label).size(13.0).color(if selected { p.accent } else { p.text });
            let text = if selected { text.font(theme::semibold(13.0)) } else { text };
            let r = ui.add(egui::Button::new(text).frame(false).min_size(egui::vec2(0.0, 30.0)));
            ui.add_space(10.0);
            if selected {
                let y = r.rect.bottom() + 1.0;
                ui.painter().line_segment(
                    [egui::pos2(r.rect.left() + 6.0, y), egui::pos2(r.rect.right() - 6.0, y)],
                    egui::Stroke::new(2.5, p.accent),
                );
            }
            if r.clicked() {
                app.ribbon_tab = tab;
            }
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let present = egui::Button::new(RichText::new(format!("{}  Present", icon::PLAY)).color(p.on_accent).font(theme::semibold(13.0)))
                .fill(p.accent)
                .corner_radius(8)
                .min_size(egui::vec2(104.0, 30.0));
            if ui.add_enabled(!app.doc.path.is_empty(), present).on_hover_text("Present from the beginning (F5)").on_disabled_hover_text("Add a frame to create your first slide").clicked() {
                out.push(Action::Present { from_start: true });
            }
            if ui.add(egui::Button::new(RichText::new(icon::INFO).size(16.0)).frame(false).min_size(egui::vec2(30.0, 30.0))).on_hover_text("About GlideDeck · check for updates").clicked() {
                out.push(Action::ShowAbout);
            }
            let dark = ui.ctx().theme() == egui::Theme::Dark;
            let glyph = if dark { icon::SUN } else { icon::MOON };
            if ui.add(egui::Button::new(RichText::new(glyph).size(16.0)).frame(false).min_size(egui::vec2(30.0, 30.0))).on_hover_text("Toggle light / dark").clicked() {
                out.push(Action::SetTheme(if dark { ThemePreference::Light } else { ThemePreference::Dark }));
            }
            ui.add_space(6.0);
            let dirty = if app.is_dirty() { "  •  Edited" } else { "" };
            ui.label(RichText::new(format!("{}{dirty}", app.display_name())).color(p.text_muted).size(12.5));
        });
    });

    // Ribbon body.
    let body = egui::Frame::new().fill(p.surface).inner_margin(egui::Margin { left: 2, right: 2, top: 6, bottom: 6 });
    body.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.set_height(BODY_H);
        egui::ScrollArea::horizontal().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                match app.ribbon_tab {
                    RibbonTab::Home => home_tab(app, ui, &mut out),
                    RibbonTab::Insert => insert_tab(app, ui, &mut out),
                    RibbonTab::Design => design_tab(app, ui, &mut out),
                    RibbonTab::Transitions => transitions_tab(app, ui),
                    RibbonTab::Animate => crate::anim_ui::animate_tab(app, ui, &mut out),
                    RibbonTab::Present => present_tab(app, ui, &mut out),
                    RibbonTab::View => view_tab(app, ui, &mut out),
                }
            });
        });
    });
    let r = ui.max_rect();
    ui.painter().hline(r.x_range(), ui.min_rect().bottom(), egui::Stroke::new(1.0, p.border));
    out
}

fn file_menu(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    let ctx = ui.ctx().clone();
    let label = RichText::new("File").size(13.0).color(p.accent).font(theme::semibold(13.0));
    let mut recent_pick: Option<Action> = None;
    ui.menu_button(label, |ui| {
        ui.set_min_width(240.0);
        let mut item = |ui: &mut egui::Ui, glyph: &str, text: &str, sc: Option<egui::KeyboardShortcut>, a: Action| {
            let mut b = egui::Button::new(format!("{glyph}   {text}"));
            if let Some(sc) = sc {
                b = b.shortcut_text(ctx.format_shortcut(&sc));
            }
            if ui.add(b).clicked() {
                out.push(a);
                ui.close();
            }
        };
        item(ui, icon::FILE_PLUS, "New", Some(SC_NEW), Action::New);
        item(ui, icon::SQUARES_FOUR, "New from template…", None, Action::ShowTemplates);
        item(ui, icon::FOLDER_OPEN, "Open…", Some(SC_OPEN), Action::Open);
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
                    recent_pick = Some(Action::OpenRecent(path));
                    ui.close();
                }
            }
            if !app.recent.paths.is_empty() {
                ui.separator();
                if ui.button(format!("{}   Clear recent", icon::TRASH)).clicked() {
                    recent_pick = Some(Action::ClearRecent);
                    ui.close();
                }
            }
        });
        item(ui, icon::FILE_PPT, "Import PowerPoint…", None, Action::ImportPptx);
        ui.separator();
        item(ui, icon::FLOPPY_DISK, "Save", Some(SC_SAVE), Action::Save);
        item(ui, icon::FLOPPY_DISK, "Save As…", Some(SC_SAVE_AS), Action::SaveAs);
        ui.separator();
        item(ui, icon::FILE_PPT, "Export PowerPoint (Morph)…", None, Action::ExportPptx);
        item(ui, icon::FILE_PDF, "Export PDF…", None, Action::ExportPdf);
        item(ui, icon::FILE_HTML, "Export web page (HTML)…", None, Action::ExportHtml);
        item(ui, icon::FILE_IMAGE, "Export view as PNG…", None, Action::ExportPng);
        ui.separator();
        item(ui, icon::QUESTION, "Keyboard shortcuts", None, Action::ToggleShortcuts);
        item(ui, icon::ARROWS_CLOCKWISE, "Check for updates…", None, Action::CheckForUpdates);
        item(ui, icon::INFO, "About GlideDeck", None, Action::ShowAbout);
        if ui.button(format!("{}   Quit", icon::X)).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
    out.extend(recent_pick);
}

fn home_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let has_sel = !app.selection.is_empty();
    group(ui, "Clipboard", |ui| {
        if big(ui, icon::CLIPBOARD, "Paste", false, true).on_hover_text("Paste images, files, text or objects (Ctrl+V)").clicked() {
            out.push(Action::Paste);
        }
        column(ui, |ui| {
            if small(ui, icon::SCISSORS, "Cut", false, has_sel).clicked() {
                out.push(Action::Cut);
            }
            if small(ui, icon::COPY, "Copy", false, has_sel).clicked() {
                out.push(Action::Copy);
            }
            if small(ui, icon::COPY_SIMPLE, "Duplicate", false, has_sel).clicked() {
                out.push(Action::Duplicate);
            }
        });
    });
    group(ui, "Tools", |ui| {
        for t in [Tool::Select, Tool::Hand] {
            if big(ui, t.icon(), t.label(), app.tool == t, true).on_hover_text(format!("{} ({:?})", t.label(), t.key())).clicked() {
                out.push(Action::SetTool(t));
            }
        }
    });
    group(ui, "History", |ui| {
        column(ui, |ui| {
            if small(ui, icon::ARROW_COUNTER_CLOCKWISE, "Undo", false, app.history.can_undo() || app.is_dirty()).clicked() {
                out.push(Action::Undo);
            }
            if small(ui, icon::ARROW_CLOCKWISE, "Redo", false, app.history.can_redo()).clicked() {
                out.push(Action::Redo);
            }
        });
    });
    group(ui, "Arrange", |ui| {
        column(ui, |ui| {
            if small(ui, icon::ARROW_LINE_UP, "Bring to front", false, has_sel).clicked() {
                out.push(Action::BringToFront);
            }
            if small(ui, icon::ARROW_LINE_DOWN, "Send to back", false, has_sel).clicked() {
                out.push(Action::SendToBack);
            }
            if small(ui, icon::TRASH, "Delete", false, has_sel).clicked() {
                out.push(Action::Delete);
            }
        });
    });
    group(ui, "Align", |ui| {
        let n = app.selection.len();
        let single_hint = if n == 1 { " (to its frame)" } else { "" };
        let b = |ui: &mut egui::Ui, glyph: &str, tip: &str, enabled: bool| {
            ui.add_enabled(enabled, egui::Button::new(RichText::new(glyph).size(15.0)).frame(false).min_size(egui::vec2(26.0, 24.0)))
                .on_hover_text(tip)
                .clicked()
        };
        column(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (g, tip, op) in [
                    (icon::ALIGN_LEFT, "Align left", AlignOp::Left),
                    (icon::ALIGN_CENTER_HORIZONTAL, "Align centers", AlignOp::HCenter),
                    (icon::ALIGN_RIGHT, "Align right", AlignOp::Right),
                    (icon::COLUMNS, "Distribute horizontally", AlignOp::DistributeH),
                ] {
                    let distribute = matches!(op, AlignOp::DistributeH);
                    if b(ui, g, &format!("{tip}{}", if distribute { "" } else { single_hint }), if distribute { n >= 3 } else { n >= 1 }) {
                        out.push(Action::Align(op));
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (g, tip, op) in [
                    (icon::ALIGN_TOP, "Align top", AlignOp::Top),
                    (icon::ALIGN_CENTER_VERTICAL, "Align middles", AlignOp::VMiddle),
                    (icon::ALIGN_BOTTOM, "Align bottom", AlignOp::Bottom),
                    (icon::ROWS, "Distribute vertically", AlignOp::DistributeV),
                ] {
                    let distribute = matches!(op, AlignOp::DistributeV);
                    if b(ui, g, &format!("{tip}{}", if distribute { "" } else { single_hint }), if distribute { n >= 3 } else { n >= 1 }) {
                        out.push(Action::Align(op));
                    }
                }
            });
        });
    });
    // Quick text formatting for the selected text box.
    let text_id = match app.selection.as_slice() {
        [id] if matches!(app.doc.get(id).map(|e| &e.kind), Some(ElementKind::Text(_))) => Some(id.clone()),
        _ => None,
    };
    let Some(text_id) = text_id else { return };
    let role = match app.doc.get(&text_id).map(|e| &e.kind) {
        Some(ElementKind::Text(t)) => t.role,
        _ => None,
    };
    group(ui, "Text style", |ui| {
        column(ui, |ui| {
            for row in TextRole::ALL.chunks(2) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for r in row {
                        if ui.selectable_label(role == Some(*r), RichText::new(r.label()).size(12.0)).clicked() {
                            out.push(Action::ApplyTextRole(*r));
                        }
                    }
                });
            }
        });
    });
    group(ui, "Text format", |ui| {
        let format = app.current_format_state().unwrap_or_default();
        let Some(glide_core::Element { kind: ElementKind::Text(t), .. }) = app.doc.get_mut(&text_id) else { return };
        column(ui, |ui| {
            ui.scope(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.add(egui::DragValue::new(&mut t.font_size).speed(0.5).range(1.0..=f64::MAX).max_decimals(1));
                    if ui.small_button(icon::MINUS).clicked() {
                        t.font_size = (t.font_size / 1.15).max(1.0);
                    }
                    if ui.small_button(icon::PLUS).clicked() {
                        t.font_size *= 1.15;
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    use crate::textformat::TextFormat as F;
                    let cmd = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" };
                    for (on, glyph, f, tip) in [
                        (format.bold, icon::TEXT_B, F::Bold, format!("Bold ({cmd}B)")),
                        (format.italic, icon::TEXT_ITALIC, F::Italic, format!("Italic ({cmd}I)")),
                        (format.underline, icon::TEXT_UNDERLINE, F::Underline, format!("Underline ({cmd}U)")),
                        (format.strike, icon::TEXT_STRIKETHROUGH, F::Strike, format!("Strikethrough ({cmd}Shift+X)")),
                    ] {
                        if ui.selectable_label(on, RichText::new(glyph).size(14.0)).on_hover_text(tip).clicked() {
                            out.push(Action::FormatText(f));
                        }
                    }
                    let mut color = t.color.0;
                    if ui.color_edit_button_srgba_unmultiplied(&mut color).changed() {
                        out.push(Action::FormatText(F::Color(glide_core::Color(color))));
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (a, g) in [(TextAlign::Left, icon::TEXT_ALIGN_LEFT), (TextAlign::Center, icon::TEXT_ALIGN_CENTER), (TextAlign::Right, icon::TEXT_ALIGN_RIGHT)] {
                        ui.selectable_value(&mut t.align, a, RichText::new(g).size(14.0));
                    }
                    ui.separator();
                    for (l, g, tip) in [(ListStyle::Bullet, icon::LIST_BULLETS, "Bullets"), (ListStyle::Numbered, icon::LIST_NUMBERS, "Numbering")] {
                        let on = t.list == l;
                        if ui.selectable_label(on, RichText::new(g).size(14.0)).on_hover_text(tip).clicked() {
                            t.list = if on { ListStyle::None } else { l };
                        }
                    }
                });
            });
        });
    });
}

fn insert_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    group(ui, "Slides", |ui| {
        if big(ui, icon::FRAME_CORNERS, "Frame", app.tool == Tool::Frame, true).on_hover_text("Draw a frame — each frame on the path is a slide (F)").clicked() {
            out.push(Action::SetTool(Tool::Frame));
        }
        if big(ui, icon::SELECTION, "Frame view", false, true).on_hover_text("Create a frame from the current view (Ctrl+Shift+F)").clicked() {
            out.push(Action::FrameFromView);
        }
        if big(ui, icon::LAYOUT, "Layouts", app.show_layouts, true).on_hover_text("Insert a pre-designed slide (Ctrl+Shift+L)").clicked() {
            out.push(Action::ShowLayouts);
        }
    });
    group(ui, "Shapes", |ui| {
        for t in [Tool::Rect, Tool::Ellipse] {
            if big(ui, t.icon(), t.label(), app.tool == t, true).on_hover_text(format!("Drag to draw ({:?})", t.key())).clicked() {
                out.push(Action::SetTool(t));
            }
        }
    });
    group(ui, "Text", |ui| {
        if big(ui, icon::TEXT_T, "Text box", app.tool == Tool::Text, true).on_hover_text("Click to type, drag for a wrapping box (T)").clicked() {
            out.push(Action::SetTool(Tool::Text));
        }
        column(ui, |ui| {
            for (g, role) in [(icon::TEXT_H_ONE, TextRole::Title), (icon::TEXT_H_TWO, TextRole::Heading), (icon::PARAGRAPH, TextRole::Body)] {
                if small(ui, g, role.label(), false, true).on_hover_text(format!("Insert a {} styled by the theme", role.label().to_lowercase())).clicked() {
                    out.push(Action::InsertText(role));
                }
            }
        });
    });
    group(ui, "Library", |ui| {
        if big(ui, icon::SHAPES, "Icons", app.icon_picker.open, true).on_hover_text("1,500 vector icons in four weights (Ctrl+Shift+K)").clicked() {
            out.push(Action::ShowIcons);
        }
    });
    group(ui, "Media", |ui| {
        if big(ui, icon::IMAGE, "Image", false, true).on_hover_text("Insert pictures from this computer (Ctrl+Shift+I)").clicked() {
            out.push(Action::InsertImage);
        }
        if big(ui, icon::FILM_STRIP, "Video", false, true).on_hover_text("Embed a video (MP4, MOV, WebM…) — plays in the editor and the show").clicked() {
            out.push(Action::InsertVideo);
        }
        if big(ui, icon::BEZIER_CURVE, "SVG", false, true).on_hover_text("Insert a vector graphic — stays crisp at any zoom").clicked() {
            out.push(Action::InsertImage);
        }
    });
}

fn design_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    group(ui, "Start", |ui| {
        if big(ui, icon::SQUARES_FOUR, "Templates", app.show_templates, true).on_hover_text("Start a new presentation from a template").clicked() {
            out.push(Action::ShowTemplates);
        }
    });
    group(ui, "Slides", |ui| {
        if big(ui, icon::PATH, "Arrange", app.arrange_dialog.is_some(), !app.doc.path.is_empty())
            .on_hover_text("Rearrange all slides into a spiral, orbits, a tree, a zoom tunnel…")
            .clicked()
        {
            out.push(Action::ShowArrange);
        }
    });
    group(ui, "Themes", |ui| {
        crate::galleries::themes_row(app, ui, out);
    });
    group(ui, "Fonts", |ui| {
        column(ui, |ui| {
            let mut theme = app.theme();
            let used = crate::fontpick::used_families(&app.doc);
            let (mut heading, mut body) = (theme.heading_font.clone(), theme.body_font.clone());
            let mut add = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Headings").size(12.0).color(p.text_muted));
                let r = crate::fontpick::font_picker(ui, &mut app.renderer.text, "heading-font", &mut heading, &used, 120.0);
                add |= r.add_file;
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Body").size(12.0).color(p.text_muted));
                ui.add_space(22.0);
                let r = crate::fontpick::font_picker(ui, &mut app.renderer.text, "body-font", &mut body, &used, 120.0);
                add |= r.add_file;
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
    });
    group(ui, "Background", |ui| {
        column(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::PAINT_BUCKET).size(14.0));
                ui.color_edit_button_srgba_unmultiplied(&mut app.doc.background.0);
            });
            let mut gradient = app.doc.background_to.is_some();
            ui.horizontal(|ui| {
                if ui.checkbox(&mut gradient, RichText::new("Gradient").size(12.0)).changed() {
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
    });
    let selection_supports_fx = app.selection.iter().any(|id| {
        app.doc.get(id).is_some_and(|e| matches!(e.kind, ElementKind::Rect(_) | ElementKind::Ellipse(_) | ElementKind::Image(_) | ElementKind::Frame(_)))
    });
    let current_shadow = app.selection.first().and_then(|id| app.doc.get(id)).and_then(|e| e.shadow);
    group(ui, "Effects", |ui| {
        column(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (label, s) in [("None", None), ("Soft", Some(Shadow::soft())), ("Lifted", Some(Shadow::lifted())), ("Crisp", Some(Shadow::crisp()))] {
                    let active = current_shadow == s;
                    if ui.add_enabled(selection_supports_fx, egui::Button::selectable(active, RichText::new(label).size(12.0))).clicked() {
                        out.push(Action::SetShadow(s));
                    }
                }
            });
            ui.label(RichText::new("Shadow").size(11.0).color(p.text_muted));
            let has_gradient = app.selection.first().and_then(|id| app.doc.get(id)).is_some_and(|e| match &e.kind {
                ElementKind::Rect(r) => r.gradient.is_some(),
                ElementKind::Ellipse(e) => e.gradient.is_some(),
                _ => false,
            });
            let shape_selected = app.selection.iter().any(|id| app.doc.get(id).is_some_and(|e| matches!(e.kind, ElementKind::Rect(_) | ElementKind::Ellipse(_))));
            if small(ui, icon::GRADIENT, "Gradient fill", has_gradient, shape_selected).clicked() {
                out.push(Action::ToggleGradient);
            }
        });
    });
    crate::galleries::refresh_design_check(app);
    let count = app.design_check.issues.len();
    group(ui, "Review", |ui| {
        let r = big(ui, icon::SEAL_CHECK, "Design check", app.design_check.open, true)
            .on_hover_text("Contrast, readability, empty slides, missing media and font consistency");
        if count > 0 {
            let badge = egui::Rect::from_center_size(r.rect.right_top() + egui::vec2(-12.0, 10.0), egui::vec2(20.0, 16.0));
            ui.painter().rect_filled(badge, 8, p.danger);
            ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, count.min(99).to_string(), egui::FontId::proportional(10.5), Color32::WHITE);
        }
        if r.clicked() {
            out.push(Action::ToggleDesignCheck);
        }
    });
}

fn transitions_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui) {
    let p = theme::palette(ui.ctx());
    let Some(index) = app.current_step.filter(|i| *i < app.doc.path.len()) else {
        ui.add_space(8.0);
        ui.label(RichText::new("Select a slide to set how the camera travels into it.").color(p.text_muted));
        return;
    };
    let mut t = app.doc.path[index].transition;
    let mut apply_all = false;
    group(ui, "Camera move", |ui| {
        for style in TransitionStyle::ALL {
            if big(ui, theme::transition_icon(style), style.label(), t.style == style, true).on_hover_text(style.description()).clicked() {
                t.style = style;
            }
        }
    });
    group(ui, "Timing", |ui| {
        column(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::CLOCK).size(14.0));
                ui.add(egui::Slider::new(&mut t.duration, 0.2..=5.0).suffix(" s").fixed_decimals(1));
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::BEZIER_CURVE).size(14.0));
                egui::ComboBox::from_id_salt("easing").selected_text(easing_name(t.easing)).show_ui(ui, |ui| {
                    for e in [Easing::EaseInOutCubic, Easing::EaseInOutSine, Easing::EaseInOutQuint, Easing::EaseOutExpo, Easing::Linear] {
                        ui.selectable_value(&mut t.easing, e, easing_name(e));
                    }
                });
            });
        });
    });
    group(ui, "Apply", |ui| {
        column(ui, |ui| {
            apply_all = small(ui, icon::SQUARES_FOUR, "Apply to all slides", false, true).clicked();
            ui.label(RichText::new(format!("Slide {} of {}", index + 1, app.doc.path.len())).size(11.5).color(p.text_muted));
        });
    });
    if apply_all {
        for s in &mut app.doc.path {
            s.transition = t;
        }
    } else {
        app.doc.path[index].transition = t;
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

fn present_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let has_path = !app.doc.path.is_empty();
    group(ui, "Start", |ui| {
        if big(ui, icon::PLAY, "From start", false, has_path).on_hover_text("F5").clicked() {
            out.push(Action::Present { from_start: true });
        }
        if big(ui, icon::MONITOR_PLAY, "From current", false, has_path).on_hover_text("Shift+F5").clicked() {
            out.push(Action::Present { from_start: false });
        }
        if big(ui, icon::PRESENTATION_CHART, "Presenter view", false, has_path)
            .on_hover_text("Alt+F5 · your notes, the next slide and a timer on your screen; the show on the other display")
            .clicked()
        {
            out.push(Action::PresenterView { from_start: false });
        }
    });
    group(ui, "Prepare", |ui| {
        if big(ui, icon::NOTE, "Speaker notes", app.show_notes, true).on_hover_text("Write notes for each slide in the panel under the canvas").clicked() {
            out.push(Action::ToggleNotes);
        }
    });
    group(ui, "Export", |ui| {
        if big(ui, icon::FILE_PPT, "PowerPoint", false, true)
            .on_hover_text("One slide per frame, animated with PowerPoint Morph")
            .clicked()
        {
            out.push(Action::ExportPptx);
        }
        if big(ui, icon::FILE_PDF, "PDF", false, !app.doc.path.is_empty()).on_hover_text("One page per slide, in show order").clicked() {
            out.push(Action::ExportPdf);
        }
        if big(ui, icon::FILE_HTML, "Web page", false, !app.doc.path.is_empty())
            .on_hover_text("A single HTML file that plays exactly like the show (transitions, animations, parallax) in any browser")
            .clicked()
        {
            out.push(Action::ExportHtml);
        }
        if big(ui, icon::FILE_IMAGE, "PNG image", false, true).on_hover_text("Export the current view at 2×").clicked() {
            out.push(Action::ExportPng);
        }
    });
}

fn view_tab(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    group(ui, "Navigate", |ui| {
        if big(ui, icon::CORNERS_OUT, "Fit all", false, true).on_hover_text("Home").clicked() {
            out.push(Action::FitAll);
        }
        column(ui, |ui| {
            if small(ui, icon::MAGNIFYING_GLASS_PLUS, "Zoom to selection", false, !app.selection.is_empty()).clicked() {
                out.push(Action::FitSelection);
            }
            if small(ui, icon::ARROWS_CLOCKWISE, "Reset view", false, true).clicked() {
                out.push(Action::ResetView);
            }
        });
    });
    group(ui, "Show", |ui| {
        column(ui, |ui| {
            if small(ui, icon::DOTS_NINE, "Grid", app.show_grid, true).clicked() {
                out.push(Action::ToggleGrid);
            }
            if small(ui, icon::FRAME_CORNERS, "Frame outlines", app.show_frames, true).clicked() {
                out.push(Action::ToggleFrameOutlines);
            }
            if small(ui, icon::HAND, "Trackpad mode", app.scroll_pans, true).on_hover_text("Two-finger scroll pans; Ctrl/⌘+scroll zooms").clicked() {
                app.scroll_pans = !app.scroll_pans;
            }
        });
    });
    group(ui, "Panels", |ui| {
        column(ui, |ui| {
            if small(ui, icon::SLIDESHOW, "Slides", app.show_slides, true).clicked() {
                out.push(Action::ToggleSlides);
            }
            if small(ui, icon::SIDEBAR_SIMPLE, "Properties", app.show_inspector, true).clicked() {
                out.push(Action::ToggleInspector);
            }
            if small(ui, icon::NOTE, "Speaker notes", app.show_notes, true).clicked() {
                out.push(Action::ToggleNotes);
            }
        });
    });
    group(ui, "Theme", |ui| {
        let pref = ui.ctx().options(|o| o.theme_preference);
        column(ui, |ui| {
            for (t, g, l) in [(ThemePreference::Light, icon::SUN, "Light"), (ThemePreference::Dark, icon::MOON, "Dark"), (ThemePreference::System, icon::CIRCLE_HALF, "System")] {
                if small(ui, g, l, pref == t, true).clicked() {
                    out.push(Action::SetTheme(t));
                }
            }
        });
    });
}

pub fn status_bar(app: &mut GlideDeckApp, ui: &mut egui::Ui) {
    let p = theme::palette(ui.ctx());
    let muted = |s: String| RichText::new(s).size(12.0).color(p.text_muted);
    ui.horizontal(|ui| {
        let slide = match app.current_step {
            Some(i) if !app.doc.path.is_empty() => format!("Slide {} of {}", i + 1, app.doc.path.len()),
            _ => format!("{} slides", app.doc.path.len()),
        };
        ui.label(muted(format!("{}  {slide}", icon::SLIDESHOW)));
        ui.label(muted("·".into()));
        ui.label(muted(format!("{} objects", app.doc.elements.len())));
        if !app.selection.is_empty() {
            ui.label(muted("·".into()));
            ui.label(RichText::new(format!("{} selected", app.selection.len())).size(12.0).color(p.accent));
        }
        if let Some((msg, until)) = app.toast().cloned()
            && ui.input(|i| i.time) < until
        {
            ui.add_space(12.0);
            ui.label(RichText::new(format!("{}  {msg}", icon::CHECK_CIRCLE)).size(12.0).color(p.text));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(muted(format!(
                "{} drawn · {} tris",
                app.stats.drawn, app.stats.triangles
            )))
            .on_hover_text(format!("{} culled · {} cached meshes", app.stats.culled, app.stats.cached_meshes));
            ui.label(muted("·".into()));
            ui.label(muted(format!("{:.0}°", app.camera.rotation.to_degrees())));
            ui.label(muted(format_zoom(app.camera.zoom)));
            if let Some(w) = app.cursor_world {
                ui.label(muted(format!("{:.1}, {:.1}", w.x, w.y)));
            }
        });
    });
}

pub fn shortcuts_window(app: &mut GlideDeckApp, ctx: &egui::Context) {
    let cmd = |s: &egui::KeyboardShortcut| ctx.format_shortcut(s);
    let rows: Vec<(String, &str)> = vec![
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
