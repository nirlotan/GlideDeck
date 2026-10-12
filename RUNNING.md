# Running GlideDeck on Windows and macOS

GlideDeck is a pure-Rust desktop app. It needs no runtime, browser engine, account or
network connection: once it's built, it runs fully offline. The only network access
ever involved is Cargo downloading crate sources during the **first build**.

---

## 1. Prerequisites

| | Windows 10/11 (x64 or ARM64) | macOS 12 Monterey or newer (Apple Silicon or Intel) |
|---|---|---|
| Rust toolchain | [rustup](https://rustup.rs) → `rustup-init.exe`, accept defaults (stable, MSVC host) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| C/C++ linker | **Visual Studio 2022 Build Tools** with the *Desktop development with C++* workload (rustup offers to install it) | **Xcode Command Line Tools**: `xcode-select --install` |
| GPU API | DirectX 12 (default) or Vulkan; any GPU from the last ~10 years | Metal (built in) |
| Minimum Rust | 1.88 or newer (`rustup update stable`) | same |

Check your setup:

```bash
rustc --version
```

```bash
cargo --version
```

**Optional: FFmpeg for video.** GlideDeck plays embedded videos with a locally installed
FFmpeg (it is run as a subprocess, so nothing is linked at build time). Everything else
works without it.

```bash
winget install Gyan.FFmpeg
```

```bash
brew install ffmpeg
```

GlideDeck looks for `ffmpeg`/`ffprobe` next to its own executable, then on `PATH` (and in
Homebrew's folders on macOS). Set `GLIDEDECK_FFMPEG` and `GLIDEDECK_FFPROBE` to point
somewhere else.

---

## 2. Build and launch the editor

From the repository root (the folder containing this file):

```bash
cargo run --release -p glidedeck
```

The first build compiles about 300 crates and takes a few minutes; later builds are
incremental. With no arguments, the app opens the built-in welcome canvas.

Open a specific document:

```bash
cargo run --release -p glidedeck -- examples/demo.glide.json
```

The compiled binary is written to:

- Windows: `target\release\glidedeck.exe`
- macOS: `target/release/glidedeck`

You can copy it anywhere and run it directly. It has no external file dependencies.
On Windows the release build is a GUI-subsystem app (no console window when it's
double-clicked); CLI subcommands still print to the terminal that launched them.

### macOS notes

- **Gatekeeper:** a binary you build yourself isn't quarantined, so it runs directly.
  If you copy a build to another Mac and macOS blocks it, right-click it, choose
  **Open**, then confirm. You can also clear the flag:
  `xattr -d com.apple.quarantine ./glidedeck`.
- **Trackpads:** pinch to zoom works out of the box. Two-finger scrolling zooms by
  default, matching a mouse wheel. To make two-finger scroll pan instead, turn on
  **View → Trackpad Mode** (⌘-scroll then zooms).
- Shortcuts use **⌘** wherever this guide says Ctrl.

### Windows notes

- If the window opens blank or the app reports *"wgpu unavailable"*, update your GPU
  driver. To force a specific backend, set `WGPU_BACKEND=vulkan` or `WGPU_BACKEND=dx12`
  before launching (PowerShell: `$env:WGPU_BACKEND="vulkan"`).
- Per-monitor high-DPI scaling is handled automatically.

---

## 3. Using the editor

The window has a PowerPoint-style **ribbon** (File · Home · Insert · Design · Transitions ·
Present · View), the **Slides** strip on the left, the canvas in the middle and **Properties** on the
right. The app follows your system's light/dark setting; the sun/moon button switches it.

### Slides are frames

- **Insert → Frame** (`F`), then drag on the canvas to draw a 16:9 frame (hold Alt for any
  aspect ratio). Click instead of dragging to get a default-sized frame. **Frame view**
  (Ctrl/⌘+Shift+F, or `+` in the Slides strip) frames exactly what you're looking at.
- Each new frame is added to the presentation path and appears as a slide thumbnail.
  Drag thumbnails to reorder them; right-click for Duplicate, Move, Remove from path, or
  Present from here. Click a thumbnail to fly there; PageUp/PageDown step through slides.
- Frames are **invisible** by default. In the editor they only show a dashed outline and a
  name tag when you hover over or select them, and they never appear in the show or the
  export. Set *Properties → Frame → Border* or *Filled* to make one visible.
- Moving a frame moves everything inside it, so a frame works as a logical group.
  Select several objects and choose **Frame them** to wrap them in a new slide.
- The **Transitions** tab (or Properties → Slide) sets how the camera enters each slide:
  **Smooth zoom** (a cinematic zoom-out-and-dive arc), **Direct** or **Cut**, plus the
  duration and easing. Speaker notes (Markdown) go in Properties → Slide.

### Design tools

- **Templates** (File → New from template, or Design → Templates): complete zooming stories,
  each with a theme, frames and a path. Choose from a pitch deck, a journey along rotated
  frames, a big-picture mind map, a lecture with nested detail, or a blank canvas. The
  previews are live renders.
- **Themes** (Design tab): eight curated colour and font pairings, some with gradient
  backgrounds. Switching theme restyles text by its role, and any colour taken from the
  previous theme moves to the matching colour in the new one. Colours you picked by hand
  are left alone. Theme colours also appear as one-click swatches next to every colour
  picker.
- **Slide layouts** (Insert → Layouts, Ctrl/⌘+Shift+L): Title, Section, Three points, Big
  number, Quote, Agenda, Timeline and Image + text. Each one is added as a new frame next to
  the current slide, styled by the theme, and added to the path.
- **Icons** (Insert → Icons, Ctrl/⌘+Shift+K): about 1,500 Phosphor icons in Light, Regular,
  Bold and Fill weights, with search and categories. Icons are true vectors: crisp at any
  zoom, recolourable, and exported to PowerPoint as editable vector shapes.
- **Text styles**: Insert → Title / Heading / Body, or select text and pick a style on Home
  (Title, Heading, Subtitle, Body, Caption, Quote). Styled text follows theme changes.
- **Smart guides**: while dragging, edges and centres snap to nearby objects and red guide
  lines appear. Hold Alt to move freely. **Align & distribute** is on Home → Align; with a
  single object selected, it aligns to the frame that contains it.
- **Effects** (Design tab or Properties → Effects): soft, lifted or crisp drop shadows,
  two-colour gradient fills, and rounded or circular masks for images.
- **Design check** (Design → Design check): flags low-contrast text (WCAG), text too small to
  read on its slide, empty slides, missing or low-resolution images, and too many fonts.
  Each issue has a *Show* button, and most have a one-click *Fix* (or *Fix all*).

### Presenting

**Present** (F5 from the start, Shift+F5 from the current slide) goes fullscreen.
→ / Space / PageDown / click advance; ← / PageUp / right-click go back; Home / End jump;
Esc ends the show. The mouse wheel still zooms freely, and the next step returns to the
path. Presenter view on a second screen is planned for the next milestone.

### Editing text in place

Double-click a text box (or select it and press **Enter** / **F2**) to edit it directly on
the canvas. Editing uses the same text engine that draws it, so Hebrew, Arabic and mixed
right-to-left / left-to-right text, word motion (Ctrl/⌥+arrows), selection
(Shift+arrows, drag, double-click a word), clipboard and IME input all behave as in a word
processor, at any zoom or rotation. **Esc** or clicking outside finishes; an emptied text
box is removed. The multi-line field in Properties also edits text, but it shows
right-to-left words in left-to-right order, so use the canvas for those scripts.

**Bullets and numbering:** select a text box, then use the list buttons in Home → Text
format (or Properties → List). Each paragraph gets a marker with a proper hanging indent,
placed on the right for right-to-left paragraphs. Lists export to PowerPoint as real
bullets.

### Resizing and rotating

Select a single object to show its handles. Drag a **corner** to resize it uniformly from
the opposite corner, so images, icons and SVGs never distort. Drag the **round knob**
above the object to rotate it; hold **Shift** to snap to 15°. Exact values are always
editable in Properties → Layout.

### Images, SVG, video, paste and drag & drop

- **Insert → Image** (Ctrl/⌘+Shift+I) or **Insert → SVG**. You can also drag image or SVG
  files onto the window, or paste with Ctrl/⌘+V: screenshots, images copied from a
  browser or another app, files copied in Explorer/Finder, plain text (which becomes a
  text box), and GlideDeck objects copied with Ctrl/⌘+C.
- **SVGs** are parsed and drawn as true vectors: fills, strokes, linear and radial
  gradients, and text converted to outlines. They stay sharp at any zoom.
- **Insert → Video** (MP4, MOV, WebM, MKV…). Videos sit on the canvas like a framed
  screen with rounded corners and a shadow. Hover over one and click ▶ to play or pause;
  a progress bar shows the position. In Properties → Video you can set **Autoplay in
  show** (it plays when its slide is reached), **Loop** and **Mute**. In the show,
  clicking a video toggles playback instead of advancing. Audio plays through the system
  output. Videos need FFmpeg (see Prerequisites); without it they show as a poster
  placeholder.
- Media is **never embedded in the JSON**. Files are copied into an `assets/` folder next to
  the document, named by content hash, and referenced by relative path
  (`"path": "assets/1f3c….png"`). Save As copies the referenced assets to the new folder.
  Before the first save, assets live in a temporary session folder.

### Navigation and editing

| Action | Input |
|---|---|
| Zoom at cursor | Mouse wheel · trackpad pinch · Ctrl/⌘ + `=` / `-` · zoom pill (bottom right) |
| Pan | Space + drag · middle-mouse drag · Hand tool (`H`) · Shift + wheel |
| Rotate view | Alt/⌥ + wheel · `[` and `]` (±15°) |
| Fit everything / zoom to selection / reset | `Home` or Ctrl/⌘+`1` · Ctrl/⌘+`2` · Ctrl/⌘+`0` |
| Tools | `V` select · `H` hand · `F` frame · `R` rectangle · `O` ellipse · `T` text |
| Draw | Drag with a tool (click for a default size); text: drag to set a wrap width |
| Multi-select | Shift- or Ctrl/⌘-click · drag a marquee on empty canvas |
| Move / nudge | Drag a selection · arrow keys (Shift = 10×) |
| Cut / copy / paste / duplicate | Ctrl/⌘ + `X` / `C` / `V` / `D` |
| Delete | `Delete` / `Backspace` |
| Z-order | Ctrl/⌘ + Shift + `]` / `[` |
| Undo / redo | Ctrl/⌘ + `Z` · Ctrl/⌘ + Shift + `Z` (or Ctrl + `Y`) |
| Save / open / new | Ctrl/⌘ + `S` (Shift for Save As) · `O` · `N` |
| All shortcuts | `F1` or File → Keyboard shortcuts |

New objects are sized for the current zoom level and aligned with the current view
rotation, so you can zoom in and build nested "worlds".

### Exporting

- **Present → PowerPoint** (or File → Export PowerPoint) writes a `.pptx` with one 16:9
  slide per step. SVGs are embedded natively (with a PNG fallback for older Office), and
  videos are embedded with a poster frame (click to play in PowerPoint). Every object keeps its identity as a shape named `!!<id>`, and slides are
  linked with PowerPoint's **Morph** transition, so PowerPoint itself animates the zooms,
  pans and rotations. Invisible frames are left out; bordered or filled frames are
  exported as shapes. Requires PowerPoint 2019 / Microsoft 365, or PowerPoint for Mac
  16.x. Older versions show a fade instead.
- **Present → PNG image** saves the current view at 2× resolution.

---

## 4. Headless CLI (scripts and agents)

The same binary has subcommands that work without opening a window. Replace `glidedeck`
with `cargo run --release -p glidedeck --` if you haven't installed the binary.

```bash
glidedeck new talk.glide.json --title "My Talk"
```

```bash
glidedeck new pitch.glide.json --template pitch-deck
```

```bash
glidedeck apply talk.glide.json examples/add-slide.ops.json
```

```bash
glidedeck show talk.glide.json
```

```bash
glidedeck render talk.glide.json --out slide3.png --step 3
```

```bash
glidedeck export-pptx talk.glide.json --out talk.pptx
```

- `new [--demo | --template NAME] [--force]` creates an empty document, the welcome canvas, or
  a template (`pitch-deck`, `journey`, `big-picture`, `lecture`, `blank`).
- `apply FILE [OPS]` applies a JSON operation, or an array of them, read from a file or
  from stdin. The batch is **atomic**: if any op fails, the file is left untouched and the
  command exits non-zero with `{"ok":false,...}`.
- `show` prints a JSON summary of the elements.
- `render` draws a view to PNG with the GPU and no window. By default it frames the whole
  canvas. Use `--step N` to render slide N exactly as presented, `--element ID` to frame one
  element, or `--center=x,y --zoom Z --rotation DEG` for an explicit camera.
- `export-pptx` writes the Morph deck described above.

### Operation reference

```json
{"op": "add_element", "element": {"id": "optional", "name": "optional",
  "transform": {"x": 0, "y": 0, "scale": 1, "rotation": 0},
  "kind": {"type": "text", "content": "Hello", "font_size": 64}}}
{"op": "remove_element", "id": "..."}
{"op": "set_transform", "id": "...", "x": 10, "y": 20, "scale": 0.5, "rotation": 45}
{"op": "set_kind", "id": "...", "kind": {"type": "rect", "width": 100, "height": 60}}
{"op": "set_name", "id": "...", "name": "..."}
{"op": "set_opacity", "id": "...", "opacity": 0.5}
{"op": "reorder", "id": "...", "index": 0}
{"op": "set_title", "title": "..."}
{"op": "set_background", "color": "#f7f6f2"}
{"op": "add_step", "frame": "frame-id", "index": 0, "notes": "optional",
  "transition": {"style": "smooth", "duration": 1.4, "easing": "ease_in_out_cubic"}}
{"op": "remove_step", "index": 2}
{"op": "move_step", "from": 2, "to": 0}
{"op": "update_step", "index": 1, "notes": "...", "transition": {"style": "direct"}}
```

Element kinds:
- `rect`, `ellipse` and `text`.
- `frame`: `width` and `height`; optional `border` (`{"color","width"}`), `fill` and
  `corner_radius`. With none of these, the frame is invisible.
- `image`: `path` (relative to the document), `width`, `height`; optional `corner_radius`.
- `icon`: `name` (a Phosphor name such as `"rocket-launch"`), `style` (`light`, `regular`,
  `bold` or `fill`), `size` and `color`.
- `rect` and `ellipse` accept an optional `gradient`: `{"to": "#hex", "angle": 45}`.
- Any element can have a `shadow`: `{"color": "#10142430", "blur": 18, "x": 0, "y": 8}`.
- `text` accepts an optional `role` (`title`, `heading`, `subtitle`, `body`, `caption`,
  `quote`), which themes use to restyle it.
- At document level: `theme` (colours and fonts) and `background_to` (a gradient
  background).
- `svg`: `path`, `width`, `height`.
- `video`: `path`, `width`, `height`; optional `corner_radius`, `autoplay` (default
  `true`), `looping` and `muted`.
- `text` also accepts `list`: `none`, `bullet` or `numbered`.

Transition styles are `smooth`, `direct` and `cut`. Easings are `linear`,
`ease_in_out_sine`, `ease_in_out_cubic`, `ease_in_out_quint` and `ease_out_expo`. Colours
are `#RRGGBB` or `#RRGGBBAA`.

---

## 5. Development

```bash
cargo test --workspace
```

The UI tests drive the real editor headlessly through `egui_kittest` (with a real wgpu
device). Set `GLIDEDECK_SNAPSHOTS` to a folder to also save screenshots of each scenario.

```bash
cargo run -p glidedeck
```

Debug builds compile dependencies with optimizations (see `[profile.dev.package."*"]`
in `Cargo.toml`), so `cargo run` without `--release` is still smooth.
