//! Visual design: typography, palette and egui styling.
//!
//! The UI uses the platform's own modern UI typeface (Segoe UI Variable on
//! Windows 11, SF Pro on macOS) loaded from the local system — nothing is
//! downloaded — with Phosphor icons bundled into the binary.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke, TextStyle,
    Theme, Visuals,
};
use std::sync::Arc;

pub use egui_phosphor::regular as icon;

pub const SEMIBOLD: &str = "ui-semibold";

/// Semantic colors for one theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Window background behind panels (also canvas surround).
    pub app_bg: Color32,
    /// Ribbon, sidebars.
    pub surface: Color32,
    /// Hover fills, inputs, cards on surfaces.
    pub surface_alt: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
}

pub const LIGHT: Palette = Palette {
    app_bg: Color32::from_rgb(0xEE, 0xEE, 0xF2),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface_alt: Color32::from_rgb(0xF4, 0xF4, 0xF7),
    border: Color32::from_rgb(0xE3, 0xE3, 0xE9),
    text: Color32::from_rgb(0x18, 0x18, 0x1B),
    text_muted: Color32::from_rgb(0x6E, 0x6E, 0x7A),
    accent: Color32::from_rgb(0x5B, 0x4F, 0xE9),
    accent_soft: Color32::from_rgb(0xEC, 0xEA, 0xFE),
    on_accent: Color32::WHITE,
    danger: Color32::from_rgb(0xDC, 0x26, 0x26),
};

pub const DARK: Palette = Palette {
    app_bg: Color32::from_rgb(0x0E, 0x0E, 0x11),
    surface: Color32::from_rgb(0x17, 0x17, 0x1B),
    surface_alt: Color32::from_rgb(0x22, 0x22, 0x28),
    border: Color32::from_rgb(0x2B, 0x2B, 0x33),
    text: Color32::from_rgb(0xEC, 0xEC, 0xF1),
    text_muted: Color32::from_rgb(0x9A, 0x9A, 0xA8),
    accent: Color32::from_rgb(0x8B, 0x80, 0xFF),
    accent_soft: Color32::from_rgb(0x2A, 0x26, 0x50),
    on_accent: Color32::WHITE,
    danger: Color32::from_rgb(0xF8, 0x71, 0x71),
};

pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.theme() == Theme::Dark { DARK } else { LIGHT }
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

fn read_first(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

/// Installs the system UI font (with semibold variant) and the icon font.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    #[cfg(target_os = "windows")]
    let (regular, bold) = (
        read_first(&[r"C:\Windows\Fonts\SegUIVar.ttf", r"C:\Windows\Fonts\segoeui.ttf"]),
        read_first(&[r"C:\Windows\Fonts\seguisb.ttf", r"C:\Windows\Fonts\segoeuib.ttf"]),
    );
    #[cfg(target_os = "macos")]
    let (regular, bold) = (
        read_first(&["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Helvetica.ttc"]),
        read_first(&["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Helvetica.ttc"]),
    );
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let (regular, bold) = (
        read_first(&[
            "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
            "/usr/share/fonts/opentype/inter/Inter-Regular.otf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]),
        read_first(&[
            "/usr/share/fonts/truetype/inter/Inter-SemiBold.ttf",
            "/usr/share/fonts/opentype/inter/Inter-SemiBold.otf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        ]),
    );

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default().clone();
    let mut regular_chain = proportional.clone();
    if let Some(bytes) = regular {
        fonts.font_data.insert("ui".into(), Arc::new(FontData::from_owned(bytes)));
        regular_chain.insert(0, "ui".into());
    }
    let mut bold_chain = regular_chain.clone();
    if let Some(bytes) = bold {
        fonts.font_data.insert("ui-bold".into(), Arc::new(FontData::from_owned(bytes)));
        bold_chain.insert(0, "ui-bold".into());
    }
    // Script fallbacks so every UI field can show Hebrew, Arabic, Indic and CJK
    // text (the primary UI face may not cover them). Loaded from local fonts.
    #[cfg(target_os = "windows")]
    let fallbacks: &[(&str, &[&str])] = &[
        ("fb-segoe", &[r"C:\Windows\Fonts\segoeui.ttf"]),
        ("fb-arial", &[r"C:\Windows\Fonts\arial.ttf"]),
        ("fb-indic", &[r"C:\Windows\Fonts\Nirmala.ttc"]),
        ("fb-thai", &[r"C:\Windows\Fonts\LeelawUI.ttf"]),
        ("fb-cjk", &[r"C:\Windows\Fonts\YuGothM.ttc", r"C:\Windows\Fonts\msyh.ttc"]),
        ("fb-korean", &[r"C:\Windows\Fonts\malgun.ttf"]),
        ("fb-symbols", &[r"C:\Windows\Fonts\seguisym.ttf"]),
    ];
    #[cfg(target_os = "macos")]
    let fallbacks: &[(&str, &[&str])] = &[
        ("fb-hebrew", &["/System/Library/Fonts/ArialHB.ttc", "/System/Library/Fonts/Supplemental/Arial Hebrew.ttc"]),
        ("fb-arabic", &["/System/Library/Fonts/GeezaPro.ttc", "/System/Library/Fonts/Supplemental/GeezaPro.ttc"]),
        ("fb-indic", &["/System/Library/Fonts/Kohinoor.ttc", "/System/Library/Fonts/Supplemental/DevanagariMT.ttc"]),
        ("fb-cjk", &["/System/Library/Fonts/Hiragino Sans GB.ttc", "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc"]),
        ("fb-korean", &["/System/Library/Fonts/AppleSDGothicNeo.ttc"]),
    ];
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let fallbacks: &[(&str, &[&str])] = &[
        ("fb-noto", &["/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"]),
        ("fb-hebrew", &["/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf"]),
        ("fb-arabic", &["/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf"]),
        ("fb-cjk", &["/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"]),
    ];
    for (name, paths) in fallbacks {
        if let Some(bytes) = read_first(paths) {
            fonts.font_data.insert((*name).into(), Arc::new(FontData::from_owned(bytes)));
            regular_chain.push((*name).into());
            bold_chain.push((*name).into());
        }
    }
    fonts.families.insert(FontFamily::Proportional, regular_chain);
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), bold_chain);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    egui_phosphor::add_font_bytes_as_family(&mut fonts, "phosphor-light", egui_phosphor::bytes::light::FONT);
    egui_phosphor::add_font_bytes_as_family(&mut fonts, "phosphor-bold", egui_phosphor::bytes::bold::FONT);
    egui_phosphor::add_font_bytes_as_family(&mut fonts, "phosphor-fill", egui_phosphor::bytes::fill::FONT);

    // A serif face for theme previews (falls back to the UI font).
    let serif = read_first(&[
        r"C:\Windows\Fonts\georgia.ttf",
        "/System/Library/Fonts/Supplemental/Georgia.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
    ]);
    let mut serif_chain = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let Some(bytes) = serif {
        fonts.font_data.insert("ui-serif".into(), Arc::new(FontData::from_owned(bytes)));
        serif_chain.insert(0, "ui-serif".into());
    }
    fonts.families.insert(FontFamily::Name("ui-serif".into()), serif_chain);
    // Icons must also resolve inside semibold text.
    if let Some(chain) = fonts.families.get_mut(&FontFamily::Name(SEMIBOLD.into())) {
        chain.insert(1.min(chain.len()), "phosphor".into());
    }
    ctx.set_fonts(fonts);
}

fn visuals(p: &Palette, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let r = CornerRadius::same(7);
    v.panel_fill = p.surface;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = p.surface_alt;
    v.text_edit_bg_color = Some(p.surface_alt);
    v.faint_bg_color = p.surface_alt;
    v.code_bg_color = p.surface_alt;
    v.hyperlink_color = p.accent;
    v.error_fg_color = p.danger;
    let shadow_alpha = if dark { 120 } else { 28 };
    v.window_shadow = Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(shadow_alpha) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(shadow_alpha) };
    v.selection.bg_fill = if dark { p.accent.gamma_multiply(0.45) } else { p.accent_soft };
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.slider_trailing_fill = true;

    let hover = if dark { Color32::from_rgb(0x2C, 0x2C, 0x34) } else { Color32::from_rgb(0xEB, 0xEB, 0xF0) };
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;

    w.inactive.bg_fill = p.surface_alt;
    w.inactive.weak_bg_fill = p.surface_alt;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;

    w.hovered.bg_fill = hover;
    w.hovered.weak_bg_fill = hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.border);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.accent_soft;
    w.active.weak_bg_fill = p.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.5, p.accent);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;

    w.open = w.active;
    v
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, false));
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, true));
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.window_margin = egui::Margin::same(14);
        style.spacing.slider_width = 120.0;
        style.spacing.combo_width = 120.0;
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Heading, semibold(17.0)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();
        style.interaction.tooltip_delay = 0.35;
    });
}

/// A small uppercase-style section caption.
pub fn caption(ui: &mut egui::Ui, text: &str) {
    let p = palette(ui.ctx());
    ui.label(egui::RichText::new(text.to_uppercase()).font(semibold(10.5)).color(p.text_muted).extra_letter_spacing(0.6));
}
