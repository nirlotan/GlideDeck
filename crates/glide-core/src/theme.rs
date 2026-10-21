//! Design themes: a coordinated set of colors and fonts.
//!
//! Switching themes restyles a document non-destructively in spirit: text
//! with a semantic role (title, body, …) adopts the new theme's fonts and
//! colors, and any color that exactly matches a slot of the *old* theme is
//! mapped to the same slot of the new one. Colors the user picked by hand
//! stay untouched.

use crate::color::Color;
use crate::document::Document;
use crate::element::{ElementKind, TextBlock, TextRole};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    /// Optional bottom color for a vertical background gradient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_to: Option<Color>,
    /// Card / panel fill that sits on the background.
    pub surface: Color,
    pub heading: Color,
    pub text: Color,
    pub muted: Color,
    pub accents: [Color; 4],
    pub heading_font: String,
    pub body_font: String,
}

const fn c(hex: u32) -> Color {
    Color::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    /// The color slots in a fixed order; used to map one theme onto another.
    fn slots(&self) -> [Color; 9] {
        let a = self.accents;
        [self.background, self.surface, self.heading, self.text, self.muted, a[0], a[1], a[2], a[3]]
    }

    /// Colors offered as quick swatches in color pickers.
    pub fn swatches(&self) -> Vec<Color> {
        let mut v = vec![self.heading, self.text, self.muted, self.surface, self.background];
        v.extend(self.accents);
        v.dedup();
        v
    }

    pub fn is_dark(&self) -> bool {
        relative_luminance(self.background) < 0.2
    }

    pub fn role_color(&self, role: TextRole) -> Color {
        match role {
            TextRole::Title | TextRole::Heading | TextRole::Quote => self.heading,
            TextRole::Subtitle | TextRole::Caption => self.muted,
            TextRole::Body => self.text,
        }
    }

    pub fn role_font(&self, role: TextRole) -> &str {
        if role.is_display() { &self.heading_font } else { &self.body_font }
    }

    /// A text block styled for `role` (content left empty).
    pub fn text_style(&self, role: TextRole) -> TextBlock {
        let (size, weight, line_height, italic) = match role {
            TextRole::Title => (96.0, 700, 1.1, false),
            TextRole::Heading => (56.0, 650, 1.15, false),
            TextRole::Subtitle => (34.0, 400, 1.3, false),
            TextRole::Body => (28.0, 400, 1.45, false),
            TextRole::Caption => (20.0, 500, 1.35, false),
            TextRole::Quote => (46.0, 400, 1.3, true),
        };
        TextBlock {
            font_size: size,
            weight,
            line_height,
            italic,
            color: self.role_color(role),
            font_family: self.role_font(role).to_owned(),
            role: Some(role),
            ..TextBlock::new("")
        }
    }

    /// Restyles `text` for `role`, keeping its content, alignment and wrap width.
    pub fn apply_role(&self, text: &mut TextBlock, role: TextRole) {
        let s = self.text_style(role);
        text.font_size = s.font_size;
        text.weight = s.weight;
        text.line_height = s.line_height;
        text.italic = s.italic;
        text.color = s.color;
        text.font_family = s.font_family;
        text.role = Some(role);
    }
}

/// The curated theme gallery.
pub fn builtin_themes() -> Vec<Theme> {
    let t = |name: &str, bg: u32, bg_to: Option<u32>, surface: u32, heading: u32, text: u32, muted: u32, accents: [u32; 4], hf: &str, bf: &str| Theme {
        name: name.into(),
        background: c(bg),
        background_to: bg_to.map(c),
        surface: c(surface),
        heading: c(heading),
        text: c(text),
        muted: c(muted),
        accents: accents.map(c),
        heading_font: hf.into(),
        body_font: bf.into(),
    };
    vec![
        t("Paper", 0xF7F6F2, None, 0xFFFFFF, 0x1F232B, 0x2E3440, 0x5B6478, [0x17A398, 0xE4572E, 0x3D5A80, 0xF2A541], "sans-serif", "sans-serif"),
        t("Midnight", 0x0B1020, Some(0x1B1340), 0x161D33, 0xFFFFFF, 0xE2E6F0, 0x9AA3B8, [0x8B7CFF, 0x2DD4BF, 0xF472B6, 0xFBBF24], "sans-serif", "sans-serif"),
        t("Editorial", 0xFBFAF7, None, 0xFFFFFF, 0x111111, 0x2B2B2B, 0x6B6B6B, [0xB4232C, 0x1D4ED8, 0x0F766E, 0xD97706], "serif", "sans-serif"),
        t("Aurora", 0xECFEFF, Some(0xF5F3FF), 0xFFFFFF, 0x1E1B4B, 0x312E81, 0x6B6A9A, [0x6366F1, 0x06B6D4, 0xEC4899, 0x10B981], "sans-serif", "sans-serif"),
        t("Forest", 0x0F1F1A, Some(0x0A1511), 0x18302A, 0xF4F1DE, 0xE7F0EA, 0x9DB4A8, [0x81B29A, 0xF2CC8F, 0xE07A5F, 0x3D9970], "serif", "sans-serif"),
        t("Sunset", 0xFFF4EC, Some(0xFFE4E6), 0xFFFFFF, 0x6B1D3F, 0x3A1F2B, 0x80506A, [0xF43F5E, 0xF59E0B, 0x8B5CF6, 0xEC4899], "sans-serif", "sans-serif"),
        t("Mono", 0xFFFFFF, None, 0xF4F4F5, 0x09090B, 0x18181B, 0x71717A, [0x18181B, 0x52525B, 0xA1A1AA, 0x2563EB], "sans-serif", "sans-serif"),
        t("Ocean", 0x0A1929, Some(0x0F2D4A), 0x12304D, 0xFFFFFF, 0xDCEBFA, 0x8FA9C4, [0x38BDF8, 0x34D399, 0xFBBF24, 0xF87171], "sans-serif", "sans-serif"),
        // 2026 palettes: a deep "future dusk" with aurora accents, and its
        // daylight twin on a warm cloud white.
        t("Dusk", 0x0E0B1F, Some(0x1E1646), 0x1A1536, 0xF5F2EA, 0xE4E0F2, 0x9C95BD, [0x8F7BFF, 0x2EE6C5, 0xFF7A5C, 0xE4FF6B], "sans-serif", "sans-serif"),
        t("Cloud", 0xF4F1EA, Some(0xECE7F6), 0xFFFFFF, 0x1B1733, 0x2B2740, 0x625E7A, [0x5B47E0, 0x0E9E8C, 0xF0643C, 0xD9A21B], "sans-serif", "sans-serif"),
    ]
}

pub fn default_theme() -> Theme {
    builtin_themes().remove(0)
}

fn same_rgb(a: Color, b: Color) -> bool {
    a.r() == b.r() && a.g() == b.g() && a.b() == b.b()
}

/// Maps a color through old → new theme slots (keeping its alpha).
fn remap(color: &mut Color, old: &[Color; 9], new: &[Color; 9]) {
    if let Some(i) = old.iter().position(|o| same_rgb(*o, *color)) {
        let n = new[i];
        *color = Color::rgba(n.r(), n.g(), n.b(), color.a());
    }
}

/// Restyles a document with `theme`.
pub fn apply_theme(doc: &mut Document, theme: &Theme) {
    let old = doc.theme.clone().unwrap_or_else(default_theme);
    let (os, ns) = (old.slots(), theme.slots());
    let fix = |c: &mut Color| remap(c, &os, &ns);
    let fix_opt = |c: &mut Option<Color>| {
        if let Some(c) = c {
            remap(c, &os, &ns);
        }
    };
    for el in &mut doc.elements {
        match &mut el.kind {
            ElementKind::Rect(r) => {
                fix_opt(&mut r.fill);
                if let Some(g) = &mut r.gradient {
                    fix(&mut g.to);
                }
                if let Some(s) = &mut r.stroke {
                    fix(&mut s.color);
                }
            }
            ElementKind::Ellipse(e) => {
                fix_opt(&mut e.fill);
                if let Some(g) = &mut e.gradient {
                    fix(&mut g.to);
                }
                if let Some(s) = &mut e.stroke {
                    fix(&mut s.color);
                }
            }
            ElementKind::Frame(f) => {
                fix_opt(&mut f.fill);
                if let Some(s) = &mut f.border {
                    fix(&mut s.color);
                }
            }
            ElementKind::Icon(i) => fix(&mut i.color),
            ElementKind::Text(t) => match t.role {
                Some(role) => {
                    let alpha = t.color.a();
                    let n = theme.role_color(role);
                    t.color = Color::rgba(n.r(), n.g(), n.b(), alpha);
                    t.font_family = theme.role_font(role).to_owned();
                }
                None => {
                    fix(&mut t.color);
                    if t.font_family == old.heading_font && old.heading_font != old.body_font {
                        t.font_family = theme.heading_font.clone();
                    } else if t.font_family == old.body_font {
                        t.font_family = theme.body_font.clone();
                    }
                }
            },
            ElementKind::Image(_) | ElementKind::Svg(_) | ElementKind::Video(_) => {}
        }
    }
    doc.background = theme.background;
    doc.background_to = theme.background_to;
    doc.theme = Some(theme.clone());
}

/// WCAG relative luminance of an sRGB color.
pub fn relative_luminance(c: Color) -> f64 {
    let ch = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// WCAG contrast ratio, from 1 (none) to 21 (black on white).
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Blends `top` (with its alpha) over an opaque `bottom`.
pub fn blend_over(top: Color, bottom: Color) -> Color {
    let a = top.a() as f64 / 255.0;
    let mix = |t: u8, b: u8| (t as f64 * a + b as f64 * (1.0 - a)).round() as u8;
    Color::rgb(mix(top.r(), bottom.r()), mix(top.g(), bottom.g()), mix(top.b(), bottom.b()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Element, RectShape};

    #[test]
    fn theme_switch_maps_slots_and_roles_but_keeps_custom_colors() {
        let themes = builtin_themes();
        let (paper, midnight) = (&themes[0], &themes[1]);
        let mut doc = Document::new("t");
        doc.theme = Some(paper.clone());
        let mut title = paper.text_style(TextRole::Title);
        title.content = "Hello".into();
        doc.add(Element::new(ElementKind::Text(title)));
        let accent_card = RectShape { width: 10.0, height: 10.0, fill: Some(paper.accents[1]), ..Default::default() };
        doc.add(Element::new(ElementKind::Rect(accent_card)));
        let custom = Color::rgb(1, 2, 3);
        doc.add(Element::new(ElementKind::Rect(RectShape { width: 1.0, height: 1.0, fill: Some(custom), ..Default::default() })));

        apply_theme(&mut doc, midnight);
        let ElementKind::Text(t) = &doc.elements[0].kind else { panic!() };
        assert_eq!(t.color, midnight.heading);
        let ElementKind::Rect(r) = &doc.elements[1].kind else { panic!() };
        assert_eq!(r.fill, Some(midnight.accents[1]));
        let ElementKind::Rect(r) = &doc.elements[2].kind else { panic!() };
        assert_eq!(r.fill, Some(custom), "hand-picked colors are preserved");
        assert_eq!(doc.background, midnight.background);
        assert_eq!(doc.background_to, midnight.background_to);
    }

    #[test]
    fn contrast_math() {
        assert!((contrast_ratio(Color::BLACK, Color::WHITE) - 21.0).abs() < 1e-6);
        assert!((contrast_ratio(Color::WHITE, Color::WHITE) - 1.0).abs() < 1e-9);
        for t in builtin_themes() {
            assert!(contrast_ratio(t.text, t.background) >= 4.5, "{} body text contrast", t.name);
            assert!(contrast_ratio(t.heading, t.background) >= 4.5, "{} heading contrast", t.name);
            // Subtitles and captions use the muted color: it must read on both gradient stops.
            for bg in [Some(t.background), t.background_to].into_iter().flatten() {
                assert!(contrast_ratio(t.muted, bg) >= 4.5, "{} muted contrast", t.name);
            }
        }
    }
}
