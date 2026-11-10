//! Right-to-left text in egui widgets.
//!
//! egui lays text out in logical (typing) order, so Hebrew and Arabic come out
//! reversed in text fields and labels. The canvas uses cosmic-text, which does
//! bidi properly; for the side panels we keep egui's galley (its glyph
//! positions stay logical, so egui's own editing logic keeps working) and only
//! move each glyph's mesh to its visual position. Text fields then draw the
//! caret and selection themselves and map clicks and ←/→ through the same
//! visual layout. Text without right-to-left characters is left to egui.

use eframe::egui::{self, text::CCursor, text_selection::CCursorRange, Color32, Galley, Pos2, Rect, Stroke, TextBuffer};
use std::sync::Arc;
use unicode_bidi::BidiInfo;

/// True when `s` has a strong right-to-left character (Hebrew, Arabic, …).
pub fn has_rtl(s: &str) -> bool {
    s.chars().any(is_rtl_char)
}

fn is_rtl_char(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// Gives every right-to-left character its own section. egui shapes each run
/// in the direction it guesses from the run's first letter, which reverses
/// some Hebrew runs and not others and breaks its one-glyph-per-char mapping;
/// shaped one by one, the glyphs stay in logical order for [`reorder`].
pub fn split_rtl_sections(job: &mut egui::text::LayoutJob) {
    let mut sections = Vec::with_capacity(job.sections.len());
    for section in job.sections.drain(..) {
        let range = section.byte_range.start.0..section.byte_range.end.0;
        let mut start = range.start;
        let mut first = true;
        let mut push = |r: std::ops::Range<usize>, sections: &mut Vec<egui::text::LayoutSection>| {
            if !r.is_empty() {
                let leading_space = if first { section.leading_space } else { 0.0 };
                first = false;
                sections.push(egui::text::LayoutSection { leading_space, byte_range: egui::text::ByteIndex(r.start)..egui::text::ByteIndex(r.end), format: section.format.clone() });
            }
        };
        for (b, c) in job.text[range.clone()].char_indices() {
            if is_rtl_char(c) {
                let at = range.start + b;
                push(start..at, &mut sections);
                push(at..at + c.len_utf8(), &mut sections);
                start = at + c.len_utf8();
            }
        }
        push(start..range.end, &mut sections);
    }
    job.sections = sections;
}

/// Where each glyph of a reordered galley ended up.
#[derive(Clone, Default)]
pub struct VisualMap {
    rows: Vec<RowMap>,
}

#[derive(Clone, Default)]
struct RowMap {
    /// Global char index of the row's first glyph.
    start: usize,
    min_y: f32,
    max_y: f32,
    /// Visual left / right edge of each glyph, in galley coordinates.
    left: Vec<f32>,
    right: Vec<f32>,
    rtl: Vec<bool>,
    /// Where the caret sits on an empty row.
    empty_x: f32,
}

impl RowMap {
    fn len(&self) -> usize {
        self.left.len()
    }

    /// Caret position before the row's glyph `col` (after the previous one).
    fn caret_x(&self, col: usize) -> f32 {
        if self.len() == 0 {
            self.empty_x
        } else if col > 0 {
            let p = (col - 1).min(self.len() - 1);
            if self.rtl[p] { self.left[p] } else { self.right[p] }
        } else if self.rtl[0] {
            self.right[0]
        } else {
            self.left[0]
        }
    }

    /// Column for a point at `x` on this row.
    fn col_at(&self, x: f32) -> usize {
        if self.len() == 0 {
            return 0;
        }
        let hit = (0..self.len()).find(|&i| self.left[i] <= x && x < self.right[i]).unwrap_or_else(|| {
            let by = |f: fn(f32, f32) -> bool, edge: &Vec<f32>| (0..self.len()).reduce(|a, b| if f(edge[b], edge[a]) { b } else { a }).unwrap();
            if x < self.left.iter().copied().fold(f32::INFINITY, f32::min) { by(|a, b| a < b, &self.left) } else { by(|a, b| a > b, &self.right) }
        });
        let left_half = x < (self.left[hit] + self.right[hit]) / 2.0;
        if left_half == self.rtl[hit] { hit + 1 } else { hit }
    }
}

impl VisualMap {
    fn row_of(&self, index: usize) -> Option<&RowMap> {
        // A wrapped row's end is the next row's start: prefer the next row.
        self.rows.iter().rev().find(|r| r.start <= index).filter(|r| index <= r.start + r.len())
    }

    /// The caret rectangle (zero width) for a char index, in galley coordinates.
    pub fn caret(&self, index: usize) -> Option<Rect> {
        let r = self.row_of(index)?;
        let x = r.caret_x(index - r.start);
        Some(Rect::from_x_y_ranges(x..=x, r.min_y..=r.max_y))
    }

    /// The char index under a point in galley coordinates.
    pub fn index_at(&self, p: Pos2) -> usize {
        let Some(r) = self.rows.iter().find(|r| p.y < r.max_y).or(self.rows.last()) else { return 0 };
        r.start + r.col_at(p.x)
    }

    /// Selection rectangles for chars `a..b`, in galley coordinates.
    pub fn selection(&self, a: usize, b: usize) -> Vec<Rect> {
        let mut out = Vec::new();
        for r in &self.rows {
            for i in 0..r.len() {
                if (a..b).contains(&(r.start + i)) {
                    out.push(Rect::from_x_y_ranges(r.left[i]..=r.right[i], r.min_y..=r.max_y));
                }
            }
        }
        out
    }

    /// The caret one step to the left or right of `index`, visually.
    pub fn step(&self, index: usize, right: bool, total: usize) -> usize {
        let Some(r) = self.row_of(index) else { return index };
        let here = r.caret_x(index - r.start);
        let best = (0..=r.len())
            .map(|c| (c, r.caret_x(c)))
            .filter(|&(_, x)| if right { x > here + 0.5 } else { x < here - 0.5 })
            .min_by(|a, b| (a.1 - here).abs().total_cmp(&(b.1 - here).abs()));
        match best {
            Some((c, _)) => r.start + c,
            // Off the end of the row: continue in the row's reading direction.
            None => {
                let forward = right != r.rtl.first().copied().unwrap_or(false);
                if forward { (index + 1).min(total) } else { index.saturating_sub(1) }
            }
        }
    }
}

/// Moves each glyph of `galley` to its visual (bidi) position. Right-to-left
/// paragraphs are right-aligned to `wrap_width` when it is finite.
pub fn reorder(galley: Arc<Galley>, wrap_width: f32) -> (Arc<Galley>, VisualMap) {
    let text = galley.job.text.clone();
    let mut g = (*galley).clone();
    let mut map = VisualMap::default();
    let paragraphs: Vec<&str> = text.split('\n').collect();
    let (mut para, mut para_char) = (0usize, 0usize);
    let mut global = 0usize;
    let mut bounds = g.mesh_bounds;
    let mut info = BidiInfo::new(paragraphs.first().copied().unwrap_or(""), None);
    for placed in &mut g.rows {
        let row = Arc::make_mut(&mut placed.row);
        let n = row.glyphs.len();
        let ptext = paragraphs.get(para).copied().unwrap_or("");
        let para_rtl = info.paragraphs.first().is_some_and(|p| p.level.is_rtl());
        // Byte range of this row inside its paragraph.
        let byte_at = |c: usize| ptext.char_indices().nth(c).map_or(ptext.len(), |(b, _)| b);
        let (b0, b1) = (byte_at(para_char), byte_at(para_char + n));
        let mut order: Vec<(usize, bool)> = Vec::with_capacity(n);
        if let Some(p) = info.paragraphs.first()
            && b1 > b0
        {
            let (levels, runs) = info.visual_runs(p, b0..b1);
            for run in runs {
                let rtl = levels[run.start].is_rtl();
                let mut chars: Vec<usize> = ptext[run.clone()].char_indices().map(|(b, _)| ptext[..run.start + b].chars().count() - para_char).collect();
                if rtl {
                    chars.reverse();
                }
                order.extend(chars.into_iter().filter(|&i| i < n).map(|i| (i, rtl)));
            }
        }
        if order.len() != n {
            order = (0..n).map(|i| (i, false)).collect();
        }
        let width: f32 = row.glyphs.iter().map(|gl| gl.advance_width).sum();
        let base = if para_rtl && wrap_width.is_finite() { (wrap_width - width).max(0.0) } else { 0.0 };
        let mut m = RowMap {
            start: global,
            min_y: placed.pos.y,
            max_y: placed.pos.y + row.size.y,
            left: vec![0.0; n],
            right: vec![0.0; n],
            rtl: vec![false; n],
            empty_x: placed.pos.x + if para_rtl && wrap_width.is_finite() { wrap_width } else { 0.0 },
        };
        let mut x = base;
        for &(i, rtl) in &order {
            let gl = &mut row.glyphs[i];
            let dx = x - gl.pos.x;
            if !gl.uv_rect.is_nothing() {
                let v0 = gl.first_vertex as usize;
                for v in &mut row.visuals.mesh.vertices[v0..v0 + 4] {
                    v.pos.x += dx;
                }
            }
            m.left[i] = placed.pos.x + x;
            m.right[i] = placed.pos.x + x + gl.advance_width;
            m.rtl[i] = rtl;
            x += gl.advance_width;
        }
        if base > 0.0 && n > 0 {
            row.visuals.mesh_bounds = row.visuals.mesh_bounds.translate(egui::vec2(base, 0.0)).union(row.visuals.mesh_bounds);
            bounds = bounds.union(row.visuals.mesh_bounds.translate(placed.pos.to_vec2()));
        }
        global += n;
        para_char += n;
        if placed.ends_with_newline {
            global += 1;
            para += 1;
            para_char = 0;
            info = BidiInfo::new(paragraphs.get(para).copied().unwrap_or(""), None);
        }
        map.rows.push(m);
    }
    g.mesh_bounds = bounds;
    if bounds.max.x > g.rect.max.x {
        // Right-aligned rows reach the wrap width; widgets clip to the galley.
        g.rect.max.x = bounds.max.x.max(wrap_width.min(bounds.max.x + 1.0));
    }
    (Arc::new(g), map)
}

/// Hint text for a text field, laid out in visual order when it is right-to-left.
pub fn hint(ui: &egui::Ui, text: &str) -> egui::WidgetText {
    if !has_rtl(text) {
        return text.into();
    }
    let font = egui::FontSelection::default().resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, ui.visuals().weak_text_color());
    split_rtl_sections(&mut job);
    reorder(ui.fonts_mut(|f| f.layout_job(job)), f32::INFINITY).0.into()
}

/// A label for `job` that shows right-to-left text in visual order.
pub fn label(ui: &mut egui::Ui, job: egui::text::LayoutJob) -> egui::Response {
    if !has_rtl(&job.text) {
        return ui.label(job);
    }
    let wrap = job.wrap.max_width;
    let mut job = job;
    split_rtl_sections(&mut job);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let (galley, _) = reorder(galley, wrap);
    ui.add(egui::Label::new(galley))
}

/// A multi-line text field that shows and edits right-to-left text in visual
/// order. `configure` sets hint text, sizes and so on.
pub fn multiline(ui: &mut egui::Ui, id_salt: impl std::hash::Hash + std::fmt::Debug, text: &mut String, configure: impl FnOnce(egui::TextEdit) -> egui::TextEdit) -> egui::Response {
    edit(ui, id_salt, text, true, configure)
}

/// A one-line text field that shows right-to-left text in visual order.
pub fn singleline(ui: &mut egui::Ui, id_salt: impl std::hash::Hash + std::fmt::Debug, text: &mut String, configure: impl FnOnce(egui::TextEdit) -> egui::TextEdit) -> egui::Response {
    edit(ui, id_salt, text, false, configure)
}

fn edit(ui: &mut egui::Ui, id_salt: impl std::hash::Hash + std::fmt::Debug, text: &mut String, multiline: bool, configure: impl FnOnce(egui::TextEdit) -> egui::TextEdit) -> egui::Response {
    let id = ui.make_persistent_id(id_salt);
    fn new(text: &mut String, multiline: bool) -> egui::TextEdit<'_> {
        if multiline { egui::TextEdit::multiline(text) } else { egui::TextEdit::singleline(text) }
    }
    if !has_rtl(text) {
        return ui.add(configure(new(text, multiline).id(id)));
    }
    let map_id = id.with("bidi-map");
    // ←/→ move visually, using the layout from the previous frame.
    if ui.memory(|m| m.has_focus(id))
        && let Some(map) = ui.data(|d| d.get_temp::<Arc<VisualMap>>(map_id))
        && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id)
        && let Some(range) = state.cursor.char_range()
    {
        let total = text.chars().count();
        let mut moved = None;
        ui.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Key { key: key @ (egui::Key::ArrowLeft | egui::Key::ArrowRight), pressed, modifiers, .. } if !modifiers.command && !modifiers.alt && !modifiers.ctrl => {
                    if *pressed {
                        let right = *key == egui::Key::ArrowRight;
                        let target = if !modifiers.shift && !range.is_empty() {
                            let [a, b] = range.sorted_cursors();
                            let (ca, cb) = (map.caret(a.index.0).map_or(0.0, |r| r.left()), map.caret(b.index.0).map_or(0.0, |r| r.left()));
                            if (ca < cb) == right { b.index.0 } else { a.index.0 }
                        } else {
                            map.step(range.primary.index.0, right, total)
                        };
                        moved = Some((target, modifiers.shift));
                    }
                    false
                }
                _ => true,
            })
        });
        if let Some((target, shift)) = moved {
            let secondary = if shift { range.secondary } else { CCursor::new(target) };
            state.cursor.set_char_range(Some(CCursorRange { primary: CCursor::new(target), secondary, h_pos: None }));
            state.store(ui.ctx(), id);
        }
    }

    let built = std::cell::RefCell::new(None);
    let mut layouter = |ui: &egui::Ui, buf: &dyn TextBuffer, wrap_width: f32| {
        let font = egui::FontSelection::default().resolve(ui.style());
        let color = ui.visuals().override_text_color.unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
        let mut job = if multiline {
            egui::text::LayoutJob::simple(buf.as_str().to_owned(), font.clone(), color, wrap_width)
        } else {
            egui::text::LayoutJob::simple_singleline(buf.as_str().to_owned(), font.clone(), color)
        };
        job.keep_trailing_whitespace = true;
        let line_height = ui.fonts_mut(|f| f.row_height(&font)) + ui.spacing().extra_text_line_spacing;
        for s in &mut job.sections {
            s.format.line_height = Some(line_height);
        }
        split_rtl_sections(&mut job);
        let (galley, map) = reorder(ui.fonts_mut(|f| f.layout_job(job)), if multiline { wrap_width } else { f32::INFINITY });
        *built.borrow_mut() = Some(map);
        galley
    };
    let caret_stroke = ui.visuals().text_cursor.stroke;
    let selection = ui.visuals().selection.bg_fill;
    let out = ui
        .scope(|ui| {
            // egui would draw its caret and selection at the logical positions.
            ui.visuals_mut().text_cursor.stroke.color = Color32::TRANSPARENT;
            ui.visuals_mut().selection.bg_fill = Color32::TRANSPARENT;
            configure(new(text, multiline).id(id)).layouter(&mut layouter).show(ui)
        })
        .inner;
    let Some(map) = built.into_inner() else { return out.response.response };
    let map = Arc::new(map);
    ui.data_mut(|d| d.insert_temp(map_id, map.clone()));
    let origin = out.galley_pos - egui::vec2(out.galley.rect.left(), 0.0);
    let response = out.response.response.clone();

    // Clicks and drags land on the glyph under the pointer.
    let mut state = out.state;
    let mut range = out.cursor_range;
    if response.is_pointer_button_down_on() && !response.double_clicked() && !response.triple_clicked() {
        let pointer = ui.input(|i| (i.pointer.press_origin(), i.pointer.interact_pos()));
        if let (Some(from), Some(to)) = pointer {
            let r = CCursorRange {
                primary: CCursor::new(map.index_at(to - origin.to_vec2())),
                secondary: CCursor::new(map.index_at(from - origin.to_vec2())),
                h_pos: None,
            };
            state.cursor.set_char_range(Some(r));
            state.clone().store(ui.ctx(), id);
            range = Some(r);
        }
    }

    if ui.memory(|m| m.has_focus(id))
        && let Some(r) = range
    {
        let painter = ui.painter().with_clip_rect(out.text_clip_rect);
        let [a, b] = r.sorted_cursors();
        for rect in map.selection(a.index.0, b.index.0) {
            painter.rect_filled(rect.translate(origin.to_vec2()), 0.0, selection.gamma_multiply(0.55));
        }
        if let Some(c) = map.caret(r.primary.index.0) {
            let c = c.translate(origin.to_vec2());
            painter.line_segment([c.left_top(), c.left_bottom()], Stroke::new(caret_stroke.width.max(1.5), caret_stroke.color));
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(ctx: &egui::Context, text: &str) -> (Arc<Galley>, VisualMap) {
        let mut out = None;
        crate::theme::install_fonts(ctx);
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        let mut full = ctx.run_ui(Default::default(), |ui| {
            let job = egui::text::LayoutJob::simple(text.to_owned(), egui::FontId::proportional(14.0), Color32::WHITE, 300.0);
            let mut job = job;
            split_rtl_sections(&mut job);
            out = Some(reorder(ui.fonts_mut(|f| f.layout_job(job)), 300.0));
        });
        full.textures_delta.clear();
        out.unwrap()
    }

    #[test]
    fn hebrew_runs_right_to_left_and_right_aligned() {
        let ctx = egui::Context::default();
        let (_, map) = layout(&ctx, "שלום");
        let r = &map.rows[0];
        // First letter is the rightmost, flush with the wrap width.
        assert!((r.right[0] - 300.0).abs() < 0.01, "{}", r.right[0]);
        assert!(r.left[0] > r.left[1] && r.left[1] > r.left[2] && r.left[2] > r.left[3]);
        // Typing at the end puts the caret at the left edge of the last letter.
        assert_eq!(map.caret(4).unwrap().left(), r.left[3]);
        // Clicking just left of the text lands at the end, right of it at the start.
        assert_eq!(map.index_at(Pos2::new(r.left[3] - 5.0, 5.0)), 4);
        assert_eq!(map.index_at(Pos2::new(299.0, 5.0)), 0);
        // ← moves forward through Hebrew.
        assert_eq!(map.step(1, false, 4), 2);
        assert_eq!(map.step(1, true, 4), 0);
    }

    #[test]
    fn mixed_lines_keep_latin_words_left_to_right() {
        let ctx = egui::Context::default();
        let (_, map) = layout(&ctx, "abc שלום\nשלום abc");
        let (a, b) = (&map.rows[0], &map.rows[1]);
        assert_eq!(b.start, 9);
        // Line 1 is left-to-right: "abc" first, then the Hebrew word reversed.
        assert!(a.left[0] < a.left[1] && a.left[4] > a.left[5]);
        // Line 2 is right-to-left: Hebrew on the right, "abc" on its left in order.
        assert!(b.left[0] > b.left[5] && b.left[5] < b.left[6] && b.left[6] < b.left[7]);
        assert!(!has_rtl("plain"));
    }
}
