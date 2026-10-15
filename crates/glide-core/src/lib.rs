//! Core of GlideDeck: the human-readable document model, camera math and the
//! JSON operation set shared by the GUI, the CLI and (later) the MCP server.
//!
//! This crate is deliberately free of GUI and GPU dependencies so that agents
//! and scripts can manipulate presentations headlessly.

pub mod assets;
pub mod build;
pub mod camera;
pub mod color;
pub mod document;
pub mod easing;
pub mod element;
pub mod geom;
pub mod ops;
pub mod theme;

pub use build::{Build, BuildKind, Direction, Effect, Trigger};
pub use camera::{Camera, CameraTween};
pub use color::Color;
pub use document::{Document, DocumentError, Step, Transition, TransitionStyle, FORMAT_ID};
pub use easing::Easing;
pub use element::{
    SpanStyle, TextSpan,
    Element, ElementKind, EllipseShape, Frame, Gradient, Icon, IconStyle, ListStyle, Media, RectShape, Shadow, Stroke, TextAlign,
    TextBlock, TextRole, Transform,
};
pub use theme::Theme;
pub use geom::Aabb;
pub use ops::{Op, OpError, OpOutcome};

pub use glam::{DAffine2, DVec2};
