//! Visual pickers: icon library, templates, slide layouts, themes and the
//! design check. Template and layout previews are rendered by the real canvas
//! renderer, so what you see is exactly what you get.

use crate::app::{Action, GlideDeckApp};
use crate::design::{self, Fix, Issue, LayoutKind, Level, Template};
use crate::theme::{self as ui_theme, icon};
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Sense, StrokeKind};
use eframe::wgpu;
use glam::DVec2;
use std::collections::HashMap;
use glide_core::theme::builtin_themes;
use glide_core::{Camera, Color, Document, IconStyle, Theme};
use glide_render::{RenderOptions, RenderTarget, TextEngine, ICONS};

/// GPU-rendered thumbnails keyed by a content key.
#[derive(Default)]
pub struct Previews {
    items: HashMap<String, (RenderTarget, egui::TextureId)>,
}

pub struct IconPicker {
    pub open: bool,
    pub query: String,
    pub style: IconStyle,
    pub color: Option<Color>,
    /// When set, picking replaces this icon element's glyph.
    pub replace: Option<String>,
}

impl Default for IconPicker {
    fn default() -> Self {
        IconPicker { open: false, query: String::new(), style: IconStyle::Regular, color: None, replace: None }
    }
}

#[derive(Default)]
pub struct DesignCheckState {
    pub open: bool,
    pub revision: Option<u64>,
    pub issues: Vec<Issue>,
}

pub fn icon_family(style: IconStyle) -> FontFamily {
    FontFamily::Name(
        match style {
            IconStyle::Regular => "phosphor",
            IconStyle::Light => "phosphor-light",
            IconStyle::Bold => "phosphor-bold",
            IconStyle::Fill => "phosphor-fill",
        }
        .into(),
    )
}

fn c32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), c.a())
}

/// Renders (once) a document view into a texture for previews: the frame named
/// "Overview" if there is one, else the first slide.
fn preview(
    app: &mut GlideDeckApp,
    frame: &eframe::Frame,
    key: &str,
    size: egui::Vec2,
    ppp: f32,
    build: impl FnOnce(&mut TextEngine) -> Document,
) -> Option<egui::TextureId> {
    if let Some((_, tex)) = app.previews.items.get(key) {
        return Some(*tex);
    }
    let rs = frame.wgpu_render_state()?;
    let doc = build(&mut app.renderer.text);
    let vp = DVec2::new(size.x as f64, size.y as f64);
    let shown = doc.elements.iter().find(|e| e.kind.is_frame() && e.name == "Overview").or_else(|| doc.step_frame(0));
    let camera = match shown.and_then(|f| Camera::for_frame(f, vp, 0.0)) {
        Some(c) => c,
        None => {
            let b = doc
                .elements
                .iter()
                .fold(glide_core::Aabb::EMPTY, |b, e| b.union(e.world_aabb(app.renderer.local_size(e))));
            Camera::framing(b, 0.0, vp, 8.0)
        }
    };
    let px = [(size.x * ppp).round() as u32, (size.y * ppp).round() as u32];
    let target = RenderTarget::new(&rs.device, px);
    app.renderer.render(&target, &doc, &camera, vp, &RenderOptions { show_grid: false, pixels_per_point: ppp as f64 });
    let tex = rs.renderer.write().register_native_texture(&rs.device, &target.unorm_view, wgpu::FilterMode::Linear);
    app.previews.items.insert(key.to_owned(), (target, tex));
    Some(tex)
}

fn card_frame(ui: &mut egui::Ui, rect: egui::Rect, hovered: bool, selected: bool) {
    let p = ui_theme::palette(ui.ctx());
    let stroke = if selected {
        egui::Stroke::new(2.0, p.accent)
    } else if hovered {
        egui::Stroke::new(1.5, p.accent.gamma_multiply(0.6))
    } else {
        egui::Stroke::new(1.0, p.border)
    };
    ui.painter().rect_stroke(rect, 10, stroke, StrokeKind::Outside);
}

/// A small section heading inside a gallery.
fn section_heading(ui: &mut egui::Ui, label: &str, count: usize) {
    let p = ui_theme::palette(ui.ctx());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).font(ui_theme::semibold(13.5)));
        ui.label(RichText::new(count.to_string()).size(11.5).color(p.text_muted));
    });
    ui.add_space(2.0);
}

// --- icon picker ---------------------------------------------------------------------

const CATEGORIES: [(&str, &str); 12] = [
    ("All", ""),
    ("Arrows", "arrow"),
    ("Charts", "chart"),
    ("People", "user"),
    ("Chat", "chat"),
    ("Media", "play"),
    ("Files", "file"),
    ("Devices", "device"),
    ("Nature", "leaf"),
    ("Weather", "cloud"),
    ("Money", "currency"),
    ("Places", "map"),
];

pub fn icon_picker(app: &mut GlideDeckApp, ctx: &egui::Context) {
    if !app.icon_picker.open {
        return;
    }
    let p = ui_theme::palette(ctx);
    let theme = app.theme();
    let mut open = true;
    let mut picked: Option<String> = None;
    egui::Window::new(RichText::new(format!("{}  Icons", icon::SHAPES)).font(ui_theme::semibold(15.0)))
        .open(&mut open)
        .default_size([560.0, 560.0])
        .min_width(420.0)
        .collapsible(false)
        .show(ctx, |ui| {
            let ip = &mut app.icon_picker;
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::MAGNIFYING_GLASS).size(16.0).color(p.text_muted));
                let r = ui.add(egui::TextEdit::singleline(&mut ip.query).hint_text("Search 1,500 icons…").desired_width(f32::INFINITY));
                if ui.memory(|m| m.focused().is_none()) {
                    r.request_focus();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for (label, q) in CATEGORIES {
                    let active = ip.query == q;
                    if ui.selectable_label(active, RichText::new(label).size(12.0)).clicked() {
                        ip.query = q.into();
                    }
                }
            });
            ui.horizontal(|ui| {
                for s in IconStyle::ALL {
                    ui.selectable_value(&mut ip.style, s, s.label());
                }
                ui.separator();
                let current = ip.color.unwrap_or(theme.accents[0]);
                for c in [theme.accents[0], theme.accents[1], theme.accents[2], theme.accents[3], theme.heading, theme.muted] {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::click());
                    ui.painter().circle_filled(rect.center(), 9.0, c32(c));
                    if c == current {
                        ui.painter().circle_stroke(rect.center(), 10.5, egui::Stroke::new(2.0, p.accent));
                    }
                    if resp.clicked() {
                        ip.color = Some(c);
                    }
                }
            });
            ui.separator();

            let tokens: Vec<String> = ip.query.to_lowercase().split_whitespace().map(str::to_owned).collect();
            let matches: Vec<&(&str, char)> = ICONS.iter().filter(|(name, _)| tokens.iter().all(|t| name.contains(t.as_str()))).collect();
            let family = icon_family(ip.style);
            let color = c32(ip.color.unwrap_or(theme.accents[0]));
            let cell = 46.0;
            let cols = ((ui.available_width() / cell).floor() as usize).max(1);
            let rows = matches.len().div_ceil(cols);
            ui.label(RichText::new(format!("{} icons · click to insert", matches.len())).size(11.5).color(p.text_muted));
            egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, cell, rows, |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (name, ch) in matches.iter().skip(row * cols).take(cols) {
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(cell, cell), Sense::click());
                            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Icon {name}")));
                            if resp.hovered() {
                                ui.painter().rect_filled(rect.shrink(2.0), 8, p.surface_alt);
                            }
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ch.to_string(), FontId::new(26.0, family.clone()), color);
                            if resp.on_hover_text(*name).clicked() {
                                picked = Some((*name).to_owned());
                            }
                        }
                    });
                }
            });
        });
    if let Some(name) = picked {
        let style = app.icon_picker.style;
        let color = app.icon_picker.color.unwrap_or(theme.accents[0]);
        match app.icon_picker.replace.take() {
            Some(id) => {
                if let Some(glide_core::Element { kind: glide_core::ElementKind::Icon(i), .. }) = app.doc.get_mut(&id) {
                    i.name = name;
                    i.style = style;
                }
                app.icon_picker.open = false;
            }
            None => app.insert_icon(&name, style, color),
        }
    }
    if !open {
        app.icon_picker.open = false;
        app.icon_picker.replace = None;
    }
}

// --- templates -----------------------------------------------------------------------

pub fn template_gallery(app: &mut GlideDeckApp, ctx: &egui::Context, frame: &eframe::Frame) -> Vec<Action> {
    let mut out = Vec::new();
    if !app.show_templates {
        return out;
    }
    let p = ui_theme::palette(ctx);
    let ppp = ctx.pixels_per_point();
    let mut open = true;
    egui::Window::new(RichText::new(format!("{}  Start from a template", icon::SQUARES_FOUR)).font(ui_theme::semibold(16.0)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Each template is a complete zooming story with a theme, frames and a path. Make it yours.").color(p.text_muted));
            ui.add_space(6.0);
            let size = egui::vec2(240.0, 135.0);
            let cols = 4;
            egui::ScrollArea::vertical()
                .max_height(ctx.content_rect().height() * 0.74)
                .min_scrolled_height(ctx.content_rect().height() * 0.74)
                .auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(cols as f32 * (size.x + 14.0));
                for category in Template::CATEGORIES {
                    let items: Vec<Template> = Template::ALL.into_iter().filter(|t| t.category() == category).collect();
                    section_heading(ui, category, items.len());
                    egui::Grid::new(("templates", category)).num_columns(cols).spacing([14.0, 14.0]).show(ui, |ui| {
                        for (i, t) in items.into_iter().enumerate() {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Use template {}", t.label())));
                                if let Some(tex) = preview(app, frame, &format!("template:{t:?}"), size, ppp, |text| design::build_template_with(t, text)) {
                                    egui::Image::new(egui::load::SizedTexture::new(tex, size)).corner_radius(10).paint_at(ui, rect);
                                }
                                card_frame(ui, rect, resp.hovered(), false);
                                ui.label(RichText::new(t.label()).font(ui_theme::semibold(13.5)));
                                ui.allocate_ui_with_layout(egui::vec2(size.x, 32.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                    ui.add(egui::Label::new(RichText::new(t.blurb()).size(11.5).color(p.text_muted)).wrap());
                                });
                                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    out.push(Action::NewFromTemplate(t));
                                }
                            });
                            if i % cols == cols - 1 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            });
        });
    if !open {
        app.show_templates = false;
    }
    out
}

// --- layouts -------------------------------------------------------------------------

pub fn layout_gallery(app: &mut GlideDeckApp, ctx: &egui::Context, frame: &eframe::Frame) -> Vec<Action> {
    let mut out = Vec::new();
    if !app.show_layouts {
        return out;
    }
    let ppp = ctx.pixels_per_point();
    let theme = app.theme();
    let p = ui_theme::palette(ctx);
    let mut open = true;
    egui::Window::new(RichText::new(format!("{}  Slide layouts", icon::LAYOUT)).font(ui_theme::semibold(15.0)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos(ctx.content_rect().left_top() + egui::vec2(260.0, 150.0))
        .show(ctx, |ui| {
            ui.label(RichText::new(format!("Adds a ready-made slide next to the current one, styled with “{}”.", theme.name)).color(p.text_muted));
            ui.add_space(4.0);
            let size = egui::vec2(208.0, 117.0);
            let cols = 4;
            egui::ScrollArea::vertical()
                .max_height(ctx.content_rect().height() * 0.68)
                .min_scrolled_height(ctx.content_rect().height() * 0.68)
                .auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(cols as f32 * (size.x + 12.0));
                for category in LayoutKind::CATEGORIES {
                    let items: Vec<LayoutKind> = LayoutKind::ALL.into_iter().filter(|k| k.category() == category).collect();
                    section_heading(ui, category, items.len());
                    egui::Grid::new(("layouts", category)).num_columns(cols).spacing([12.0, 10.0]).show(ui, |ui| {
                        for (i, kind) in items.into_iter().enumerate() {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Insert layout {}", kind.label())));
                                let key = format!("layout:{kind:?}:{}", theme.name);
                                let th = theme.clone();
                                if let Some(tex) = preview(app, frame, &key, size, ppp, move |_| {
                                    let mut d = Document::new("preview");
                                    glide_core::theme::apply_theme(&mut d, &th);
                                    let id = design::place_layout(&mut d, kind, &th, DVec2::ZERO, design::LAYOUT_W, 0.0);
                                    d.path.push(glide_core::Step::new(id));
                                    d
                                }) {
                                    egui::Image::new(egui::load::SizedTexture::new(tex, size)).corner_radius(8).paint_at(ui, rect);
                                }
                                card_frame(ui, rect, resp.hovered(), false);
                                ui.label(RichText::new(kind.label()).size(12.5));
                                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    out.push(Action::InsertLayout(kind));
                                }
                            });
                            if i % cols == cols - 1 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.add_space(6.0);
                }
            });
        });
    if !open {
        app.show_layouts = false;
    }
    out
}

// --- themes --------------------------------------------------------------------------

/// A theme swatch card: background (gradient), "Aa" in the heading color and accent dots.
pub fn theme_card(ui: &mut egui::Ui, theme: &Theme, selected: bool, size: egui::Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Theme {}", theme.name)));
    let top = c32(theme.background);
    let bottom = c32(theme.background_to.unwrap_or(theme.background));
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
    let family = if theme.heading_font == "serif" { FontFamily::Name("ui-serif".into()) } else { FontFamily::Name(ui_theme::SEMIBOLD.into()) };
    ui.painter().text(rect.left_top() + egui::vec2(9.0, 6.0), egui::Align2::LEFT_TOP, "Aa", FontId::new(size.y * 0.36, family), c32(theme.heading));
    for (i, a) in theme.accents.iter().enumerate() {
        ui.painter().circle_filled(rect.left_bottom() + egui::vec2(12.0 + i as f32 * 12.0, -10.0), 4.5, c32(*a));
    }
    card_frame(ui, rect, resp.hovered(), selected);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(&theme.name)
}

pub fn themes_row(app: &mut GlideDeckApp, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let current = app.doc.theme.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| "Paper".into());
    for (i, t) in builtin_themes().iter().enumerate() {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if theme_card(ui, t, t.name == current, egui::vec2(66.0, 44.0)).clicked() {
                out.push(Action::ApplyTheme(i));
            }
            ui.label(RichText::new(&t.name).size(10.5));
        });
    }
}

// --- design check --------------------------------------------------------------------

pub fn refresh_design_check(app: &mut GlideDeckApp) {
    let rev = app.history.revision();
    if app.design_check.revision == Some(rev) {
        return;
    }
    let theme = app.theme();
    let base = app.base_dir();
    app.design_check.issues = design::design_check(&app.doc, &mut app.renderer.text, &base, &theme);
    app.design_check.revision = Some(rev);
}

pub fn design_check_window(app: &mut GlideDeckApp, ctx: &egui::Context) {
    if !app.design_check.open {
        return;
    }
    refresh_design_check(app);
    let p = ui_theme::palette(ctx);
    let mut open = true;
    let mut fix: Option<Fix> = None;
    let mut fix_all = false;
    let mut show: Option<(Option<String>, Option<usize>)> = None;
    egui::Window::new(RichText::new(format!("{}  Design check", icon::SEAL_CHECK)).font(ui_theme::semibold(15.0)))
        .open(&mut open)
        .default_size([440.0, 420.0])
        .collapsible(false)
        .show(ctx, |ui| {
            let issues = &app.design_check.issues;
            if issues.is_empty() {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(icon::SEAL_CHECK).size(40.0).color(p.accent));
                    ui.label(RichText::new("Looking sharp").font(ui_theme::semibold(15.0)));
                    ui.label(RichText::new("No contrast, readability or consistency issues found.").color(p.text_muted));
                });
                return;
            }
            ui.horizontal(|ui| {
                let warnings = issues.iter().filter(|i| i.level == Level::Warning).count();
                ui.label(RichText::new(format!("{} issues · {warnings} to fix", issues.len())).color(p.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if issues.iter().any(|i| i.fix.is_some()) && ui.button(format!("{}  Fix all", icon::MAGIC_WAND)).clicked() {
                        fix_all = true;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for issue in issues {
                    egui::Frame::new().fill(p.surface_alt).corner_radius(8).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (glyph, color) = match issue.level {
                                Level::Warning => (icon::WARNING, Color32::from_rgb(0xE0, 0x8A, 0x00)),
                                Level::Suggestion => (icon::LIGHTBULB, p.accent),
                            };
                            ui.label(RichText::new(glyph).size(16.0).color(color));
                            ui.label(RichText::new(&issue.title).font(ui_theme::semibold(13.0)));
                        });
                        ui.label(RichText::new(&issue.detail).size(12.0).color(p.text_muted));
                        ui.horizontal(|ui| {
                            if (issue.target.is_some() || issue.step.is_some()) && ui.small_button(format!("{}  Show", icon::EYE)).clicked() {
                                show = Some((issue.target.clone(), issue.step));
                            }
                            if let Some(f) = &issue.fix
                                && ui.small_button(format!("{}  Fix", icon::MAGIC_WAND)).clicked()
                            {
                                fix = Some(f.clone());
                            }
                        });
                    });
                    ui.add_space(6.0);
                }
            });
        });
    let theme = app.theme();
    if fix_all {
        let fixes: Vec<Fix> = app.design_check.issues.iter().filter_map(|i| i.fix.clone()).collect();
        for f in fixes {
            design::apply_fix(&mut app.doc, &theme, &f);
        }
        app.design_check.revision = None;
    } else if let Some(f) = fix {
        design::apply_fix(&mut app.doc, &theme, &f);
        app.design_check.revision = None;
    }
    if let Some((target, step)) = show {
        match (step, target) {
            (Some(s), Some(t)) => {
                app.go_to_step(s);
                app.selection = vec![t];
            }
            (Some(s), None) => app.go_to_step(s),
            (None, Some(t)) => {
                app.selection = vec![t.clone()];
                app.frame_element(&t);
            }
            _ => {}
        }
    }
    if !open {
        app.design_check.open = false;
    }
}
