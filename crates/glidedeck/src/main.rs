#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod agents;
mod anim;
mod anim_ui;
mod app;
mod arrange;
mod arrange_ui;
mod canvas;
mod cli;
mod demo;
mod displays;
mod design;
mod galleries;
mod history;
mod inspector;
mod layouts;
mod mcp;
mod media;
mod office;
mod pdf;
mod pptx_import;
mod present;
mod recent;
mod ribbon;
mod slides;
mod templates;
mod textedit;
mod textformat;
mod theme;
#[cfg(test)]
mod ui_tests;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let (file, present) = match cli.command {
        None => (cli.file, None),
        Some(cli::Command::Open { file }) => (file, None),
        Some(cli::Command::Present { file, presenter_view, slide }) => {
            (Some(file), Some(app::StartShow { step: slide.saturating_sub(1), presenter: presenter_view.then_some(true) }))
        }
        // The MCP server talks JSON over the pipes its client gives it, so it
        // must not attach to a console.
        Some(cmd @ cli::Command::Mcp { .. }) => return cli::run(cmd),
        Some(cmd) => {
            attach_console();
            return cli::run(cmd);
        }
    };
    run_gui(file, present)
}

/// The app icon, for the main window and the presenter's audience window.
pub fn app_icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon/glidedeck-256.png")).expect("valid icon")
}

fn run_gui(file: Option<std::path::PathBuf>, present: Option<app::StartShow>) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("GlideDeck")
            .with_inner_size([1480.0, 920.0])
            .with_min_inner_size([960.0, 600.0])
            .with_drag_and_drop(true)
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "GlideDeck",
        options,
        Box::new(move |cc| {
            let mut app = app::GlideDeckApp::new(cc, file)?;
            app.start_show = present;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("failed to start the editor: {e}"))
}

/// Release builds on Windows use the GUI subsystem; reattach to the parent
/// console so CLI subcommands can still print.
#[cfg(all(not(debug_assertions), target_os = "windows"))]
fn attach_console() {
    unsafe extern "system" {
        fn AttachConsole(pid: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // SAFETY: plain Win32 call with no pointers involved.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(all(not(debug_assertions), target_os = "windows")))]
fn attach_console() {}
