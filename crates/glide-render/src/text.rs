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
use glide_core::{Color, IconStyle, ListStyle, SpanStyle, TextAlign, TextBlock};

/// All text is shaped at this size and the outlines scaled afterwards, which
/// keeps shaping well-behaved for both microscopic and gigantic font sizes.
const REFERENCE_SIZE: f32 = 64.0;

struct PlacedGlyph {
    key: CacheKey,
    /// Pen position (baseline) relative to the block's top-left, in reference units.
    x: f32,
    y: f32,
    /// Formatted color, when it differs from the text box's.
    color: Option<Color>,
    /// Byte offset of the glyph's character in the text (None for list markers).
    start: Option<usize>,
}

/// An underline or strikethrough bar (reference units, block top-left origin).
struct Decoration {
    x0: f32,
    x1: f32,
    /// Top edge of the bar.
    y: f32,
    thickness: f32,
    color: Option<Color>,
    /// Byte offset of the first character it underlines.
    start: Option<usize>,
}

/// One glyph (or decoration bar) of laid-out text, as an outline in local
/// space centred on the origin. Used by exporters that animate characters.
pub struct GlyphOutline {
    /// Byte offset of the character in the text; None for list markers.
    pub start: Option<usize>,
    /// The formatted color, or None for the text box's own color.
    pub color: Option<Color>,
    pub path: Path,
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
    decorations: Vec<Decoration>,
    /// Ink extent of each laid-out line: x0, top, x1, bottom (reference units).
    lines: Vec<[f32; 4]>,
    /// Local units per reference unit.
    scale: f64,
}

impl TextLayout {
    /// Each laid-out line's box (x0, top, x1, bottom) in local units from the
    /// block's top-left: as wide as its glyphs, as tall as the line.
    pub fn line_boxes(&self) -> Vec<[f64; 4]> {
        self.lines.iter().map(|l| l.map(|v| v as f64 * self.scale)).collect()
    }
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

    /// Every font family available for text: the fonts installed on this
    /// computer plus any loaded from files, sorted by name.
    pub fn families(&self) -> Vec<String> {
        let icon_ids: Vec<cosmic_text::fontdb::ID> = self.icon_fonts.values().copied().collect();
        let mut names: Vec<String> = self
            .fonts
            .db()
            .faces()
            .filter(|f| !icon_ids.contains(&f.id))
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .filter(|n| !n.is_empty() && !n.starts_with('.') && !n.to_lowercase().starts_with("phosphor"))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        names
    }

    /// Whether a family is available (installed or loaded).
    pub fn has_family(&self, name: &str) -> bool {
        let want = name.trim();
        self.fonts.db().faces().any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(want)))
    }

    /// Loads a font file (TTF, OTF or a TTC collection) so its families can
    /// be used; returns their names. Cached layouts are dropped, since text
    /// that fell back to another font may now shape differently.
    pub fn load_font_data(&mut self, bytes: Vec<u8>) -> Vec<String> {
        let ids = self.fonts.db_mut().load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(bytes)));
        let mut names: Vec<String> = ids
            .iter()
            .filter_map(|id| self.fonts.db().face(*id))
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        names.dedup();
        self.layouts.clear();
        names
    }

    /// Glyph-by-glyph outlines (plus underline and strikethrough bars), in
    /// local space centred on the origin, for exporters that animate
    /// individual characters.
    pub fn glyph_outlines(&mut self, layout: &TextLayout) -> Vec<GlyphOutline> {
        let k = layout.scale as f32;
        let half = (layout.size * 0.5).as_vec2();
        let mut out = Vec::with_capacity(layout.glyphs.len() + layout.decorations.len());
        for (i, g) in layout.glyphs.iter().enumerate() {
            let path = self.glyph_path_at(layout, i).build();
            out.push(GlyphOutline { start: g.start, color: g.color, path });
        }
        for d in &layout.decorations {
            let (x0, x1) = (d.x0 * k - half.x, d.x1 * k - half.x);
            let (y0, y1) = (d.y * k - half.y, (d.y + d.thickness) * k - half.y);
            let mut b = Path::builder();
            b.begin(point(x0, y0));
            b.line_to(point(x1, y0));
            b.line_to(point(x1, y1));
            b.line_to(point(x0, y1));
            b.end(true);
            out.push(GlyphOutline { start: d.start, color: d.color, path: b.build() });
        }
        out
    }

    fn glyph_path_at(&mut self, layout: &TextLayout, index: usize) -> lyon::path::path::Builder {
        let target: *const PlacedGlyph = &layout.glyphs[index];
        self.glyph_path(layout, |g| std::ptr::eq(g, target))
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

    /// Attributes for a formatted span on top of the text box's own.
    fn span_attrs<'a>(text: &'a TextBlock, style: &SpanStyle, line_height: f32) -> Attrs<'a> {
        let mut a = Self::attrs(text);
        if let Some(bold) = style.bold {
            let w = match (bold, text.weight >= 600) {
                (true, true) | (false, false) => text.weight,
                (true, false) => 700,
                (false, true) => 400,
            };
            a = a.weight(Weight(w.clamp(1, 1000)));
        }
        if let Some(italic) = style.italic {
            a = a.style(if italic { Style::Italic } else { Style::Normal });
        }
        if let Some(c) = style.color {
            a = a.color(cosmic_text::Color::rgba(c.r(), c.g(), c.b(), c.a()));
        }
        if style.underline {
            a = a.underline(cosmic_text::UnderlineStyle::Single);
        }
        if style.strike {
            a = a.strikethrough();
        }
        if let Some(size) = style.size {
            let r = (size / text.font_size.abs().max(1e-12)).clamp(0.05, 50.0) as f32;
            a = a.metrics(Metrics::new(REFERENCE_SIZE * r, line_height * r));
        }
        a
    }

    /// Puts the text (with its formatted spans) into a buffer.
    fn set_buffer_text(b: &mut cosmic_text::BorrowedWithFontSystem<'_, Buffer>, text: &TextBlock, line_height: f32, align: Option<cosmic_text::Align>) {
        let attrs = Self::attrs(text);
        if text.spans.is_empty() {
            b.set_text(&text.content, &attrs, Shaping::Advanced, align);
        } else {
            let segments = text.segments();
            let spans = segments.iter().map(|(start, end, style)| (&text.content[*start..*end], Self::span_attrs(text, style, line_height)));
            b.set_rich_text(spans, &attrs, Shaping::Advanced, align);
        }
    }

    /// Shapes `text` into a cosmic-text buffer exactly as it is drawn. Both
    /// rendering and in-place editing use this, so carets line up perfectly.
    fn build(&mut self, text: &TextBlock) -> Built {
        let font_size = text.font_size.abs().max(1e-12);
        let scale = font_size / REFERENCE_SIZE as f64;
        let line_height = REFERENCE_SIZE * text.line_height.max(0.1) as f32;
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
            Self::set_buffer_text(&mut b, text, line_height, None);
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
            Self::set_buffer_text(&mut b, text, line_height, align);
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
                glyphs.push(PlacedGlyph { key: phys.cache_key, x: g.x, y: g.y, color: None, start: None });
            }
        }
        (glyphs, width)
    }

    fn shape(&mut self, text: &TextBlock) -> TextLayout {
        let built = self.build(text);
        let line_starts: Vec<usize> = std::iter::once(0).chain(text.content.match_indices('\n').map(|(i, _)| i + 1)).collect();
        let mut glyphs = Vec::new();
        let mut decorations = Vec::new();
        let mut lines: Vec<[f32; 4]> = Vec::new();
        let to_color = |c: Option<cosmic_text::Color>| c.map(|c| Color::rgba(c.r(), c.g(), c.b(), c.a()));
        let mut markers: Vec<(f32, f32, bool, usize)> = Vec::new(); // (line_y, line start x, rtl, number)
        let mut last_line = usize::MAX;
        let mut number = 0usize;
        for run in built.buffer.layout_runs() {
            let dx = built.offset(run.rtl);
            let ink: Vec<&cosmic_text::LayoutGlyph> = run.glyphs.iter().filter(|g| run.text.get(g.start..g.end).is_some_and(|s| !s.trim().is_empty())).collect();
            if !ink.is_empty() {
                let x0 = ink.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
                let x1 = ink.iter().map(|g| g.x + g.w).fold(f32::NEG_INFINITY, f32::max);
                lines.push([dx + x0, run.line_top, dx + x1, run.line_top + run.line_height]);
            }
            for g in run.glyphs {
                let phys = g.physical((0.0, 0.0), 1.0);
                glyphs.push(PlacedGlyph {
                    key: phys.cache_key,
                    x: dx + g.x + g.font_size * g.x_offset,
                    y: run.line_y + g.y - g.font_size * g.y_offset,
                    color: to_color(g.color_opt),
                    start: line_starts.get(run.line_i).map(|l| l + g.start),
                });
            }
            // Underlines and strikethroughs, placed with the font's own metrics.
            for span in run.decorations {
                let Some(gs) = run.glyphs.get(span.glyph_range.clone()) else { continue };
                let x0 = gs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
                let x1 = gs.iter().map(|g| g.x + g.w).fold(f32::NEG_INFINITY, f32::max);
                if x1 <= x0 {
                    continue;
                }
                let d = &span.data;
                let fs = span.font_size;
                let color = to_color(span.color_opt);
                let start = gs.first().and_then(|g| line_starts.get(run.line_i).map(|l| l + g.start));
                if d.text_decoration.underline != cosmic_text::UnderlineStyle::None {
                    let thickness = (d.underline_metrics.thickness * fs).max(fs * 0.04);
                    decorations.push(Decoration { x0: dx + x0, x1: dx + x1, y: run.line_y - d.underline_metrics.offset * fs, thickness, color, start });
                }
                if d.text_decoration.strikethrough {
                    let thickness = (d.strikethrough_metrics.thickness * fs).max(fs * 0.04);
                    decorations.push(Decoration { x0: dx + x0, x1: dx + x1, y: run.line_y - d.strikethrough_metrics.offset * fs, thickness, color, start });
                }
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
            glyphs.extend(mg.into_iter().map(|g| PlacedGlyph { key: g.key, x: x0 + g.x, y: line_y + g.y, color: None, start: None }));
        }
        TextLayout {
            size: DVec2::new(built.box_w() as f64 * built.scale, built.height as f64 * built.scale),
            glyphs,
            decorations,
            lines,
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
        self.colored_outlines(layout).into_iter().fold(Path::builder(), |mut b, (_, p)| {
            b.extend_from_paths(&[p.as_slice()]);
            b
        })
        .build()
    }

    /// Glyph outlines and decorations grouped by color (`None` = the text
    /// box's color), in local space centered on the origin.
    pub fn colored_outlines(&mut self, layout: &TextLayout) -> Vec<(Option<Color>, Path)> {
        let mut colors: Vec<Option<Color>> = Vec::new();
        for c in layout.glyphs.iter().map(|g| g.color).chain(layout.decorations.iter().map(|d| d.color)) {
            if !colors.contains(&c) {
                colors.push(c);
            }
        }
        let k = layout.scale as f32;
        let half = (layout.size * 0.5).as_vec2();
        colors
            .into_iter()
            .map(|color| {
                let mut b = self.glyph_path(layout, |g| g.color == color);
                for d in layout.decorations.iter().filter(|d| d.color == color) {
                    let (x0, x1) = (d.x0 * k - half.x, d.x1 * k - half.x);
                    let (y0, y1) = (d.y * k - half.y, (d.y + d.thickness) * k - half.y);
                    b.begin(point(x0, y0));
                    b.line_to(point(x1, y0));
                    b.line_to(point(x1, y1));
                    b.line_to(point(x0, y1));
                    b.end(true);
                }
                (color, b.build())
            })
            .collect()
    }

    fn glyph_path(&mut self, layout: &TextLayout, include: impl Fn(&PlacedGlyph) -> bool) -> lyon::path::path::Builder {
        let mut b = Path::builder();
        let k = layout.scale as f32;
        let half = (layout.size * 0.5).as_vec2();
        for g in layout.glyphs.iter().filter(|g| include(g)) {
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
        b
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
    for s in &t.spans {
        (s.start, s.end, s.style.bold, s.style.italic, s.style.underline, s.style.strike, s.style.color).hash(&mut h);
        s.style.size.map(f64::to_bits).hash(&mut h);
    }
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

    #[test]
    fn formatted_spans_render_bold_color_decorations_and_size() {
        let mut engine = TextEngine::new();
        let plain = TextBlock::new("Make it bold");
        let base = engine.layout(&plain);

        let mut bold = plain.clone();
        bold.format_range(8..12, |s| s.bold = Some(true));
        let l = engine.layout(&bold);
        assert!(l.size.x > base.size.x, "a bold word is wider: {} vs {}", l.size.x, base.size.x);

        let mut colored = plain.clone();
        colored.format_range(0..4, |s| s.color = Some(Color::rgb(220, 20, 60)));
        colored.format_range(5..7, |s| s.underline = true);
        let l = engine.layout(&colored);
        let groups = engine.colored_outlines(&l);
        assert_eq!(groups.len(), 2, "the box color plus one formatted color");
        assert!(groups.iter().any(|(c, _)| *c == Some(Color::rgb(220, 20, 60))));
        assert_eq!(l.decorations.len(), 1, "one underline");
        let d = &l.decorations[0];
        assert!(d.x1 > d.x0 && d.thickness > 0.0);

        let mut big = plain.clone();
        big.format_range(8..12, |s| s.size = Some(plain.font_size * 2.0));
        assert!(engine.layout(&big).size.y > base.size.y * 1.5, "a larger word makes its line taller");

        // Caret geometry follows the formatted layout.
        let r = engine.edit(&big, EditState { cursor: Cursor::new(0, 12), anchor: None }, &[]);
        let caret = r.caret.unwrap();
        assert!((caret[1].y - caret[0].y) > base.size.y * 1.5);
    }
}
