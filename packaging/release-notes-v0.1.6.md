## GlideDeck 0.1.6

**Carousel with parallax, a working presenter view on one screen, speaker notes where you expect them, and updates from inside the app.**

### Install
- **Windows 10/11 (x64):** download **`GlideDeck-0.1.6-x64.msi`** below and run it. It upgrades earlier versions in place. `GlideDeck-x64.msi` is the same file under a name that never changes.
- **macOS (Apple Silicon and Intel):** download **`GlideDeck-0.1.6.dmg`** below, open it and drag GlideDeck into **Applications**. The universal app and DMG are Developer ID-signed, and the DMG is notarized by Apple with the ticket stapled.

> The MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

### New
- **Carousel template.** Seven slides on frosted glass cards glide past side by side. Behind them, aurora glows, rings and sparks drift at three different depths (parallax). Every slide builds in as it arrives.
- **Carousel arrangement** (Design → Arrange). It turns any deck, including an imported PowerPoint, into the same gliding carousel, with a parallax backdrop and smooth sideways transitions.
- **Parallax for any object.** Set it in Properties → Layout → Depth.
  - Far objects drift slower than the slides; near ones move faster.
  - The effect fades away in overviews, so the layout reads as designed.
- **Speaker notes panel** under the canvas, like PowerPoint. It shows the current slide's notes, supports Markdown, and has toggles in View and Present.
- **About GlideDeck** (the ⓘ button, or File → About) shows the version and links to GitHub.
- **Check for updates** (File → Check for updates).
  - It finds a newer release on GitHub and downloads the installer for your system to your Downloads folder: the `.msi` on Windows, the `.dmg` on macOS.
  - **Install now** upgrades GlideDeck in place.
  - This is the only time GlideDeck goes online.
- **A new welcome canvas**, with **two new 2026 themes**.
  - **Dusk:** deep violet with aurora accents.
  - **Cloud:** warm white with teal and coral.
  - The welcome canvas uses Dusk, with glass cards, parallax glows, animations, varied camera moves and speaker notes.

### Fixed
- **Presenter view on one screen.** The audience window used to open on top of the presenter console and hide your notes. Now, with a single display, the console runs the show in its NOW pane, so your notes, the next slide and the timer stay visible. **Open audience window** adds a slides-only window when you need one. With two displays, the show still goes fullscreen on the second one automatically.

### For Claude and GitHub Copilot
- `arrange_slides` accepts `carousel`, and the `carousel` template is in `catalog`.
- New `set_notes` tool for speaker notes. `get_presentation` now returns each slide's notes.
- `update_elements` accepts `parallax`.
- The server instructions and the skill recommend:
  - the carousel for linear decks and imported PowerPoint files;
  - speaker notes on every slide.

### Checksums
SHA-256 of the MSI is listed in `GlideDeck-0.1.6-x64.msi.sha256`.
SHA-256 of the DMG is listed in `GlideDeck-0.1.6.dmg.sha256`.
