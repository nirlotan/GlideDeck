# Use GlideDeck with Claude and GitHub Copilot

GlideDeck includes an [MCP](https://modelcontextprotocol.io) server and an agent
skill, so an AI assistant can build presentations for you. It picks a template,
writes every slide, renders each one to check it, and saves a GlideDeck project
you can open, edit and present, or export to PowerPoint.

- **MCP server.** `glidedeck mcp` is built into the app, so there's nothing
  else to install. It runs locally over stdio and never touches the network.
- **Skill.** `SKILL.md` teaches the assistant a good workflow: plan the
  story, choose a template, replace every placeholder, render and check each
  slide. The same file works for Claude Code and GitHub Copilot.
- **Projects.** Every change is saved to a `.glide.json` project file, with
  images copied to `assets/` next to it. Open it in GlideDeck any time to keep
  editing.

## Quick setup (any project folder)

Install GlideDeck (see the README), open a terminal in the folder where you
want your presentations, and run:

```bash
glidedeck agent-setup
```

That writes:

| File | Used by |
|---|---|
| `.claude/skills/glidedeck/SKILL.md` | Claude Code (Copilot reads it too) |
| `.github/skills/glidedeck/SKILL.md` | GitHub Copilot (VS Code, Copilot CLI, coding agent) |
| `.mcp.json` | Claude Code and Copilot CLI |
| `.vscode/mcp.json` | Copilot in VS Code |

Existing config files are merged and your other servers are kept. A file
GlideDeck can't parse, such as JSON with comments, is left alone, and the
snippet to add is printed instead.

To set up the skill for every project at once, plus Copilot CLI's MCP config,
run this; it also prints the commands for the remaining clients:

```bash
glidedeck agent-setup --user
```

Then ask, for example:

> Create a 7-slide GlideDeck presentation about our Q3 results in `q3.glide.json`:
> revenue up 18%, two new enterprise customers, hiring plan for Q4. Use the roadmap
> template, check every slide, and export a PowerPoint copy.

## GitHub Copilot in VS Code

1. **Register the server.** Run `glidedeck agent-setup` in your workspace
   folder, or create `.vscode/mcp.json` yourself:

   ```json
   {
     "servers": {
       "glidedeck": { "type": "stdio", "command": "glidedeck", "args": ["mcp"] }
     }
   }
   ```

   If VS Code can't find `glidedeck` (it reads PATH when it starts), use the
   full path. After an MSI install that's
   `C:\Program Files\GlideDeck\glidedeck.exe` (in JSON, write each `\` as
   `\\`).
2. **Start it.** Open `.vscode/mcp.json`. VS Code shows a **Start** link above
   the server; click it, or run **MCP: List Servers** from the Command
   Palette, and trust the server when asked.
3. **Chat.** Open Copilot Chat and switch the mode to **Agent**; MCP tools
   only run in agent mode. Under the 🛠 **Tools** button you should see the
   `glidedeck` tools. Ask for a presentation.
4. **Skill.** Copilot picks up `.github/skills/glidedeck/SKILL.md`
   automatically when your request matches its description. Agent Skills
   arrived in VS Code 1.108 as a preview. If the skill isn't used, enable the
   **Chat: Use Agent Skills** setting (`chat.useAgentSkills`).

Copilot asks before running each tool. You can allow the GlideDeck tools for the
session or the workspace.

**For all workspaces:** run **MCP: Open User Configuration** and add the same
`glidedeck` entry under `"servers"`.

## GitHub Copilot CLI

`glidedeck agent-setup` writes `.mcp.json`, which Copilot CLI reads from your
working directory up to the repository root. For every directory, use
`glidedeck agent-setup --user` (writes `~/.copilot/mcp-config.json`) or:

```bash
copilot mcp add glidedeck -- glidedeck mcp
```

The skill is read from `.github/skills/` in the project or `~/.copilot/skills/`
for your user. Start `copilot` in the folder and ask for a presentation.

## Claude Code

`glidedeck agent-setup` writes `.mcp.json` (project scope) and
`.claude/skills/glidedeck/SKILL.md`. Restart Claude Code in that folder and
approve the `glidedeck` server when asked. Check it with:

```bash
claude mcp list
```

To use it from every folder:

```bash
claude mcp add --scope user glidedeck -- glidedeck mcp
```

Install the skill for your user with `glidedeck agent-setup --user --no-mcp`.

## Claude Desktop

Open **Settings → Developer → Edit Config** and add:

```json
{
  "mcpServers": {
    "glidedeck": {
      "command": "C:\\Program Files\\GlideDeck\\glidedeck.exe",
      "args": ["mcp", "--root", "C:\\Users\\YOU\\Documents\\GlideDeck"]
    }
  }
}
```

On macOS use `/Applications/GlideDeck.app/Contents/MacOS/GlideDeck` and a folder
such as `/Users/YOU/Documents/GlideDeck`. `--root` is where projects go when you
give a bare name like `q3-review`. It's created if missing.

Restart Claude Desktop. The GlideDeck tools appear under the tools button in a
new chat.

Skills in Claude Desktop are uploaded rather than read from disk. To add this
one, zip `skills/glidedeck` (or the `glidedeck` folder `agent-setup` created)
and add it under **Settings → Capabilities → Skills**. It isn't required; the
MCP server also tells the assistant the workflow when it connects.

## The tools

| Tool | What it does |
|---|---|
| `catalog` | Templates, slide layouts (with their text fields and icon slots) and themes |
| `create_presentation` | New project from a template; returns every slide's element ids and placeholder text |
| `get_presentation` | Slides in show order, with the text, icons and images on each |
| `add_slide` | Adds a layout slide and fills its text fields and icons |
| `update_elements` | Changes text, icons or colors by id |
| `add_text` / `add_image` | Adds a text box or an image (copied into `assets/`) to a slide |
| `set_theme` | Restyles the whole deck |
| `remove_slide` | Removes a slide and its content |
| `render_slide` | Renders a slide to PNG and returns the image, so the assistant can check its work |
| `export_pptx` | PowerPoint export with Morph transitions |
| `export_pdf` | PDF, one page per slide |
| `import_pptx` | Converts a PowerPoint file into a GlideDeck project (slides, text, pictures, notes) |
| `arrange_slides` | Rearranges every slide into a spiral, orbits, tree, zoom tunnel… with a matching path |
| `set_transitions` | Camera moves per slide: smooth, swoop, spin, punch, fade, direct or cut (or a cycle of them) |
| `auto_animate` | Gives slides an entrance cascade in reading order that plays on arrival |
| `animate` | Adds appear, emphasis (spotlight, highlight, count up…) and disappear animations by element id |
| `clear_animations` | Removes slides' animations |
| `save_as` | Saves a copy or version of the project (media included) |
| `open_in_editor` | Opens the project in the GlideDeck editor |
| `search_icons` | Finds names among ~1,500 Phosphor icons |
| `edit_presentation` | Low-level atomic edit operations, for anything else |

Paths are absolute or relative to the server's folder (the workspace, or
`--root`). `.glide.json` is added when there is no extension.

## Outstanding by default

The server's instructions and the skill tell the assistant to finish every deck with
GlideDeck's motion features, unless you ask for a plain one:

1. **A path with a shape.** `arrange_slides` with a layout that mirrors the story: orbits
   or a tree for a hub and its topics, a spiral or staircase for growth, a zoom tunnel for
   drilling down.
2. **Varied camera moves.** `set_transitions`, e.g. cycling smooth, swoop and punch.
3. **Builds.** `auto_animate` on every slide, then `animate` for the key moments: a
   spotlight on the number that matters, a highlight on a phrase, count-up metrics, a
   quote revealed word by word.

To get a quieter deck, say so in your request ("no animations", "simple transitions").

## Editing afterwards

The project the assistant writes is a normal GlideDeck project. Double-click it,
use **File → Open**, or ask the assistant to call `open_in_editor`. From there,
edit anything, rearrange the path in the slide strip, and press **F5** to
present.

The editor doesn't reload a file that changed on disk. If you're editing while
the assistant works, reopen the file to see its latest changes. Use `save_as` (or
**File → Save As**) to keep separate versions.

## Troubleshooting

- **The server doesn't start, or no tools appear.** Run `glidedeck mcp` in a
  terminal. It should wait silently for input (Ctrl+C to quit). If the command
  isn't found, use the full path to `glidedeck.exe` in the config.
- **"no GPU available for rendering".** `render_slide` needs a GPU or a
  software renderer. Every other tool still works.
- **Relative paths land in an unexpected folder.** Pass absolute paths, or set
  `--root`.
