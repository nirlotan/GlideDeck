//! Headless command line interface. Every command reads and writes the same
//! human-readable JSON document the GUI uses, so scripts and agents can build
//! presentations without opening a window.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::io::Read;
use std::path::{Path, PathBuf};
use glide_core::ops::apply_all;
use glam::DVec2;
use glide_core::{Camera, Document, Op};
use glide_render::HeadlessRenderer;

#[derive(Parser, Debug)]
#[command(name = "glidedeck", version, about = "Local-first zooming presentations", args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Document to open in the editor.
    pub file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Open a document (or the welcome canvas) in the editor.
    Open { file: Option<PathBuf> },
    /// Create a new document.
    New {
        file: PathBuf,
        #[arg(long, default_value = "Untitled")]
        title: String,
        /// Start from the welcome canvas instead of an empty one.
        #[arg(long)]
        demo: bool,
        /// Start from a template: pitch-deck, roadmap, showcase, branches, journey, summit,
        /// spiral, deep-dive, big-picture, hive, orbit, mood-board, lecture, chapters,
        /// workshop, cycle or blank.
        #[arg(long)]
        template: Option<String>,
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Apply a JSON array of operations (or a single operation) to a document.
    ///
    /// The batch is atomic: if any operation fails, the file is not modified.
    /// Results are printed to stdout as JSON.
    Apply {
        file: PathBuf,
        /// File containing the operations; `-` or omitted reads stdin.
        ops: Option<PathBuf>,
        /// Create the document if it does not exist.
        #[arg(long)]
        create: bool,
    },
    /// Print a JSON summary of a document's elements.
    Show { file: PathBuf },
    /// Export the presentation path to PowerPoint (one slide per step, Morph transitions).
    ExportPptx {
        file: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Run the MCP server on stdio, so AI assistants (Claude, GitHub Copilot, …)
    /// can create and edit presentations.
    Mcp {
        /// Folder that relative project paths resolve against (default: the current folder).
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Install the GlideDeck skill and MCP server for Claude Code and GitHub Copilot.
    ///
    /// By default sets up the current folder: the skill in `.claude/skills` (Claude
    /// Code) and `.github/skills` (Copilot), `.mcp.json` (Claude Code, Copilot CLI)
    /// and `.vscode/mcp.json` (Copilot in VS Code). Existing entries are kept.
    AgentSetup {
        /// Project folder to set up (default: the current folder).
        #[arg(long)]
        project: Option<PathBuf>,
        /// Set up for your user account (all projects) instead of one project.
        #[arg(long, conflicts_with = "project")]
        user: bool,
        /// Only install the skill.
        #[arg(long, conflicts_with = "no_skill")]
        no_mcp: bool,
        /// Only register the MCP server.
        #[arg(long)]
        no_skill: bool,
    },
    /// Render a view of the canvas to a PNG (offscreen, no window).
    ///
    /// Frames the whole document by default; pass --center/--zoom to choose a view,
    /// or --element to frame one element the way a waypoint would.
    Render {
        file: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        /// View center as `x,y` in world units.
        #[arg(long, value_parser = parse_point, allow_hyphen_values = true)]
        center: Option<DVec2>,
        /// Screen pixels per world unit.
        #[arg(long)]
        zoom: Option<f64>,
        /// View rotation in degrees.
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        rotation: f64,
        /// Frame this element id (overrides --center/--zoom/--rotation).
        #[arg(long)]
        element: Option<String>,
        /// Render the view of this slide (1-based step on the path).
        #[arg(long)]
        step: Option<usize>,
        #[arg(long)]
        no_grid: bool,
    },
}

fn parse_point(s: &str) -> Result<DVec2, String> {
    let (x, y) = s.split_once(',').ok_or("expected x,y")?;
    let p = |v: &str| v.trim().parse::<f64>().map_err(|e| e.to_string());
    Ok(DVec2::new(p(x)?, p(y)?))
}

pub fn run(cmd: Command) -> Result<()> {
    match cmd {
        Command::Open { .. } => unreachable!("handled by main"),
        Command::Mcp { root } => {
            let root = match root {
                Some(r) => r,
                None => std::env::current_dir()?,
            };
            std::fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
            crate::mcp::serve(root)?;
        }
        Command::AgentSetup { project, user, no_mcp, no_skill } => {
            let outcomes = if user {
                crate::agents::setup_user(!no_skill, !no_mcp)?
            } else {
                let dir = match project {
                    Some(p) => p,
                    None => std::env::current_dir()?,
                };
                crate::agents::setup_project(&dir, !no_skill, !no_mcp)?
            };
            for o in &outcomes {
                println!("{}: {}", o.path.display(), o.note);
            }
            if user && !no_mcp {
                println!("
To register the MCP server with the other assistants:

{}", crate::agents::user_hints());
            } else if !no_mcp {
                println!("
Reload VS Code (or restart Claude Code / Copilot CLI) and approve the \"glidedeck\" MCP server when asked.");
            }
        }
        Command::New { file, title, demo, template, force } => {
            if file.exists() && !force {
                bail!("{} already exists (use --force to overwrite)", file.display());
            }
            let mut doc = match (demo, template) {
                (true, _) => crate::demo::demo_document(),
                (false, Some(name)) => {
                    let slug = |s: &str| s.to_lowercase().replace(' ', "-");
                    let t = crate::design::Template::ALL
                        .into_iter()
                        .find(|t| slug(t.label()) == slug(&name) || (slug(&name) == "blank" && *t == crate::design::Template::Blank))
                        .with_context(|| {
                            let names: Vec<String> = crate::design::Template::ALL.iter().map(|t| if *t == crate::design::Template::Blank { "blank".into() } else { slug(t.label()) }).collect();
                            format!("unknown template {name:?} (try {})", names.join(", "))
                        })?;
                    crate::design::build_template(t)
                }
                (false, None) => Document::default(),
            };
            if title != "Untitled" || doc.title.is_empty() {
                doc.title = title;
            }
            doc.save(&file)?;
            println!("{}", json!({ "created": file }));
        }
        Command::Apply { file, ops, create } => {
            let mut doc = if !file.exists() && create { Document::default() } else { Document::load(&file)? };
            let text = read_input(ops.as_deref())?;
            let value: serde_json::Value = serde_json::from_str(&text).context("operations are not valid JSON")?;
            let ops: Vec<Op> = if value.is_array() {
                serde_json::from_value(value)
            } else {
                serde_json::from_value(value).map(|op| vec![op])
            }
            .context("operations do not match the op schema")?;
            match apply_all(&mut doc, ops) {
                Ok(results) => {
                    doc.save(&file)?;
                    println!("{}", serde_json::to_string_pretty(&json!({ "ok": true, "results": results }))?);
                }
                Err((index, err)) => {
                    println!("{}", json!({ "ok": false, "failed_op": index, "error": err.to_string() }));
                    bail!("operation {index} failed: {err}; document left unchanged");
                }
            }
        }
        Command::Show { file } => {
            let doc = Document::load(&file)?;
            let elements: Vec<_> = doc
                .elements
                .iter()
                .map(|e| {
                    json!({
                        "id": e.id,
                        "name": e.display_name(),
                        "type": e.kind.type_name(),
                        "transform": e.transform,
                    })
                })
                .collect();
            let summary = json!({ "title": doc.title, "format": doc.format, "elements": elements });
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::ExportPptx { file, out } => {
            let doc = Document::load(&file)?;
            let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
            let mut text = glide_render::TextEngine::new();
            let report = glide_pptx::export(&doc, Some(&base), &mut text, &out)?;
            println!(
                "{}",
                json!({ "written": out, "slides": report.slides, "shapes": report.shapes, "skipped_media": report.skipped })
            );
        }
        Command::Render { file, out, width, height, center, zoom, rotation, element, step, no_grid } => {
            let doc = Document::load(&file)?;
            let mut r = HeadlessRenderer::new()?;
            r.canvas.set_base_dir(file.parent().map(Path::to_path_buf));
            let viewport = DVec2::new(width as f64, height as f64);
            let step_frame = step.and_then(|i| doc.step_frame(i.saturating_sub(1)).map(|f| f.id.clone()));
            let camera = if let Some(id) = element.or(step_frame) {
                let el = doc.get(&id).with_context(|| format!("no element {id:?}"))?;
                let size = (r.canvas.local_size(el) * el.transform.scale.abs()).max(DVec2::splat(1e-12));
                let avail = if el.kind.is_frame() { viewport } else { viewport * 0.9 };
                Camera {
                    center: el.transform.position(),
                    zoom: (avail.x / size.x).min(avail.y / size.y),
                    rotation: el.transform.rotation.to_radians(),
                }
            } else {
                let fit = Camera::framing(r.document_bounds(&doc), rotation.to_radians(), viewport, 48.0);
                Camera { center: center.unwrap_or(fit.center), zoom: zoom.unwrap_or(fit.zoom), rotation: rotation.to_radians() }
            };
            let pixels = r.render_rgba(&doc, &camera, [width, height], !no_grid)?;
            write_png(&out, width, height, &pixels)?;
            println!("{}", json!({ "written": out, "camera": camera }));
        }
    }
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn read_input(path: Option<&Path>) -> Result<String> {
    match path {
        Some(p) if p.as_os_str() != "-" => {
            std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))
        }
        _ => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Ok(s)
        }
    }
}
