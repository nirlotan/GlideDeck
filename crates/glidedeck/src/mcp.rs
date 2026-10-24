//! A Model Context Protocol server, so AI assistants (Claude, GitHub Copilot,
//! and any other MCP client) can build GlideDeck presentations.
//!
//! `glidedeck mcp` speaks JSON-RPC 2.0 over stdio: one message per line on
//! stdin, one response per line on stdout. Nothing else is ever written to
//! stdout; diagnostics go to stderr. It runs entirely locally.
//!
//! Every tool works on a `.glide.json` GlideDeck project file and saves its
//! changes immediately, so whatever an assistant builds can be opened, edited
//! and presented in the GlideDeck editor afterwards.

use crate::layouts::{layout_elements, place_layout, slot_after, LayoutKind, LAYOUT_W};
use crate::templates::{build_template_with, Template};
use anyhow::{bail, Context, Result};
use glam::DVec2;
use glide_core::theme::{apply_theme, builtin_themes, default_theme};
use glide_core::{assets, ops::apply_all, Aabb, Camera, Color, Document, Element, ElementKind, Media, Op, Step, TextAlign, TextRole, Theme, Transform};
use glide_render::{HeadlessRenderer, TextEngine, ICONS};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "GlideDeck builds zooming presentations: slides are frames on an infinite canvas and the \
show flies the camera between them. Every tool reads and writes a `.glide.json` GlideDeck project file, saved \
immediately, which the user can later open and edit in the GlideDeck app (or with `open_in_editor`). \
Workflow: `catalog` → `create_presentation` (pick a template that fits the story) → `get_presentation` to see \
each slide's placeholder text with element ids → `update_elements` to replace ALL placeholder text → \
`add_slide` with a layout and its text fields for more slides → `render_slide` to check every slide visually → \
`export_pptx` / `export_pdf` if the user wants PowerPoint or PDF, or `export_html` for a web page that plays the show in a browser with every effect intact. To start from an existing PowerPoint file, use `import_pptx`, then `arrange_slides` to give it a zooming path. Keep slide text short: one idea per slide. \
Write speaker notes for every slide with `set_notes` (what to say, the number to stress); the presenter sees them in presenter view. \
MAKE IT OUTSTANDING: unless the user wants a plain deck, finish every presentation with all three of these. \
(1) Shape the journey: `arrange_slides` with a layout that fits the story (orbits or a tree for a hub and its topics, \
spiral or sunflower for growth, staircase for progress, zoom-tunnel for drilling down, journey or wave for a story, \
carousel for a sleek linear deck: slides glide side by side over a parallax backdrop, the best match for an imported \
PowerPoint that should stay linear) instead of a flat row. The `carousel` template starts there directly. (2) Vary the camera: `set_transitions` (swoop, spin, punch and fade next to smooth; e.g. \
styles [\"smooth\", \"swoop\", \"punch\"] cycles them), with smooth for zooming out to an overview. (3) Animate inside the \
slides: `auto_animate` gives every slide a polished entrance cascade (titles focus, text rises, icons pop, numbers \
count up), then `animate` the key moments: `spotlight` or `highlight` on the one number or phrase that matters, \
`count_up` on metrics, `words` or `typewriter` on a quote, on-click `rise` to reveal points as the speaker talks, \
`fade_out` / `fly_out` to clear a slide before its punchline. Use at most one or two emphasis effects per slide.";

/// Runs the server until stdin closes.
pub fn serve(root: PathBuf) -> Result<()> {
    let mut server = Server::new(root);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.context("reading stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.into_iter().filter_map(|m| server.handle(m)).collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            Ok(msg) => server.handle(msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("parse error: {e}") } })),
        };
        if let Some(reply) = reply {
            serde_json::to_writer(&mut stdout, &reply)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

pub struct Server {
    root: PathBuf,
    text: Option<TextEngine>,
    renderer: Option<HeadlessRenderer>,
}

impl Server {
    pub fn new(root: PathBuf) -> Self {
        Server { root, text: None, renderer: None }
    }

    /// Handles one JSON-RPC message; notifications get no reply.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or_default().to_owned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let id = id?; // Notifications (initialized, cancelled, …) need no answer.
        let result = match method.as_str() {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSIONS[0]);
                let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "glidedeck", "title": "GlideDeck", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                Ok(match self.call(&name, &args) {
                    Ok(content) => json!({ "content": content, "isError": false }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": format!("Error: {e:#}") }], "isError": true }),
                })
            }
            "resources/list" => Ok(json!({ "resources": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        })
    }

    fn text(&mut self) -> &mut TextEngine {
        self.text.get_or_insert_with(TextEngine::new)
    }

    /// Resolves a project path: relative to the server's root folder, with
    /// `.glide.json` added when no extension is given.
    fn project_path(&self, args: &Value, key: &str) -> Result<PathBuf> {
        let raw = str_arg(args, key)?;
        let mut p = PathBuf::from(raw);
        if p.extension().is_none() {
            p.set_extension("glide.json");
        }
        Ok(if p.is_absolute() { p } else { self.root.join(p) })
    }

    fn resolve(&self, raw: &str) -> PathBuf {
        let p = PathBuf::from(raw);
        if p.is_absolute() { p } else { self.root.join(p) }
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Vec<Value>> {
        match name {
            "catalog" => Ok(vec![text_json(&catalog())]),
            "search_icons" => {
                let q = str_arg(args, "query")?.to_lowercase();
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(40) as usize;
                let tokens: Vec<&str> = q.split_whitespace().collect();
                let names: Vec<&str> = ICONS.iter().map(|(n, _)| *n).filter(|n| tokens.iter().all(|t| n.contains(t))).take(limit).collect();
                Ok(vec![text_json(&json!({ "icons": names }))])
            }
            "create_presentation" => self.create(args),
            "get_presentation" => {
                let path = self.project_path(args, "path")?;
                let doc = load(&path)?;
                Ok(vec![text_json(&self.summary(&doc, &path))])
            }
            "add_slide" => self.add_slide(args),
            "update_elements" => self.update_elements(args),
            "add_text" => self.add_text(args),
            "add_image" => self.add_image(args),
            "set_theme" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let theme = find_theme(str_arg(args, "theme")?)?;
                apply_theme(&mut doc, &theme);
                save(&doc, &path)?;
                Ok(vec![text(format!("Applied the {} theme to {}.", theme.name, path.display()))])
            }
            "remove_slide" => self.remove_slide(args),
            "edit_presentation" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let ops: Vec<Op> = serde_json::from_value(args.get("ops").cloned().unwrap_or(Value::Null))
                    .context("`ops` must be an array of operations (see the tool description)")?;
                let results = apply_all(&mut doc, ops).map_err(|(i, e)| anyhow::anyhow!("operation {i} failed: {e}; nothing was changed"))?;
                save(&doc, &path)?;
                Ok(vec![text_json(&json!({ "ok": true, "results": results }))])
            }
            "render_slide" => self.render(args),
            "export_pptx" => {
                let path = self.project_path(args, "path")?;
                let doc = load(&path)?;
                let out = match args.get("out").and_then(Value::as_str) {
                    Some(o) => self.resolve(o),
                    None => with_suffix(&path, "pptx"),
                };
                let base = base_dir(&path);
                let report = glide_pptx::export(&doc, Some(&base), self.text(), &out)?;
                Ok(vec![text_json(&json!({
                    "written": out, "slides": report.slides, "shapes": report.shapes, "skipped_media": report.skipped,
                }))])
            }
            "export_html" => {
                let path = self.project_path(args, "path")?;
                let doc = load(&path)?;
                let out = match args.get("out").and_then(Value::as_str) {
                    Some(o) => self.resolve(o),
                    None => with_suffix(&path, "html"),
                };
                let base = base_dir(&path);
                crate::html_export::load_document_fonts(self.text(), &doc, Some(&base));
                let report = crate::html_export::export(&doc, self.text(), Some(&base), &out).map_err(|e| anyhow::anyhow!(e))?;
                Ok(vec![text_json(&json!({
                    "written": out, "slides": report.slides, "elements": report.elements, "bytes": report.bytes,
                    "media_files": report.media_files, "missing_media": report.missing_media,
                }))])
            }
            "list_fonts" => {
                if let Some(rel) = args.get("path").and_then(Value::as_str) {
                    let path = self.project_path(&json!({ "path": rel }), "path")?;
                    let doc = load(&path)?;
                    crate::html_export::load_document_fonts(self.text(), &doc, Some(&base_dir(&path)));
                }
                let query = args.get("query").and_then(Value::as_str).unwrap_or("").to_lowercase();
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(200) as usize;
                let all = self.text().families();
                let matches: Vec<&String> = all.iter().filter(|f| f.to_lowercase().contains(&query)).collect();
                Ok(vec![text_json(&json!({ "total": matches.len(), "fonts": matches.into_iter().take(limit).collect::<Vec<_>>() }))])
            }
            "add_font" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let src = self.resolve(str_arg(args, "font")?);
                let reference = assets::import_file(&base_dir(&path), &src).with_context(|| format!("copying {}", src.display()))?;
                let bytes = std::fs::read(&src).with_context(|| format!("reading {}", src.display()))?;
                let families = self.text().load_font_data(bytes);
                if families.is_empty() {
                    bail!("{} is not a font file GlideDeck can read (ttf, otf, ttc, otc)", src.display());
                }
                if !doc.fonts.contains(&reference) {
                    doc.fonts.push(reference.clone());
                }
                save(&doc, &path)?;
                Ok(vec![text_json(&json!({ "added": reference, "families": families }))])
            }
            "save_as" => self.save_as(args),
            "import_pptx" => {
                let pptx = self.resolve(str_arg(args, "pptx")?);
                let path = match args.get("path").and_then(Value::as_str) {
                    Some(_) => self.project_path(args, "path")?,
                    None => with_suffix(&pptx, "glide.json"),
                };
                if path.exists() && !args.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
                    bail!("{} already exists; pass overwrite: true or another path", path.display());
                }
                let (doc, skipped) = crate::pptx_import::import_to_project(&pptx, &path, self.text()).map_err(|e| anyhow::anyhow!(e))?;
                let mut summary = self.summary(&doc, &path);
                summary["skipped"] = json!(skipped);
                Ok(vec![text_json(&summary)])
            }
            "arrange_slides" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let name = str_arg(args, "layout")?;
                let kind = crate::arrange::Arrangement::from_id(name).with_context(|| format!("unknown layout {name:?}; see `catalog` (arrangements)"))?;
                let flag = |k: &str, default: bool| args.get(k).and_then(Value::as_bool).unwrap_or(default);
                let opts = crate::arrange::ArrangeOptions {
                    spacing: args.get("spacing").and_then(Value::as_f64).unwrap_or(0.25),
                    tilt: flag("tilt", kind.default_tilt()),
                    trail: flag("trail", true),
                    overview: flag("overview", true),
                };
                let report = crate::arrange::arrange(&mut doc, kind, &opts).map_err(|e| anyhow::anyhow!(e))?;
                save(&doc, &path)?;
                Ok(vec![text(format!(
                    "Arranged {} slides as {} ({} steps on the path). Render a few slides and step 0 to check the result.",
                    report.slides,
                    kind.label(),
                    doc.path.len()
                ))])
            }
            "export_pdf" => {
                let path = self.project_path(args, "path")?;
                let doc = load(&path)?;
                let out = match args.get("out").and_then(Value::as_str) {
                    Some(o) => self.resolve(o),
                    None => with_suffix(&path, "pdf"),
                };
                if self.renderer.is_none() {
                    self.renderer = Some(HeadlessRenderer::new().context("no GPU available for rendering")?);
                }
                let r = self.renderer.as_mut().expect("just created");
                r.canvas.set_base_dir(Some(base_dir(&path)));
                let width = args.get("width").and_then(Value::as_u64).unwrap_or(2560) as u32;
                let report = crate::pdf::export(&mut r.canvas, &doc, &out, crate::pdf::PdfOptions { width_px: width, ..Default::default() })
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(vec![text_json(&json!({ "written": out, "pages": report.pages, "bytes": report.bytes }))])
            }
            "animate" => self.animate(args),
            "auto_animate" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let steps = step_list(&doc, args)?;
                let text = self.text.get_or_insert_with(TextEngine::new);
                let mut report = Vec::new();
                for &i in &steps {
                    let builds = crate::anim::auto_builds(&doc, i, text);
                    report.push(json!({ "step": i + 1, "animated": builds.len() }));
                    doc.path[i].builds = builds;
                }
                save(&doc, &path)?;
                Ok(vec![text_json(&json!({
                    "ok": true, "slides": report,
                    "next": "Add emphasis on the key moments with `animate` (spotlight, highlight, count_up, …).",
                }))])
            }
            "set_notes" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let notes = args.get("notes").and_then(Value::as_array).context("`notes` must be an array of {step, text}")?;
                for n in notes {
                    let step = n.get("step").and_then(Value::as_u64).context("each note needs a 1-based `step`")? as usize;
                    if step == 0 || step > doc.path.len() {
                        bail!("slide {step} does not exist ({} slides)", doc.path.len());
                    }
                    doc.path[step - 1].notes = n.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
                }
                save(&doc, &path)?;
                Ok(vec![text(format!("Set speaker notes on {} slides.", notes.len()))])
            }
            "set_transitions" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let steps = step_list(&doc, args)?;
                let styles: Vec<glide_core::TransitionStyle> = match (args.get("style").and_then(Value::as_str), args.get("styles").and_then(Value::as_array)) {
                    (_, Some(list)) => list.iter().map(|v| v.as_str().unwrap_or_default()).map(|n| glide_core::TransitionStyle::from_id(n).with_context(|| format!("unknown transition {n:?}; see `catalog`"))).collect::<Result<_>>()?,
                    (Some(n), None) => vec![glide_core::TransitionStyle::from_id(n).with_context(|| format!("unknown transition {n:?}; see `catalog`"))?],
                    (None, None) => bail!("pass `style` or `styles`"),
                };
                if styles.is_empty() {
                    bail!("`styles` is empty");
                }
                let duration = args.get("duration").and_then(Value::as_f64);
                for (k, &i) in steps.iter().enumerate() {
                    let t = &mut doc.path[i].transition;
                    t.style = styles[k % styles.len()];
                    if let Some(d) = duration {
                        t.duration = d.clamp(0.1, 10.0);
                    }
                }
                save(&doc, &path)?;
                Ok(vec![text(format!("Set the transition of {} slides.", steps.len()))])
            }
            "clear_animations" => {
                let path = self.project_path(args, "path")?;
                let mut doc = load(&path)?;
                let steps = step_list(&doc, args)?;
                let mut n = 0;
                for &i in &steps {
                    n += doc.path[i].builds.len();
                    doc.path[i].builds.clear();
                }
                save(&doc, &path)?;
                Ok(vec![text(format!("Removed {n} animations."))])
            }
            "open_in_editor" => {
                let path = self.project_path(args, "path")?;
                if !path.exists() {
                    bail!("{} does not exist", path.display());
                }
                let exe = std::env::current_exe()?;
                std::process::Command::new(exe)
                    .arg(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .context("could not start the GlideDeck editor")?;
                Ok(vec![text(format!("Opened {} in the GlideDeck editor.", path.display()))])
            }
            _ => bail!("unknown tool {name:?}"),
        }
    }

    fn animate(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let mut doc = load(&path)?;
        let list = args.get("animations").and_then(Value::as_array).context("`animations` must be an array")?;
        let fixed_step = match args.get("step").and_then(Value::as_u64) {
            Some(n) if n == 0 || n as usize > doc.path.len() => bail!("slide {n} does not exist ({} slides)", doc.path.len()),
            Some(n) => Some(n as usize - 1),
            None => None,
        };
        let mut planned: Vec<(usize, glide_core::Build)> = Vec::new();
        for (k, a) in list.iter().enumerate() {
            let id = a.get("id").and_then(Value::as_str).with_context(|| format!("animation {k}: missing `id`"))?;
            if doc.get(id).is_none() {
                bail!("animation {k}: no element {id:?}; see get_presentation");
            }
            let name = a.get("effect").and_then(Value::as_str).with_context(|| format!("animation {k}: missing `effect`"))?;
            let effect = glide_core::Effect::from_id(name).with_context(|| format!("animation {k}: unknown effect {name:?}; see `catalog` (effects)"))?;
            let mut b = glide_core::Build::new(id, effect);
            if let Some(t) = a.get("trigger").and_then(Value::as_str) {
                b.trigger = parse_trigger(t)?;
            }
            if let Some(d) = a.get("duration").and_then(Value::as_f64) {
                b.duration = d.clamp(0.05, 20.0);
            }
            if let Some(d) = a.get("delay").and_then(Value::as_f64) {
                b.delay = d.clamp(0.0, 60.0);
            }
            if let Some(d) = a.get("direction").and_then(Value::as_str) {
                b.direction = parse_direction(d)?;
            } else if effect == glide_core::Effect::FlyOut {
                b.direction = glide_core::Direction::Right;
            }
            let step = match fixed_step {
                Some(s) => s,
                None => crate::anim::owner_step(&doc, id, None).with_context(|| format!("animation {k}: {id:?} is not on any slide; pass `step`"))?,
            };
            planned.push((step, b));
        }
        if args.get("replace").and_then(Value::as_bool).unwrap_or(false) {
            for (step, _) in &planned {
                doc.path[*step].builds.clear();
            }
        }
        for (step, b) in planned {
            doc.path[step].builds.push(b);
        }
        save(&doc, &path)?;
        let slides: Vec<Value> = doc
            .path
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.builds.is_empty())
            .map(|(i, s)| json!({ "step": i + 1, "animations": s.builds.len(), "clicks": glide_core::build::click_groups(&s.builds).len() }))
            .collect();
        Ok(vec![text_json(&json!({ "ok": true, "slides": slides }))])
    }

    fn create(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        if path.exists() && !args.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
            bail!("{} already exists; pass overwrite: true to replace it, or pick another path", path.display());
        }
        let template = match args.get("template").and_then(Value::as_str) {
            Some(t) => find_template(t)?,
            None => Template::Blank,
        };
        let mut doc = build_template_with(template, self.text());
        if let Some(theme) = args.get("theme").and_then(Value::as_str) {
            apply_theme(&mut doc, &find_theme(theme)?);
        }
        doc.title = args.get("title").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| template.label().to_owned());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        save(&doc, &path)?;
        let mut summary = self.summary(&doc, &path);
        summary["next"] = json!("Replace every placeholder text with update_elements (ids are listed per slide), then render_slide to check.");
        Ok(vec![text_json(&summary)])
    }

    fn add_slide(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let mut doc = load(&path)?;
        let kind = find_layout(str_arg(args, "layout")?)?;
        let theme = doc.theme.clone().unwrap_or_else(default_theme);
        // After the requested slide (1-based), or after the last one.
        let after = match args.get("after_step").and_then(Value::as_u64) {
            Some(n) => (n as usize).min(doc.path.len()),
            None => doc.path.len(),
        };
        let anchor = after.checked_sub(1).and_then(|i| doc.step_frame(i)).cloned();
        let (center, width, rotation) = match &anchor {
            Some(f) => slot_after(&doc, f),
            None => {
                let b = doc.elements.iter().fold(Aabb::EMPTY, |b, e| b.union(e.world_aabb(glide_render::local_size(self.text(), &e.kind))));
                let start = if b.is_empty() { DVec2::ZERO } else { DVec2::new(b.max.x + LAYOUT_W * 0.65, b.center().y) };
                (crate::layouts::free_spot(&doc, start, LAYOUT_W, 0.0), LAYOUT_W, 0.0)
            }
        };
        let frame_id = place_layout(&mut doc, kind, &theme, center, width, rotation);
        if let Some(name) = args.get("name").and_then(Value::as_str)
            && let Some(f) = doc.get_mut(&frame_id)
        {
            f.name = name.to_owned();
        }
        let start = doc.index_of(&frame_id).unwrap_or(0) + 1;
        fill_fields(&mut doc.elements[start..], args.get("text"), args.get("icons"))?;
        doc.path.insert(after, Step::new(frame_id.clone()));
        save(&doc, &path)?;
        let slide = self.summary(&doc, &path)["slides"][after].clone();
        Ok(vec![text_json(&json!({ "added": slide, "slides": doc.path.len() }))])
    }

    fn update_elements(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let mut doc = load(&path)?;
        let updates = args.get("updates").and_then(Value::as_array).context("`updates` must be an array of {id, …}")?;
        for u in updates {
            let id = str_arg(u, "id")?;
            let el = doc.get_mut(id).with_context(|| format!("no element with id {id:?}"))?;
            if let Some(t) = u.get("text").and_then(Value::as_str) {
                let ElementKind::Text(block) = &mut el.kind else { bail!("{id} is not a text element") };
                set_text(block, t, u.get("markdown").and_then(Value::as_bool).unwrap_or(false));
            }
            if let Some(font) = u.get("font").and_then(Value::as_str) {
                if !self.text().has_family(font) {
                    bail!("the font {font:?} is not installed; see `list_fonts`, or add a font file with `add_font`");
                }
                let el = doc.get_mut(id).with_context(|| format!("no element with id {id:?}"))?;
                let ElementKind::Text(block) = &mut el.kind else { bail!("{id} is not a text element") };
                block.font_family = font.to_owned();
                block.role = None;
            }
            let el = doc.get_mut(id).with_context(|| format!("no element with id {id:?}"))?;
            if let Some(name) = u.get("icon").and_then(Value::as_str) {
                let ElementKind::Icon(icon) = &mut el.kind else { bail!("{id} is not an icon") };
                check_icon(name)?;
                icon.name = name.to_owned();
            }
            if let Some(c) = u.get("color").and_then(Value::as_str) {
                let color: Color = c.parse().map_err(|_| anyhow::anyhow!("{c:?} is not a #RRGGBB color"))?;
                match &mut el.kind {
                    ElementKind::Text(t) => {
                        t.color = color;
                        t.role = None;
                    }
                    ElementKind::Icon(i) => i.color = color,
                    ElementKind::Rect(r) => r.fill = Some(color),
                    ElementKind::Ellipse(e) => e.fill = Some(color),
                    _ => bail!("{id} has no color to change"),
                }
            }
            if let Some(depth) = u.get("parallax").and_then(Value::as_f64) {
                el.parallax = depth.clamp(-1.0, 0.95);
            }
        }
        save(&doc, &path)?;
        Ok(vec![text(format!("Updated {} element(s) in {}.", updates.len(), path.display()))])
    }

    fn add_text(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let mut doc = load(&path)?;
        let frame = slide_frame(&doc, args)?;
        let theme = doc.theme.clone().unwrap_or_else(default_theme);
        let role = match args.get("role").and_then(Value::as_str).unwrap_or("body") {
            "title" => TextRole::Title,
            "heading" => TextRole::Heading,
            "subtitle" => TextRole::Subtitle,
            "caption" => TextRole::Caption,
            "quote" => TextRole::Quote,
            _ => TextRole::Body,
        };
        let mut block = theme.text_style(role);
        set_text(&mut block, str_arg(args, "text")?, args.get("markdown").and_then(Value::as_bool).unwrap_or(false));
        if let Some(size) = args.get("size").and_then(Value::as_f64) {
            block.font_size = size;
        }
        block.max_width = Some(args.get("width").and_then(Value::as_f64).unwrap_or(1200.0));
        block.align = match args.get("align").and_then(Value::as_str) {
            Some("center") => TextAlign::Center,
            Some("right") => TextAlign::Right,
            _ => TextAlign::Left,
        };
        if args.get("bullets").and_then(Value::as_bool).unwrap_or(false) {
            block.list = glide_core::ListStyle::Bullet;
        }
        let local = DVec2::new(args.get("x").and_then(Value::as_f64).unwrap_or(0.0), args.get("y").and_then(Value::as_f64).unwrap_or(0.0));
        let el = Element::new(ElementKind::Text(block)).with_transform(on_slide(&frame, local, 1.0));
        let id = doc.add(el);
        save(&doc, &path)?;
        Ok(vec![text_json(&json!({ "added": id }))])
    }

    fn add_image(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let mut doc = load(&path)?;
        let frame = slide_frame(&doc, args)?;
        let src = self.resolve(str_arg(args, "image")?);
        let is_svg = src.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        let (w, h) = if is_svg {
            glide_render::svg::svg_dimensions(&src).map(|(w, h)| (w as f64, h as f64))
        } else {
            glide_render::image_dimensions(&src).map(|(w, h)| (w as f64, h as f64))
        }
        .with_context(|| format!("{} is not a readable image", src.display()))?;
        let reference = assets::import_file(&base_dir(&path), &src).with_context(|| format!("copying {}", src.display()))?;
        let media = Media::new(reference, w, h);
        let width = args.get("width").and_then(Value::as_f64).unwrap_or(800.0);
        let local = DVec2::new(args.get("x").and_then(Value::as_f64).unwrap_or(0.0), args.get("y").and_then(Value::as_f64).unwrap_or(0.0));
        let kind = if is_svg { ElementKind::Svg(media) } else { ElementKind::Image(media) };
        let id = doc.add(Element::new(kind).with_transform(on_slide(&frame, local, width / w.max(1.0))));
        save(&doc, &path)?;
        Ok(vec![text_json(&json!({ "added": id, "note": "The image was copied into the project's assets/ folder." }))])
    }

    fn remove_slide(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let mut doc = load(&path)?;
        let step = args.get("step").and_then(Value::as_u64).context("`step` (1-based) is required")? as usize;
        if step == 0 || step > doc.path.len() {
            bail!("step {step} does not exist (the presentation has {} slides)", doc.path.len());
        }
        let frame_id = doc.path.remove(step - 1).frame;
        let keep = args.get("keep_content").and_then(Value::as_bool).unwrap_or(false);
        let mut removed = 0;
        if !keep && !doc.path.iter().any(|s| s.frame == frame_id) {
            let owned: Vec<String> = {
                let text = self.text.get_or_insert_with(TextEngine::new);
                doc.elements.iter().filter(|e| owner_frame(&doc, e, text).as_deref() == Some(frame_id.as_str())).map(|e| e.id.clone()).collect()
            };
            for id in owned.iter().chain([&frame_id]) {
                removed += doc.remove(id).is_some() as usize;
            }
        }
        save(&doc, &path)?;
        Ok(vec![text(format!("Removed slide {step} ({removed} elements deleted); {} slides remain.", doc.path.len()))])
    }

    fn render(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let doc = load(&path)?;
        let width = args.get("width").and_then(Value::as_u64).unwrap_or(1280).clamp(160, 3840) as u32;
        let height = width * 9 / 16;
        if self.renderer.is_none() {
            self.renderer = Some(HeadlessRenderer::new().context("no GPU available for rendering")?);
        }
        let r = self.renderer.as_mut().expect("just created");
        r.canvas.set_base_dir(Some(base_dir(&path)));
        let viewport = DVec2::new(width as f64, height as f64);
        let step = args.get("step").and_then(Value::as_u64).unwrap_or(1) as usize;
        let camera = if step == 0 {
            Camera::framing(r.document_bounds(&doc), 0.0, viewport, 24.0)
        } else {
            let frame = doc.step_frame(step - 1).with_context(|| format!("slide {step} does not exist ({} slides)", doc.path.len()))?;
            let size = (r.canvas.local_size(frame) * frame.transform.scale.abs()).max(DVec2::splat(1e-12));
            Camera { center: frame.transform.position(), zoom: (viewport.x / size.x).min(viewport.y / size.y), rotation: frame.transform.rotation.to_radians() }
        };
        let pixels = r.render_rgba(&doc, &camera, [width, height], false)?;
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, width, height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            enc.write_header()?.write_image_data(&pixels)?;
        }
        if let Some(out) = args.get("save_to").and_then(Value::as_str) {
            std::fs::write(self.resolve(out), &png)?;
        }
        let label = if step == 0 { "the whole canvas".to_owned() } else { format!("slide {step} of {}", doc.path.len()) };
        Ok(vec![text(format!("Rendered {label}.")), json!({ "type": "image", "data": base64(&png), "mimeType": "image/png" })])
    }

    fn save_as(&mut self, args: &Value) -> Result<Vec<Value>> {
        let path = self.project_path(args, "path")?;
        let target = self.project_path(args, "new_path")?;
        if target.exists() && !args.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
            bail!("{} already exists; pass overwrite: true to replace it", target.display());
        }
        let mut doc = load(&path)?;
        let (from, to) = (base_dir(&path), base_dir(&target));
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Media travels with the project into the new folder's assets/.
        if from != to {
            for el in &mut doc.elements {
                if let ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) = &mut el.kind {
                    let src = assets::resolve(Some(&from), &m.path);
                    if src.exists() {
                        m.path = assets::import_file(&to, &src)?;
                    }
                }
            }
        }
        if let Some(title) = args.get("title").and_then(Value::as_str) {
            doc.title = title.to_owned();
        }
        save(&doc, &target)?;
        Ok(vec![text(format!("Saved a copy as {}. Open it in GlideDeck to keep editing.", target.display()))])
    }

    /// A compact description of the slides and the editable content on each.
    fn summary(&mut self, doc: &Document, path: &Path) -> Value {
        let text = self.text.get_or_insert_with(TextEngine::new);
        let mut per_frame: std::collections::HashMap<String, Vec<Value>> = Default::default();
        let mut shapes: std::collections::HashMap<String, usize> = Default::default();
        for el in doc.elements.iter().filter(|e| !e.kind.is_frame()) {
            let Some(owner) = owner_frame(doc, el, text) else { continue };
            let entry = match &el.kind {
                ElementKind::Text(t) => json!({ "id": el.id, "name": el.name, "type": "text", "text": t.content }),
                ElementKind::Icon(i) => json!({ "id": el.id, "name": el.name, "type": "icon", "icon": i.name }),
                ElementKind::Image(m) | ElementKind::Svg(m) | ElementKind::Video(m) => {
                    json!({ "id": el.id, "name": el.name, "type": el.kind.type_name(), "file": m.path })
                }
                _ => {
                    *shapes.entry(owner).or_default() += 1;
                    continue;
                }
            };
            per_frame.entry(owner).or_default().push(entry);
        }
        let slides: Vec<Value> = doc
            .path
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let name = doc.get(&step.frame).map(|f| f.name.clone()).unwrap_or_default();
                let groups = glide_core::build::click_groups(&step.builds);
                let animations: Vec<Value> = step
                    .builds
                    .iter()
                    .enumerate()
                    .map(|(k, b)| {
                        let click = groups.iter().position(|g| g.contains(&k)).unwrap_or(0) + 1;
                        json!({ "id": b.element, "effect": b.effect.id(), "trigger": serde_json::to_value(b.trigger).unwrap_or_default(), "click": click })
                    })
                    .collect();
                json!({
                    "step": i + 1,
                    "frame_id": step.frame,
                    "name": name,
                    "transition": step.transition.style.id(),
                    "elements": per_frame.get(&step.frame).cloned().unwrap_or_default(),
                    "decorative_shapes": shapes.get(&step.frame).copied().unwrap_or(0),
                    "notes": step.notes,
                    "animations": animations,
                })
            })
            .collect();
        json!({
            "file": path,
            "title": doc.title,
            "theme": doc.theme.as_ref().map(|t| t.name.clone()),
            "slide_count": doc.path.len(),
            "slides": slides,
            "note": "Slides that revisit the same frame (e.g. an overview at the start and end) list its elements each time.",
        })
    }
}

// --- helpers ------------------------------------------------------------------------

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key).and_then(Value::as_str).with_context(|| format!("missing string argument `{key}`"))
}

fn text(s: impl Into<String>) -> Value {
    json!({ "type": "text", "text": s.into() })
}

fn text_json(v: &Value) -> Value {
    text(serde_json::to_string_pretty(v).unwrap_or_default())
}

fn load(path: &Path) -> Result<Document> {
    Document::load(path).with_context(|| format!("opening {}", path.display()))
}

fn save(doc: &Document, path: &Path) -> Result<()> {
    // Animations of removed objects go with them.
    let mut doc = doc.clone();
    crate::anim::prune(&mut doc);
    doc.save(path).with_context(|| format!("saving {}", path.display()))
}

/// 1-based slide numbers from `args.steps` (all slides when absent).
fn step_list(doc: &Document, args: &Value) -> Result<Vec<usize>> {
    match args.get("steps") {
        None | Some(Value::Null) => Ok((0..doc.path.len()).collect()),
        Some(Value::String(s)) if s == "all" => Ok((0..doc.path.len()).collect()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| {
                let n = v.as_u64().context("`steps` must be 1-based slide numbers")? as usize;
                if n == 0 || n > doc.path.len() {
                    bail!("slide {n} does not exist ({} slides)", doc.path.len());
                }
                Ok(n - 1)
            })
            .collect(),
        Some(_) => bail!("`steps` must be an array of 1-based slide numbers, or \"all\""),
    }
}

fn parse_trigger(s: &str) -> Result<glide_core::Trigger> {
    use glide_core::Trigger::*;
    Ok(match s.trim().to_lowercase().replace([' ', '-'], "_").as_str() {
        "on_click" | "click" => OnClick,
        "with_previous" | "with" => WithPrevious,
        "after_previous" | "after" => AfterPrevious,
        other => bail!("unknown trigger {other:?}: use on_click, with_previous or after_previous"),
    })
}

fn parse_direction(s: &str) -> Result<glide_core::Direction> {
    use glide_core::Direction::*;
    Ok(match s.trim().to_lowercase().as_str() {
        "left" => Left,
        "right" => Right,
        "up" | "top" => Up,
        "down" | "bottom" => Down,
        other => bail!("unknown direction {other:?}: use left, right, up or down"),
    })
}

fn base_dir(path: &Path) -> PathBuf {
    path.parent().filter(|d| !d.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// `talk.glide.json` → `talk.pptx` next to it.
fn with_suffix(path: &Path, ext: &str) -> PathBuf {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("presentation");
    let stem = name.strip_suffix(".glide.json").or_else(|| name.rsplit_once('.').map(|(stem, _)| stem)).unwrap_or(name);
    path.with_file_name(format!("{stem}.{ext}"))
}

fn slug(s: &str) -> String {
    s.to_lowercase().replace([' ', '_'], "-").replace('&', "and").replace("--", "-").replace("-and-", "-")
}

fn template_id(t: Template) -> String {
    if t == Template::Blank { "blank".into() } else { slug(t.label()) }
}

fn layout_id(k: LayoutKind) -> String {
    slug(k.label()).replace('+', "").replace("--", "-")
}

fn find_template(name: &str) -> Result<Template> {
    let want = slug(name);
    Template::ALL
        .into_iter()
        .find(|t| template_id(*t) == want || slug(t.label()) == want)
        .with_context(|| format!("unknown template {name:?}; call `catalog` for the list"))
}

fn find_layout(name: &str) -> Result<LayoutKind> {
    let want = layout_id_from(name);
    LayoutKind::ALL.into_iter().find(|k| layout_id(*k) == want).with_context(|| format!("unknown layout {name:?}; call `catalog` for the list"))
}

fn layout_id_from(name: &str) -> String {
    slug(name).replace('+', "").replace("--", "-")
}

fn find_theme(name: &str) -> Result<Theme> {
    builtin_themes()
        .into_iter()
        .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
        .with_context(|| format!("unknown theme {name:?}; call `catalog` for the list"))
}

fn check_icon(name: &str) -> Result<()> {
    if ICONS.iter().any(|(n, _)| *n == name) {
        Ok(())
    } else {
        bail!("unknown icon {name:?}; use search_icons to find one")
    }
}

/// Text fields and icon slots a layout offers, by element name with counts.
fn layout_fields(kind: LayoutKind) -> (Value, Value) {
    let mut texts: Vec<(String, usize)> = Vec::new();
    let mut icons: Vec<(String, usize)> = Vec::new();
    for el in layout_elements(kind, &default_theme()) {
        let list = match el.kind {
            ElementKind::Text(_) => &mut texts,
            ElementKind::Icon(_) => &mut icons,
            _ => continue,
        };
        match list.iter_mut().find(|(n, _)| *n == el.name) {
            Some((_, c)) => *c += 1,
            None => list.push((el.name.clone(), 1)),
        }
    }
    let obj = |v: Vec<(String, usize)>| Value::Object(v.into_iter().map(|(n, c)| (n, json!(c))).collect());
    (obj(texts), obj(icons))
}

fn catalog() -> Value {
    let templates: Vec<Value> = Template::ALL
        .into_iter()
        .map(|t| json!({ "id": template_id(t), "name": t.label(), "category": t.category(), "description": t.blurb() }))
        .collect();
    let layouts: Vec<Value> = LayoutKind::ALL
        .into_iter()
        .map(|k| {
            let (fields, icons) = layout_fields(k);
            json!({ "id": layout_id(k), "name": k.label(), "category": k.category(), "text_fields": fields, "icon_slots": icons })
        })
        .collect();
    let themes: Vec<Value> = builtin_themes()
        .into_iter()
        .map(|t| json!({ "name": t.name, "dark": t.is_dark(), "heading_font": t.heading_font, "body_font": t.body_font }))
        .collect();
    let arrangements: Vec<Value> = crate::arrange::Arrangement::ALL
        .into_iter()
        .map(|a| json!({ "id": a.id(), "name": a.label(), "description": a.blurb() }))
        .collect();
    let effects: Vec<Value> = glide_core::Effect::all()
        .map(|e| {
            let kind = match e.kind() {
                glide_core::BuildKind::Appear => "appear",
                glide_core::BuildKind::Emphasis => "emphasis",
                glide_core::BuildKind::Disappear => "disappear",
            };
            json!({ "id": e.id(), "name": e.label(), "kind": kind, "description": e.description(), "default_duration": e.default_duration(), "has_direction": e.has_direction() })
        })
        .collect();
    let transitions: Vec<Value> = glide_core::TransitionStyle::ALL
        .into_iter()
        .map(|t| json!({ "id": t.id(), "name": t.label(), "description": t.description() }))
        .collect();
    json!({
        "templates": templates,
        "layouts": layouts,
        "themes": themes,
        "arrangements": arrangements,
        "effects": effects,
        "transitions": transitions,
        "triggers": ["on_click", "with_previous", "after_previous"],
        "notes": "Layout text_fields/icon_slots map an element name to how many there are; pass a string for one, or an \
                  array to fill several in order. Slide coordinates for add_text/add_image: the slide is 1600 × 900 \
                  units with (0, 0) at its center. Icons are Phosphor names (search_icons).",
    })
}

/// Fills a freshly placed layout's text fields and icon slots by element name.
fn fill_fields(elements: &mut [Element], texts: Option<&Value>, icons: Option<&Value>) -> Result<()> {
    let values = |v: &Value| -> Vec<String> {
        match v {
            Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
            Value::String(s) => vec![s.clone()],
            _ => Vec::new(),
        }
    };
    for (map, is_text) in [(texts, true), (icons, false)] {
        let Some(map) = map.and_then(Value::as_object) else { continue };
        for (field, v) in map {
            let targets: Vec<usize> = elements
                .iter()
                .enumerate()
                .filter(|(_, e)| e.name == *field && matches!((&e.kind, is_text), (ElementKind::Text(_), true) | (ElementKind::Icon(_), false)))
                .map(|(i, _)| i)
                .collect();
            if targets.is_empty() {
                let kind = if is_text { "text field" } else { "icon slot" };
                bail!("this layout has no {kind} named {field:?}; see `catalog`");
            }
            for (i, value) in targets.into_iter().zip(values(v)) {
                match &mut elements[i].kind {
                    ElementKind::Text(t) => {
                        t.spans.clear();
                        t.content = value;
                    }
                    ElementKind::Icon(icon) => {
                        check_icon(&value)?;
                        icon.name = value;
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// The frame of the slide named by `step` (1-based; default: the last slide).
fn slide_frame(doc: &Document, args: &Value) -> Result<Element> {
    if doc.path.is_empty() {
        bail!("the presentation has no slides yet; use add_slide first");
    }
    let step = args.get("step").and_then(Value::as_u64).map(|s| s as usize).unwrap_or(doc.path.len());
    doc.step_frame(step.wrapping_sub(1)).cloned().with_context(|| format!("slide {step} does not exist ({} slides)", doc.path.len()))
}

/// A transform for a point in slide units (1600 wide, origin at the center).
fn on_slide(frame: &Element, local: DVec2, scale: f64) -> Transform {
    let frame_w = frame.kind.intrinsic_size().map(|s| s.x).unwrap_or(LAYOUT_W);
    let k = frame_w / LAYOUT_W;
    let p = frame.transform.affine().transform_point2(local * k);
    Transform { x: p.x, y: p.y, scale: frame.transform.scale * k * scale, rotation: frame.transform.rotation }
}

/// The smallest frame containing an element's center.
fn owner_frame(doc: &Document, el: &Element, text: &mut TextEngine) -> Option<String> {
    let c = el.transform.position();
    let own_size = el.world_aabb(glide_render::local_size(text, &el.kind)).size();
    doc.elements
        .iter()
        .filter(|f| f.kind.is_frame() && f.id != el.id)
        .filter_map(|f| {
            let size = f.kind.intrinsic_size()?;
            let local = f.transform.affine().inverse().transform_point2(c);
            let inside = local.x.abs() <= size.x / 2.0 && local.y.abs() <= size.y / 2.0;
            let area = size.x * size.y * f.transform.scale * f.transform.scale;
            // Skip backdrops much bigger than the frame (e.g. a sun behind a slide).
            let fits = own_size.x * own_size.y <= area * 4.0;
            (inside && fits).then_some((area, f.id.clone()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Sets a text box's text: plain (keeping formatting on unchanged text), or
/// with inline Markdown (`**bold**`, `*italic*`, `__underline__`, `~~strike~~`)
/// turned into formatting.
fn set_text(block: &mut glide_core::TextBlock, text: &str, markdown: bool) {
    if !markdown {
        block.set_content(text.to_owned());
        return;
    }
    let (plain, spans) = inline_markdown(text);
    block.content = plain;
    block.spans = spans;
    block.normalize_spans();
}

/// Parses `**bold**`, `*italic*`/`_italic_`, `__underline__` and `~~strike~~`.
fn inline_markdown(src: &str) -> (String, Vec<glide_core::TextSpan>) {
    const MARKS: [&str; 5] = ["**", "__", "~~", "*", "_"];
    let mut out = String::new();
    let mut spans = Vec::new();
    let mut open: Vec<(&str, usize)> = Vec::new();
    let mut rest = src;
    while !rest.is_empty() {
        let mark = MARKS.iter().find(|m| rest.starts_with(**m)).copied();
        // A single `_` inside a word (snake_case) is text, not italics.
        let word_inside = mark == Some("_") && out.chars().last().is_some_and(char::is_alphanumeric) && rest[1..].chars().next().is_some_and(char::is_alphanumeric);
        match mark {
            Some(m) if !word_inside && (open.iter().any(|(o, _)| *o == m) || rest[m.len()..].contains(m)) => {
                if let Some(i) = open.iter().rposition(|(o, _)| *o == m) {
                    let (_, start) = open.remove(i);
                    let mut style = glide_core::SpanStyle::default();
                    match m {
                        "**" => style.bold = Some(true),
                        "__" => style.underline = true,
                        "~~" => style.strike = true,
                        _ => style.italic = Some(true),
                    }
                    spans.push(glide_core::TextSpan { start, end: out.len(), style });
                } else {
                    open.push((m, out.len()));
                }
                rest = &rest[m.len()..];
            }
            _ => {
                let c = rest.chars().next().expect("non-empty");
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    // Nested markers (`**bold _and italic_**`) overlap: flatten them into
    // consecutive styled segments.
    let mut block = glide_core::TextBlock::new(out.clone());
    for s in spans {
        let st = s.style.clone();
        block.format_range(s.start..s.end, |x| {
            x.bold = x.bold.or(st.bold);
            x.italic = x.italic.or(st.italic);
            x.underline |= st.underline;
            x.strike |= st.strike;
        });
    }
    (out, block.spans)
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

// --- tool definitions -----------------------------------------------------------------

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
        "annotations": { "title": title, "readOnlyHint": read_only, "openWorldHint": false },
    })
}

fn path_prop() -> Value {
    json!({ "type": "string", "description": "The GlideDeck project file (.glide.json). Absolute, or relative to the server's folder; `.glide.json` is added when there is no extension." })
}

fn tool_definitions() -> Vec<Value> {
    let fields = json!({
        "type": "object",
        "description": "Element name → text (or an array of texts for repeated names, filled in order). Names per layout are in `catalog`.",
        "additionalProperties": { "anyOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }] },
    });
    vec![
        tool("catalog", "List templates, layouts and themes",
            "Lists the presentation templates, slide layouts (with the text fields and icon slots each one has) and color themes. Call this first.",
            json!({}), &[], true),
        tool("search_icons", "Search icons",
            "Searches the ~1,500 bundled Phosphor icon names (e.g. 'rocket', 'chart', 'users').",
            json!({ "query": { "type": "string" }, "limit": { "type": "integer", "default": 40 } }), &["query"], true),
        tool("create_presentation", "Create a presentation",
            "Creates a new GlideDeck project file from a template (default 'blank') and returns its slides with every element id and placeholder text. The file is a regular GlideDeck project the user can open and edit later.",
            json!({
                "path": path_prop(),
                "template": { "type": "string", "description": "Template id from `catalog`, e.g. pitch-deck, roadmap, orbit, journey, blank." },
                "title": { "type": "string" },
                "theme": { "type": "string", "description": "Optional theme name from `catalog` (overrides the template's)." },
                "overwrite": { "type": "boolean", "default": false },
            }), &["path"], false),
        tool("get_presentation", "Read a presentation",
            "Returns the title, theme and slides in show order, with the id, name and current text of every text element, icon and image on each slide.",
            json!({ "path": path_prop() }), &["path"], true),
        tool("add_slide", "Add a slide",
            "Adds a slide from a layout (see `catalog`) after the given slide (default: at the end), styled with the presentation's theme, and fills its text fields and icons.",
            json!({
                "path": path_prop(),
                "layout": { "type": "string", "description": "Layout id, e.g. title, section, three-points, comparison, quadrants, dashboard, hub-spokes, venn, process, timeline." },
                "text": fields,
                "icons": { "type": "object", "description": "Icon slot name → Phosphor icon name (or array).", "additionalProperties": { "anyOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }] } },
                "after_step": { "type": "integer", "description": "1-based slide to insert after; 0 inserts first. Default: last." },
                "name": { "type": "string", "description": "Slide name shown in the slide list." },
            }), &["path", "layout"], false),
        tool("update_elements", "Update text, icons or colors",
            "Changes elements by id: `text` for text boxes (use \\n for new paragraphs; with `markdown: true`, **bold**, *italic*, __underline__ and ~~strike~~ become formatting), `icon` for icons, `color` (#RRGGBB) for text, icons and shapes, `font` (a family from `list_fonts`) for text boxes, `parallax` for depth in the show (0.3–0.7 far backdrop that drifts slower, negative = near, moves faster).",
            json!({
                "path": path_prop(),
                "updates": { "type": "array", "items": { "type": "object", "properties": {
                    "id": { "type": "string" }, "text": { "type": "string" }, "icon": { "type": "string" }, "color": { "type": "string" },
                    "font": { "type": "string", "description": "Font family for a text box: any font on this computer or added with `add_font` (see `list_fonts`)." },
                    "parallax": { "type": "number", "description": "Parallax depth: 0 = on the canvas, up to 0.95 = far away, negative = near." },
                    "markdown": { "type": "boolean", "description": "Read **bold**, *italic*, __underline__ and ~~strike~~ in `text` as formatting." },
                }, "required": ["id"] } },
            }), &["path", "updates"], false),
        tool("add_text", "Add a text box",
            "Adds a themed text box to a slide. Slide coordinates are 1600 × 900 units with (0, 0) at the center.",
            json!({
                "path": path_prop(),
                "text": { "type": "string" },
                "step": { "type": "integer", "description": "1-based slide; default: the last one." },
                "role": { "type": "string", "enum": ["title", "heading", "subtitle", "body", "caption", "quote"], "default": "body" },
                "x": { "type": "number", "default": 0 }, "y": { "type": "number", "default": 0 },
                "width": { "type": "number", "description": "Wrap width in slide units.", "default": 1200 },
                "size": { "type": "number", "description": "Font size in slide units (body ≈ 32, heading ≈ 64)." },
                "align": { "type": "string", "enum": ["left", "center", "right"] },
                "bullets": { "type": "boolean" },
                "markdown": { "type": "boolean", "description": "Read **bold**, *italic*, __underline__ and ~~strike~~ as formatting." },
            }), &["path", "text"], false),
        tool("add_image", "Add an image",
            "Copies an image or SVG file into the project's assets/ folder and places it on a slide (slide units: 1600 × 900, origin at the center).",
            json!({
                "path": path_prop(),
                "image": { "type": "string", "description": "Image file (png, jpg, gif, webp, svg…)." },
                "step": { "type": "integer" },
                "x": { "type": "number", "default": 0 }, "y": { "type": "number", "default": 0 },
                "width": { "type": "number", "default": 800 },
            }), &["path", "image"], false),
        tool("set_theme", "Change the theme",
            "Restyles the whole presentation with a theme from `catalog` (colors, fonts and background).",
            json!({ "path": path_prop(), "theme": { "type": "string" } }), &["path", "theme"], false),
        tool("remove_slide", "Remove a slide",
            "Removes a slide from the show; its frame and content are deleted unless keep_content is true or another step uses the same frame.",
            json!({ "path": path_prop(), "step": { "type": "integer" }, "keep_content": { "type": "boolean", "default": false } }), &["path", "step"], false),
        tool("edit_presentation", "Apply raw edit operations",
            "Applies a batch of low-level GlideDeck operations atomically (all or nothing). Ops: add_element {element}, remove_element {id}, set_transform {id, x?, y?, scale?, rotation?}, set_kind {id, kind}, set_name {id, name}, set_opacity {id, opacity}, reorder {id, index?}, set_title {title}, set_background {color}, add_step {frame, index?}, remove_step {index}, move_step {from, to}. Each op is an object with an \"op\" field, e.g. {\"op\":\"set_transform\",\"id\":\"text-1\",\"y\":120}. Coordinates here are world units.",
            json!({ "path": path_prop(), "ops": { "type": "array", "items": { "type": "object" } } }), &["path", "ops"], false),
        tool("render_slide", "Render a slide to an image",
            "Renders a slide (1-based step; 0 = the whole canvas) to a PNG and returns it, so you can check layout, overflow and contrast. Optionally also saves it.",
            json!({
                "path": path_prop(),
                "step": { "type": "integer", "default": 1 },
                "width": { "type": "integer", "default": 1280 },
                "save_to": { "type": "string", "description": "Optional .png path to also write." },
            }), &["path"], true),
        tool("export_pptx", "Export to PowerPoint",
            "Exports the presentation to .pptx: one slide per step, with Morph transitions. Defaults to the project's name with .pptx.",
            json!({ "path": path_prop(), "out": { "type": "string" } }), &["path"], false),
        tool("export_html", "Export to a web page",
            "Exports the presentation to a single self-contained .html file that plays the show in any browser exactly like the app: the same camera moves, transitions, builds, parallax and effects, with text as vector outlines (no fonts needed) and images embedded. Videos are copied to a `<name>_media` folder next to it. Defaults to the project's name with .html.",
            json!({ "path": path_prop(), "out": { "type": "string" } }), &["path"], false),
        tool("list_fonts", "List fonts",
            "Lists the font families available for text: the fonts installed on this computer plus, when `path` is given, the font files added to that presentation.",
            json!({
                "query": { "type": "string", "description": "Only families whose name contains this (case-insensitive)." },
                "path": { "type": "string", "description": "Optional project, to include its added fonts." },
                "limit": { "type": "integer", "default": 200 },
            }), &[], true),
        tool("add_font", "Add a font file",
            "Copies a .ttf/.otf/.ttc/.otc font file into the project's assets/ folder so it travels with the presentation, and returns its family names for `update_elements` (`font`).",
            json!({ "path": path_prop(), "font": { "type": "string", "description": "The font file." } }), &["path", "font"], false),
        tool("import_pptx", "Import a PowerPoint file",
            "Converts a .pptx into a GlideDeck project: each slide becomes a frame with its text (with formatting), shapes, pictures, background and speaker notes, in slide order. Tables, charts and SmartArt are listed as skipped. Returns the slides like get_presentation.",
            json!({
                "pptx": { "type": "string", "description": "The .pptx file to import." },
                "path": { "type": "string", "description": "Project file to create (default: next to the .pptx, as .glide.json)." },
                "overwrite": { "type": "boolean", "default": false },
            }), &["pptx"], false),
        tool("arrange_slides", "Arrange all slides",
            "Moves every slide on the path into a new layout and path (each slide's content moves with it; the order is kept). Great after import_pptx, which arrives as a plain grid. Layouts: carousel (slides side by side gliding over a parallax backdrop), spiral, orbits, sunflower, ring, honeycomb, journey, wave, staircase, tree, zoom-tunnel, snake, grid.",
            json!({
                "path": path_prop(),
                "layout": { "type": "string", "description": "Arrangement id from `catalog`." },
                "spacing": { "type": "number", "default": 0.25, "description": "Gap between slides as a fraction of the slide size." },
                "tilt": { "type": "boolean", "description": "Turn slides along the path (default depends on the layout)." },
                "trail": { "type": "boolean", "default": true, "description": "Draw a dotted trail between slides (for carousel: the parallax backdrop)." },
                "overview": { "type": "boolean", "default": true, "description": "Overview of everything at the start and end." },
            }), &["path", "layout"], false),
        tool("animate", "Animate objects on a slide",
            "Adds in-slide animations (builds) to objects, played in order during the show: each `on_click` animation waits for the next click; `with_previous` starts together with the one before; `after_previous` starts when it ends. Kinds: appear (object hidden until it plays: fade, rise, focus, zoom, pop, drop, fly, spin, typewriter, words, count_up), emphasis (pulse, wiggle, glow, spotlight, highlight, underline, color_pop) and disappear (fade_out, sink, zoom_out, fly_out, shrink). Each object animates on the slide that shows it (or `step`). The first animation of a slide set to with/after_previous plays as soon as the camera arrives.",
            json!({
                "path": path_prop(),
                "animations": { "type": "array", "items": { "type": "object", "properties": {
                    "id": { "type": "string", "description": "Element id from get_presentation." },
                    "effect": { "type": "string", "description": "Effect id from `catalog` (effects)." },
                    "trigger": { "type": "string", "enum": ["on_click", "with_previous", "after_previous"], "default": "on_click" },
                    "duration": { "type": "number", "description": "Seconds (default per effect)." },
                    "delay": { "type": "number", "description": "Seconds after the trigger." },
                    "direction": { "type": "string", "enum": ["left", "right", "up", "down"], "description": "For fly / fly_out." },
                }, "required": ["id", "effect"] } },
                "step": { "type": "integer", "description": "1-based slide to put them on (default: the slide showing each object)." },
                "replace": { "type": "boolean", "default": false, "description": "Replace the existing animations of those slides." },
            }), &["path", "animations"], false),
        tool("auto_animate", "Auto-animate slides",
            "Gives slides a polished entrance cascade in reading order that plays as the camera arrives: titles focus, text rises, numbers count up, icons pop, pictures zoom. Replaces those slides' animations. Then add key-moment emphasis with `animate`.",
            json!({ "path": path_prop(), "steps": { "type": "array", "items": { "type": "integer" }, "description": "1-based slides (default: all)." } }), &["path"], false),
        tool("set_transitions", "Set slide transitions",
            "Sets how the camera travels into slides: smooth (zoom out, then in), direct, swoop (sideways arc with a roll), spin (barrel roll), punch (overshoot and settle), fade (dip through the background) or cut. `styles` cycles a list over the slides for variety.",
            json!({
                "path": path_prop(),
                "style": { "type": "string", "description": "One transition id from `catalog`." },
                "styles": { "type": "array", "items": { "type": "string" }, "description": "Transition ids to cycle over the slides." },
                "duration": { "type": "number", "description": "Seconds (default unchanged)." },
                "steps": { "type": "array", "items": { "type": "integer" }, "description": "1-based slides (default: all)." },
            }), &["path"], false),
        tool("set_notes", "Set speaker notes",
            "Sets the speaker notes of slides (Markdown: # heading, - bullet, **bold**). The presenter sees them in presenter view, next to the live slide; the audience doesn't.",
            json!({
                "path": path_prop(),
                "notes": { "type": "array", "items": { "type": "object", "properties": {
                    "step": { "type": "integer", "description": "1-based slide." }, "text": { "type": "string" },
                }, "required": ["step", "text"] } },
            }), &["path", "notes"], false),
        tool("clear_animations", "Remove animations",
            "Removes the in-slide animations of the given slides (default: all).",
            json!({ "path": path_prop(), "steps": { "type": "array", "items": { "type": "integer" } } }), &["path"], false),
        tool("export_pdf", "Export to PDF",
            "Exports the presentation to PDF: one page per slide on the path, rendered exactly as in the show. Defaults to the project's name with .pdf.",
            json!({ "path": path_prop(), "out": { "type": "string" }, "width": { "type": "integer", "default": 2560, "description": "Rendered slide width in pixels." } }), &["path"], false),
        tool("save_as", "Save a copy of the project",
            "Saves the project under a new name or folder (media files are copied along), e.g. to keep a version the user will edit later in GlideDeck.",
            json!({ "path": path_prop(), "new_path": { "type": "string" }, "title": { "type": "string" }, "overwrite": { "type": "boolean" } }), &["path", "new_path"], false),
        tool("open_in_editor", "Open in GlideDeck",
            "Opens the project in the GlideDeck editor on the user's computer so they can edit and present it.",
            json!({ "path": path_prop() }), &["path"], false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &mut Server, id: u64, method: &str, params: Value) -> Value {
        server.handle(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).expect("a reply")
    }

    fn tool_text(reply: &Value) -> String {
        assert_eq!(reply["result"]["isError"], json!(false), "{reply}");
        reply["result"]["content"][0]["text"].as_str().unwrap().to_owned()
    }

    #[test]
    fn handshake_and_tool_list() {
        let mut s = Server::new(std::env::temp_dir());
        let init = call(&mut s, 1, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }));
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(init["result"]["capabilities"]["tools"].is_object());
        assert!(s.handle(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none(), "notifications get no reply");
        let tools = call(&mut s, 2, "tools/list", json!({}));
        let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in ["catalog", "create_presentation", "add_slide", "update_elements", "render_slide", "export_pptx", "export_pdf", "export_html", "list_fonts", "add_font", "import_pptx", "arrange_slides", "animate", "auto_animate", "set_transitions", "set_notes", "clear_animations", "save_as", "open_in_editor"] {
            assert!(names.contains(&expected), "{expected} in {names:?}");
        }
        let unknown = call(&mut s, 3, "nope", json!({}));
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn catalog_ids_round_trip() {
        for t in Template::ALL {
            assert_eq!(find_template(&template_id(t)).unwrap(), t);
        }
        for k in LayoutKind::ALL {
            assert_eq!(find_layout(&layout_id(k)).unwrap(), k, "{}", layout_id(k));
            assert_eq!(find_layout(k.label()).unwrap(), k, "by label");
        }
        assert_eq!(layout_id(LayoutKind::HubSpokes), "hub-spokes");
        assert_eq!(layout_id(LayoutKind::ImageText), "image-text");
    }

    #[test]
    fn build_a_deck_end_to_end() {
        let dir = std::env::temp_dir().join(format!("glidedeck-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Server::new(dir.clone());
        let tools = |s: &mut Server, id, name: &str, args: Value| call(s, id, "tools/call", json!({ "name": name, "arguments": args }));

        let created = tools(&mut s, 1, "create_presentation", json!({ "path": "deck", "template": "blank", "title": "Q3 review" }));
        let summary: Value = serde_json::from_str(&tool_text(&created)).unwrap();
        assert_eq!(summary["slide_count"], 1);
        let title_id = summary["slides"][0]["elements"].as_array().unwrap().iter().find(|e| e["name"] == "Title").unwrap()["id"].as_str().unwrap().to_owned();
        tool_text(&tools(&mut s, 2, "update_elements", json!({ "path": "deck", "updates": [{ "id": title_id, "text": "Q3 in review" }] })));

        let added = tools(&mut s, 3, "add_slide", json!({
            "path": "deck", "layout": "three-points",
            "text": { "Heading": "What went well", "Card title": ["Speed", "Quality", "Reach"] },
            "icons": { "Icon 1": "rocket-launch" },
        }));
        let added: Value = serde_json::from_str(&tool_text(&added)).unwrap();
        assert_eq!(added["slides"], 2);
        let texts: Vec<&str> = added["added"]["elements"].as_array().unwrap().iter().filter_map(|e| e["text"].as_str()).collect();
        assert!(texts.contains(&"What went well") && texts.contains(&"Reach"), "{texts:?}");

        let bad = tools(&mut s, 4, "add_slide", json!({ "path": "deck", "layout": "venn", "text": { "Nope": "x" } }));
        assert_eq!(bad["result"]["isError"], true, "unknown fields are reported");

        tool_text(&tools(&mut s, 5, "add_text", json!({ "path": "deck", "text": "Footnote", "role": "caption", "y": 380 })));
        tool_text(&tools(&mut s, 6, "set_theme", json!({ "path": "deck", "theme": "midnight" })));
        let doc = Document::load(&dir.join("deck.glide.json")).unwrap();
        assert_eq!(doc.title, "Q3 review");
        assert_eq!(doc.theme.as_ref().unwrap().name, "Midnight");
        assert_eq!(doc.path.len(), 2);

        let pptx = tools(&mut s, 7, "export_pptx", json!({ "path": "deck" }));
        tool_text(&pptx);
        assert!(dir.join("deck.pptx").exists());

        let html: Value = serde_json::from_str(&tool_text(&tools(&mut s, 30, "export_html", json!({ "path": "deck" })))).unwrap();
        assert_eq!(html["slides"], 2);
        let page = std::fs::read_to_string(dir.join("deck.html")).unwrap();
        assert!(page.contains("Q3 review") && page.contains("<script"));

        let fonts: Value = serde_json::from_str(&tool_text(&tools(&mut s, 31, "list_fonts", json!({ "path": "deck" })))).unwrap();
        assert!(!fonts["fonts"].as_array().unwrap().iter().any(|f| f.as_str().unwrap().starts_with("Phosphor")));
        if let Some(family) = fonts["fonts"][0].as_str() {
            tool_text(&tools(&mut s, 32, "update_elements", json!({ "path": "deck", "updates": [{ "id": title_id, "font": family }] })));
            let doc = Document::load(&dir.join("deck.glide.json")).unwrap();
            let ElementKind::Text(t) = &doc.get(&title_id).unwrap().kind else { panic!() };
            assert_eq!(t.font_family, family);
        }
        let missing = tools(&mut s, 33, "update_elements", json!({ "path": "deck", "updates": [{ "id": title_id, "font": "No Such Font 123" }] }));
        assert_eq!(missing["result"]["isError"], true);

        tool_text(&tools(&mut s, 8, "save_as", json!({ "path": "deck", "new_path": "copies/deck-v2" })));
        assert!(dir.join("copies/deck-v2.glide.json").exists());

        // Animations and transitions.
        let anim = tools(&mut s, 20, "animate", json!({ "path": "deck", "animations": [
            { "id": title_id, "effect": "rise" },
            { "id": title_id, "effect": "spotlight", "trigger": "after_previous" },
        ] }));
        let anim: Value = serde_json::from_str(&tool_text(&anim)).unwrap();
        assert_eq!(anim["slides"][0]["animations"], 2);
        assert_eq!(anim["slides"][0]["clicks"], 1);
        let bad = tools(&mut s, 21, "animate", json!({ "path": "deck", "animations": [{ "id": title_id, "effect": "explode" }] }));
        assert_eq!(bad["result"]["isError"], true);
        tool_text(&tools(&mut s, 22, "auto_animate", json!({ "path": "deck", "steps": [2] })));
        tool_text(&tools(&mut s, 23, "set_transitions", json!({ "path": "deck", "styles": ["swoop", "punch"] })));
        let doc = Document::load(&dir.join("deck.glide.json")).unwrap();
        assert_eq!(doc.path[0].builds.len(), 2);
        assert!(doc.path[1].builds.len() >= 3, "the three-points slide cascades in: {:?}", doc.path[1].builds);
        assert_eq!(doc.path[1].builds[0].trigger, glide_core::Trigger::AfterPrevious, "plays on arrival");
        assert_eq!((doc.path[0].transition.style, doc.path[1].transition.style), (glide_core::TransitionStyle::Swoop, glide_core::TransitionStyle::Punch));
        let read: Value = serde_json::from_str(&tool_text(&tools(&mut s, 24, "get_presentation", json!({ "path": "deck" })))).unwrap();
        assert_eq!(read["slides"][0]["animations"][1]["effect"], "spotlight");
        assert_eq!(read["slides"][1]["transition"], "punch");
        tool_text(&tools(&mut s, 26, "set_notes", json!({ "path": "deck", "notes": [{ "step": 1, "text": "# Hello\n- **Smile**" }] })));
        tool_text(&tools(&mut s, 27, "update_elements", json!({ "path": "deck", "updates": [{ "id": title_id, "parallax": 0.4 }] })));
        let read: Value = serde_json::from_str(&tool_text(&tools(&mut s, 28, "get_presentation", json!({ "path": "deck" })))).unwrap();
        assert_eq!(read["slides"][0]["notes"], "# Hello\n- **Smile**");
        assert_eq!(Document::load(&dir.join("deck.glide.json")).unwrap().get(&title_id).unwrap().parallax, 0.4);
        let cat: Value = serde_json::from_str(&tool_text(&tools(&mut s, 25, "catalog", json!({})))).unwrap();
        assert!(cat["arrangements"].as_array().unwrap().iter().any(|a| a["id"] == "carousel"));
        assert!(cat["templates"].as_array().unwrap().iter().any(|t| t["id"] == "carousel"));
        assert_eq!(cat["effects"].as_array().unwrap().len(), 23);
        assert_eq!(cat["transitions"].as_array().unwrap().len(), 7);

        tool_text(&tools(&mut s, 9, "remove_slide", json!({ "path": "deck", "step": 2 })));
        let doc = Document::load(&dir.join("deck.glide.json")).unwrap();
        assert_eq!(doc.path.len(), 1);
        assert!(!doc.elements.iter().any(|e| matches!(&e.kind, ElementKind::Text(t) if t.content == "Reach")), "the slide's content went with it");

        if HeadlessRenderer::new().is_ok() {
            let r = tools(&mut s, 10, "render_slide", json!({ "path": "deck", "step": 1, "width": 320 }));
            assert_eq!(r["result"]["content"][1]["type"], "image");
            assert!(r["result"]["content"][1]["data"].as_str().unwrap().starts_with("iVBORw0KGgo"), "a base64 PNG");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn inline_markdown_becomes_spans() {
        let (text, spans) = inline_markdown("Grow **revenue** by *18%* and ~~cut~~ __costs__");
        assert_eq!(text, "Grow revenue by 18% and cut costs");
        let styled: Vec<&str> = spans.iter().map(|s| &text[s.start..s.end]).collect();
        assert_eq!(styled, ["revenue", "18%", "cut", "costs"]);
        assert_eq!(spans[0].style.bold, Some(true));
        assert_eq!(spans[1].style.italic, Some(true));
        assert!(spans[2].style.strike && spans[3].style.underline);
        // Lone markers and snake_case stay literal.
        let (text, spans) = inline_markdown("2 * 3 = 6 in my_var_name");
        assert_eq!(text, "2 * 3 = 6 in my_var_name");
        assert!(spans.is_empty());
        // Nesting.
        let (text, spans) = inline_markdown("**bold *both* bold**");
        assert_eq!(text, "bold both bold");
        let both = spans.iter().find(|s| &text[s.start..s.end] == "both").unwrap();
        assert_eq!((both.style.bold, both.style.italic), (Some(true), Some(true)));
    }
}
