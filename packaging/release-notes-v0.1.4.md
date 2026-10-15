## GlideDeck 0.1.4

**Arrange slides: turn any deck, including an imported PowerPoint, into a zooming story.**

### Install
- **Windows 10/11 (x64):** download **`GlideDeck-0.1.4-x64.msi`** below and run it. It upgrades earlier versions in place. `GlideDeck-x64.msi` is the same file.
- **macOS (Apple Silicon):** the DMG linked from the README is still 0.1.3; it doesn't include Arrange slides yet.

> This preview MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

### New
- **Arrange slides** (Design → Arrange, or the path button above the slide list) moves every slide into a new layout and path. Each slide's content moves and turns with it, and the order is kept.
  - **Spiral**: winds outward, slides turning with the curve.
  - **Orbits**: the first slide at the center, the rest on rings.
  - **Sunflower**: golden-angle seeds; every step swings around the center.
  - **Ring**: one circle, the camera spinning around it.
  - **Honeycomb**: hex rings spiralling out.
  - **Journey** and **Wave**: meandering and sine-wave roads that wrap into rows.
  - **Staircase**: climbing in switchback flights.
  - **Tree**: branches like an org chart, visited branch by branch.
  - **Zoom tunnel**: each slide nested inside the previous one.
  - **Snake** and **Grid**: rows.
- **Works for any number of slides.** Every layout is a formula, and is tested to never overlap slides, from 1 slide up to 120.
- **Options**: spacing, turning slides along the path (the show's camera turns with them), a dotted trail (tree lines for Tree), and an overview at the start and end.
- **Live previews**: the dialog draws every arrangement for your actual number of slides. One step to undo.
- **After a PowerPoint import**, GlideDeck suggests arranging the slides.
- **CLI and MCP**: `glidedeck arrange deck.glide.json --layout orbits`, and an `arrange_slides` tool for Claude and Copilot (listed in `catalog`).

### Checksums
SHA-256 of the MSI is listed in `GlideDeck-0.1.4-x64.msi.sha256`.
