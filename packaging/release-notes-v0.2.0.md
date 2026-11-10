## GlideDeck 0.2.0

**A new look and a new window: tools on the left, details on the right, your path along the bottom. Plus command search, a house style, web page export, any font on your computer, web videos and AutoSave.**

### Install
- **Windows 10/11 (x64):** download **`GlideDeck-0.2.0-x64.msi`** below and run it. It upgrades earlier versions in place. `GlideDeck-x64.msi` is the same file under a name that never changes.
- **macOS (Apple Silicon and Intel):** download **`GlideDeck-0.2.0.dmg`** below, open it and drag GlideDeck into **Applications**. The universal app and DMG are Developer ID-signed, and the DMG is notarized by Apple with the ticket stapled. `GlideDeck.dmg` is the same file under a name that never changes.

> The MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

### A new window
- **The ribbon is gone.** The window now has four parts:
  - a **top bar** with the File, Edit, Insert and View menus, Undo and Redo, Share and Present;
  - a **tool rail** on the left: Select, Hand, Frame, Layouts, Text, shapes, Picture, Icons, Video and Templates;
  - a **side panel** on the right with three tabs: **Slide** (camera move, the slide's animations, speaker notes), **Design** (the selection's properties, or the deck's theme, fonts and background) and **Animate**;
  - the **path strip** along the bottom: every stop as a thumbnail, with the camera move and its duration between each pair. Drag to reorder; right-click for more.
- **Command search.** Press **Ctrl/⌘+K**, type a few letters and run any command, or jump to a slide by its name or its text.
- **Dark and light.** The half-circle button in the top bar switches between a deep navy and a clean white scheme. The layout is the same in both.
- **Share menu.** Web page, PowerPoint, PDF and PNG exports are together under **Share**.
- Speaker notes moved into the **Slide** tab. The status bar is gone: **Saved** shows next to the deck name, and short messages appear over the canvas.

### A house style
- **Two new themes:** **Darkroom** (flat graphite, bone-white type, one signal yellow) and **Daylight** (cool white with a cobalt accent). There are now twelve themes.
- **New presentations start in Darkroom.** Switch in the Design tab with one click.
- **A new welcome canvas** in Darkroom: four tilted slabs the camera straightens as it flies in, a full-width wordmark, and a yellow portal that zooms a million times deeper.
- **A new app icon.**

### Also new since 0.1.6
- **Web page export.** Share → Web page (HTML)… writes one `.html` file that plays the show in any browser exactly as GlideDeck does: camera moves, animations, parallax and effects.
- **Any font on your computer.** The font picker lists every installed font in its own face. **Add font file…** copies a font into the presentation so it travels with it.
- **Web videos.** Paste a YouTube, Vimeo or Loom link onto the canvas. The video sits on the slide with its poster and plays in the web page export; with yt-dlp installed it plays inside GlideDeck too.
- **AutoSave and crash recovery.** A presentation that has a file is saved a moment after each change. Unsaved work is kept and offered back after a crash.
- Hebrew and Arabic show in reading order in the side panel fields.

### For Claude and GitHub Copilot
- The MCP server and the skill now follow the **house style**. `catalog` returns it under `house_style` and marks the two themes.
- `create_presentation` with no template starts in Darkroom. Pass `theme: "darkroom"` or `"daylight"` with a template to build it in the house style from the start.
- Labels on light accent colors (such as the yellow) use dark ink, so they stay readable.
- New tools since 0.1.6: `export_html`, `add_web_video`, `list_fonts`, `add_font`.

### Checksums
SHA-256 of each installer is listed in the `.sha256` file next to it.
