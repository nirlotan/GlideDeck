## GlideDeck 0.1.1

**More templates and layouts, a fix for faded images, and pasting from PowerPoint.**

### Install (Windows 10/11, x64)
1. Download **`GlideDeck-0.1.1-x64.msi`** below and run it. It upgrades 0.1.0 in place.
2. *(Optional, for video)* install FFmpeg: `winget install Gyan.FFmpeg`.

> This preview MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

### New
- **17 templates** (was 5), grouped by Business, Storytelling, Ideas & maps and Teaching. The 12 new ones use structures only a zooming canvas can do:
  - **Orbit**: planets around a sun.
  - **Summit**: camps up a mountain.
  - **Roadmap**: quarters × teams.
  - **Hive**: a honeycomb.
  - **Deep dive**: slides nested in portholes.
  - **Showcase**: a bento grid of slides.
  - **Branches**: a tree of topics.
  - **Spiral**: slides along a spiral.
  - **Cycle**: a turning wheel.
  - **Mood board**: tilted polaroids.
  - **Workshop**: sticky notes.
  - **Chapters**: a bookshelf.
- **20 slide layouts** (was 8). New: Comparison, Quadrants, Bento grid, Dashboard, Team, Hub & spokes, Cycle, Venn, Pyramid, Process, Staircase and Winding road.
- Template and layout galleries are now grouped and scrollable, and template previews show each story's overview.
- **Paste from PowerPoint** (Windows): copied shapes arrive as editable text boxes, rectangles, ellipses and pictures, keeping their layout, fonts and colours.
- **Ctrl/⌘+Shift+V** pastes the clipboard's text as plain text, even when a picture came with it.
- `glidedeck new --template` accepts all 17 templates.

### Fixed
- Images and videos rendered at about half opacity regardless of their Opacity setting. They now draw at full strength, and rounded image corners work as intended.

### Checksums
SHA-256 of the MSI is listed in `GlideDeck-0.1.1-x64.msi.sha256`.
