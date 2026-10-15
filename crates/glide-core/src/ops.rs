//! The JSON control-channel vocabulary.
//!
//! Every programmatic change to a document — from the CLI, the MCP server or a
//! script — is expressed as an [`Op`]. Ops are plain, tagged JSON objects:
//!
//! ```json
//! {"op": "add_element", "element": {"kind": {"type": "text", "content": "Hi"}}}
//! {"op": "set_transform", "id": "text-1a2b3c4d", "x": 100, "scale": 0.5}
//! ```

use crate::color::Color;
use crate::document::{Document, Step, Transition};
use crate::element::{Element, ElementKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Adds an element on top of the z-order. An id is generated when omitted.
    AddElement { element: Element },
    RemoveElement { id: String },
    /// Updates any subset of the transform fields.
    SetTransform {
        id: String,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        scale: Option<f64>,
        #[serde(default)]
        rotation: Option<f64>,
    },
    /// Replaces the element's content (geometry, text, media reference).
    SetKind { id: String, kind: ElementKind },
    SetName { id: String, name: String },
    SetOpacity { id: String, opacity: f64 },
    /// Moves the element to a z-order index (clamped); omit to bring to front.
    Reorder {
        id: String,
        #[serde(default)]
        index: Option<usize>,
    },
    SetTitle { title: String },
    SetBackground { color: Color },
    /// Inserts a waypoint for a frame element (appends when `index` is omitted).
    AddStep {
        frame: String,
        #[serde(default)]
        index: Option<usize>,
        #[serde(default)]
        transition: Option<Transition>,
        #[serde(default)]
        notes: Option<String>,
    },
    RemoveStep { index: usize },
    MoveStep { from: usize, to: usize },
    UpdateStep {
        index: usize,
        #[serde(default)]
        transition: Option<Transition>,
        #[serde(default)]
        notes: Option<String>,
        /// Replaces the step's in-slide animations.
        #[serde(default)]
        builds: Option<Vec<crate::build::Build>>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum OpOutcome {
    Added { id: String },
    Removed { element: Element },
    StepAdded { index: usize },
    Updated,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum OpError {
    #[error("no element with id {0:?}")]
    NotFound(String),
    #[error("invalid value: {0}")]
    Invalid(String),
}

impl Op {
    pub fn apply(self, doc: &mut Document) -> Result<OpOutcome, OpError> {
        fn find<'a>(doc: &'a mut Document, id: &str) -> Result<&'a mut Element, OpError> {
            doc.get_mut(id).ok_or_else(|| OpError::NotFound(id.to_owned()))
        }
        fn finite(name: &str, v: Option<f64>) -> Result<(), OpError> {
            match v {
                Some(v) if !v.is_finite() => Err(OpError::Invalid(format!("{name} must be finite"))),
                _ => Ok(()),
            }
        }

        match self {
            Op::AddElement { element } => {
                let t = &element.transform;
                for (n, v) in [("x", t.x), ("y", t.y), ("scale", t.scale), ("rotation", t.rotation)] {
                    finite(n, Some(v))?;
                }
                Ok(OpOutcome::Added { id: doc.add(element) })
            }
            Op::RemoveElement { id } => doc
                .remove(&id)
                .map(|element| OpOutcome::Removed { element })
                .ok_or(OpError::NotFound(id)),
            Op::SetTransform { id, x, y, scale, rotation } => {
                for (n, v) in [("x", x), ("y", y), ("scale", scale), ("rotation", rotation)] {
                    finite(n, v)?;
                }
                if scale == Some(0.0) {
                    return Err(OpError::Invalid("scale must be non-zero".into()));
                }
                let t = &mut find(doc, &id)?.transform;
                if let Some(v) = x {
                    t.x = v;
                }
                if let Some(v) = y {
                    t.y = v;
                }
                if let Some(v) = scale {
                    t.scale = v;
                }
                if let Some(v) = rotation {
                    t.rotation = v;
                }
                Ok(OpOutcome::Updated)
            }
            Op::SetKind { id, kind } => {
                find(doc, &id)?.kind = kind;
                Ok(OpOutcome::Updated)
            }
            Op::SetName { id, name } => {
                find(doc, &id)?.name = name;
                Ok(OpOutcome::Updated)
            }
            Op::SetOpacity { id, opacity } => {
                if !(0.0..=1.0).contains(&opacity) {
                    return Err(OpError::Invalid("opacity must be within [0, 1]".into()));
                }
                find(doc, &id)?.opacity = opacity;
                Ok(OpOutcome::Updated)
            }
            Op::Reorder { id, index } => {
                let from = doc.index_of(&id).ok_or_else(|| OpError::NotFound(id.clone()))?;
                let el = doc.elements.remove(from);
                let to = index.unwrap_or(doc.elements.len()).min(doc.elements.len());
                doc.elements.insert(to, el);
                Ok(OpOutcome::Updated)
            }
            Op::SetTitle { title } => {
                doc.title = title;
                Ok(OpOutcome::Updated)
            }
            Op::SetBackground { color } => {
                doc.background = color;
                Ok(OpOutcome::Updated)
            }
            Op::AddStep { frame, index, transition, notes } => {
                match doc.get(&frame) {
                    None => return Err(OpError::NotFound(frame)),
                    Some(e) if !e.kind.is_frame() => {
                        return Err(OpError::Invalid(format!("{frame:?} is a {}, not a frame", e.kind.type_name())));
                    }
                    Some(_) => {}
                }
                let at = index.unwrap_or(doc.path.len()).min(doc.path.len());
                let mut step = Step::new(frame);
                step.transition = transition.unwrap_or_default();
                step.notes = notes.unwrap_or_default();
                doc.path.insert(at, step);
                Ok(OpOutcome::StepAdded { index: at })
            }
            Op::RemoveStep { index } => {
                step_index(doc, index)?;
                doc.path.remove(index);
                Ok(OpOutcome::Updated)
            }
            Op::MoveStep { from, to } => {
                step_index(doc, from)?;
                let step = doc.path.remove(from);
                let to = to.min(doc.path.len());
                doc.path.insert(to, step);
                Ok(OpOutcome::Updated)
            }
            Op::UpdateStep { index, transition, notes, builds } => {
                step_index(doc, index)?;
                let step = &mut doc.path[index];
                if let Some(t) = transition {
                    step.transition = t;
                }
                if let Some(n) = notes {
                    step.notes = n;
                }
                if let Some(b) = builds {
                    step.builds = b;
                }
                Ok(OpOutcome::Updated)
            }
        }
    }
}

fn step_index(doc: &Document, index: usize) -> Result<(), OpError> {
    if index < doc.path.len() {
        Ok(())
    } else {
        Err(OpError::Invalid(format!("step {index} out of range (path has {} steps)", doc.path.len())))
    }
}

/// Applies a batch atomically: either every op succeeds or the document is
/// left untouched.
pub fn apply_all(doc: &mut Document, ops: Vec<Op>) -> Result<Vec<OpOutcome>, (usize, OpError)> {
    let mut scratch = doc.clone();
    let mut out = Vec::with_capacity(ops.len());
    for (i, op) in ops.into_iter().enumerate() {
        out.push(op.apply(&mut scratch).map_err(|e| (i, e))?);
    }
    *doc = scratch;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_is_atomic() {
        let mut doc = Document::new("t");
        let ops: Vec<Op> = serde_json::from_str(
            r##"[
              {"op":"add_element","element":{"id":"a","kind":{"type":"rect","width":10,"height":10}}},
              {"op":"set_transform","id":"a","x":5,"rotation":45},
              {"op":"set_name","id":"missing","name":"x"}
            ]"##,
        )
        .unwrap();
        let err = apply_all(&mut doc, ops).unwrap_err();
        assert_eq!(err.0, 2);
        assert!(doc.elements.is_empty());

        let ops: Vec<Op> = serde_json::from_str(
            r##"[
              {"op":"add_element","element":{"id":"a","kind":{"type":"rect","width":10,"height":10}}},
              {"op":"set_transform","id":"a","x":5,"rotation":45}
            ]"##,
        )
        .unwrap();
        apply_all(&mut doc, ops).unwrap();
        assert_eq!(doc.elements[0].transform.x, 5.0);
        assert_eq!(doc.elements[0].transform.rotation, 45.0);
    }

    #[test]
    fn path_ops() {
        let mut doc = Document::new("t");
        let ops: Vec<Op> = serde_json::from_str(
            r##"[
              {"op":"add_element","element":{"id":"f1","kind":{"type":"frame","width":160,"height":90}}},
              {"op":"add_element","element":{"id":"f2","kind":{"type":"frame","width":160,"height":90}}},
              {"op":"add_step","frame":"f1"},
              {"op":"add_step","frame":"f2","transition":{"style":"direct","duration":0.5}},
              {"op":"add_step","frame":"f1","notes":"back to start"},
              {"op":"move_step","from":2,"to":0}
            ]"##,
        )
        .unwrap();
        apply_all(&mut doc, ops).unwrap();
        let frames: Vec<_> = doc.path.iter().map(|s| s.frame.as_str()).collect();
        assert_eq!(frames, ["f1", "f1", "f2"]);
        assert_eq!(doc.path[0].notes, "back to start");
        assert_eq!(doc.path[2].transition.duration, 0.5);
        // Steps can only target frames; removing a frame drops its steps.
        let bad: Vec<Op> = serde_json::from_str(
            r##"[{"op":"add_element","element":{"id":"r","kind":{"type":"rect","width":1,"height":1}}},{"op":"add_step","frame":"r"}]"##,
        )
        .unwrap();
        assert!(apply_all(&mut doc, bad).is_err());
        Op::RemoveElement { id: "f1".into() }.apply(&mut doc).unwrap();
        assert_eq!(doc.path.len(), 1);
    }
}
