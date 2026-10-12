//! Text shaping (cosmic-text) and glyph outline extraction.
//!
//! Text is rendered as tessellated vector outlines rather than a bitmap
//! atlas, so it stays crisp at every zoom level of the infinite canvas.

use cosmic_text::{
    Action, Attrs, Buffer, CacheKey, CacheKeyFlags, Command, Cursor, Edit, Editor, Family, FontSystem, Metrics, Motion,
    Selection, Shaping, Style, SwashCache, Weight, Wrap,
};
use glam::DVec2;
use lyon::math::point;
use lyon::path::Path;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use glide_core::{IconStyle, ListStyle, TextAlign, TextBlock};

/// All text is shaped at this size and the outlines scaled afterwards, which
/// keeps shaping well-behaved for both microscopic and gigantic font sizes.
const REFERENCE_SIZE: f32 = 64.0;

struct PlacedGlyph {
    key: CacheKey,
    /// Pen position (baseline) relative to the block's top-left, in reference units.
    x: f32,
    y: f32,
}

/// A shaped buffer plus the geometry needed to place it.
struct Built {
    buffer: Buffer,
    /// Width of the text column (reference units, excluding the list indent).
    text_w: f32,
    /// Hanging indent for list markers (reference units).
    indent: f32,
    height: f32,
    scale: f64,
}

impl Built {
    fn box_w(&self) -> f32 {
        self.text_w + self.indent
    }
    /// Horizontal offset of a run: left-to-right list text sits after the
    /// indent; right-to-left paragraphs keep the indent on their right.
    fn offset(&self, rtl: bool) -> f32 {
        if rtl { 0.0 } else { self.indent }
    }
}

/// Caret and selection anchor while editing text in place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditState {
    pub cursor: Cursor,
    pub anchor: Option<Cursor>,
}

impl EditState {
    /// Everything selected, caret at the end.
    pub fn select_all(content: &str) -> Self {
        let lines: Vec<&str> = content.split('\n').collect();
        let last = lines.len().saturating_sub(1);
        EditState { cursor: Cursor::new(last, lines[last].len()), anchor: Some(Cursor::new(0, 0)) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditAction {
    Insert(String),
    Enter,
    Backspace,
    Delete,
    Move { motion: Motion, select: bool },
    SelectAll,
    /// A click at a point in the element's local space (`extend` = shift-click).
    Click { at: DVec2, extend: bool },
    DoubleClick { at: DVec2 },
    Drag { at: DVec2 },
    Copy,
    Cut,
}

pub struct EditResult {
    pub content: String,
    pub state: EditState,
    /// Caret top and bottom, local space.
    pub caret: Option<[DVec2; 2]>,
    /// Selection rectangles (min, max corners), local space.
    pub selection: Vec<[DVec2; 2]>,
    /// Text produced by Copy/Cut.
    pub copied: Option<String>,
}

/// The shaped form of a [`TextBlock`].
pub struct TextLayout {
    /// Block size in the element's local units.
    pub size: DVec2,
    glyphs: Vec<PlacedGlyph>,
    /// Local units per reference unit.
    scale: f64,
}

/// A resolution-independent path command (local units, y down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCmd {
    Move([f32; 2]),
    Line([f32; 2]),
    Quad([f32; 2], [f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Close,
}

/// Looks up a Phosphor icon codepoint by kebab-case name.
pub fn icon_codepoint(name: &str) -> Option<char> {
    static MAP: std::sync::OnceLock<HashMap<&'static str, char>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| crate::icon_table::ICONS.iter().copied().collect()).get(name).copied()
}

fn icon_font_bytes(style: IconStyle) -> &'static [u8] {
    match style {
        IconStyle::Regular => include_bytes!("../assets/phosphor/Phosphor.ttf"),
        IconStyle::Light => include_bytes!("../assets/phosphor/Phosphor-Light.ttf"),
        IconStyle::Bold => include_bytes!("../assets/phosphor/Phosphor-Bold.ttf"),
        IconStyle::Fill => include_bytes!("../assets/phosphor/Phosphor-Fill.ttf"),
    }
}

pub struct TextEngine {
    fonts: FontSystem,
    swash: SwashCache,
    layouts: HashMap<u64, Arc<TextLayout>>,
    icon_fonts: HashMap<IconStyle, cosmic_text::fontdb::ID>,
    /// Icon outlines normalised to a unit box centred on the origin.
    icons: HashMap<(String, IconStyle), Option<Arc<Vec<PathCmd>>>>,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine {
    /// Loads the locally installed system fonts. Nothing is downloaded.
    ///
    /// Generic families (`sans-serif`, …) are pinned to fonts that are
    /// actually installed, so weights resolve predictably and exports name
    /// the same typeface the canvas draws.
    pub fn new() -> Self {
        let mut fonts = FontSystem::new();
        let installed = |fs: &FontSystem, name: &str| fs.db().faces().any(|f| f.families.iter().any(|(n, _)| n == name));
        let pick = |fs: &FontSystem, names: &[&str]| names.iter().find(|n| installed(fs, n)).map(|n| n.to_string());
        let sans = pick(&fonts, &["Segoe UI", "Helvetica Neue", "Inter", "Noto Sans", "DejaVu Sans", "Arial"]);
        let serif = pick(&fonts, &["Georgia", "Charter", "Noto Serif", "DejaVu Serif", "Times New Roman"]);
        let mono = pick(&fonts, &["Cascadia Mono", "Consolas", "SF Mono", "Menlo", "JetBrains Mono", "DejaVu Sans Mono"]);
        let db = fonts.db_mut();
        if let Some(f) = sans {
            db.set_sans_serif_family(f);
        }
        if let Some(f) = serif {
            db.set_serif_family(f);
        }
        if let Some(f) = mono {
            db.set_monospace_family(f);
        }
        TextEngine { fonts, swash: SwashCache::new(), layouts: HashMap::new(), icon_fonts: HashMap::new(), icons: HashMap::new() }
    }

    /// The outline of an icon in a `size` × `size` box centred on the origin.
    pub fn icon_outline(&mut self, name: &str, style: IconStyle, size: f32) -> Option<Vec<PathCmd>> {
        let key = (name.to_owned(), style);
        if !self.icons.contains_key(&key) {
            let outline = self.build_icon(name, style).map(Arc::new);
            self.icons.insert(key.clone(), outline);
        }
        let unit = self.icons.get(&key)?.as_ref()?;
        let s = |p: [f32; 2]| [p[0] * size, p[1] * size];
        Some(
            unit.iter()
                .map(|c| match *c {
                    PathCmd::Move(p) => PathCmd::Move(s(p)),
                    PathCmd::Line(p) => PathCmd::Line(s(p)),
                    PathCmd::Quad(a, p) => PathCmd::Quad(s(a), s(p)),
                    PathCmd::Cubic(a, b, p) => PathCmd::Cubic(s(a), s(b), s(p)),
                    PathCmd::Close => PathCmd::Close,
                })
                .collect(),
        )
    }

    fn build_icon(&mut self, name: &str, style: IconStyle) -> Option<Vec<PathCmd>> {
        const REF: f32 = 256.0;
        let ch = icon_codepoint(name)?;
        let id = match self.icon_fonts.get(&style) {
            Some(id) => *id,
            None => {
                let src = cosmic_text::fontdb::Source::Binary(Arc::new(icon_font_bytes(style)));
                let id = *self.fonts.db_mut().load_font_source(src).first()?;
                self.icon_fonts.insert(style, id);
                id
            }
        };
        let font = self.fonts.get_font(id, Weight::NORMAL)?;
        let swash = font.as_swash();
        let glyph = swash.charmap().map(ch as u32);
        if glyph == 0 {
            return None;
        }
        let m = swash.metrics(&[]);
        let upm = m.units_per_em as f32;
        // Centre of the em box: horizontally half the advance, vertically
        // between ascender and descender.
        let advance = swash.glyph_metrics(&[]).advance_width(glyph) / upm * REF;
        let cx = if advance > 0.0 { advance / 2.0 } else { REF / 2.0 };
        let cy = (m.ascent - m.descent) / 2.0 / upm * REF;
        let (key, _, _) = CacheKey::new(id, glyph, REF, (0.0, 0.0), Weight::NORMAL, CacheKeyFlags::DISABLE_HINTING);
        let cmds = self.swash.get_outline_commands(&mut self.fonts, key)?;
        let map = |x: f32, y: f32| [(x - cx) / REF, (cy - y) / REF];
        Some(
            cmds.iter()
                .map(|c| match *c {
                    Command::MoveTo(p) => PathCmd::Move(map(p.x, p.y)),
                    Command::LineTo(p) => PathCmd::Line(map(p.x, p.y)),
                    Command::QuadTo(a, p) => PathCmd::Quad(map(a.x, a.y), map(p.x, p.y)),
                    Command::CurveTo(a, b, p) => PathCmd::Cubic(map(a.x, a.y), map(b.x, b.y), map(p.x, p.y)),
                    Command::Close => PathCmd::Close,
                })
                .collect(),
        )
    }

    /// The concrete installed family a generic name (`sans-serif`, …) maps to.
    pub fn resolve_family(&self, family: &str) -> String {
        let f = family_of(family);
        self.fonts.db().family_name(&f).to_owned()
    }

    pub fn layout(&mut self, text: &TextBlock) -> Arc<TextLayout> {
        let key = layout_hash(text);
        if let Some(l) = self.layouts.get(&key) {
            return l.clone();
        }
        let layout = Arc::new(self.shape(text));
        if self.layouts.len() > 4096 {
            self.layouts.clear();
        }
        self.layouts.insert(key, layout.clone());
        layout
    }

    fn attrs(text: &TextBlock) -> Attrs<'_> {
        Attrs::new()
            .family(family_of(&text.font_family))
            .weight(Weight(text.weight.clamp(1, 1000)))
            .style(if text.italic { Style::Italic } else { Style::Normal })
            .cache_key_flags(CacheKeyFlags::DISABLE_HINTING)
    }

    /// Shapes `text` into a cosmic-text buffer exactly as it is drawn. Both
    /// rendering and in-place editing use this, so carets line up perfectly.
    fn build(&mut self, text: &TextBlock) -> Built {
        let font_size = text.font_size.abs().max(1e-12);
        let scale = font_size / REFERENCE_SIZE as f64;
        let line_height = REFERENCE_SIZE * text.line_height.max(0.1) as f32;
        let attrs = Self::attrs(text);
        let indent = match text.list {
            ListStyle::None => 0.0,
            ListStyle::Bullet => REFERENCE_SIZE * 0.95,
            ListStyle::Numbered => REFERENCE_SIZE * 1.55,
        };
        let wrap_width = text.max_width.map(|w| (w / scale) as f32 - indent).filter(|w| *w > 0.0);

        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(REFERENCE_SIZE, line_height));
        {
            let mut b = buffer.borrow_with(&mut self.fonts);
            b.set_wrap(if wrap_width.is_some() { Wrap::WordOrGlyph } else { Wrap::None });
            b.set_size(wrap_width, None);
            b.set_text(&text.content, &attrs, Shaping::Advanced, None);
            b.shape_until_scroll(false);
        }
        // Measure, then lay out once more so alignment is relative to the block width.
        let measured = buffer.layout_runs().fold(0f32, |w, run| w.max(run.line_w));
        let text_w = wrap_width.unwrap_or(measured);
        // "Left" means the paragraph's start edge: right for Hebrew/Arabic.
        let align = match text.align {
            TextAlign::Left => None,
            TextAlign::Center => Some(cosmic_text::Align::Center),
            TextAlign::Right => Some(cosmic_text::Align::Right),
        };
        {
            let mut b = buffer.borrow_with(&mut self.fonts);
            b.set_size(Some(text_w + 0.01), None);
            b.set_text(&text.content, &attrs, Shaping::Advanced, align);
            b.shape_until_scroll(false);
        }
        let height = buffer
            .layout_runs()
            .fold(0f32, |h, run| h.max(run.line_top + run.line_height))
            .max(line_height);
        Built { buffer, text_w, indent, height, scale }
    }

    /// Glyphs of a short marker string ("•", "12.") relative to its own origin.
    fn marker_glyphs(&mut self, marker: &str, text: &TextBlock) -> (Vec<PlacedGlyph>, f32) {
        let attrs = Self::attrs(text);
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(REFERENCE_SIZE, REFERENCE_SIZE));
        let mut b = buffer.borrow_with(&mut self.fonts);
        b.set_size(None, None);
        b.set_text(marker, &attrs, Shaping::Advanced, None);
        b.shape_until_scroll(false);
        let mut glyphs = Vec::new();
        let mut width = 0f32;
        for run in b.layout_runs() {
            width = width.max(run.line_w);
            for g in run.glyphs {
                let phys = g.physical((0.0, 0.0), 1.0);
                glyphs.push(PlacedGlyph { key: phys.cache_key, x: g.x, y: g.y });
            }
        }
        (glyphs, width)
    }

    fn shape(&mut self, text: &TextBlock) -> TextLayout {
        let built = self.build(text);
        let mut glyphs = Vec::new();
        let mut markers: Vec<(f32, f32, bool, usize)> = Vec::new(); // (line_y, line start x, rtl, number)
        let mut last_line = usize::MAX;
        let mut number = 0usize;
        for run in built.buffer.layout_runs() {
            let dx = built.offset(run.rtl);
            for g in run.glyphs {
                let phys = g.physical((0.0, 0.0), 1.0);
                glyphs.push(PlacedGlyph {
                    key: phys.cache_key,
                    x: dx + g.x + g.font_size * g.x_offset,
                    y: run.line_y + g.y - g.font_size * g.y_offset,
                });
            }
            if run.line_i != last_line {
                last_line = run.line_i;
                if text.list != ListStyle::None && !run.text.trim().is_empty() {
                    number += 1;
                    markers.push((run.line_y, run.line_w, run.rtl, number));
                }
            }
        }
        // List markers in the hanging indent (right side for right-to-left paragraphs).
        for (line_y, _w, rtl, n) in markers {
            let label = match text.list {
                ListStyle::Numbered => format!("{n}."),
                _ => "•".to_owned(),
            };
            let (mg, mw) = self.marker_glyphs(&label, text);
            let gap = REFERENCE_SIZE * 0.3;
            let x0 = if rtl {
                built.text_w + gap
            } else if text.list == ListStyle::Numbered {
                built.indent - gap - mw
            } else {
                REFERENCE_SIZE * 0.2
            };
            glyphs.extend(mg.into_iter().map(|g| PlacedGlyph { key: g.key, x: x0 + g.x, y: line_y + g.y }));
        }
        TextLayout {
            size: DVec2::new(built.box_w() as f64 * built.scale, built.height as f64 * built.scale),
            glyphs,
            scale: built.scale,
        }
    }

    /// Applies editing actions to a text block's content (with full bidi,
    /// cursor and selection semantics from cosmic-text) and returns the new
    /// content plus caret/selection geometry in the element's local space.
    pub fn edit(&mut self, text: &TextBlock, state: EditState, actions: &[EditAction]) -> EditResult {
        let built = self.build(text);
        let k = built.scale;
        let half = DVec2::new(built.box_w() as f64, built.height as f64) * k * 0.5;
        let to_buffer = |p: DVec2, rtl_offset: f32| -> (i32, i32) {
            let b = (p + half) / k;
            ((b.x as f32 - rtl_offset) as i32, b.y as i32)
        };
        let mut editor = Editor::new(built.buffer);
        let lines = editor.with_buffer(|b| b.lines.len());
        let clamp = |c: Cursor, ed: &Editor| -> Cursor {
            ed.with_buffer(|b| {
                let line = c.line.min(b.lines.len().saturating_sub(1));
                let len = b.lines.get(line).map(|l| l.text().len()).unwrap_or(0);
                let mut index = c.index.min(len);
                while index > 0 && !b.lines[line].text().is_char_boundary(index) {
                    index -= 1;
                }
                Cursor::new_with_affinity(line, index, c.affinity)
            })
        };
        let cursor = clamp(state.cursor, &editor);
        editor.set_cursor(cursor);
        if let Some(anchor) = state.anchor {
            editor.set_selection(Selection::Normal(clamp(anchor, &editor)));
        }
        let mut copied = None;
        let indent = built.indent;
        for action in actions {
            let fs = &mut self.fonts;
            match action {
                EditAction::Insert(s) => editor.insert_string(s, None),
                EditAction::Enter => {
                    editor.delete_selection();
                    editor.action(fs, Action::Enter);
                }
                EditAction::Backspace => editor.action(fs, Action::Backspace),
                EditAction::Delete => editor.action(fs, Action::Delete),
                EditAction::Move { motion, select } => {
                    if *select {
                        if editor.selection() == Selection::None {
                            editor.set_selection(Selection::Normal(editor.cursor()));
                        }
                    } else if let Some((a, b)) = editor.selection_bounds() {
                        // Collapse an existing selection toward the motion, like other editors.
                        editor.set_selection(Selection::None);
                        let toward_start = matches!(motion, Motion::Left | Motion::Up | Motion::Home | Motion::PreviousWord | Motion::BufferStart);
                        editor.set_cursor(if toward_start { a } else { b });
                        continue;
                    }
                    editor.action(fs, Action::Motion(*motion));
                }
                EditAction::SelectAll => {
                    editor.set_selection(Selection::Normal(Cursor::new(0, 0)));
                    let end = editor.with_buffer(|b| {
                        let last = b.lines.len().saturating_sub(1);
                        Cursor::new(last, b.lines.get(last).map(|l| l.text().len()).unwrap_or(0))
                    });
                    editor.set_cursor(end);
                }
                EditAction::Click { at, extend } => {
                    let (x, y) = to_buffer(*at, indent);
                    if *extend {
                        if editor.selection() == Selection::None {
                            editor.set_selection(Selection::Normal(editor.cursor()));
                        }
                        editor.action(fs, Action::Drag { x, y });
                    } else {
                        editor.action(fs, Action::Click { x, y });
                    }
                }
                EditAction::DoubleClick { at } => {
                    let (x, y) = to_buffer(*at, indent);
                    editor.action(fs, Action::DoubleClick { x, y });
                }
                EditAction::Drag { at } => {
                    let (x, y) = to_buffer(*at, indent);
                    editor.action(fs, Action::Drag { x, y });
                }
                EditAction::Copy => copied = editor.copy_selection(),
                EditAction::Cut => {
                    copied = editor.copy_selection();
                    editor.delete_selection();
                }
            }
        }
        let _ = lines;
        let content = editor.with_buffer(|b| b.lines.iter().map(|l| l.text()).collect::<Vec<_>>().join("\n"));
        let new_state = EditState {
            cursor: editor.cursor(),
            anchor: match editor.selection() {
                Selection::None => None,
                Selection::Normal(c) | Selection::Line(c) | Selection::Word(c) => Some(c),
            },
        };

        // Geometry from a fresh layout of the new content (what the renderer draws).
        let mut after = text.clone();
        after.content = content.clone();
        let built = self.build(&after);
        let k = built.scale;
        let half = DVec2::new(built.box_w() as f64, built.height as f64) * k * 0.5;
        let local = |x: f32, y: f32| DVec2::new(x as f64 * k, y as f64 * k) - half;
        let mut caret = None;
        let mut selection = Vec::new();
        let bounds = new_state.anchor.map(|a| if (a.line, a.index) <= (new_state.cursor.line, new_state.cursor.index) { (a, new_state.cursor) } else { (new_state.cursor, a) });
        for run in built.buffer.layout_runs() {
            let dx = built.offset(run.rtl);
            if caret.is_none()
                && let Some(x) = run.cursor_position(&new_state.cursor)
            {
                caret = Some([local(dx + x, run.line_top), local(dx + x, run.line_top + run.line_height)]);
            }
            if let Some((a, b)) = bounds {
                for (x, w) in run.highlight(a, b) {
                    selection.push([local(dx + x, run.line_top), local(dx + x + w.max(REFERENCE_SIZE * 0.15), run.line_top + run.line_height)]);
                }
            }
        }
        if caret.is_none() {
            let x = if text.align == TextAlign::Center { built.box_w() / 2.0 } else { built.offset(false) };
            caret = Some([local(x, 0.0), local(x, built.height)]);
        }
        EditResult { content, state: new_state, caret, selection, copied }
    }

    /// Builds the glyph outlines as one path in local space, centered on the origin.
    pub fn outline(&mut self, layout: &TextLayout) -> Path {
        let mut b = Path::builder();
        let k = layout.scale as f32;
        let half = (layout.size * 0.5).as_vec2();
        for g in &layout.glyphs {
            let Some(cmds) = self.swash.get_outline_commands(&mut self.fonts, g.key) else {
                continue;
            };
            let map = |x: f32, y: f32| point((g.x + x) * k - half.x, (g.y - y) * k - half.y);
            let mut open = false;
            for cmd in cmds {
                match *cmd {
                    Command::MoveTo(p) => {
                        if open {
                            b.end(false);
                        }
                        b.begin(map(p.x, p.y));
                        open = true;
                    }
                    Command::LineTo(p) if open => {
                        b.line_to(map(p.x, p.y));
                    }
                    Command::QuadTo(c, p) if open => {
                        b.quadratic_bezier_to(map(c.x, c.y), map(p.x, p.y));
                    }
                    Command::CurveTo(c1, c2, p) if open => {
                        b.cubic_bezier_to(map(c1.x, c1.y), map(c2.x, c2.y), map(p.x, p.y));
                    }
                    Command::Close if open => {
                        b.end(true);
                        open = false;
                    }
                    _ => {}
                }
            }
            if open {
                b.end(false);
            }
        }
        b.build()
    }
}

fn family_of(name: &str) -> Family<'_> {
    match name.trim() {
        "" | "sans-serif" | "sans" => Family::SansSerif,
        "serif" => Family::Serif,
        "monospace" | "mono" => Family::Monospace,
        name => Family::Name(name),
    }
}

pub(crate) fn layout_hash(t: &TextBlock) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.content.hash(&mut h);
    t.font_size.to_bits().hash(&mut h);
    t.font_family.hash(&mut h);
    t.weight.hash(&mut h);
    t.italic.hash(&mut h);
    t.align.hash(&mut h);
    t.line_height.to_bits().hash(&mut h);
    t.max_width.map(f64::to_bits).hash(&mut h);
    t.list.hash(&mut h);
    h.finish()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavier_weights_are_wider() {
        // Bold glyphs advance further than regular ones in every common UI face,
        // which catches weights silently falling back to a single face.
        let mut engine = TextEngine::new();
        let mut t = TextBlock::new("Weight check");
        t.weight = 400;
        let regular = engine.layout(&t).size.x;
        t.weight = 800;
        let heavy = engine.layout(&t).size.x;
        assert!(heavy > regular * 1.02, "regular {regular} vs heavy {heavy}");
    }

    #[test]
    fn icons_resolve_and_fit_their_box() {
        let mut engine = TextEngine::new();
        for style in IconStyle::ALL {
            let cmds = engine.icon_outline("rocket-launch", style, 100.0).expect("icon outline");
            let pts: Vec<[f32; 2]> = cmds
                .iter()
                .filter_map(|c| match c {
                    PathCmd::Move(p) | PathCmd::Line(p) | PathCmd::Quad(_, p) | PathCmd::Cubic(_, _, p) => Some(*p),
                    PathCmd::Close => None,
                })
                .collect();
            assert!(!pts.is_empty());
            for p in pts {
                assert!(p[0].abs() <= 50.5 && p[1].abs() <= 50.5, "{style:?} point {p:?} outside the box");
            }
        }
        assert!(engine.icon_outline("definitely-not-an-icon", IconStyle::Regular, 1.0).is_none());
    }

    #[test]
    fn bullets_add_a_hanging_indent() {
        let mut engine = TextEngine::new();
        let mut t = TextBlock::new("First point\nSecond point");
        let plain = engine.layout(&t).size;
        t.list = ListStyle::Bullet;
        let bulleted = engine.layout(&t);
        assert!(bulleted.size.x > plain.x, "indent widens the block");
        assert_eq!(bulleted.size.y, plain.y, "markers don't add lines");
        assert_eq!(bulleted.glyphs.len(), engine.layout(&TextBlock::new("First point\nSecond point")).glyphs.len() + 2);
    }

    #[test]
    fn editing_handles_hebrew_and_selection() {
        let mut engine = TextEngine::new();
        let t = TextBlock::new("שלום");
        let mut state = EditState { cursor: Cursor::new(0, "שלום".len()), anchor: None };
        let r = engine.edit(&t, state, &[EditAction::Insert(" עולם".into())]);
        assert_eq!(r.content, "שלום עולם");
        state = r.state;
        let t = TextBlock::new(r.content.clone());
        let r = engine.edit(&t, state, &[EditAction::SelectAll]);
        assert!(!r.selection.is_empty());
        let r = engine.edit(&t, r.state, &[EditAction::Cut]);
        assert_eq!(r.copied.as_deref(), Some("שלום עולם"));
        assert_eq!(r.content, "");
        // The caret of right-to-left text after typing sits at the visual left edge.
        let t = TextBlock::new("שלום");
        let end = EditState { cursor: Cursor::new(0, "שלום".len()), anchor: None };
        let r = engine.edit(&t, end, &[]);
        let caret = r.caret.unwrap();
        let size = engine.layout(&t).size;
        assert!(caret[0].x < -size.x * 0.4, "RTL end-of-text caret is on the left: {caret:?}");
    }
}
