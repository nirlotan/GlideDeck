//! `glidedeck agent-setup`: installs the GlideDeck skill and registers the MCP
//! server for Claude Code, GitHub Copilot (VS Code and CLI) in one step.
//!
//! Config files are merged, never replaced: existing servers are kept, and a
//! file that cannot be parsed (for example JSON with comments) is left alone
//! with the snippet printed instead.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const SKILL_MD: &str = include_str!("../../../skills/glidedeck/SKILL.md");

/// What happened to one file.
pub struct Outcome {
    pub path: PathBuf,
    pub note: String,
}

fn exe() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "glidedeck".into())
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

fn write_skill(skills_dir: &Path) -> Result<Outcome> {
    let dir = skills_dir.join("glidedeck");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("SKILL.md");
    std::fs::write(&path, SKILL_MD).with_context(|| format!("writing {}", path.display()))?;
    Ok(Outcome { path, note: "skill installed".into() })
}

/// Adds `glidedeck` under `key` ("mcpServers" or "servers") in a JSON config.
fn merge_server(path: &Path, key: &str, entry: Value) -> Result<Outcome> {
    let mut root = if path.exists() {
        let text = std::fs::read_to_string(path)?;
        match serde_json::from_str::<Value>(&text) {
            Ok(v) if v.is_object() => v,
            _ => {
                return Ok(Outcome {
                    path: path.to_path_buf(),
                    note: format!("not changed (could not parse it); add this under \"{key}\":\n    \"glidedeck\": {entry}"),
                });
            }
        }
    } else {
        json!({})
    };
    let servers = root.as_object_mut().expect("object").entry(key).or_insert_with(|| json!({}));
    let Some(servers) = servers.as_object_mut() else {
        return Ok(Outcome { path: path.to_path_buf(), note: format!("not changed (\"{key}\" is not an object)") });
    };
    let existed = servers.insert("glidedeck".into(), entry).is_some();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&root)? + "\n").with_context(|| format!("writing {}", path.display()))?;
    Ok(Outcome { path: path.to_path_buf(), note: if existed { "MCP server updated".into() } else { "MCP server added".into() } })
}

/// Sets up one project folder:
/// - `.claude/skills/glidedeck/SKILL.md` (Claude Code) and
///   `.github/skills/glidedeck/SKILL.md` (GitHub Copilot). Newer Copilot
///   versions also read `.claude/skills`; the copies are identical.
/// - `.mcp.json`: project MCP servers for Claude Code and Copilot CLI.
/// - `.vscode/mcp.json`: MCP servers for Copilot in VS Code.
pub fn setup_project(dir: &Path, skill: bool, mcp: bool) -> Result<Vec<Outcome>> {
    let mut out = Vec::new();
    if skill {
        out.push(write_skill(&dir.join(".claude").join("skills"))?);
        out.push(write_skill(&dir.join(".github").join("skills"))?);
    }
    if mcp {
        let exe = exe();
        out.push(merge_server(&dir.join(".mcp.json"), "mcpServers", json!({ "type": "stdio", "command": exe, "args": ["mcp"] }))?);
        out.push(merge_server(&dir.join(".vscode").join("mcp.json"), "servers", json!({ "type": "stdio", "command": exe, "args": ["mcp"] }))?);
    }
    Ok(out)
}

/// Sets up the current user:
/// - `~/.claude/skills/glidedeck/SKILL.md` and `~/.copilot/skills/glidedeck/SKILL.md`:
///   personal skill for Claude Code and Copilot.
/// - `~/.copilot/mcp-config.json`: MCP server for Copilot CLI.
pub fn setup_user(skill: bool, mcp: bool) -> Result<Vec<Outcome>> {
    let home = home().context("could not find your home folder")?;
    let mut out = Vec::new();
    if skill {
        out.push(write_skill(&home.join(".claude").join("skills"))?);
        out.push(write_skill(&home.join(".copilot").join("skills"))?);
    }
    if mcp {
        out.push(merge_server(&home.join(".copilot").join("mcp-config.json"), "mcpServers", json!({ "type": "local", "command": exe(), "args": ["mcp"], "tools": ["*"] }))?);
    }
    Ok(out)
}

/// The commands for clients whose user-level config is best changed by the
/// client itself.
pub fn user_hints() -> String {
    let exe = exe();
    let json_exe = serde_json::to_string(&exe).unwrap_or_default();
    format!(
        "Claude Code (all projects):\n  claude mcp add --scope user glidedeck -- \"{exe}\" mcp\n\n\
         Claude Desktop: Settings → Developer → Edit Config, then add to \"mcpServers\":\n  \"glidedeck\": {{ \"command\": {json_exe}, \"args\": [\"mcp\", \"--root\", \"<folder for presentations>\"] }}\n\n\
         VS Code (all workspaces): run \"MCP: Open User Configuration\" and add to \"servers\":\n  \"glidedeck\": {{ \"type\": \"stdio\", \"command\": {json_exe}, \"args\": [\"mcp\"] }}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_without_losing_other_servers() {
        let dir = std::env::temp_dir().join(format!("glidedeck-agents-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".vscode")).unwrap();
        std::fs::write(dir.join(".vscode/mcp.json"), r#"{"servers": {"other": {"command": "x"}}, "inputs": []}"#).unwrap();
        let out = setup_project(&dir, true, true).unwrap();
        assert_eq!(out.len(), 4);
        assert!(dir.join(".github/skills/glidedeck/SKILL.md").exists());
        let vs: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".vscode/mcp.json")).unwrap()).unwrap();
        assert_eq!(vs["servers"]["other"]["command"], "x", "existing servers are kept");
        assert_eq!(vs["servers"]["glidedeck"]["args"], json!(["mcp"]));
        assert!(vs["inputs"].is_array());
        let claude: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
        assert!(claude["mcpServers"]["glidedeck"]["command"].is_string());
        let skill = std::fs::read_to_string(dir.join(".claude/skills/glidedeck/SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: glidedeck\n"));

        // JSON with comments is left untouched.
        std::fs::write(dir.join(".mcp.json"), "{ // mine\n}").unwrap();
        let out = setup_project(&dir, false, true).unwrap();
        assert!(out[0].note.starts_with("not changed"));
        assert_eq!(std::fs::read_to_string(dir.join(".mcp.json")).unwrap(), "{ // mine\n}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
