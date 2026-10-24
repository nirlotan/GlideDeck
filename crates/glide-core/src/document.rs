use crate::color::Color;
use crate::easing::Easing;
use crate::element::Element;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Identifier written into every document; bumped on breaking schema changes.
pub const FORMAT_ID: &str = "glidedeck/1";
/// The identifier used before the project was renamed.
const LEGACY_FORMAT_ID: &str = "zoomcraft/1";

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("failed to read or write {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path} is not a valid GlideDeck document: {source}")]
    Parse { path: String, source: serde_json::Error },
    #[error("unsupported document format {0:?} (this build reads {FORMAT_ID:?})")]
    UnsupportedFormat(String),
}

/// How the camera travels into a step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionStyle {
    /// Cinematic arc: zooms out just enough to keep both views in context,
    /// then dives into the destination (van Wijk & Nuij optimal path).
    #[default]
    Smooth,
    /// Straight interpolation of position, zoom and rotation.
    Direct,
    /// Jump without animation.
    Cut,
    /// A sideways arc with a slight roll, like a camera on a crane.
    Swoop,
    /// The smooth zoom plus a full barrel roll on the way.
    Spin,
    /// Dips through the background color to the next slide.
    Fade,
    /// Zooms slightly past the slide, then settles.
    Punch,
}

impl TransitionStyle {
    pub const ALL: [TransitionStyle; 7] =
        [TransitionStyle::Smooth, TransitionStyle::Direct, TransitionStyle::Swoop, TransitionStyle::Spin, TransitionStyle::Punch, TransitionStyle::Fade, TransitionStyle::Cut];

    pub fn label(self) -> &'static str {
        match self {
            TransitionStyle::Smooth => "Smooth zoom",
            TransitionStyle::Direct => "Direct",
            TransitionStyle::Cut => "Cut",
            TransitionStyle::Swoop => "Swoop",
            TransitionStyle::Spin => "Spin",
            TransitionStyle::Fade => "Fade",
            TransitionStyle::Punch => "Punch",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            TransitionStyle::Smooth => "Cinematic arc: zoom out for context, then dive in",
            TransitionStyle::Direct => "Straight pan, zoom and rotate",
            TransitionStyle::Cut => "Jump without animation",
            TransitionStyle::Swoop => "A sideways arc with a gentle roll",
            TransitionStyle::Spin => "Smooth zoom with a full barrel roll",
            TransitionStyle::Fade => "Dip through the background to the next slide",
            TransitionStyle::Punch => "Overshoot the zoom a little, then settle",
        }
    }

    /// Stable id for the CLI and MCP.
    pub fn id(self) -> &'static str {
        match self {
            TransitionStyle::Smooth => "smooth",
            TransitionStyle::Direct => "direct",
            TransitionStyle::Cut => "cut",
            TransitionStyle::Swoop => "swoop",
            TransitionStyle::Spin => "spin",
            TransitionStyle::Fade => "fade",
            TransitionStyle::Punch => "punch",
        }
    }

    pub fn from_id(s: &str) -> Option<TransitionStyle> {
        let want = s.trim().to_lowercase();
        Self::ALL.into_iter().find(|t| t.id() == want || t.label().to_lowercase() == want)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transition {
    pub style: TransitionStyle,
    /// Seconds.
    pub duration: f64,
    pub easing: Easing,
}

impl Default for Transition {
    fn default() -> Self {
        Transition { style: TransitionStyle::Smooth, duration: 1.4, easing: Easing::EaseInOutCubic }
    }
}

/// One stop on the presentation path. Steps reference frames by id; a frame
/// may appear several times (e.g. returning to an overview).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub frame: String,
    #[serde(default)]
    pub transition: Transition,
    /// Speaker notes (Markdown).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// In-slide animations, in play order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub builds: Vec<crate::build::Build>,
}

impl Step {
    pub fn new(frame: impl Into<String>) -> Self {
        Step { frame: frame.into(), transition: Transition::default(), notes: String::new(), builds: Vec::new() }
    }
}

/// A presentation: an infinite canvas of elements. Element order is z-order
/// (first = bottom).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "format_id")]
    pub format: String,
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_background")]
    pub background: Color,
    /// Optional second background color: a top → bottom gradient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_to: Option<Color>,
    /// The design theme (colors and fonts) the document was styled with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<crate::theme::Theme>,
    #[serde(default)]
    pub elements: Vec<Element>,
    /// The presentation path: the ordered waypoints (slides).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Step>,
    /// Font files that travel with the document (relative paths into its
    /// assets folder), for typefaces that may not be installed elsewhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<String>,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            format: format_id(),
            title: String::new(),
            background: default_background(),
            background_to: None,
            theme: None,
            elements: Vec::new(),
            path: Vec::new(),
            fonts: Vec::new(),
        }
    }
}

impl Document {
    pub fn new(title: impl Into<String>) -> Self {
        Document { title: title.into(), ..Default::default() }
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("document serialization is infallible")
    }

    pub fn load(path: &Path) -> Result<Self, DocumentError> {
        let display = path.display().to_string();
        let text = std::fs::read_to_string(path)
            .map_err(|source| DocumentError::Io { path: display.clone(), source })?;
        let mut doc = Self::from_json(&text).map_err(|source| DocumentError::Parse { path: display, source })?;
        // Documents from before the GlideDeck rename are format-identical.
        if doc.format == LEGACY_FORMAT_ID {
            doc.format = FORMAT_ID.to_owned();
        }
        if doc.format != FORMAT_ID {
            return Err(DocumentError::UnsupportedFormat(doc.format));
        }
        Ok(doc)
    }

    /// Writes atomically (temp file + rename) so a crash never leaves a
    /// half-written presentation behind.
    pub fn save(&self, path: &Path) -> Result<(), DocumentError> {
        let display = path.display().to_string();
        let io = |source| DocumentError::Io { path: display.clone(), source };
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = std::path::PathBuf::from(tmp);
        let mut json = self.to_json();
        json.push('\n');
        std::fs::write(&tmp, json).map_err(io)?;
        std::fs::rename(&tmp, path).map_err(io)
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.elements.iter().position(|e| e.id == id)
    }

    pub fn get(&self, id: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Element> {
        self.elements.iter_mut().find(|e| e.id == id)
    }

    /// Generates an id unique within this document, prefixed by the kind.
    pub fn generate_id(&self, prefix: &str) -> String {
        loop {
            let raw = uuid::Uuid::new_v4().simple().to_string();
            let id = format!("{prefix}-{}", &raw[..8]);
            if self.get(&id).is_none() {
                return id;
            }
        }
    }

    /// Adds an element, generating an id if it has none. Returns the id.
    pub fn add(&mut self, mut element: Element) -> String {
        if element.id.is_empty() || self.get(&element.id).is_some() {
            element.id = self.generate_id(element.kind.type_name());
        }
        let id = element.id.clone();
        self.elements.push(element);
        id
    }

    /// Removes an element; removing a frame also drops its path steps.
    pub fn remove(&mut self, id: &str) -> Option<Element> {
        let el = self.index_of(id).map(|i| self.elements.remove(i))?;
        self.path.retain(|s| s.frame != id);
        Some(el)
    }

    /// The frame element a step points at.
    pub fn step_frame(&self, index: usize) -> Option<&Element> {
        self.path.get(index).and_then(|s| self.get(&s.frame)).filter(|e| e.kind.is_frame())
    }

    /// Paths of all media files referenced by the document.
    pub fn media_paths(&self) -> Vec<String> {
        let mut v: Vec<String> = self.elements.iter().filter_map(|e| e.kind.media_path().map(str::to_owned)).collect();
        v.sort();
        v.dedup();
        v
    }
}

fn format_id() -> String {
    FORMAT_ID.to_owned()
}

fn default_background() -> Color {
    Color::rgb(0xf7, 0xf6, 0xf2)
}
