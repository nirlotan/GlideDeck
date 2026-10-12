//! Design intelligence: alignment and the design check, plus re-exports of the
//! slide layouts and starter templates. Everything here is plain document
//! manipulation, so the same building blocks serve the editor, tests and agents.

use glam::DVec2;
use glide_core::theme::{blend_over, contrast_ratio};
use glide_core::{assets, Aabb, Color, Document, ElementKind, TextBlock, TextRole, Theme};
use glide_render::TextEngine;

pub use crate::layouts::{place_layout, LayoutKind, LAYOUT_W};
pub use crate::templates::{build_template, build_template_with, Template};

// --- alignment ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignOp {
    Left,
    HCenter,
    Right,
    Top,
    VMiddle,
    Bottom,
    DistributeH,
    DistributeV,
}

/// Aligns elements in view space (so results look right in a rotated view).
/// A single element aligns to the smallest frame that contains it.
pub fn align(doc: &mut Document, text: &mut TextEngine, ids: &[String], op: AlignOp, view_rotation: f64) {
    let to_view = DVec2::from_angle(-view_rotation);
    let to_world = DVec2::from_angle(view_rotation);
    let mut boxes: Vec<(usize, Aabb)> = Vec::new();
    for id in ids {
        let Some(i) = doc.index_of(id) else { continue };
        let el = &doc.elements[i];
        if el.locked {
            continue;
        }
        let size = glide_render::local_size(text, &el.kind);
        boxes.push((i, Aabb::from_points(el.world_corners(size).map(|c| to_view.rotate(c)))));
    }
    if boxes.is_empty() {
        return;
    }
    let reference = if boxes.len() == 1 {
        let (i, _) = boxes[0];
        let center = doc.elements[i].transform.position();
        let mut best: Option<Aabb> = None;
        for (j, f) in doc.elements.iter().enumerate() {
            if j == i || !f.kind.is_frame() {
                continue;
            }
            let size = f.kind.intrinsic_size().unwrap_or_default();
            let inv = f.transform.affine().inverse();
            let local = inv.transform_point2(center);
            if local.x.abs() <= size.x / 2.0 && local.y.abs() <= size.y / 2.0 {
                let fb = Aabb::from_points(f.world_corners(size).map(|c| to_view.rotate(c)));
                if best.is_none_or(|bb| fb.size().x * fb.size().y < bb.size().x * bb.size().y) {
                    best = Some(fb);
                }
            }
        }
        match best {
            Some(f) => f,
            None => return,
        }
    } else {
        boxes.iter().fold(Aabb::EMPTY, |acc, (_, b)| acc.union(*b))
    };

    let mut deltas: Vec<(usize, DVec2)> = Vec::new();
    match op {
        AlignOp::DistributeH | AlignOp::DistributeV => {
            if boxes.len() < 3 {
                return;
            }
            let horizontal = op == AlignOp::DistributeH;
            let key = |b: &Aabb| if horizontal { b.center().x } else { b.center().y };
            boxes.sort_by(|a, b| key(&a.1).total_cmp(&key(&b.1)));
            let first = key(&boxes[0].1);
            let last = key(&boxes[boxes.len() - 1].1);
            let n = boxes.len() - 1;
            for (k, (i, b)) in boxes.iter().enumerate() {
                let target = first + (last - first) * k as f64 / n as f64;
                let d = target - key(b);
                deltas.push((*i, if horizontal { DVec2::new(d, 0.0) } else { DVec2::new(0.0, d) }));
            }
        }
        _ => {
            for (i, b) in &boxes {
                let d = match op {
                    AlignOp::Left => DVec2::new(reference.min.x - b.min.x, 0.0),
                    AlignOp::HCenter => DVec2::new(reference.center().x - b.center().x, 0.0),
                    AlignOp::Right => DVec2::new(reference.max.x - b.max.x, 0.0),
                    AlignOp::Top => DVec2::new(0.0, reference.min.y - b.min.y),
                    AlignOp::VMiddle => DVec2::new(0.0, reference.center().y - b.center().y),
                    AlignOp::Bottom => DVec2::new(0.0, reference.max.y - b.max.y),
                    _ => DVec2::ZERO,
                };
                deltas.push((*i, d));
            }
        }
    }
    for (i, d) in deltas {
        let t = &mut doc.elements[i].transform;
        t.set_position(t.position() + to_world.rotate(d));
    }
}

// --- design check ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Warning,
    Suggestion,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    TextColor { id: String, color: Color },
    ScaleText { id: String, factor: f64 },
    UnifyFonts,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub target: Option<String>,
    pub step: Option<usize>,
    pub fix: Option<Fix>,
}

/// Smallest path frame containing each point, with its step index.
fn owning_step(doc: &Document, p: DVec2) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64, f64)> = None;
    for (i, step) in doc.path.iter().enumerate() {
        let Some(f) = doc.get(&step.frame) else { continue };
        let Some(size) = f.kind.intrinsic_size() else { continue };
        let local = f.transform.affine().inverse().transform_point2(p);
        if local.x.abs() <= size.x / 2.0 && local.y.abs() <= size.y / 2.0 {
            let h = size.y * f.transform.scale.abs();
            let area = size.x * size.y * f.transform.scale * f.transform.scale;
            if best.is_none_or(|(_, a, _)| area < a) {
                best = Some((i, area, h));
            }
        }
    }
    best.map(|(i, _, h)| (i, h))
}

/// The opaque color behind a point, looking at filled shapes beneath `below`.
fn backdrop_at(doc: &Document, text: &mut TextEngine, p: DVec2, below: usize) -> Vec<Color> {
    let mut base = vec![doc.background];
    if let Some(to) = doc.background_to {
        base.push(to);
    }
    for el in doc.elements[..below].iter().rev().filter(|e| !e.hidden) {
        let fill = match &el.kind {
            ElementKind::Rect(r) => r.fill,
            ElementKind::Ellipse(e) => e.fill,
            ElementKind::Frame(f) => f.fill,
            _ => None,
        };
        let Some(fill) = fill else { continue };
        if fill.a() < 0x40 {
            continue;
        }
        let size = glide_render::local_size(text, &el.kind);
        if el.hit_test(p, size, 0.0) {
            return base.iter().map(|b| blend_over(fill, *b)).collect();
        }
    }
    base
}

pub fn design_check(doc: &Document, text: &mut TextEngine, base_dir: &std::path::Path, theme: &Theme) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut fonts = std::collections::BTreeSet::new();

    for (i, el) in doc.elements.iter().enumerate().filter(|(_, e)| !e.hidden) {
        let center = el.transform.position();
        match &el.kind {
            ElementKind::Text(t) => {
                if t.content.trim().is_empty() {
                    continue;
                }
                fonts.insert(t.font_family.clone());
                let backs = backdrop_at(doc, text, center, i);
                let fg = |b: Color| blend_over(t.color, b);
                let worst = backs.iter().map(|b| contrast_ratio(fg(*b), *b)).fold(f64::INFINITY, f64::min);
                let large = t.weight >= 600 || t.role.is_some_and(|r| r.is_display());
                let needed = if large { 3.0 } else { 4.5 };
                if worst < needed {
                    let candidates = [theme.text, theme.heading, Color::WHITE, Color::rgb(0x11, 0x11, 0x14)];
                    let best = candidates
                        .into_iter()
                        .max_by(|a, b| {
                            let ca = backs.iter().map(|bg| contrast_ratio(*a, *bg)).fold(f64::INFINITY, f64::min);
                            let cb = backs.iter().map(|bg| contrast_ratio(*b, *bg)).fold(f64::INFINITY, f64::min);
                            ca.total_cmp(&cb)
                        })
                        .unwrap_or(Color::BLACK);
                    issues.push(Issue {
                        level: Level::Warning,
                        title: format!("Low contrast: “{}”", el.display_name()),
                        detail: format!("Contrast is {worst:.1}:1; aim for at least {needed:.1}:1 so it reads on a projector."),
                        target: Some(el.id.clone()),
                        step: owning_step(doc, center).map(|(s, _)| s),
                        fix: Some(Fix::TextColor { id: el.id.clone(), color: best }),
                    });
                }
                if let Some((step, frame_h)) = owning_step(doc, center) {
                    let px = t.font_size * el.transform.scale.abs() * 1080.0 / frame_h.max(1e-12);
                    if px < 18.0 {
                        issues.push(Issue {
                            level: Level::Warning,
                            title: format!("Small text on slide {}", step + 1),
                            detail: format!("“{}” would be about {px:.0} px tall on a 1080p screen. 24 px or more reads comfortably.", el.display_name()),
                            target: Some(el.id.clone()),
                            step: Some(step),
                            fix: Some(Fix::ScaleText { id: el.id.clone(), factor: 24.0 / px.max(1e-9) }),
                        });
                    }
                }
            }
            ElementKind::Image(m) => {
                let path = assets::resolve(Some(base_dir), &m.path);
                match glide_render::image_dimensions(&path) {
                    None => issues.push(Issue {
                        level: Level::Warning,
                        title: format!("Missing image: {}", el.display_name()),
                        detail: format!("{} could not be found next to the document.", m.path),
                        target: Some(el.id.clone()),
                        step: None,
                        fix: None,
                    }),
                    Some((_, h)) => {
                        if let Some((step, frame_h)) = owning_step(doc, center) {
                            let shown = m.height * el.transform.scale.abs() * 1080.0 / frame_h.max(1e-12);
                            if shown > h as f64 * 1.6 {
                                issues.push(Issue {
                                    level: Level::Suggestion,
                                    title: format!("Low-resolution image on slide {}", step + 1),
                                    detail: format!("Shown at ~{shown:.0} px tall but the file is {h} px; it may look soft."),
                                    target: Some(el.id.clone()),
                                    step: Some(step),
                                    fix: None,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Empty slides.
    for (i, step) in doc.path.iter().enumerate() {
        let Some(f) = doc.get(&step.frame) else { continue };
        let Some(size) = f.kind.intrinsic_size() else { continue };
        let inv = f.transform.affine().inverse();
        let has_content = doc.elements.iter().any(|e| {
            !e.kind.is_frame() && !e.hidden && {
                let l = inv.transform_point2(e.transform.position());
                l.x.abs() <= size.x / 2.0 && l.y.abs() <= size.y / 2.0
            }
        });
        if !has_content {
            issues.push(Issue {
                level: Level::Suggestion,
                title: format!("Slide {} is empty", i + 1),
                detail: "Add content, or insert a layout, so the camera has something to show.".into(),
                target: Some(f.id.clone()),
                step: Some(i),
                fix: None,
            });
        }
    }

    if fonts.len() > 3 {
        issues.push(Issue {
            level: Level::Suggestion,
            title: format!("{} different fonts", fonts.len()),
            detail: format!("Using the theme's two fonts ({} / {}) gives a calmer, more consistent look.", theme.heading_font, theme.body_font),
            target: None,
            step: None,
            fix: Some(Fix::UnifyFonts),
        });
    }
    issues.sort_by_key(|i| i.level);
    issues
}

pub fn apply_fix(doc: &mut Document, theme: &Theme, fix: &Fix) {
    match fix {
        Fix::TextColor { id, color } => {
            if let Some(ElementKind::Text(t)) = doc.get_mut(id).map(|e| &mut e.kind) {
                t.color = *color;
            }
        }
        Fix::ScaleText { id, factor } => {
            if let Some(el) = doc.get_mut(id) {
                el.transform.scale *= factor;
            }
        }
        Fix::UnifyFonts => {
            for el in &mut doc.elements {
                if let ElementKind::Text(t) = &mut el.kind {
                    let display = t.role.map(|r| r.is_display()).unwrap_or(t.weight >= 600);
                    t.font_family = if display { theme.heading_font.clone() } else { theme.body_font.clone() };
                }
            }
        }
    }
}

/// A text box with a theme role, for quick inserts.
pub fn styled_text(theme: &Theme, role: TextRole) -> TextBlock {
    let mut t = theme.text_style(role);
    t.content = match role {
        TextRole::Title => "Title",
        TextRole::Heading => "Heading",
        TextRole::Subtitle => "Subtitle",
        TextRole::Body => "Body text",
        TextRole::Caption => "Caption",
        TextRole::Quote => "“A memorable quote”",
    }
    .into();
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layouts::{at, layout_elements, LAYOUT_H};
    use glide_core::{Element, RectShape};

    #[test]
    fn layouts_fit_inside_their_frame() {
        let theme = glide_core::theme::default_theme();
        let mut text = TextEngine::new();
        for kind in LayoutKind::ALL {
            for el in layout_elements(kind, &theme) {
                let b = el.world_aabb(glide_render::local_size(&mut text, &el.kind));
                assert!(
                    b.min.x >= -LAYOUT_W / 2.0 && b.max.x <= LAYOUT_W / 2.0 && b.min.y >= -LAYOUT_H / 2.0 && b.max.y <= LAYOUT_H / 2.0,
                    "{kind:?}: {} overflows the frame: {b:?}",
                    el.name
                );
            }
        }
    }

    #[test]
    fn align_and_distribute() {
        let mut doc = Document::new("t");
        let mut text = TextEngine::new();
        let mut ids = Vec::new();
        for (x, y, w) in [(0.0, 0.0, 10.0), (50.0, 30.0, 20.0), (200.0, -10.0, 10.0)] {
            let el = Element::new(ElementKind::Rect(RectShape { width: w, height: 10.0, ..Default::default() })).with_transform(at(x, y));
            ids.push(doc.add(el));
        }
        align(&mut doc, &mut text, &ids, AlignOp::Left, 0.0);
        let lefts: Vec<f64> = doc.elements.iter().map(|e| e.transform.x - e.kind.intrinsic_size().unwrap().x / 2.0).collect();
        assert!(lefts.iter().all(|l| (l - lefts[0]).abs() < 1e-9));
        align(&mut doc, &mut text, &ids, AlignOp::Top, 0.0);
        assert!(doc.elements.iter().all(|e| (e.transform.y - doc.elements[0].transform.y).abs() < 1e-9));
        doc.elements[2].transform.x = 300.0;
        align(&mut doc, &mut text, &ids, AlignOp::DistributeH, 0.0);
        let xs: Vec<f64> = doc.elements.iter().map(|e| e.transform.x).collect();
        assert!(((xs[1] - xs[0]) - (xs[2] - xs[1])).abs() < 1e-9, "{xs:?}");
    }

    #[test]
    fn design_check_flags_and_fixes_contrast() {
        let theme = glide_core::theme::default_theme();
        let mut doc = Document::new("t");
        doc.theme = Some(theme.clone());
        let mut t = TextBlock::new("Hard to read");
        t.color = Color::rgb(0xEE, 0xEE, 0xEE);
        let id = doc.add(Element::new(ElementKind::Text(t)));
        let mut text = TextEngine::new();
        let issues = design_check(&doc, &mut text, std::path::Path::new("."), &theme);
        let issue = issues.iter().find(|i| i.target.as_deref() == Some(id.as_str())).expect("contrast issue");
        apply_fix(&mut doc, &theme, issue.fix.as_ref().unwrap());
        let issues = design_check(&doc, &mut text, std::path::Path::new("."), &theme);
        assert!(issues.iter().all(|i| i.target.as_deref() != Some(id.as_str())), "fixed: {issues:?}");
    }
}
