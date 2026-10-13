//! Rich text formatting: bold, italic, underline, strikethrough, color and
//! size for a selection while editing in place (or for the next characters
//! typed), and for whole text boxes when they're selected but not edited.

use crate::app::GlideDeckApp;
use crate::theme::{self, icon};
use eframe::egui::{self, Color32, RichText};
use glide_core::{Color, ElementKind, SpanStyle, TextBlock};
use glide_render::EditState;
use std::ops::Range;

/// One step of font size change (Ctrl/⌘+] and [).
const SIZE_STEP: f64 = 1.15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextFormat {
    Bold,
    Italic,
    Underline,
    Strike,
    Color(Color),
    Bigger,
    Smaller,
    Clear,
}

/// Formatting chosen with no selection, applied to the next characters typed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pending {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub color: Option<Color>,
    pub size: Option<f64>,
}

impl Pending {
    fn apply(&self, s: &mut SpanStyle) {
        if self.bold.is_some() {
            s.bold = self.bold;
        }
        if self.italic.is_some() {
            s.italic = self.italic;
        }
        if let Some(u) = self.underline {
            s.underline = u;
        }
        if let Some(x) = self.strike {
            s.strike = x;
        }
        if self.color.is_some() {
            s.color = self.color;
        }
        if self.size.is_some() {
            s.size = self.size;
        }
    }
}

/// Which toggles are on, for highlighting buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FormatState {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

/// Byte offset in `content` of a caret position (paragraph line, byte index).
pub fn byte_offset(content: &str, line: usize, index: usize) -> usize {
    let mut at = 0;
    for (i, l) in content.split('\n').enumerate() {
        if i == line {
            return at + index.min(l.len());
        }
        at += l.len() + 1;
    }
    content.len()
}

/// The selected byte range while editing, if anything is selected.
pub fn selection_range(content: &str, state: &EditState) -> Option<Range<usize>> {
    let anchor = state.anchor?;
    let a = byte_offset(content, anchor.line, anchor.index);
    let b = byte_offset(content, state.cursor.line, state.cursor.index);
    (a != b).then(|| a.min(b)..a.max(b))
}

/// The formatting a caret would type with: the character before it.
fn style_before(t: &TextBlock, at: usize) -> SpanStyle {
    let prev = t.content[..at.min(t.content.len())].chars().next_back().map(|c| at - c.len_utf8());
    match prev {
        Some(i) => t.style_at(i),
        None => t.style_at(0),
    }
}

/// Applies a format to a byte range: toggles flip to "on" unless the whole
/// range already has it, like word processors.
pub fn format_range(t: &mut TextBlock, range: Range<usize>, fmt: TextFormat) {
    let base_bold = t.weight >= 600;
    let base_italic = t.italic;
    let base_size = t.font_size;
    match fmt {
        TextFormat::Bold => {
            let on = !t.range_all(range.clone(), |s| s.bold.unwrap_or(base_bold));
            t.format_range(range, |s| s.bold = Some(on));
        }
        TextFormat::Italic => {
            let on = !t.range_all(range.clone(), |s| s.italic.unwrap_or(base_italic));
            t.format_range(range, |s| s.italic = Some(on));
        }
        TextFormat::Underline => {
            let on = !t.range_all(range.clone(), |s| s.underline);
            t.format_range(range, |s| s.underline = on);
        }
        TextFormat::Strike => {
            let on = !t.range_all(range.clone(), |s| s.strike);
            t.format_range(range, |s| s.strike = on);
        }
        TextFormat::Color(c) => t.format_range(range, |s| s.color = Some(c)),
        TextFormat::Bigger => t.format_range(range, |s| s.size = Some(s.size.unwrap_or(base_size) * SIZE_STEP)),
        TextFormat::Smaller => t.format_range(range, |s| s.size = Some((s.size.unwrap_or(base_size) / SIZE_STEP).max(1e-9))),
        TextFormat::Clear => t.format_range(range, |s| *s = SpanStyle::default()),
    }
}

/// Applies a format to a whole text box: its own style for bold, italic,
/// color and size; underline and strikethrough across all its text.
pub fn format_box(t: &mut TextBlock, fmt: TextFormat) {
    let all = 0..t.content.len();
    match fmt {
        TextFormat::Bold => {
            let base_bold = t.weight >= 600;
            let on = !t.range_all(all.clone(), |s| s.bold.unwrap_or(base_bold));
            t.weight = if on { 700 } else { 400 };
            t.format_range(all, |s| s.bold = None);
        }
        TextFormat::Italic => {
            let base_italic = t.italic;
            let on = !t.range_all(all.clone(), |s| s.italic.unwrap_or(base_italic));
            t.italic = on;
            t.format_range(all, |s| s.italic = None);
        }
        TextFormat::Underline | TextFormat::Strike => format_range(t, all, fmt),
        TextFormat::Color(c) => {
            t.color = c;
            t.format_range(all, |s| s.color = None);
        }
        TextFormat::Bigger | TextFormat::Smaller => {
            let k = if fmt == TextFormat::Bigger { SIZE_STEP } else { 1.0 / SIZE_STEP };
            t.font_size *= k;
            for s in &mut t.spans {
                s.style.size = s.style.size.map(|z| z * k);
            }
        }
        TextFormat::Clear => t.spans.clear(),
    }
    t.normalize_spans();
}

/// Toggles formatting for the next characters typed at `at`.
fn toggle_pending(t: &TextBlock, at: usize, pending: &mut Pending, fmt: TextFormat) {
    let mut current = style_before(t, at);
    pending.apply(&mut current);
    match fmt {
        TextFormat::Bold => pending.bold = Some(!t.is_bold(&current)),
        TextFormat::Italic => pending.italic = Some(!t.is_italic(&current)),
        TextFormat::Underline => pending.underline = Some(!current.underline),
        TextFormat::Strike => pending.strike = Some(!current.strike),
        TextFormat::Color(c) => pending.color = Some(c),
        TextFormat::Bigger => pending.size = Some(current.size.unwrap_or(t.font_size) * SIZE_STEP),
        TextFormat::Smaller => pending.size = Some(current.size.unwrap_or(t.font_size) / SIZE_STEP),
        TextFormat::Clear => {
            *pending = Pending {
                bold: Some(t.weight >= 600),
                italic: Some(t.italic),
                underline: Some(false),
                strike: Some(false),
                color: Some(t.color),
                size: Some(t.font_size),
            }
        }
    }
}

/// Formats newly typed text with the pending formatting.
pub fn apply_pending(t: &mut TextBlock, range: Range<usize>, pending: &Pending) {
    t.format_range(range, |s| pending.apply(s));
}

/// Toggle states for a selection, a caret (with pending formatting) or a whole box.
pub fn format_state(t: &TextBlock, range: Option<Range<usize>>, pending: Option<&Pending>) -> FormatState {
    match range {
        Some(r) if !r.is_empty() => FormatState {
            bold: t.range_all(r.clone(), |s| t.is_bold(s)),
            italic: t.range_all(r.clone(), |s| t.is_italic(s)),
            underline: t.range_all(r.clone(), |s| s.underline),
            strike: t.range_all(r, |s| s.strike),
        },
        Some(r) => {
            let mut s = style_before(t, r.start);
            if let Some(p) = pending {
                p.apply(&mut s);
            }
            FormatState { bold: t.is_bold(&s), italic: t.is_italic(&s), underline: s.underline, strike: s.strike }
        }
        None => {
            let all = 0..t.content.len();
            if all.is_empty() {
                return FormatState { bold: t.weight >= 600, italic: t.italic, ..Default::default() };
            }
            format_state(t, Some(all), None)
        }
    }
}

impl GlideDeckApp {
    /// Formats the selection or caret while editing in place, or the selected
    /// text boxes as a whole otherwise.
    pub fn format_text(&mut self, fmt: TextFormat) {
        if let Some(session) = &mut self.text_edit {
            let Some(ElementKind::Text(t)) = self.doc.get_mut(&session.id).map(|e| &mut e.kind) else { return };
            match selection_range(&t.content, &session.state) {
                Some(r) => format_range(t, r, fmt),
                None => {
                    let at = byte_offset(&t.content, session.state.cursor.line, session.state.cursor.index);
                    let mut pending = session.pending.unwrap_or_default();
                    toggle_pending(t, at, &mut pending, fmt);
                    session.pending = Some(pending);
                }
            }
            // Refresh caret and selection geometry for the new layout.
            self.refresh_text_edit();
            return;
        }
        for id in self.selection.clone() {
            if let Some(ElementKind::Text(t)) = self.doc.get_mut(&id).map(|e| &mut e.kind) {
                format_box(t, fmt);
            }
        }
    }

    /// Toggle states for the ribbon and the floating bar.
    pub fn current_format_state(&self) -> Option<FormatState> {
        if let Some(session) = &self.text_edit {
            let ElementKind::Text(t) = &self.doc.get(&session.id)?.kind else { return None };
            let caret = byte_offset(&t.content, session.state.cursor.line, session.state.cursor.index);
            let range = selection_range(&t.content, &session.state).unwrap_or(caret..caret);
            return Some(format_state(t, Some(range), session.pending.as_ref()));
        }
        let id = self.selection.iter().find(|id| matches!(self.doc.get(id).map(|e| &e.kind), Some(ElementKind::Text(_))))?;
        let ElementKind::Text(t) = &self.doc.get(id)?.kind else { return None };
        Some(format_state(t, None, None))
    }

    /// A small formatting toolbar floating above the text being edited.
    pub fn format_bar(&mut self, ui: &egui::Ui, canvas: egui::Rect) {
        let Some(session) = &self.text_edit else { return };
        let Some(el) = self.doc.get(&session.id).cloned() else { return };
        let size = self.renderer.local_size(&el);
        let vp = self.viewport();
        let corners = el.world_corners(size).map(|c| {
            let s = self.camera.world_to_screen(c, vp);
            canvas.min + egui::vec2(s.x as f32, s.y as f32)
        });
        let bounds = egui::Rect::from_points(&corners);
        let state = self.current_format_state().unwrap_or_default();
        let theme = self.theme();
        let p = theme::palette(ui.ctx());
        let bar_h = 40.0;
        let want = egui::pos2(bounds.left(), bounds.top() - bar_h - 12.0);
        let pos = egui::pos2(
            want.x.clamp(canvas.left() + 8.0, (canvas.right() - 470.0).max(canvas.left() + 8.0)),
            if want.y < canvas.top() + 8.0 { (bounds.bottom() + 12.0).min(canvas.bottom() - bar_h - 8.0) } else { want.y },
        );
        let mut clicked: Option<TextFormat> = None;
        let area = egui::Area::new(egui::Id::new("text-format-bar")).fixed_pos(pos).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(p.surface)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(10)
                .shadow(egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) })
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let toggle = |ui: &mut egui::Ui, glyph: &str, on: bool, tip: &str| {
                            let text = RichText::new(glyph).size(16.0).color(if on { p.on_accent } else { p.text });
                            let r = ui.add(egui::Button::new(text).fill(if on { p.accent } else { Color32::TRANSPARENT }).corner_radius(6).min_size(egui::vec2(30.0, 28.0)));
                            // Name the button for screen readers (the visible label is an icon).
                            r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, tip));
                            r.on_hover_text(tip)
                        };
                        let cmd = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" };
                        if toggle(ui, icon::TEXT_B, state.bold, &format!("Bold ({cmd}B)")).clicked() {
                            clicked = Some(TextFormat::Bold);
                        }
                        if toggle(ui, icon::TEXT_ITALIC, state.italic, &format!("Italic ({cmd}I)")).clicked() {
                            clicked = Some(TextFormat::Italic);
                        }
                        if toggle(ui, icon::TEXT_UNDERLINE, state.underline, &format!("Underline ({cmd}U)")).clicked() {
                            clicked = Some(TextFormat::Underline);
                        }
                        if toggle(ui, icon::TEXT_STRIKETHROUGH, state.strike, &format!("Strikethrough ({cmd}Shift+X)")).clicked() {
                            clicked = Some(TextFormat::Strike);
                        }
                        ui.separator();
                        let swatches = [theme.heading, theme.text, theme.muted, theme.accents[0], theme.accents[1], theme.accents[2], theme.accents[3]];
                        for c in swatches {
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 28.0), egui::Sense::click());
                            let fill = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), c.a());
                            let r = if resp.hovered() { 9.0 } else { 7.5 };
                            ui.painter().circle(rect.center(), r, fill, egui::Stroke::new(1.0, p.border));
                            if resp.on_hover_text("Text color").clicked() {
                                clicked = Some(TextFormat::Color(c));
                            }
                        }
                        ui.separator();
                        if toggle(ui, "A−", false, &format!("Smaller ({cmd}[)")).clicked() {
                            clicked = Some(TextFormat::Smaller);
                        }
                        if toggle(ui, "A+", false, &format!("Larger ({cmd}])")).clicked() {
                            clicked = Some(TextFormat::Bigger);
                        }
                        if toggle(ui, icon::ERASER, false, &format!("Clear formatting ({cmd}\\\\)")).clicked() {
                            clicked = Some(TextFormat::Clear);
                        }
                    });
                });
        });
        if let Some(s) = &mut self.text_edit {
            s.format_bar = Some(area.response.rect);
        }
        if let Some(fmt) = clicked {
            self.format_text(fmt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(t: &TextBlock) -> Vec<(&str, SpanStyle)> {
        t.spans.iter().map(|s| (&t.content[s.start..s.end], s.style.clone())).collect()
    }

    #[test]
    fn toggles_follow_word_processor_rules() {
        let mut t = TextBlock::new("one two three");
        format_range(&mut t, 4..7, TextFormat::Bold);
        assert_eq!(spans(&t)[0].0, "two");
        // A range that is partly bold becomes all bold…
        format_range(&mut t, 0..7, TextFormat::Bold);
        assert_eq!(spans(&t)[0].0, "one two");
        // …and toggling again makes it all regular.
        format_range(&mut t, 0..7, TextFormat::Bold);
        assert!(t.spans.is_empty());
        format_range(&mut t, 8..13, TextFormat::Bigger);
        let size = spans(&t)[0].1.size.unwrap();
        assert!((size - t.font_size * SIZE_STEP).abs() < 1e-9);
        format_range(&mut t, 0..13, TextFormat::Clear);
        assert!(t.spans.is_empty());
    }

    #[test]
    fn pending_formatting_applies_to_typed_text() {
        let mut t = TextBlock::new("Hello ");
        let mut pending = Pending::default();
        toggle_pending(&t, 6, &mut pending, TextFormat::Bold);
        toggle_pending(&t, 6, &mut pending, TextFormat::Underline);
        let changed = t.set_content("Hello world");
        apply_pending(&mut t, changed, &pending);
        let got = spans(&t);
        assert_eq!(got[0].0, "world");
        assert!(got[0].1.bold == Some(true) && got[0].1.underline);
        let st = format_state(&t, Some(11..11), None);
        assert!(st.bold && st.underline, "the caret after the bold word types bold");
    }

    #[test]
    fn whole_box_formatting_changes_the_box_style() {
        let mut t = TextBlock::new("Title");
        format_box(&mut t, TextFormat::Bold);
        assert_eq!(t.weight, 700);
        assert!(t.spans.is_empty());
        format_box(&mut t, TextFormat::Underline);
        assert!(spans(&t)[0].1.underline);
        let red = Color::rgb(200, 0, 0);
        format_box(&mut t, TextFormat::Color(red));
        assert_eq!(t.color, red);
        let before = t.font_size;
        format_box(&mut t, TextFormat::Bigger);
        assert!(t.font_size > before);
    }

    #[test]
    fn offsets_cross_paragraphs() {
        let s = "ab\ncd\nשלום";
        assert_eq!(byte_offset(s, 0, 1), 1);
        assert_eq!(byte_offset(s, 1, 0), 3);
        assert_eq!(byte_offset(s, 2, 2), 8);
        let state = EditState { cursor: glide_render::TextCursor::new(1, 2), anchor: Some(glide_render::TextCursor::new(0, 1)) };
        assert_eq!(selection_range(s, &state), Some(1..5));
    }
}
