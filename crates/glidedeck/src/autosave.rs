//! AutoSave and crash recovery.
//!
//! - **AutoSave**: a presentation that has a file is saved a couple of seconds
//!   after each change (File → AutoSave turns it off).
//! - **Recovery copies**: while there are unsaved changes, including in a
//!   presentation that was never saved, a copy is kept in the settings folder
//!   (`recovery/`), with a heartbeat. Saving or closing normally removes it.
//!   Copies left behind by a session that stopped without closing (a crash, a
//!   power cut) are offered on the next start.

use crate::app::GlideDeckApp;
use eframe::egui;
use glide_core::Document;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Seconds after the last change before AutoSave writes the file.
const SAVE_DELAY: f64 = 2.0;
/// Seconds after the last change before the recovery copy is refreshed.
const SNAPSHOT_DELAY: f64 = 5.0;
/// How often a session with a recovery copy says it is still running.
const HEARTBEAT: Duration = Duration::from_secs(30);
/// A copy whose session has been silent this long was left behind.
const STALE_AFTER: u64 = 120;

/// What a recovery copy belongs to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// The presentation's file, if it had one.
    pub original: Option<PathBuf>,
    /// Folder its media references resolve against.
    pub base_dir: PathBuf,
    pub title: String,
    /// When the copy was written (seconds since 1970).
    pub saved: u64,
    /// When its session last said it was running.
    pub heartbeat: u64,
    pub pid: u32,
}

/// A recovery copy left behind by an earlier session.
#[derive(Clone, Debug)]
pub struct Recovery {
    pub doc_file: PathBuf,
    pub meta_file: PathBuf,
    pub meta: Meta,
}

pub struct AutoSave {
    /// Save presentations that have a file a moment after each change.
    pub enabled: bool,
    /// The settings folder; `None` keeps no preferences or copies (tests).
    dir: Option<PathBuf>,
    /// File stem of this session's recovery copy.
    session: String,
    revision: u64,
    changed_at: f64,
    was_dirty: bool,
    saved_revision: Option<u64>,
    snapshot_revision: Option<u64>,
    snapshot_meta: Option<Meta>,
    heartbeat_at: f64,
    failed: bool,
    /// Copies from earlier sessions, offered in the Recover dialog.
    pub found: Vec<Recovery>,
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize, Deserialize, Default)]
struct Prefs {
    #[serde(default)]
    autosave_off: bool,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// The two files of a recovery copy.
fn files(dir: &Path, session: &str) -> (PathBuf, PathBuf) {
    let r = dir.join("recovery");
    (r.join(format!("{session}.glide.json")), r.join(format!("{session}.meta.json")))
}

/// Writes a recovery copy of `doc`.
pub fn write_copy(dir: &Path, session: &str, doc: &Document, meta: &Meta) -> std::io::Result<()> {
    let (doc_file, meta_file) = files(dir, session);
    std::fs::create_dir_all(doc_file.parent().expect("recovery folder"))?;
    let json = serde_json::to_vec(doc).map_err(std::io::Error::other)?;
    write_atomic(&doc_file, &json)?;
    write_atomic(&meta_file, &serde_json::to_vec_pretty(meta).map_err(std::io::Error::other)?)
}

/// Recovery copies whose session stopped without closing, newest first.
pub fn find_left_behind(dir: &Path, own_pid: u32, now: u64) -> Vec<Recovery> {
    let Ok(entries) = std::fs::read_dir(dir.join("recovery")) else { return Vec::new() };
    let mut found: Vec<Recovery> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".meta.json"))
        .filter_map(|meta_file| {
            let meta: Meta = serde_json::from_slice(&std::fs::read(&meta_file).ok()?).ok()?;
            let stem = meta_file.file_name()?.to_string_lossy().trim_end_matches(".meta.json").to_owned();
            let doc_file = meta_file.with_file_name(format!("{stem}.glide.json"));
            let left = meta.pid != own_pid && now.saturating_sub(meta.heartbeat) > STALE_AFTER;
            (left && doc_file.exists()).then_some(Recovery { doc_file, meta_file, meta })
        })
        .collect();
    found.sort_by_key(|r| std::cmp::Reverse(r.meta.saved));
    found
}

/// "3 minutes ago", "yesterday"…
pub fn ago(then: u64, now: u64) -> String {
    let s = now.saturating_sub(then);
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        86_400..=172_799 => "yesterday".into(),
        _ => format!("{} days ago", s / 86_400),
    }
}

impl AutoSave {
    /// AutoSave with the user's preference and any copies left behind.
    pub fn load() -> Self {
        if cfg!(test) {
            return AutoSave::new(None);
        }
        AutoSave::new(crate::recent::settings_dir())
    }

    /// AutoSave keeping its preference and copies in `dir` (`None`: nowhere).
    pub fn new(dir: Option<PathBuf>) -> Self {
        let prefs: Prefs = dir
            .as_ref()
            .and_then(|d| std::fs::read(d.join("settings.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let pid = std::process::id();
        let found = dir.as_deref().map(|d| find_left_behind(d, pid, now_unix())).unwrap_or_default();
        AutoSave {
            enabled: !prefs.autosave_off,
            session: format!("{pid}-{}", now_unix()),
            dir,
            revision: 0,
            changed_at: 0.0,
            was_dirty: false,
            saved_revision: None,
            snapshot_revision: None,
            snapshot_meta: None,
            heartbeat_at: 0.0,
            failed: false,
            found,
        }
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.failed = false;
        if let Some(d) = &self.dir {
            let _ = std::fs::create_dir_all(d);
            let _ = write_atomic(&d.join("settings.json"), &serde_json::to_vec_pretty(&Prefs { autosave_off: !on }).unwrap_or_default());
        }
    }

    /// Removes this session's recovery copy (the work was saved or discarded).
    pub fn discard_copy(&mut self) {
        if let (Some(d), Some(_)) = (&self.dir, self.snapshot_meta.take()) {
            let (doc_file, meta_file) = files(d, &self.session);
            let _ = std::fs::remove_file(doc_file);
            let _ = std::fs::remove_file(meta_file);
        }
        self.snapshot_revision = None;
    }

    /// Removes a copy left behind by an earlier session.
    pub fn forget(&mut self, index: usize) {
        if index < self.found.len() {
            let r = self.found.remove(index);
            let _ = std::fs::remove_file(r.doc_file);
            let _ = std::fs::remove_file(r.meta_file);
        }
    }
}

impl GlideDeckApp {
    /// Runs once per frame: saves after changes settle and keeps the recovery
    /// copy current.
    pub fn autosave_tick(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let rev = self.history.revision();
        let dirty = self.is_dirty();
        let a = &mut self.autosave;
        // The wait restarts with every committed change, and when unsaved
        // changes first appear.
        if rev != a.revision || (dirty && !a.was_dirty) {
            a.revision = rev;
            a.changed_at = now;
        }
        a.was_dirty = dirty;
        let busy = ctx.input(|i| i.pointer.any_down()) || self.text_edit.is_some() || ctx.text_edit_focused();
        let settled = now - self.autosave.changed_at;

        if self.autosave.enabled
            && dirty
            && !busy
            && settled >= SAVE_DELAY
            && self.autosave.saved_revision != Some(rev)
            && let Some(p) = self.path.clone()
        {
            self.autosave.saved_revision = Some(rev);
            match self.doc.save(&p) {
                Ok(()) => {
                    self.saved = self.doc.clone();
                    self.autosave.failed = false;
                }
                Err(e) => {
                    if !std::mem::replace(&mut self.autosave.failed, true) {
                        self.notify(ctx, format!("AutoSave couldn't save {}: {e}", p.display()));
                    }
                }
            }
        }

        let dirty = self.is_dirty();
        let Some(dir) = self.autosave.dir.clone() else { return };
        if !dirty {
            if self.autosave.snapshot_meta.is_some() {
                self.autosave.discard_copy();
            }
            return;
        }
        if settled >= SNAPSHOT_DELAY && self.autosave.snapshot_revision != Some(rev) {
            let unix = now_unix();
            let meta = Meta {
                original: self.path.clone(),
                base_dir: self.base_dir(),
                title: self.display_name(),
                saved: unix,
                heartbeat: unix,
                pid: std::process::id(),
            };
            match write_copy(&dir, &self.autosave.session, &self.doc, &meta) {
                Ok(()) => {
                    self.autosave.snapshot_meta = Some(meta);
                    self.autosave.heartbeat_at = now;
                }
                Err(e) => log::warn!("could not write the recovery copy: {e}"),
            }
            self.autosave.snapshot_revision = Some(rev);
        } else if now - self.autosave.heartbeat_at >= HEARTBEAT.as_secs_f64()
            && let Some(meta) = &mut self.autosave.snapshot_meta
        {
            meta.heartbeat = now_unix();
            let (_, meta_file) = files(&dir, &self.autosave.session);
            let _ = write_atomic(&meta_file, &serde_json::to_vec_pretty(meta).unwrap_or_default());
            self.autosave.heartbeat_at = now;
        }
        // Keep ticking while idle: the heartbeat, and a change waiting to be written.
        let wait = if self.autosave.snapshot_revision != Some(rev) { Duration::from_secs(1) } else { HEARTBEAT };
        ctx.request_repaint_after(wait);
    }

    /// Opens a recovery copy left behind by an earlier session.
    pub fn recover(&mut self, ctx: &egui::Context, index: usize) {
        let Some(r) = self.autosave.found.get(index).cloned() else { return };
        if !self.confirm_discard() {
            return;
        }
        match Document::load(&r.doc_file) {
            Ok(doc) => {
                let original = r.meta.original.clone();
                // Compare against the file on disk, so the recovered changes show as unsaved.
                let on_disk = original.as_ref().and_then(|p| Document::load(p).ok()).unwrap_or_default();
                if original.is_none() {
                    // Never saved: its media are still in that session's folder.
                    self.session_dir = r.meta.base_dir.clone();
                }
                self.replace_document(doc, original);
                self.saved = on_disk;
                self.autosave.forget(index);
                self.notify(ctx, format!("Recovered “{}”. Save it to keep it", r.meta.title));
            }
            Err(e) => self.error_dialog(&format!("Could not open the recovery copy: {e}")),
        }
    }
}

/// The Recover dialog, shown at start when an earlier session left work behind.
pub fn recover_window(app: &mut GlideDeckApp, ctx: &egui::Context) {
    if app.autosave.found.is_empty() || !app.show_recovery {
        return;
    }
    let p = crate::theme::palette(ctx);
    let now = now_unix();
    let mut recover = None;
    let mut forget = None;
    egui::Window::new("Recover unsaved work")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
        .show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label("GlideDeck didn't close normally last time. These presentations had changes that weren't saved:");
            ui.add_space(8.0);
            for (i, r) in app.autosave.found.iter().enumerate() {
                egui::Frame::new().fill(p.surface).corner_radius(8).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&r.meta.title).strong());
                            let place = match &r.meta.original {
                                Some(f) => f.display().to_string(),
                                None => "Never saved".into(),
                            };
                            ui.label(egui::RichText::new(format!("{place} · changed {}", ago(r.meta.saved, now))).size(11.5).color(p.text_muted));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Discard").on_hover_text("Delete this recovery copy").clicked() {
                                forget = Some(i);
                            }
                            if ui.button("Recover").clicked() {
                                recover = Some(i);
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
            ui.add_space(6.0);
            if ui.button("Decide later").on_hover_text("Keep the copies; they're offered again next time").clicked() {
                app.show_recovery = false;
            }
        });
    if let Some(i) = recover {
        app.recover(ctx, i);
    } else if let Some(i) = forget {
        app.autosave.forget(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("glidedeck-autosave-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn meta(pid: u32, heartbeat: u64) -> Meta {
        Meta { original: None, base_dir: PathBuf::from("/tmp/x"), title: "Talk".into(), saved: heartbeat, heartbeat, pid }
    }

    #[test]
    fn copies_left_behind_are_found_and_live_ones_are_not() {
        let dir = temp("find");
        let doc = Document::new("Talk");
        let now = now_unix();
        write_copy(&dir, "crashed", &doc, &meta(1, now - 600)).unwrap();
        write_copy(&dir, "alive", &doc, &meta(2, now - 10)).unwrap();
        write_copy(&dir, "mine", &doc, &meta(std::process::id(), now - 600)).unwrap();
        let found = find_left_behind(&dir, std::process::id(), now);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].meta.pid, 1);
        assert_eq!(Document::load(&found[0].doc_file).unwrap().title, "Talk");

        let mut a = AutoSave::new(Some(dir.clone()));
        assert_eq!(a.found.len(), 1);
        a.forget(0);
        assert!(find_left_behind(&dir, std::process::id(), now).is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_preference_is_remembered() {
        let dir = temp("prefs");
        let mut a = AutoSave::new(Some(dir.clone()));
        assert!(a.enabled);
        a.set_enabled(false);
        assert!(!AutoSave::new(Some(dir.clone())).enabled);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(ago(1000, 1030), "just now");
        assert_eq!(ago(1000, 1000 + 180), "3 min ago");
        assert_eq!(ago(0, 90_000), "yesterday");
    }
}
