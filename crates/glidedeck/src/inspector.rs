//! Right-hand panel: properties of the selection (or the document), slide
//! settings for frames, and the layer list.

use crate::app::{Action, GlideDeckApp};
use crate::ribbon::easing_name;
use crate::theme::{self, icon};
use eframe::egui::{self, DragValue, RichText};
use glide_core::{assets, Color, Easing, ElementKind, Gradient, IconStyle, Shadow, Stroke, TextAlign, TransitionStyle};

const SWATCHES: [Color; 8] = [
    Color::rgb(0xF7, 0xF6, 0xF2),
    Color::rgb(0xFF, 0xFF, 0xFF),
    Color::rgb(0xEE, 0xF2, 0xFF),
    Color::rgb(0xEC, 0xFD, 0xF5),
    Color::rgb(0xFF, 0xF7, 0xED),
    Color::rgb(0x1F, 0x23, 0x2B),
    Color::rgb(0x0B, 0x10, 0x20),
    Color::rgb(0x2A, 0x1F, 0x3D),
];

pub(crate) fn card(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    let p = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(p.surface_alt)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            theme::caption(ui, title);
            ui.add_space(4.0);
            add(ui);
        });
    ui.add_space(8.0);
}

fn grid(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 7.0]).min_col_width(72.0).show(ui, add);
}

fn label(ui: &mut egui::Ui, text: &str) {
    let p = theme::palette(ui.ctx());
    ui.label(RichText::new(text).color(p.text_muted));
}

fn color_edit(ui: &mut egui::Ui, c: &mut Color) {
    ui.color_edit_button_srgba_unmultiplied(&mut c.0);
}

thread_local! {
    /// Theme colors offered next to every color picker in the inspector.
    static THEME_SWATCHES: std::cell::RefCell<Vec<Color>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A color button followed by one-click theme swatches.
fn color_pick(ui: &mut egui::Ui, c: &mut Color) {
    let p = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        color_edit(ui, c);
        let swatches = THEME_SWATCHES.with(|s| s.borrow().clone());
        for sw in swatches {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::click());
            ui.painter().circle_filled(rect.center(), 6.5, egui::Color32::from_rgb(sw.r(), sw.g(), sw.b()));
            ui.painter().circle_stroke(rect.center(), 6.5, egui::Stroke::new(1.0, p.border));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                *c = Color::rgba(sw.r(), sw.g(), sw.b(), c.a());
            }
        }
    });
}

fn optional_color(ui: &mut egui::Ui, name: &str, c: &mut Option<Color>, default: Color) {
    label(ui, name);
    ui.horizontal(|ui| {
        let mut on = c.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *c = on.then_some(default);
        }
        if let Some(c) = c {
            color_pick(ui, c);
        }
    });
    ui.end_row();
}

fn optional_stroke(ui: &mut egui::Ui, name: &str, s: &mut Option<Stroke>, default_width: f64) {
    label(ui, name);
    ui.horizontal(|ui| {
        let mut on = s.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *s = on.then(|| Stroke { color: Color::rgb(0x1f, 0x23, 0x2b), width: default_width });
        }
        if let Some(s) = s {
            color_edit(ui, &mut s.color);
            ui.add(DragValue::new(&mut s.width).speed(0.1).range(0.0..=f64::MAX).prefix("w "));
        }
    });
    ui.end_row();
}

fn size_row(ui: &mut egui::Ui, w: &mut f64, h: &mut f64) {
    label(ui, "Size");
    ui.horizontal(|ui| {
        ui.add(DragValue::new(w).speed(1.0).range(0.0..=f64::MAX).prefix("W "));
        ui.add(DragValue::new(h).speed(1.0).range(0.0..=f64::MAX).prefix("H "));
    });
    ui.end_row();
}

pub fn inspector(app: &mut GlideDeckApp, ui: &mut egui::Ui) -> Vec<Action> {
    let mut actions = Vec::new();
    let p = theme::palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new("Properties").font(theme::semibold(15.0)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let chip = match app.selection.as_slice() {
                [] => "Document".to_owned(),
                [id] => app.doc.get(id).map(|e| e.kind.type_name().to_owned()).unwrap_or_default(),
                many => format!("{} objects", many.len()),
            };
            egui::Frame::new().fill(p.accent_soft).corner_radius(6).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                ui.label(RichText::new(chip).size(11.5).color(p.accent));
            });
        });
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        match app.selection.len() {
            0 => document_panel(app, ui),
            1 => element_panel(app, ui, &mut actions),
            n => {
                card(ui, &format!("{n} objects"), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(format!("{}  Frame them", icon::FRAME_CORNERS)).on_hover_text("Wrap the selection in a new frame and add it to the path").clicked() {
                            app.frame_around_selection();
                        }
                        if ui.button(format!("{}  Zoom to", icon::MAGNIFYING_GLASS_PLUS)).clicked() {
                            actions.push(Action::FitSelection);
                        }
                        if ui.button(format!("{}  Duplicate", icon::COPY)).clicked() {
                            actions.push(Action::Duplicate);
                        }
                        if ui.button(RichText::new(format!("{}  Delete", icon::TRASH)).color(p.danger)).clicked() {
                            actions.push(Action::Delete);
                        }
                    });
                });
            }
        }
        layers(app, ui);
    });
    actions
}

fn document_panel(app: &mut GlideDeckApp, ui: &mut egui::Ui) {
    let p = theme::palette(ui.ctx());
    card(ui, "Presentation", |ui| {
        grid(ui, "doc", |ui| {
            label(ui, "Title");
            crate::bidi::singleline(ui, "doc-title", &mut app.doc.title, |e| e.hint_text("Untitled").desired_width(f32::INFINITY));
            ui.end_row();
            label(ui, "Slides");
            ui.label(format!("{}", app.doc.path.len()));
            ui.end_row();
        });
    });
    card(ui, "Canvas background", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for c in SWATCHES {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
                let selected = app.doc.background == c;
                ui.painter().rect_filled(rect, 6, egui::Color32::from_rgb(c.r(), c.g(), c.b()));
                let stroke = if selected { egui::Stroke::new(2.0, p.accent) } else { egui::Stroke::new(1.0, p.border) };
                ui.painter().rect_stroke(rect, 6, stroke, egui::StrokeKind::Outside);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    app.doc.background = c;
                }
            }
            color_edit(ui, &mut app.doc.background);
        });
    });
    card(ui, "Tips", |ui| {
        for (g, t) in [
            (icon::FRAME_CORNERS, "Draw frames (F) to build slides. They stay invisible in the show unless you give them a border."),
            (icon::CLIPBOARD, "Paste screenshots, copied files or text with Ctrl/⌘+V, or drop images onto the canvas."),
            (icon::FILE_PPT, "Present → PowerPoint exports one slide per frame, linked with Morph."),
        ] {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(g).size(15.0).color(p.accent));
                ui.add(egui::Label::new(RichText::new(t).size(12.5).color(p.text_muted)).wrap());
            });
            ui.add_space(2.0);
        }
    });
}

fn element_panel(app: &mut GlideDeckApp, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    let p = theme::palette(ui.ctx());
    let id = app.selection[0].clone();
    let zoom = app.camera.zoom;
    let theme_now = app.theme();
    THEME_SWATCHES.with(|s| *s.borrow_mut() = theme_now.swatches());
    let mut change_icon = false;
    let focus_text = std::mem::take(&mut app.focus_text_editor);
    let steps = app.steps_of(&id);
    let base = app.base_dir();
    let mut replace_image = false;
    let mut video_status: Option<glide_core::Media> = None;
    let mut add_to_path = false;
    let used_fonts = crate::fontpick::used_families(&app.doc);
    let mut add_font = false;
    {
        let Some(el) = app.doc.get_mut(&id) else { return };

        card(ui, "Object", |ui| {
            grid(ui, "object", |ui| {
                label(ui, "Name");
                let hint = crate::bidi::hint(ui, &el.display_name());
                crate::bidi::singleline(ui, "element-name", &mut el.name, |e| e.hint_text(hint).desired_width(f32::INFINITY));
                ui.end_row();
                label(ui, "Id");
                ui.label(RichText::new(&el.id).monospace().size(11.5).color(p.text_muted));
                ui.end_row();
            });
        });

        card(ui, "Layout", |ui| {
            grid(ui, "layout", |ui| {
                let t = &mut el.transform;
                label(ui, "Position");
                ui.horizontal(|ui| {
                    let speed = 1.0 / zoom;
                    ui.add(DragValue::new(&mut t.x).speed(speed).prefix("X ").max_decimals(2));
                    ui.add(DragValue::new(&mut t.y).speed(speed).prefix("Y ").max_decimals(2));
                });
                ui.end_row();
                label(ui, "Scale");
                let speed = (t.scale.abs() * 0.01).max(1e-12);
                ui.add(DragValue::new(&mut t.scale).speed(speed).range(1e-12..=1e12).max_decimals(6));
                ui.end_row();
                label(ui, "Rotation");
                ui.add(DragValue::new(&mut t.rotation).speed(0.5).suffix("°").max_decimals(1));
                ui.end_row();
                label(ui, "Opacity");
                ui.add(egui::Slider::new(&mut el.opacity, 0.0..=1.0).show_value(true));
                ui.end_row();
                label(ui, "Depth");
                ui.add(egui::Slider::new(&mut el.parallax, -0.5..=0.9).fixed_decimals(2))
                    .on_hover_text("Parallax in the show: above 0 the object is far away and drifts slower than the slides; below 0 it is near and moves faster. 0 = on the canvas.");
                ui.end_row();
                label(ui, "");
                ui.horizontal(|ui| {
                    ui.toggle_value(&mut el.locked, format!("{} Lock", icon::LOCK));
                    ui.toggle_value(&mut el.hidden, format!("{} Hide", icon::EYE_SLASH));
                });
                ui.end_row();
            });
        });

        match &mut el.kind {
            ElementKind::Rect(r) => card(ui, "Shape", |ui| {
                grid(ui, "rect", |ui| {
                    size_row(ui, &mut r.width, &mut r.height);
                    label(ui, "Corners");
                    ui.add(DragValue::new(&mut r.corner_radius).speed(0.5).range(0.0..=f64::MAX));
                    ui.end_row();
                    optional_color(ui, "Fill", &mut r.fill, Color::rgb(0x5B, 0x4F, 0xE9));
                    optional_stroke(ui, "Outline", &mut r.stroke, 4.0);
                });
            }),
            ElementKind::Ellipse(e) => card(ui, "Shape", |ui| {
                grid(ui, "ellipse", |ui| {
                    size_row(ui, &mut e.width, &mut e.height);
                    optional_color(ui, "Fill", &mut e.fill, Color::rgb(0x17, 0xA3, 0x98));
                    optional_stroke(ui, "Outline", &mut e.stroke, 4.0);
                });
            }),
            ElementKind::Frame(f) => card(ui, "Frame", |ui| {
                // Appearance: invisible (default), bordered, or filled.
                let mut mode = if f.is_invisible() { 0 } else if f.fill.is_some() { 2 } else { 1 };
                let before = mode;
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut mode, 0, format!("{} Invisible", icon::EYE_SLASH));
                    ui.selectable_value(&mut mode, 1, format!("{} Border", icon::SQUARE));
                    ui.selectable_value(&mut mode, 2, format!("{} Filled", icon::PAINT_BUCKET));
                });
                if mode != before {
                    match mode {
                        0 => {
                            f.border = None;
                            f.fill = None;
                        }
                        1 => {
                            f.fill = None;
                            f.border.get_or_insert(Stroke { color: Color::rgb(0x5B, 0x4F, 0xE9), width: 3.0 });
                        }
                        _ => {
                            f.fill.get_or_insert(Color::rgba(0x5B, 0x4F, 0xE9, 0x18));
                        }
                    }
                }
                ui.label(
                    RichText::new(if mode == 0 {
                        "Only outlined in the editor on hover or selection — never in the show or export."
                    } else {
                        "Visible in the editor, the show and the PowerPoint export."
                    })
                    .size(11.5)
                    .color(p.text_muted),
                );
                ui.add_space(4.0);
                grid(ui, "frame", |ui| {
                    size_row(ui, &mut f.width, &mut f.height);
                    label(ui, "Aspect");
                    ui.horizontal(|ui| {
                        for (name, ratio) in [("16:9", 16.0 / 9.0), ("4:3", 4.0 / 3.0), ("1:1", 1.0)] {
                            if ui.small_button(name).clicked() {
                                f.height = f.width / ratio;
                            }
                        }
                    });
                    ui.end_row();
                    if mode != 0 {
                        optional_stroke(ui, "Border", &mut f.border, 3.0);
                        optional_color(ui, "Fill", &mut f.fill, Color::rgba(0x5B, 0x4F, 0xE9, 0x18));
                        label(ui, "Corners");
                        ui.add(DragValue::new(&mut f.corner_radius).speed(0.5).range(0.0..=f64::MAX));
                        ui.end_row();
                    }
                });
            }),
            ElementKind::Text(t) => card(ui, "Text", |ui| {
                // Edit a copy so formatted spans move with the text that changed.
                let mut content = t.content.clone();
                let r = crate::bidi::multiline(ui, "text-content", &mut content, |e| e.desired_rows(3).desired_width(f32::INFINITY));
                if r.changed() {
                    t.set_content(content);
                }
                if focus_text {
                    r.request_focus();
                }
                ui.label(
                    RichText::new(format!("{}  Double-click the text on the canvas (or press Enter) to edit it in place.", icon::CURSOR_TEXT))
                        .size(11.5)
                        .color(p.text_muted),
                );
                ui.add_space(4.0);
                grid(ui, "text", |ui| {
                    label(ui, "Font");
                    ui.horizontal(|ui| {
                        let picked = crate::fontpick::font_picker(ui, &mut app.renderer.text, "font", &mut t.font_family, &used_fonts, 120.0);
                        add_font |= picked.add_file;
                        ui.add(DragValue::new(&mut t.font_size).speed(0.5).range(0.01..=f64::MAX).max_decimals(1));
                    });
                    ui.end_row();
                    label(ui, "Style");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("weight").selected_text(weight_name(t.weight)).width(96.0).show_ui(ui, |ui| {
                            for w in [300, 400, 500, 600, 700, 800] {
                                ui.selectable_value(&mut t.weight, w, weight_name(w));
                            }
                        });
                        ui.toggle_value(&mut t.italic, RichText::new(icon::TEXT_ITALIC).size(14.0));
                    });
                    ui.end_row();
                    label(ui, "Color");
                    color_pick(ui, &mut t.color);
                    ui.end_row();
                    label(ui, "Role");
                    egui::ComboBox::from_id_salt("role")
                        .selected_text(t.role.map(|r| r.label()).unwrap_or("Custom"))
                        .show_ui(ui, |ui| {
                            for r in glide_core::TextRole::ALL {
                                if ui.selectable_label(t.role == Some(r), r.label()).clicked() {
                                    theme_now.apply_role(t, r);
                                }
                            }
                            if ui.selectable_label(t.role.is_none(), "Custom").clicked() {
                                t.role = None;
                            }
                        });
                    ui.end_row();
                    label(ui, "Align");
                    ui.horizontal(|ui| {
                        for (a, g) in [(TextAlign::Left, icon::TEXT_ALIGN_LEFT), (TextAlign::Center, icon::TEXT_ALIGN_CENTER), (TextAlign::Right, icon::TEXT_ALIGN_RIGHT)] {
                            ui.selectable_value(&mut t.align, a, RichText::new(g).size(14.0));
                        }
                    });
                    ui.end_row();
                    label(ui, "List");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut t.list, glide_core::ListStyle::None, "None");
                        ui.selectable_value(&mut t.list, glide_core::ListStyle::Bullet, format!("{} Bullets", icon::LIST_BULLETS));
                        ui.selectable_value(&mut t.list, glide_core::ListStyle::Numbered, format!("{} 1, 2, 3", icon::LIST_NUMBERS));
                    });
                    ui.end_row();
                    label(ui, "Line height");
                    ui.add(DragValue::new(&mut t.line_height).speed(0.01).range(0.5..=4.0).max_decimals(2));
                    ui.end_row();
                    label(ui, "Wrap");
                    ui.horizontal(|ui| {
                        let mut wrap = t.max_width.is_some();
                        if ui.checkbox(&mut wrap, "").changed() {
                            t.max_width = wrap.then_some(t.font_size * 12.0);
                        }
                        if let Some(w) = &mut t.max_width {
                            ui.add(DragValue::new(w).speed(1.0).range(1.0..=f64::MAX).prefix("W "));
                        }
                    });
                    ui.end_row();
                });
            }),
            ElementKind::Image(m) => card(ui, "Image", |ui| {
                let resolved = assets::resolve(Some(&base), &m.path);
                let exists = resolved.exists();
                grid(ui, "image", |ui| {
                    label(ui, "File");
                    ui.label(RichText::new(&m.path).monospace().size(11.0).color(if exists { p.text_muted } else { p.danger }))
                        .on_hover_text(resolved.display().to_string());
                    ui.end_row();
                    size_row(ui, &mut m.width, &mut m.height);
                });
                grid(ui, "image-mask", |ui| {
                    label(ui, "Corners");
                    let max = m.width.min(m.height) / 2.0;
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut m.corner_radius, 0.0..=max.max(1.0)).show_value(false));
                        if ui.small_button("Circle").on_hover_text("Square crop with a circular mask").clicked() {
                            let side = m.width.min(m.height);
                            m.width = side;
                            m.height = side;
                            m.corner_radius = side / 2.0;
                        }
                    });
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.button(format!("{}  Replace…", icon::IMAGE)).clicked() {
                        replace_image = true;
                    }
                    if ui.button(format!("{}  Original size", icon::ARROWS_OUT_SIMPLE)).clicked()
                        && let Some((w, h)) = glide_render::image_dimensions(&resolved)
                    {
                        m.width = w as f64;
                        m.height = h as f64;
                    }
                });
            }),
            ElementKind::Icon(ic) => card(ui, "Icon", |ui| {
                ui.horizontal(|ui| {
                    let ch = glide_render::icon_codepoint(&ic.name).map(|c| c.to_string()).unwrap_or_else(|| "?".into());
                    ui.label(
                        RichText::new(ch)
                            .family(crate::galleries::icon_family(ic.style))
                            .size(40.0)
                            .color(egui::Color32::from_rgb(ic.color.r(), ic.color.g(), ic.color.b())),
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&ic.name).font(theme::semibold(13.0)));
                        if ui.button(format!("{}  Change icon…", icon::SHAPES)).clicked() {
                            change_icon = true;
                        }
                    });
                });
                grid(ui, "icon", |ui| {
                    label(ui, "Weight");
                    ui.horizontal(|ui| {
                        for st in IconStyle::ALL {
                            ui.selectable_value(&mut ic.style, st, st.label());
                        }
                    });
                    ui.end_row();
                    label(ui, "Color");
                    color_pick(ui, &mut ic.color);
                    ui.end_row();
                    label(ui, "Size");
                    ui.add(DragValue::new(&mut ic.size).speed(0.5).range(1.0..=f64::MAX));
                    ui.end_row();
                });
            }),
            ElementKind::Svg(m) => card(ui, "Vector graphic", |ui| {
                grid(ui, "svg", |ui| {
                    label(ui, "File");
                    ui.label(RichText::new(&m.path).monospace().size(11.0).color(p.text_muted));
                    ui.end_row();
                    size_row(ui, &mut m.width, &mut m.height);
                });
                ui.label(RichText::new("Rendered as true vectors — crisp at any zoom, exported to PowerPoint as SVG.").size(11.5).color(p.text_muted));
            }),
            ElementKind::Video(m) => card(ui, "Video", |ui| {
                video_status = Some(m.clone());
                grid(ui, "video", |ui| {
                    label(ui, "File");
                    ui.label(RichText::new(&m.path).monospace().size(11.0).color(p.text_muted));
                    ui.end_row();
                    size_row(ui, &mut m.width, &mut m.height);
                    label(ui, "Corners");
                    let max = m.width.min(m.height) / 2.0;
                    ui.add(egui::Slider::new(&mut m.corner_radius, 0.0..=max.max(1.0)).show_value(false));
                    ui.end_row();
                    label(ui, "Playback");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut m.autoplay, "Autoplay in show");
                    });
                    ui.end_row();
                    label(ui, "");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut m.looping, "Loop");
                        ui.checkbox(&mut m.muted, "Mute");
                    });
                    ui.end_row();
                });
            }),
        }

        // Effects: shadow (shapes, visible frames, images) and gradient (shapes).
        let shadowable = match &el.kind {
            ElementKind::Rect(_) | ElementKind::Ellipse(_) | ElementKind::Image(_) => true,
            ElementKind::Frame(f) => !f.is_invisible(),
            _ => false,
        };
        if shadowable {
            card(ui, "Effects", |ui| {
                grid(ui, "fx", |ui| {
                    label(ui, "Shadow");
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (name, sh) in [("None", None), ("Soft", Some(Shadow::soft())), ("Lifted", Some(Shadow::lifted())), ("Crisp", Some(Shadow::crisp()))] {
                            if ui.selectable_label(el.shadow == sh, name).clicked() {
                                el.shadow = sh;
                            }
                        }
                    });
                    ui.end_row();
                    if let Some(sh) = &mut el.shadow {
                        label(ui, "");
                        ui.horizontal(|ui| {
                            color_edit(ui, &mut sh.color);
                            ui.add(DragValue::new(&mut sh.blur).speed(0.5).range(0.0..=f64::MAX).prefix("blur "));
                            ui.add(DragValue::new(&mut sh.y).speed(0.5).prefix("y "));
                        });
                        ui.end_row();
                    }
                    let gradient_slot = match &mut el.kind {
                        ElementKind::Rect(r) => Some((&mut r.fill, &mut r.gradient)),
                        ElementKind::Ellipse(e) => Some((&mut e.fill, &mut e.gradient)),
                        _ => None,
                    };
                    if let Some((fill, gradient)) = gradient_slot {
                        label(ui, "Gradient");
                        let mut on = gradient.is_some();
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut on, "").changed() {
                                *gradient = on.then(|| {
                                    let from = *fill.get_or_insert(theme_now.accents[0]);
                                    let to = theme_now.accents.iter().copied().find(|c| *c != from).unwrap_or(theme_now.accents[1]);
                                    Gradient { to, angle: 45.0 }
                                });
                            }
                            if let Some(g) = gradient {
                                color_edit(ui, &mut g.to);
                                ui.add(DragValue::new(&mut g.angle).speed(1.0).suffix("°").range(-360.0..=360.0));
                            }
                        });
                        ui.end_row();
                    }
                });
            });
        }

        // Slide settings for frames.
        if el.kind.is_frame() {
            if steps.is_empty() {
                card(ui, "Slide", |ui| {
                    ui.label(RichText::new("This frame is not on the presentation path.").color(p.text_muted));
                    if ui.button(format!("{}  Add to path", icon::PLUS)).clicked() {
                        add_to_path = true;
                    }
                });
            }
        }
    }
    if add_to_path {
        actions.push(Action::AddSelectedFrameToPath);
    }
    if add_font
        && let Some(family) = crate::fontpick::add_font_file(app, ui.ctx())
        && let Some(ElementKind::Text(t)) = app.doc.get_mut(&id).map(|e| &mut e.kind)
    {
        t.font_family = family;
    }
    if let Some(&first) = steps.first() {
        slide_card(app, ui, first, &steps);
        crate::anim_ui::slide_card(app, ui, first, actions);
    } else if app.doc.get(&id).is_some_and(|e| !e.kind.is_frame()) {
        crate::anim_ui::element_card(app, ui, &id, actions);
    }
    if replace_image {
        replace_image_file(app, ui.ctx(), &id);
    }
    if let Some(m) = video_status {
        card(ui, "Player", |ui| match app.renderer.video_status(&m.path) {
            glide_render::VideoStatus::Ready { playing, position, duration } => {
                ui.horizontal(|ui| {
                    let label = if playing { format!("{}  Pause", icon::PAUSE) } else { format!("{}  Play", icon::PLAY) };
                    if ui.button(label).clicked() {
                        app.renderer.video_set_playing(&m.path, !playing, m.looping, m.muted);
                    }
                    if ui.button(format!("{}  Restart", icon::ARROW_COUNTER_CLOCKWISE)).clicked() {
                        app.renderer.video_restart(&m.path);
                    }
                    ui.label(RichText::new(format!("{:.0}:{:02.0} / {:.0}:{:02.0}", (position / 60.0).floor(), position % 60.0, (duration / 60.0).floor(), duration % 60.0)).color(p.text_muted));
                });
            }
            glide_render::VideoStatus::Loading => {
                ui.label(RichText::new("Reading video…").color(p.text_muted));
            }
            glide_render::VideoStatus::Unavailable(reason) => {
                ui.label(RichText::new(format!("{}  {reason}", icon::WARNING)).color(p.danger));
                ui.label(
                    RichText::new("GlideDeck plays video with a locally installed FFmpeg. Windows: winget install Gyan.FFmpeg · macOS: brew install ffmpeg — then restart GlideDeck.")
                        .size(11.5)
                        .color(p.text_muted),
                );
            }
        });
    }
    if change_icon {
        app.icon_picker.open = true;
        app.icon_picker.replace = Some(id.clone());
    }

    ui.horizontal_wrapped(|ui| {
        for (g, tip, a) in [
            (icon::MAGNIFYING_GLASS_PLUS, "Zoom to", Action::FitSelection),
            (icon::ARROW_LINE_UP, "Bring to front", Action::BringToFront),
            (icon::ARROW_LINE_DOWN, "Send to back", Action::SendToBack),
            (icon::COPY, "Duplicate", Action::Duplicate),
        ] {
            if ui.add(egui::Button::new(RichText::new(g).size(15.0)).min_size(egui::vec2(32.0, 28.0))).on_hover_text(tip).clicked() {
                actions.push(a);
            }
        }
        if ui
            .add(egui::Button::new(RichText::new(icon::TRASH).size(15.0).color(p.danger)).min_size(egui::vec2(32.0, 28.0)))
            .on_hover_text("Delete")
            .clicked()
        {
            actions.push(Action::Delete);
        }
    });
    ui.add_space(8.0);
}

/// Transition and speaker notes for a slide.
fn slide_card(app: &mut GlideDeckApp, ui: &mut egui::Ui, index: usize, all: &[usize]) {
    let p = theme::palette(ui.ctx());
    let numbers: Vec<String> = all.iter().map(|i| (i + 1).to_string()).collect();
    let title = format!("Slide {}", numbers.join(", "));
    let step = &mut app.doc.path[index];
    card(ui, &title, |ui| {
        grid(ui, "slide", |ui| {
            label(ui, "Transition");
            egui::ComboBox::from_id_salt("tstyle").selected_text(step.transition.style.label()).show_ui(ui, |ui| {
                for style in TransitionStyle::ALL {
                    ui.selectable_value(&mut step.transition.style, style, style.label()).on_hover_text(style.description());
                }
            });
            ui.end_row();
            label(ui, "Duration");
            ui.add(egui::Slider::new(&mut step.transition.duration, 0.2..=5.0).suffix(" s").fixed_decimals(1));
            ui.end_row();
            label(ui, "Easing");
            egui::ComboBox::from_id_salt("teasing").selected_text(easing_name(step.transition.easing)).show_ui(ui, |ui| {
                for e in [Easing::EaseInOutCubic, Easing::EaseInOutSine, Easing::EaseInOutQuint, Easing::EaseOutExpo, Easing::Linear] {
                    ui.selectable_value(&mut step.transition.easing, e, easing_name(e));
                }
            });
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{}  Speaker notes", icon::NOTE)).color(p.text_muted));
        crate::bidi::multiline(ui, "step-notes", &mut step.notes, |e| e.hint_text("Markdown notes for the presenter view…").desired_rows(4).desired_width(f32::INFINITY));
    });
}

fn replace_image_file(app: &mut GlideDeckApp, ctx: &egui::Context, id: &str) {
    let Some(file) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"]).pick_file()
    else {
        return;
    };
    let Some((w, h)) = glide_render::image_dimensions(&file) else {
        app.notify(ctx, "That file is not a readable image");
        return;
    };
    match assets::import_file(&app.base_dir(), &file) {
        Ok(reference) => {
            if let Some(el) = app.doc.get_mut(id)
                && let ElementKind::Image(m) = &mut el.kind
            {
                // Keep the displayed width, adopt the new aspect ratio.
                let width = m.width;
                m.path = reference;
                m.height = width * h as f64 / w.max(1) as f64;
            }
        }
        Err(e) => app.notify(ctx, format!("Could not import image: {e}")),
    }
}

fn weight_name(w: u16) -> &'static str {
    match w {
        0..=349 => "Light",
        350..=449 => "Regular",
        450..=549 => "Medium",
        550..=649 => "Semibold",
        650..=749 => "Bold",
        _ => "Black",
    }
}

fn layers(app: &mut GlideDeckApp, ui: &mut egui::Ui) {
    let p = theme::palette(ui.ctx());
    let multi = ui.input(|i| i.modifiers.shift || i.modifiers.command);
    egui::CollapsingHeader::new(RichText::new(format!("{}  Layers", icon::STACK)).font(theme::semibold(13.0)))
        .default_open(false)
        .show(ui, |ui| {
            let mut frame_to = None;
            let mut toggle_hidden = None;
            for el in app.doc.elements.iter().rev() {
                let selected = app.selection.contains(&el.id);
                let glyph = match &el.kind {
                    ElementKind::Rect(_) => icon::SQUARE,
                    ElementKind::Ellipse(_) => icon::CIRCLE,
                    ElementKind::Text(_) => icon::TEXT_T,
                    ElementKind::Frame(_) => icon::FRAME_CORNERS,
                    ElementKind::Icon(_) => icon::SHAPES,
                    ElementKind::Image(_) => icon::IMAGE,
                    ElementKind::Svg(_) => icon::BEZIER_CURVE,
                    ElementKind::Video(_) => icon::FILM_STRIP,
                };
                ui.horizontal(|ui| {
                    let text = RichText::new(format!("{glyph}   {}", el.display_name())).color(if el.hidden { p.text_muted } else { p.text });
                    let r = ui.add_sized([ui.available_width() - 30.0, 22.0], egui::Button::selectable(selected, text));
                    if r.clicked() {
                        if multi {
                            if selected {
                                app.selection.retain(|s| *s != el.id);
                            } else {
                                app.selection.push(el.id.clone());
                            }
                        } else {
                            app.selection = vec![el.id.clone()];
                        }
                    }
                    if r.double_clicked() {
                        frame_to = Some(el.id.clone());
                    }
                    let eye = if el.hidden { icon::EYE_SLASH } else { icon::EYE };
                    if ui.add(egui::Button::new(RichText::new(eye).size(13.0).color(p.text_muted)).frame(false)).clicked() {
                        toggle_hidden = Some(el.id.clone());
                    }
                });
            }
            if let Some(id) = toggle_hidden
                && let Some(el) = app.doc.get_mut(&id)
            {
                el.hidden = !el.hidden;
            }
            if let Some(id) = frame_to {
                app.frame_element(&id);
            }
        });
}
