## GlideDeck 0.1.3

**Import PowerPoint, export PDF, presenter view on a second screen, and rich text.**

### Install (Windows 10/11, x64)
Download **`GlideDeck-0.1.3-x64.msi`** below and run it. It upgrades earlier versions in place. `GlideDeck-x64.msi` is the same file.

> This preview MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

### New
- **Import PowerPoint** (File → Import PowerPoint, or open or drop a `.pptx`):
  - every slide becomes a frame, laid out in a grid and in order on the path;
  - brought over: text with its formatting, shapes, pictures, slide backgrounds, master logos and lines, and speaker notes;
  - placeholders inherit positions, sizes and bullets from the layout and master;
  - tables, charts and SmartArt are listed as skipped.
- **Export PDF** (File → Export PDF, or Present → PDF): one page per slide, rendered exactly as in the show.
- **File → Open recent**: the last 10 documents you opened, saved or imported.
- **Presenter view**:
  - with a second display, F5 puts the show fullscreen on the projector;
  - your screen shows the live slide, the next slide, speaker notes, a timer and the clock, plus black and white screen (B/W) and Swap displays;
  - Alt+F5 opens it on a single display, with the audience view in its own window;
  - `glidedeck present FILE --presenter-view` opens a deck straight into the show.
- **Rich text**:
  - bold, italic, underline, strikethrough, colour and size for any word or phrase;
  - a floating format bar while editing on the canvas, plus Ctrl/⌘+B/I/U, Ctrl/⌘+Shift+X, Ctrl/⌘+] and [, and Ctrl/⌘+\;
  - formatting exports to PowerPoint and survives pasting from it.
- **Text wrap handles**: drag a text box's side handles to change its width; the text reflows without scaling. Shapes and frames can be stretched from any side.
- **CLI and MCP**: `glidedeck import-pptx` and `glidedeck export-pdf`, plus `import_pptx` and `export_pdf` tools for Claude and Copilot. `update_elements` accepts `markdown: true` for **bold** and *italic*.

### Fixed
- Delete / Backspace sometimes did nothing after editing text on the canvas.
- Resize cursors now point along each corner's diagonal, and stay correct while dragging.
- Drawing a frame keeps its starting corner in place instead of growing around its center.

### Checksums
SHA-256 of the MSI is listed in `GlideDeck-0.1.3-x64.msi.sha256`.
