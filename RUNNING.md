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
Animate · Present · View), the **Slides** strip on the left, the canvas in the middle,
**Properties** on the right and **Speaker notes** under the canvas. The app follows your
system's light/dark setting; the sun/moon button switches it. The ⓘ button (or
**File → About GlideDeck**) shows the version, a link to GitHub and **Check for updates**.

### Speaker notes

The **Speaker notes** panel under the canvas holds the notes for the current slide (the one
selected in the Slides strip). Only the presenter sees them, in presenter view. They support
`# headings`, `- bullets`, `1. numbers` and `**bold**`. Collapse the panel with its ˅ button,
or toggle it with **View → Speaker notes** or **Present → Speaker notes**. Notes imported
from PowerPoint appear here too.

### Updates

**File → Check for updates** asks GitHub for the latest release. If there is a newer version,
**Download** saves its installer to your Downloads folder (the `.msi` on Windows, the `.dmg`
on macOS). **Install now** starts the installer, which upgrades GlideDeck in place; on macOS,
drag the new app into Applications. GlideDeck makes no other network requests.

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
  **Smooth zoom** (a cinematic zoom-out-and-dive arc), **Direct**, **Swoop** (a sideways
  arc with a gentle roll), **Spin** (a barrel roll), **Punch** (overshoots the zoom, then
  settles), **Fade** (dips through the background color) or **Cut**, plus the duration and
  easing.
- **Depth (parallax)**: Properties → Layout → Depth. In the show, objects with a depth above 0
  are far away and drift slower than the slides as the camera travels; below 0 they are near
  and move faster. The effect fades out when the camera zooms out to an overview.

### Animating objects on a slide

The **Animate** tab adds in-slide animations ("builds") to the selected objects:

| Kind | Effects |
|---|---|
| **Appear**: hidden until it plays | Fade · Rise · Focus · Zoom · Pop · Drop · Fly in · Spin · Typewriter · Word by word · Count up |
| **Emphasis**: draws attention | Pulse · Wiggle · Glow · Spotlight (dims the rest) · Highlight (a marker sweeps behind) · Underline · Color pop |
| **Disappear**: hidden once played | Fade out · Sink · Zoom out · Fly out · Shrink |

- **Start** sets how new animations begin: *On click*, *With previous* (together with the
  one before) or *After previous* (when it ends). With several objects selected, *One by
  one* gives each its own turn and *All together* plays them at once.
- **Auto-animate** builds an entrance cascade for the current slide in reading order:
  titles focus, text rises, short numbers count up, icons pop and pictures zoom. It plays
  as soon as the camera arrives. **Auto-animate all slides** does every slide.
- An object animates on the slide that shows it (the smallest one, so an overview doesn't
  take it). On the canvas, numbered badges show each object's click (green appear, amber
  emphasis, red disappear); **Preview** plays the slide's animations on the canvas.
- **Properties → Animation** (an object selected) edits each animation: effect, start,
  duration, delay and the direction for Fly in/out. **Properties → Slide N animations** (the
  frame selected) lists them in order, with ↑ ↓ to reorder and ✕ to remove.
- The slide strip shows ☆ and the number of animated objects on each slide.

In the show, → / click plays the next animation before moving on (a second press finishes
a running one at once); ← takes back the last one, then goes to the previous slide, which
shows fully built. The presenter console counts the animations still to go. Number effects
keep the format (`$12,400`, `87.5%`). Exports (PowerPoint, PDF and renders) show every object,
as the editor does.

### Design tools

- **Templates** (File → New from template, or Design → Templates): 17 complete zooming
  stories, each with a theme, frames and a path. The previews are live renders.
  - **Business**:
    - *Pitch deck*: a grid of slides with an overview.
    - *Roadmap*: quarters × teams.
    - *Showcase*: a bento grid of slides.
    - *Branches*: a tree from one idea down to details.
  - **Storytelling**:
    - *Journey*: rotated frames along a winding road.
    - *Summit*: camps up a mountain to the peak.
    - *Spiral*: slides that grow along a spiral while the camera turns.
    - *Deep dive*: each slide hides the next inside a porthole.
  - **Ideas & maps**:
    - *Big picture*: a central idea with four branches.
    - *Hive*: a honeycomb.
    - *Orbit*: planets around a sun.
    - *Mood board*: tilted polaroids around a title.
  - **Teaching**:
    - *Lecture*: an agenda with nested detail.
    - *Chapters*: a bookshelf whose spines open into chapters.
    - *Workshop*: sticky-note clusters.
    - *Cycle*: six stages on a wheel.
  - Or start from a **blank canvas**.
- **Themes** (Design tab): eight curated colour and font pairings, some with gradient
  backgrounds. Switching theme restyles text by its role, and any colour taken from the
  previous theme moves to the matching colour in the new one. Colours you picked by hand
  are left alone. Theme colours also appear as one-click swatches next to every colour
  picker.
- **Slide layouts** (Insert → Layouts, Ctrl/⌘+Shift+L): 20 layouts in four groups.
  - **Basics**: Title, Section, Quote, Big number, Image + text, Agenda.
  - **Grids**: Three points, Comparison, Quadrants, Bento grid, Dashboard, Team.
  - **Diagrams**: Hub & spokes, Cycle, Venn, Pyramid.
  - **Flows**: Timeline, Process, Staircase, Winding road.

  Each one is added as a new frame next to the current slide, styled by the theme, and
  added to the path.
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
**B** or **W** blacks or whites out the screen; Esc ends the show. The mouse wheel still
zooms freely, and the next step returns to the path.

**Presenter view.** With a second display connected (a projector or TV), F5 shows the
audience view fullscreen on that display. Your screen becomes the presenter console:

- the live current slide, exactly as the audience sees it;
- the next slide;
- the slide's **speaker notes**, with A−/A+ to resize them. Notes support `# headings`,
  `- bullets`, `1. numbers` and `**bold**`;
- an elapsed **timer** (pause and reset) and the clock;
- Previous / Next, Black / White screen, **Swap displays** (if the show landed on the
  wrong screen) and End show.

Keys and clickers work in either window. **Alt+F5** (Present → Presenter view) opens
presenter view on a single display too, for rehearsing: the console runs the show in its
NOW pane (click it to advance). **Open audience window** adds a slides-only window you can
drag to a projector or share in a video call, and then make fullscreen. Write the notes in
the Speaker notes panel under the canvas. To open a file straight into the show:

```bash
glidedeck present talk.glide.json --presenter-view
```

### Editing text in place

Double-click a text box (or select it and press **Enter** / **F2**) to edit it directly on
the canvas. Editing uses the same text engine that draws it, so Hebrew, Arabic and mixed
right-to-left / left-to-right text, word motion (Ctrl/⌥+arrows), selection
(Shift+arrows, drag, double-click a word), clipboard and IME input all behave as in a word
processor, at any zoom or rotation. **Esc** or clicking outside finishes; an emptied text
box is removed. The multi-line field in Properties also edits text, but it shows
right-to-left words in left-to-right order, so use the canvas for those scripts.

**Formatting words and phrases:** while editing, select text and use the floating format
bar above the text box, or the shortcuts below. With nothing selected, the formatting
applies to what you type next.

| Format | Shortcut |
|---|---|
| Bold / italic / underline | Ctrl/⌘ + `B` / `I` / `U` |
| Strikethrough | Ctrl/⌘ + Shift + `X` |
| Larger / smaller | Ctrl/⌘ + `]` / `[` |
| Clear formatting | Ctrl/⌘ + `\` |

The bar also has the theme's colours. When a text box is selected but not being edited,
the same commands (and the Home → Text format buttons) format the whole box. Formatting
is exported to PowerPoint as real formatted text.

**Bullets and numbering:** select a text box, then use the list buttons in Home → Text
format (or Properties → List). Each paragraph gets a marker with a proper hanging indent,
placed on the right for right-to-left paragraphs. Lists export to PowerPoint as real
bullets.

### Resizing and rotating

Select a single object to show its handles. Drag a **corner** to resize it uniformly from
the opposite corner, so images, icons and SVGs never distort. Text boxes also have
**side handles**: drag them to change the wrap width, and the text reflows onto more or
fewer lines without changing size. Rectangles, ellipses and frames have a handle on each
side to stretch just their width or height. Drag the **round knob**
above the object to rotate it; hold **Shift** to snap to 15°. Exact values are always
editable in Properties → Layout.

### Images, SVG, video, paste and drag & drop

- **Insert → Image** (Ctrl/⌘+Shift+I) or **Insert → SVG**. You can also drag image or SVG
  files onto the window, or paste with Ctrl/⌘+V: screenshots, images copied from a
  browser or another app, files copied in Explorer/Finder, plain text (which becomes a
  text box), and GlideDeck objects copied with Ctrl/⌘+C.
- **From PowerPoint (Windows):** shapes copied in PowerPoint, Word or Excel paste as
  editable elements, keeping their arrangement:
  - text boxes, with font, size, bold, italic, colour, alignment and bullets;
  - rectangles, rounded rectangles and ellipses, with fill and outline;
  - pictures;
  - grouped shapes.

  Other shape types keep their text but lose the outline. If Office's shape data isn't
  available, which is always the case on macOS, the picture Office puts on the clipboard
  is pasted instead.
- **Ctrl/⌘+Shift+V** pastes the clipboard's text as a plain text box, even when a picture
  came with it.
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
| Paste as plain text | Ctrl/⌘ + Shift + `V` |
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
- **Present → PDF** (or File → Export PDF) writes one page per slide on the path, rendered
  exactly as in the show (2560 px wide images). It's handy for sharing a handout or
  printing.
- **Present → PNG image** saves the current view at 2× resolution.

### Importing PowerPoint

**File → Import PowerPoint…** (or File → Open, or dropping a `.pptx` on the window) turns a
PowerPoint presentation into a new GlideDeck document.

- **Layout:** every slide becomes a frame, laid out in a grid on a neutral canvas and
  added to the path in slide order, ready to rearrange into a zooming story.
- **Brought over:**
  - text boxes and placeholders with their formatting (bold, italic, underline, colours,
    sizes, alignment and bullets);
  - rectangles, rounded rectangles, ellipses and pictures;
  - slide backgrounds (colour or picture);
  - the master's and layout's logos and lines;
  - speaker notes, which presenter view shows.
- **Inheritance:** placeholders take their position and style from the slide layout and
  master, as in PowerPoint.
- **Not converted yet:** tables, charts and SmartArt are listed as skipped. Shapes other
  than rectangles and ellipses keep their text but not their outline.

The import is unsaved until you save it; Save As stores its pictures next to the document.

### Arranging slides

**Design → Arrange** (or the path button at the top of the slide list) moves every slide on
the path into a new layout. Each slide's content moves and turns with it, and the order of
the path stays the same. The dialog draws each arrangement live for your number of slides:

| Arrangement | What the path does |
|---|---|
| Carousel | Side by side; the camera glides from slide to slide over a parallax backdrop of glowing layers |
| Spiral | Winds outward from the first slide; slides turn with the curve |
| Orbits | First slide at the center, the rest on rings around it |
| Sunflower | Golden-angle seeds: every step swings ~137° around the center |
| Ring | One circle, the camera spinning around it |
| Honeycomb | Hex rings spiralling out from the center |
| Journey | A meandering road with tilted slides, wrapping into rows |
| Wave | A sine-wave road, wrapping into rows |
| Staircase | Climbs upward in switchback flights |
| Tree | Branches like an org chart, visited branch by branch |
| Zoom tunnel | Each slide inside the previous one; every 10 levels a new tunnel starts |
| Snake | Rows that turn back at each end |
| Grid | Rows, left to right |

Options:
- **Spacing** sets the gap between slides.
- **Turn slides along the path** tilts the slides; in the show, the camera turns with them.
- **Draw the path** adds a dotted trail between slides (lines for Tree). For Carousel it
  is the **Parallax backdrop**: orbs, rings and sparks at three depths. Arranging again
  replaces it. Carousel also sets each slide's transition to a gliding Direct move.
- **Overview at start and end** adds a view of everything at both ends of the path.

Every arrangement is a formula, so it works for any number of slides. It's one step to
undo (Ctrl/⌘+Z). Objects that aren't on any slide stay where they are. From the command
line:

```bash
glidedeck arrange deck.glide.json --layout orbits
```

### Recent files

**File → Open recent** lists the last 10 documents (and PowerPoint files) you opened, saved
or imported. The list lives in your user settings folder: `%APPDATA%\GlideDeck` on
Windows, `~/Library/Application Support/GlideDeck` on macOS.

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
glidedeck export-pdf talk.glide.json --out talk.pdf
glidedeck import-pptx deck.pptx --out deck.glide.json
glidedeck arrange deck.glide.json --layout spiral
```

- `new [--demo | --template NAME] [--force]` creates an empty document, the welcome canvas, or
  a template: `pitch-deck`, `roadmap`, `showcase`, `branches`, `journey`, `summit`, `spiral`,
  `deep-dive`, `big-picture`, `hive`, `orbit`, `mood-board`, `lecture`, `chapters`,
  `workshop`, `cycle` or `blank`.
- `apply FILE [OPS]` applies a JSON operation, or an array of them, read from a file or
  from stdin. The batch is **atomic**: if any op fails, the file is left untouched and the
  command exits non-zero with `{"ok":false,...}`.
- `show` prints a JSON summary of the elements.
- `render` draws a view to PNG with the GPU and no window. By default it frames the whole
  canvas. Use `--step N` to render slide N exactly as presented, `--element ID` to frame one
  element, or `--center=x,y --zoom Z --rotation DEG` for an explicit camera.
- `export-pptx` writes the Morph deck described above.
- `mcp [--root DIR]` runs the MCP server on stdio for AI assistants. Relative project paths
  resolve against `--root`, which defaults to the current folder.
- `agent-setup [--project DIR | --user] [--no-mcp | --no-skill]` installs the GlideDeck
  skill and registers the MCP server for Claude Code and GitHub Copilot. See
  [docs/AI.md](docs/AI.md).

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
{"op": "update_step", "index": 1, "notes": "...", "transition": {"style": "direct"},
  "builds": [{"element": "text-1", "effect": "rise", "duration": 0.6},
             {"element": "text-2", "effect": "spotlight", "trigger": "after_previous", "delay": 0.2}]}
```

`builds` replaces the slide's animations. Effects: `fade`, `rise`, `focus`, `zoom`, `pop`,
`drop`, `fly`, `spin`, `typewriter`, `words`, `count_up` (appear); `pulse`, `wiggle`,
`glow`, `spotlight`, `highlight`, `underline`, `color_pop` (emphasis); `fade_out`, `sink`,
`zoom_out`, `fly_out`, `shrink` (disappear). `trigger` is `on_click` (default),
`with_previous` or `after_previous`; `direction` (for `fly` / `fly_out`) is `left`,
`right`, `up` or `down`.

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

Transition styles are `smooth`, `direct`, `swoop`, `spin`, `punch`, `fade` and `cut`. Easings are `linear`,
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
