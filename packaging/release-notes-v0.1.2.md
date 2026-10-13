## GlideDeck 0.1.2

**Let Claude and GitHub Copilot build your presentations.**

### Install (Windows 10/11, x64)
1. Download **`GlideDeck-0.1.2-x64.msi`** below and run it. It upgrades earlier versions in place.
2. Open a terminal in the folder where you want your presentations and run `glidedeck agent-setup`.
3. Ask your assistant for a presentation. The full guide is in [docs/AI.md](https://github.com/nirlotan/GlideDeck/blob/main/docs/AI.md).

> This preview MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

### New
- **MCP server** (`glidedeck mcp`), built in and local-only, with 15 tools:
  - create from any of the 17 templates;
  - add any of the 20 layouts with their text and icons filled in;
  - update text, icons and colours; add text and images; change the theme; remove slides;
  - render slides to PNG so the assistant can check its work;
  - export to PowerPoint, save a copy, and open the project in the editor.
- **Agent skill** (`SKILL.md`) that teaches Claude and Copilot a good workflow: plan the story, choose a template, replace every placeholder, and render and check each slide.
- **`glidedeck agent-setup`** installs the skill and registers the server for Claude Code, Copilot CLI and VS Code in one step. It merges into existing config files and keeps your other servers. `--user` sets it up for all projects.
- Everything the assistant builds is a regular `.glide.json` GlideDeck project you can open, edit and present later.

### Checksums
SHA-256 of the MSI is listed in `GlideDeck-0.1.2-x64.msi.sha256`.
