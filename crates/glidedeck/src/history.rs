use glide_core::Document;

const MAX_UNDO: usize = 200;

/// Snapshot-based undo history.
///
/// Rather than instrumenting every edit, the app reports the document once
/// per frame; any difference from the last committed snapshot becomes an
/// undo step once the user is no longer mid-gesture. This captures canvas
/// drags, inspector edits and scripted ops uniformly.
pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
    committed: Document,
    revision: u64,
}

impl History {
    pub fn new(doc: &Document) -> Self {
        History { undo: Vec::new(), redo: Vec::new(), committed: doc.clone(), revision: 0 }
    }

    pub fn reset(&mut self, doc: &Document) {
        let revision = self.revision + 1;
        *self = History::new(doc);
        self.revision = revision;
    }

    /// Increments whenever the committed document changes; consumers such as
    /// slide thumbnails use it to know when to refresh.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Commits `doc` as an undo step if it changed and no gesture is in progress.
    pub fn track(&mut self, doc: &Document, busy: bool) {
        if !busy && *doc != self.committed {
            self.undo.push(std::mem::replace(&mut self.committed, doc.clone()));
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.revision += 1;
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        self.track(doc, false);
        let Some(prev) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.committed, prev.clone()));
        *doc = prev;
        self.revision += 1;
        true
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        self.track(doc, false);
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.committed, next.clone()));
        *doc = next;
        self.revision += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut doc = Document::new("a");
        let mut h = History::new(&doc);
        doc.title = "b".into();
        h.track(&doc, true); // mid-gesture: not committed yet
        assert!(!h.can_undo());
        h.track(&doc, false);
        doc.title = "c".into();
        assert!(h.undo(&mut doc)); // commits "c", returns to "b"
        assert_eq!(doc.title, "b");
        assert!(h.undo(&mut doc));
        assert_eq!(doc.title, "a");
        assert!(h.redo(&mut doc));
        assert!(h.redo(&mut doc));
        assert_eq!(doc.title, "c");
    }
}
