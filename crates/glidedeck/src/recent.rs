//! The File → Open recent list: the last documents opened, saved or imported,
//! kept in the user's settings folder (never in the document itself).

use std::path::{Path, PathBuf};

const MAX: usize = 10;

#[derive(Debug, Default)]
pub struct RecentFiles {
    pub paths: Vec<PathBuf>,
    /// Where the list is stored; `None` keeps it in memory only (tests).
    store: Option<PathBuf>,
}

/// The per-user settings folder for GlideDeck.
pub(crate) fn settings_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("GlideDeck"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/GlideDeck"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|d| d.join("glidedeck"))
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        // Case-insensitive file systems by default.
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

impl RecentFiles {
    /// Loads the user's list (empty and in memory only under tests).
    pub fn load() -> Self {
        if cfg!(test) {
            return RecentFiles::default();
        }
        let store = settings_dir().map(|d| d.join("recent.json"));
        let paths = store
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
            .unwrap_or_default();
        RecentFiles { paths, store }
    }

    /// A list stored in a specific file (for tests).
    #[cfg(test)]
    pub fn at(store: PathBuf) -> Self {
        let paths = std::fs::read_to_string(&store).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        RecentFiles { paths, store: Some(store) }
    }

    /// Moves `path` to the top of the list.
    pub fn add(&mut self, path: &Path) {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.paths.retain(|p| !same_file(p, &path));
        self.paths.insert(0, path);
        self.paths.truncate(MAX);
        self.save();
    }

    pub fn remove(&mut self, path: &Path) {
        self.paths.retain(|p| !same_file(p, path));
        self.save();
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.save();
    }

    fn save(&self) {
        let Some(store) = &self.store else { return };
        if let Some(dir) = store.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.paths) {
            let _ = std::fs::write(store, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_latest_ten_most_recent_first() {
        let dir = std::env::temp_dir().join(format!("glidedeck-recent-{}", std::process::id()));
        let store = dir.join("recent.json");
        let mut r = RecentFiles::at(store.clone());
        for i in 0..12 {
            r.add(&dir.join(format!("deck{i}.glide.json")));
        }
        assert_eq!(r.paths.len(), MAX);
        assert!(r.paths[0].ends_with("deck11.glide.json"));
        // Re-opening moves a file to the top instead of duplicating it.
        r.add(&dir.join("deck5.glide.json"));
        assert!(r.paths[0].ends_with("deck5.glide.json"));
        assert_eq!(r.paths.iter().filter(|p| p.ends_with("deck5.glide.json")).count(), 1);
        // It is saved and reloads.
        assert_eq!(RecentFiles::at(store.clone()).paths, r.paths);
        r.remove(&dir.join("deck5.glide.json"));
        assert!(!r.paths.iter().any(|p| p.ends_with("deck5.glide.json")));
        r.clear();
        assert!(RecentFiles::at(store).paths.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
