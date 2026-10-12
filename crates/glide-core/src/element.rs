use crate::color::Color;
use crate::geom::Aabb;
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

/// Non-destructive placement of an element on the canvas.
///
/// Geometry is always stored in the element's local space, centered on the
/// origin; the transform is applied at render time and never baked in.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    /// World-space position of the element's center.
    pub x: f64,
    pub y: f64,
    /// Uniform scale factor applied to the local geometry.
    pub scale: f64,
    /// Clockwise rotation in degrees (world space is y-down).
    pub rotation: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Transform { x: 0.0, y: 0.0, scale: 1.0, rotation: 0.0 }
    }
}

impl Transform {
    pub fn position(&self) -> DVec2 {
        DVec2::new(self.x, self.y)
    }

    pub fn set_position(&mut self, p: DVec2) {
        self.x = p.x;
        self.y = p.y;
    }

    /// Local → world affine transform.
    pub fn affine(&self) -> DAffine2 {
        DAffine2::from_scale_angle_translation(
            DVec2::splat(self.scale),
            self.rotation.to_radians(),
            self.position(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    /// Width in local units.
    pub width: f64,
}

/// A two-stop linear gradient. The shape's `fill` is the start color.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    /// End color.
    pub to: Color,
    /// Direction in degrees: 0 = left → right, 90 = top → bottom.
    #[serde(default)]
    pub angle: f64,
}

/// A soft drop shadow, in the element's local units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: Color,
    pub blur: f64,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

impl Shadow {
    pub fn soft() -> Self {
        Shadow { color: Color::rgba(0x10, 0x14, 0x24, 0x30), blur: 18.0, x: 0.0, y: 8.0 }
    }
    pub fn lifted() -> Self {
        Shadow { color: Color::rgba(0x10, 0x14, 0x24, 0x40), blur: 36.0, x: 0.0, y: 18.0 }
    }
    pub fn crisp() -> Self {
        Shadow { color: Color::rgba(0x10, 0x14, 0x24, 0x38), blur: 4.0, x: 0.0, y: 3.0 }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RectShape {
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub corner_radius: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EllipseShape {
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Paragraph list markers for a text box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListStyle {
    #[default]
    None,
    /// "•" before every non-empty paragraph, with a hanging indent.
    Bullet,
    /// "1." "2." … before every non-empty paragraph.
    Numbered,
}

impl ListStyle {
    pub fn is_none(&self) -> bool {
        *self == ListStyle::None
    }
}

/// Semantic role of a text box. Themes restyle text by role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    Title,
    Heading,
    Subtitle,
    Body,
    Caption,
    Quote,
}

impl TextRole {
    pub const ALL: [TextRole; 6] =
        [TextRole::Title, TextRole::Heading, TextRole::Subtitle, TextRole::Body, TextRole::Caption, TextRole::Quote];

    pub fn label(self) -> &'static str {
        match self {
            TextRole::Title => "Title",
            TextRole::Heading => "Heading",
            TextRole::Subtitle => "Subtitle",
            TextRole::Body => "Body",
            TextRole::Caption => "Caption",
            TextRole::Quote => "Quote",
        }
    }

    pub fn is_display(self) -> bool {
        matches!(self, TextRole::Title | TextRole::Heading | TextRole::Quote)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextBlock {
    pub content: String,
    /// Font size in local units.
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    /// A family name, or one of `sans-serif`, `serif`, `monospace`.
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_weight")]
    pub weight: u16,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default = "default_text_color")]
    pub color: Color,
    #[serde(default)]
    pub align: TextAlign,
    /// Line height as a multiple of the font size.
    #[serde(default = "default_line_height")]
    pub line_height: f64,
    /// Wrap width in local units. `None` means no wrapping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    /// Semantic style (title, body, …) used when switching themes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<TextRole>,
    /// Bullets or numbering for each paragraph.
    #[serde(default, skip_serializing_if = "ListStyle::is_none")]
    pub list: ListStyle,
}

impl TextBlock {
    pub fn new(content: impl Into<String>) -> Self {
        TextBlock {
            content: content.into(),
            font_size: default_font_size(),
            font_family: default_font_family(),
            weight: default_weight(),
            italic: false,
            color: default_text_color(),
            align: TextAlign::Left,
            line_height: default_line_height(),
            max_width: None,
            role: None,
            list: ListStyle::None,
        }
    }
}

/// A waypoint frame: a rectangle on the canvas that the camera can travel to
/// and that becomes one slide on export. Frames are invisible by default — the
/// editor only hints at them on hover/selection — unless given a fill or border.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub corner_radius: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Stroke>,
}

impl Frame {
    /// A 16:9 invisible frame of the given width.
    pub fn widescreen(width: f64) -> Self {
        Frame { width, height: width * 9.0 / 16.0, corner_radius: 0.0, fill: None, border: None }
    }

    pub fn is_invisible(&self) -> bool {
        self.fill.is_none_or(|c| c.a() == 0) && self.border.as_ref().is_none_or(|b| b.width <= 0.0 || b.color.a() == 0)
    }
}

/// A reference to a local media file. Paths are resolved relative to the
/// document file. Media is never fetched from the network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Media {
    pub path: String,
    pub width: f64,
    pub height: f64,
    /// Rounded-corner mask (images, videos). Half the shorter side gives a circle/pill.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub corner_radius: f64,
    /// Video: restart automatically at the end.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub looping: bool,
    /// Video: play without sound.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
    /// Video: start playing when its slide is reached in the show.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub autoplay: bool,
}

impl Media {
    pub fn new(path: impl Into<String>, width: f64, height: f64) -> Self {
        Media { path: path.into(), width, height, corner_radius: 0.0, looping: false, muted: false, autoplay: true }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IconStyle {
    #[default]
    Regular,
    Light,
    Bold,
    Fill,
}

impl IconStyle {
    pub const ALL: [IconStyle; 4] = [IconStyle::Light, IconStyle::Regular, IconStyle::Bold, IconStyle::Fill];
    pub fn label(self) -> &'static str {
        match self {
            IconStyle::Regular => "Regular",
            IconStyle::Light => "Light",
            IconStyle::Bold => "Bold",
            IconStyle::Fill => "Fill",
        }
    }
}

/// A vector icon from the bundled Phosphor library, by kebab-case name
/// (e.g. `"rocket-launch"`). Rendered from glyph outlines, so it is crisp at
/// any zoom and exports to PowerPoint as an editable vector shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Icon {
    pub name: String,
    #[serde(default)]
    pub style: IconStyle,
    /// Edge length of the square icon box, in local units.
    pub size: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementKind {
    Rect(RectShape),
    Ellipse(EllipseShape),
    Text(TextBlock),
    Frame(Frame),
    Icon(Icon),
    Image(Media),
    Svg(Media),
    Video(Media),
}

impl ElementKind {
    pub fn is_frame(&self) -> bool {
        matches!(self, ElementKind::Frame(_))
    }

    /// The media file this element references, if any (relative to the document).
    pub fn media_path(&self) -> Option<&str> {
        match self {
            ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) => Some(&m.path),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            ElementKind::Rect(_) => "rect",
            ElementKind::Ellipse(_) => "ellipse",
            ElementKind::Text(_) => "text",
            ElementKind::Frame(_) => "frame",
            ElementKind::Icon(_) => "icon",
            ElementKind::Image(_) => "image",
            ElementKind::Svg(_) => "svg",
            ElementKind::Video(_) => "video",
        }
    }

    /// Local size for kinds whose size is known without text layout.
    pub fn intrinsic_size(&self) -> Option<DVec2> {
        match self {
            ElementKind::Rect(r) => Some(DVec2::new(r.width, r.height)),
            ElementKind::Ellipse(e) => Some(DVec2::new(e.width, e.height)),
            ElementKind::Frame(f) => Some(DVec2::new(f.width, f.height)),
            ElementKind::Icon(i) => Some(DVec2::splat(i.size)),
            ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) => {
                Some(DVec2::new(m.width, m.height))
            }
            ElementKind::Text(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    /// Stable identifier. Left empty in operations to have one generated.
    #[serde(default)]
    pub id: String,
    /// Human-facing object name; also used as the object identity when
    /// exporting (e.g. PowerPoint Morph matching).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// Optional soft drop shadow (shapes, frames and images).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Shadow>,
    pub kind: ElementKind,
}

impl Element {
    pub fn new(kind: ElementKind) -> Self {
        Element {
            id: String::new(),
            name: String::new(),
            transform: Transform::default(),
            opacity: 1.0,
            locked: false,
            hidden: false,
            shadow: None,
            kind,
        }
    }

    pub fn with_transform(mut self, t: Transform) -> Self {
        self.transform = t;
        self
    }

    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            match &self.kind {
                ElementKind::Text(t) => {
                    let first = t.content.lines().next().unwrap_or("").trim();
                    let mut s: String = first.chars().take(24).collect();
                    if first.chars().count() > 24 {
                        s.push('…');
                    }
                    if s.is_empty() { "text".into() } else { s }
                }
                k => k.type_name().into(),
            }
        } else {
            self.name.clone()
        }
    }

    /// Corners of the local bounds (centered on the origin) in world space,
    /// in order top-left, top-right, bottom-right, bottom-left.
    pub fn world_corners(&self, local_size: DVec2) -> [DVec2; 4] {
        let a = self.transform.affine();
        let h = local_size * 0.5;
        [
            a.transform_point2(DVec2::new(-h.x, -h.y)),
            a.transform_point2(DVec2::new(h.x, -h.y)),
            a.transform_point2(DVec2::new(h.x, h.y)),
            a.transform_point2(DVec2::new(-h.x, h.y)),
        ]
    }

    pub fn world_aabb(&self, local_size: DVec2) -> Aabb {
        Aabb::from_points(self.world_corners(local_size))
    }

    /// Precise hit test of a world-space point against the element shape.
    pub fn hit_test(&self, world: DVec2, local_size: DVec2, tolerance_world: f64) -> bool {
        let a = self.transform.affine();
        if a.matrix2.determinant().abs() < 1e-300 {
            return false;
        }
        let local = a.inverse().transform_point2(world);
        let tol = tolerance_world / self.transform.scale.abs().max(1e-300);
        let h = local_size * 0.5 + DVec2::splat(tol);
        match &self.kind {
            // Frames are transparent to clicks except along their outline, so
            // the objects they group stay directly editable.
            ElementKind::Frame(_) => {
                let band = tol * 2.5;
                let inner = local_size * 0.5 - DVec2::splat(band);
                let inside_outer = local.x.abs() <= h.x + band && local.y.abs() <= h.y + band;
                inside_outer && !(local.x.abs() < inner.x && local.y.abs() < inner.y)
            }
            ElementKind::Ellipse(_) => {
                if h.x <= 0.0 || h.y <= 0.0 {
                    return false;
                }
                (local.x / h.x).powi(2) + (local.y / h.y).powi(2) <= 1.0
            }
            _ => local.x.abs() <= h.x && local.y.abs() <= h.y,
        }
    }
}

fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}
fn is_true(v: &bool) -> bool {
    *v
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn default_font_size() -> f64 {
    64.0
}
fn default_font_family() -> String {
    "sans-serif".into()
}
fn default_weight() -> u16 {
    400
}
fn default_text_color() -> Color {
    Color::rgb(0x1f, 0x23, 0x2b)
}
fn default_line_height() -> f64 {
    1.25
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_hit_test() {
        let el = Element::new(ElementKind::Rect(RectShape { width: 100.0, height: 10.0, ..Default::default() }))
        .with_transform(Transform { x: 10.0, y: 10.0, scale: 2.0, rotation: 90.0 });
        let size = DVec2::new(100.0, 10.0);
        // Rotated 90°, scaled 2x: spans y in [-90, 110], x in [0, 20].
        assert!(el.hit_test(DVec2::new(10.0, 100.0), size, 0.0));
        assert!(!el.hit_test(DVec2::new(100.0, 10.0), size, 0.0));
    }

    #[test]
    fn json_shape_is_stable() {
        let json = r##"{"id":"t","kind":{"type":"text","content":"Hi"}}"##;
        let el: Element = serde_json::from_str(json).unwrap();
        assert_eq!(el.transform, Transform::default());
        let ElementKind::Text(t) = &el.kind else { panic!() };
        assert_eq!(t.font_size, 64.0);
    }
}
