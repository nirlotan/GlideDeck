//! "About GlideDeck" and "Check for updates".
//!
//! The update check asks GitHub for the latest release (the only time
//! GlideDeck goes online, and only when you click), compares versions, and
//! downloads the installer for this platform (the .msi on Windows, the .dmg
//! on macOS) to the Downloads folder. Requests go through the system's
//! `curl`, which ships with Windows 10+ and macOS.

use crate::app::GlideDeckApp;
use crate::theme::{self, icon};
use eframe::egui::{self, RichText};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const REPO_URL: &str = "https://github.com/nirlotan/GlideDeck";
const LATEST_API: &str = "https://api.github.com/repos/nirlotan/GlideDeck/releases/latest";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate { latest: String },
    Available { version: String, notes_url: String, asset: Option<(String, String)> },
    Downloading { version: String, name: String },
    Downloaded { version: String, path: PathBuf },
    Failed(String),
}

/// The About / update window and its background work.
pub struct Updater {
    pub open: bool,
    state: Arc<Mutex<UpdateState>>,
}

impl Default for Updater {
    fn default() -> Self {
        Updater { open: false, state: Arc::new(Mutex::new(UpdateState::Idle)) }
    }
}

impl Updater {
    pub fn state(&self) -> UpdateState {
        self.state.lock().map(|s| s.clone()).unwrap_or(UpdateState::Idle)
    }

    fn set(state: &Arc<Mutex<UpdateState>>, s: UpdateState) {
        if let Ok(mut g) = state.lock() {
            *g = s;
        }
    }

    /// Asks GitHub for the latest release, in the background.
    pub fn check(&self, ctx: &egui::Context) {
        let state = self.state.clone();
        Self::set(&state, UpdateState::Checking);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = curl(&["-sSfL", "--max-time", "20", "-H", "Accept: application/vnd.github+json", LATEST_API]).and_then(|body| {
                let v: Value = serde_json::from_slice(&body).map_err(|e| format!("unexpected reply from GitHub: {e}"))?;
                Ok(evaluate(&v, VERSION))
            });
            Self::set(&state, result.unwrap_or_else(UpdateState::Failed));
            ctx.request_repaint();
        });
    }

    /// Downloads the installer of an available update to the Downloads folder.
    pub fn download(&self, ctx: &egui::Context) {
        let UpdateState::Available { version, asset: Some((name, url)), .. } = self.state() else { return };
        let state = self.state.clone();
        Self::set(&state, UpdateState::Downloading { version: version.clone(), name: name.clone() });
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let dir = downloads_dir();
            let path = dir.join(&name);
            let part = dir.join(format!("{name}.part"));
            let out = part.to_string_lossy().into_owned();
            let result = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|_| curl(&["-sSfL", "--max-time", "900", "-o", &out, &url]))
                .and_then(|_| std::fs::rename(&part, &path).map_err(|e| e.to_string()));
            match result {
                Ok(()) => Self::set(&state, UpdateState::Downloaded { version, path }),
                Err(e) => {
                    let _ = std::fs::remove_file(&part);
                    Self::set(&state, UpdateState::Failed(format!("download failed: {e}")));
                }
            }
            ctx.request_repaint();
        });
    }
}

/// Runs `curl` and returns its output (without flashing a console window on Windows).
fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(args).arg("-A").arg(format!("GlideDeck/{VERSION}"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| format!("could not run curl: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        Err(if err.is_empty() { "no connection to GitHub".into() } else { err })
    }
}

fn downloads_dir() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    match home {
        Some(h) if h.join("Downloads").is_dir() => h.join("Downloads"),
        Some(h) => h,
        None => std::env::temp_dir(),
    }
}

/// "v0.1.6" or "0.1.6" → (0, 1, 6).
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

/// The installer for this platform among a release's assets: (name, url).
pub fn pick_asset(assets: &[(String, String)], os: &str, arch: &str) -> Option<(String, String)> {
    let lower = |s: &str| s.to_lowercase();
    match os {
        "windows" => {
            let msi: Vec<&(String, String)> = assets.iter().filter(|(n, _)| lower(n).ends_with(".msi")).collect();
            // Prefer the versioned file over the stable-name copy.
            msi.iter().find(|(n, _)| lower(n) != "glidedeck-x64.msi").or(msi.first()).map(|a| (*a).clone())
        }
        "macos" => {
            let dmg: Vec<&(String, String)> = assets.iter().filter(|(n, _)| lower(n).ends_with(".dmg")).collect();
            let wants: &[&str] = if arch == "aarch64" { &["arm64", "aarch64", "apple-silicon", "applesilicon"] } else { &["x86_64", "x64", "intel"] };
            let others: &[&str] = if arch == "aarch64" { &["x86_64", "x64", "intel"] } else { &["arm64", "aarch64", "apple-silicon"] };
            dmg.iter()
                .find(|(n, _)| wants.iter().any(|w| lower(n).contains(w)))
                .or_else(|| dmg.iter().find(|(n, _)| !others.iter().any(|w| lower(n).contains(w))))
                .map(|a| (*a).clone())
        }
        _ => None,
    }
}

/// What a GitHub "latest release" reply means for version `current`.
pub fn evaluate(release: &Value, current: &str) -> UpdateState {
    let tag = release["tag_name"].as_str().unwrap_or_default().to_owned();
    let latest = tag.trim_start_matches(['v', 'V']).to_owned();
    let (Some(new), Some(now)) = (parse_version(&tag), parse_version(current)) else {
        return UpdateState::Failed("GitHub didn't report a release version".into());
    };
    if new <= now {
        return UpdateState::UpToDate { latest };
    }
    let assets: Vec<(String, String)> = release["assets"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| Some((x["name"].as_str()?.to_owned(), x["browser_download_url"].as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    let notes_url = release["html_url"].as_str().map(str::to_owned).unwrap_or_else(|| format!("{REPO_URL}/releases/latest"));
    UpdateState::Available { version: latest, notes_url, asset: pick_asset(&assets, std::env::consts::OS, std::env::consts::ARCH) }
}

/// Opens a URL or file with the system's default handler.
pub fn open_external(target: &str) {
    #[cfg(windows)]
    let result = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd").args(["/C", "start", "", target]).creation_flags(0x0800_0000).spawn()
    };
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(target).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(target).spawn();
    if let Err(e) = result {
        log::warn!("could not open {target}: {e}");
    }
}

/// Shows a file in Explorer / Finder.
fn reveal(path: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).creation_flags(0x0800_0000).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    if let Some(dir) = path.parent() {
        open_external(&dir.to_string_lossy());
    }
}

/// The About window, with the update check.
pub fn about_window(app: &mut GlideDeckApp, ctx: &egui::Context) {
    if !app.updater.open {
        return;
    }
    let p = theme::palette(ctx);
    let mut open = true;
    let state = app.updater.state();
    if matches!(state, UpdateState::Checking | UpdateState::Downloading { .. }) {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
    let mut check = false;
    let mut download = false;
    let mut install: Option<PathBuf> = None;
    egui::Window::new("About GlideDeck")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(icon::SPARKLE).size(40.0).color(p.accent));
                ui.label(RichText::new("GlideDeck").font(theme::semibold(24.0)));
                ui.label(RichText::new(format!("Version {VERSION}")).color(p.text_muted));
                ui.add_space(6.0);
                ui.label("Zooming presentations on an infinite canvas.\nNative, local-first and free.");
                ui.add_space(8.0);
                if ui.button(format!("{}  GlideDeck on GitHub", icon::GITHUB_LOGO)).on_hover_text(REPO_URL).clicked() {
                    open_external(REPO_URL);
                }
                ui.add_space(10.0);
            });
            ui.separator();
            ui.add_space(6.0);
            match &state {
                UpdateState::Idle => {
                    check = ui.button(format!("{}  Check for updates", icon::ARROWS_CLOCKWISE)).clicked();
                }
                UpdateState::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Checking GitHub for a newer version…");
                    });
                }
                UpdateState::UpToDate { latest } => {
                    ui.label(RichText::new(format!("{}  You're up to date (latest: {latest}).", icon::CHECK_CIRCLE)).color(p.accent));
                    check = ui.small_button("Check again").clicked();
                }
                UpdateState::Available { version, notes_url, asset } => {
                    ui.label(RichText::new(format!("{}  GlideDeck {version} is available.", icon::GIFT)).font(theme::semibold(14.0)));
                    ui.horizontal(|ui| {
                        match asset {
                            Some((name, _)) => {
                                download = ui
                                    .add(egui::Button::new(RichText::new(format!("{}  Download {name}", icon::DOWNLOAD_SIMPLE)).color(p.on_accent)).fill(p.accent))
                                    .clicked();
                            }
                            None => {
                                ui.label(RichText::new("No installer for this system in the release.").color(p.text_muted));
                            }
                        }
                        if ui.button("What's new").clicked() {
                            open_external(notes_url);
                        }
                    });
                }
                UpdateState::Downloading { version, name } => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Downloading {name} (GlideDeck {version})…"));
                    });
                }
                UpdateState::Downloaded { version, path } => {
                    ui.label(RichText::new(format!("{}  GlideDeck {version} is in {}.", icon::CHECK_CIRCLE, path.parent().map(|d| d.display().to_string()).unwrap_or_default())).color(p.accent));
                    ui.horizontal(|ui| {
                        let label = if cfg!(target_os = "macos") { "Open the installer" } else { "Install now" };
                        if ui.add(egui::Button::new(RichText::new(format!("{}  {label}", icon::ROCKET_LAUNCH)).color(p.on_accent)).fill(p.accent)).clicked() {
                            install = Some(path.clone());
                        }
                        if ui.button("Show in folder").clicked() {
                            reveal(path);
                        }
                    });
                    ui.label(
                        RichText::new(if cfg!(target_os = "macos") {
                            "Drag GlideDeck to Applications to replace this version, then reopen it."
                        } else {
                            "The installer upgrades this version in place. Save your work first: GlideDeck closes when it starts."
                        })
                        .size(11.5)
                        .color(p.text_muted),
                    );
                }
                UpdateState::Failed(e) => {
                    ui.label(RichText::new(format!("{}  Couldn't check for updates: {e}", icon::WARNING)).color(p.danger));
                    ui.horizontal(|ui| {
                        check = ui.button("Try again").clicked();
                        if ui.button("Open the Releases page").clicked() {
                            open_external(&format!("{REPO_URL}/releases/latest"));
                        }
                    });
                }
            }
            ui.add_space(4.0);
            ui.label(RichText::new("GlideDeck only goes online when you check for updates.").size(11.0).color(p.text_muted));
        });
    if check {
        app.updater.check(ctx);
    }
    if download {
        app.updater.download(ctx);
    }
    if let Some(path) = install {
        open_external(&path.to_string_lossy());
        // On Windows the installer replaces the running app: close (asking to save first).
        if cfg!(windows) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    if !open {
        app.updater.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn versions_compare_and_assets_match_the_platform() {
        assert_eq!(parse_version("v0.1.6"), Some((0, 1, 6)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("v2.0.0-beta.1"), Some((2, 0, 0)));
        assert!(parse_version("v0.1.10") > parse_version("0.1.9"));
        let assets = |names: &[&str]| names.iter().map(|n| (n.to_string(), format!("https://x/{n}"))).collect::<Vec<_>>();
        let a = assets(&["GlideDeck-0.2.0-x64.msi", "GlideDeck-x64.msi", "GlideDeck-0.2.0-x64.msi.sha256", "GlideDeck-0.2.0.dmg"]);
        assert_eq!(pick_asset(&a, "windows", "x86_64").unwrap().0, "GlideDeck-0.2.0-x64.msi");
        assert_eq!(pick_asset(&a, "macos", "aarch64").unwrap().0, "GlideDeck-0.2.0.dmg");
        let b = assets(&["GlideDeck-0.2.0-arm64.dmg", "GlideDeck-0.2.0-x86_64.dmg"]);
        assert_eq!(pick_asset(&b, "macos", "x86_64").unwrap().0, "GlideDeck-0.2.0-x86_64.dmg");
        assert_eq!(pick_asset(&b, "macos", "aarch64").unwrap().0, "GlideDeck-0.2.0-arm64.dmg");
        assert_eq!(pick_asset(&a, "linux", "x86_64"), None);
    }

    #[test]
    fn a_newer_release_is_offered_with_its_installer() {
        let release = json!({
            "tag_name": "v9.0.0",
            "html_url": "https://github.com/nirlotan/GlideDeck/releases/tag/v9.0.0",
            "assets": [{ "name": "GlideDeck-9.0.0-x64.msi", "browser_download_url": "https://dl/GlideDeck-9.0.0-x64.msi" },
                       { "name": "GlideDeck-9.0.0.dmg", "browser_download_url": "https://dl/GlideDeck-9.0.0.dmg" }],
        });
        match evaluate(&release, "0.1.6") {
            UpdateState::Available { version, asset, .. } => {
                assert_eq!(version, "9.0.0");
                if cfg!(any(windows, target_os = "macos")) {
                    assert!(asset.is_some());
                }
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(evaluate(&json!({ "tag_name": "v0.1.6" }), "0.1.6"), UpdateState::UpToDate { latest: "0.1.6".into() });
        assert!(matches!(evaluate(&json!({}), "0.1.6"), UpdateState::Failed(_)));
    }
}
