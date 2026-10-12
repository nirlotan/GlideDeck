## GlideDeck 0.1.0 — first preview

**Zooming presentations, built natively.** An infinite canvas you glide across: local-first, GPU-accelerated, written in Rust. No accounts, no cloud, no telemetry.

### Install (Windows 10/11, x64)
1. Download **`GlideDeck-0.1.0-x64.msi`** below and run it.
2. Launch **GlideDeck** from the Start menu. The welcome canvas is itself a presentation, so press **F5**.
3. *(Optional, for video)* install FFmpeg: `winget install Gyan.FFmpeg`.

The installer adds Start-menu and (optional) desktop shortcuts and puts the `glidedeck` CLI on your `PATH`. Uninstall from **Settings → Apps**.

> This preview MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

macOS: build a DMG with `packaging/macos/build-dmg.sh` (see the README).

### Highlights
- **Infinite canvas** with smooth zoom, pan and rotation, precise a million times deep. Text, shapes, icons and SVGs are true vectors.
- **Frames are slides**, invisible by default, with a PowerPoint-style slide strip, **cinematic smooth-zoom transitions** and fullscreen presenting.
- **Design built in**: 5 templates, 8 themes, 8 slide layouts, text styles, ~1,500 Phosphor icons, shadows, gradients, smart guides, align and distribute, and a **design check** with one-click fixes.
- **Rich content**: images (paste, drag and drop), SVG, video with sound, bullets and numbering, and in-place text editing with full **right-to-left** (Hebrew, Arabic) support.
- **Export to PowerPoint** with **Morph** transitions (object identity preserved), plus PNG export.
- **Agent-ready CLI**: create from templates, apply atomic JSON ops, render slides headlessly, export PPTX.

### Checksums
SHA-256 of the MSI is listed in `GlideDeck-0.1.0-x64.msi.sha256`.
