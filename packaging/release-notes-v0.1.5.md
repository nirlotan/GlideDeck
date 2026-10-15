## GlideDeck 0.1.5

**Animations: objects appear, take the spotlight and leave, plus five new camera moves.**

### Install
- **Windows 10/11 (x64):** download **`GlideDeck-0.1.5-x64.msi`** below and run it. It upgrades earlier versions in place. `GlideDeck-x64.msi` is the same file.
- **macOS (Apple Silicon):** the DMG linked from the README is still 0.1.3; it doesn't include animations yet.

> This preview MSI is not code-signed yet, so Windows SmartScreen may warn. Choose **More info → Run anyway**.

### New: in-slide animations (the Animate tab)
- **Appear:** Fade, Rise, Focus (settles in like a lens pulling focus), Zoom, Pop, Drop (with a bounce), Fly in, Spin, **Typewriter**, **Word by word** and **Count up**. Count up runs the numbers in a text from zero and keeps their format (`$12,400`, `87.5%`).
- **Emphasis:** **Spotlight** (dims everything else), **Highlight** (a marker sweeps behind the words, line by line), **Underline**, **Color pop**, Pulse, Glow and Wiggle.
- **Disappear:** Fade out, Sink, Zoom out, Fly out and Shrink.
- **Start** each animation on a click, with the previous one, or after it, with its own duration and delay. When several objects are selected, they can animate one by one or all together.
- **Auto-animate** gives a slide a tasteful entrance cascade in reading order: titles focus, text rises, numbers count up, icons pop and pictures zoom. It plays as the camera arrives. One click does one slide; **Auto-animate all slides** does the whole deck.
- **Numbered badges** on the canvas show the click order: green for appear, amber for emphasis, red for disappear. **Preview** plays the slide's animations right in the editor.
- **Properties → Animation** edits each animation. The slide's card lists them in order, with buttons to reorder and remove. The slide strip shows how many objects animate on each slide.
- **In the show:** each click plays the next animation before moving on, and a second press finishes a running one. Going back takes the last animation back. The presenter console counts the animations still to go.

### New camera moves (the Transitions tab)
- **Swoop:** a sideways arc with a gentle roll.
- **Spin:** a smooth zoom with a barrel roll.
- **Punch:** overshoots the zoom, then settles.
- **Fade:** dips through the background to the next slide.

These join Smooth zoom, Direct and Cut.

### For Claude and GitHub Copilot
- Four new MCP tools:
  - `animate` adds appear, emphasis and disappear builds by element id;
  - `auto_animate` gives slides an entrance cascade;
  - `set_transitions` sets one style, or cycles a list such as `["smooth", "swoop", "punch"]`;
  - `clear_animations` removes them.
- `catalog` lists the effects and transitions, and `get_presentation` shows each slide's animations and transition.
- The server instructions and the skill now tell the assistant to make every deck stand out:
  - shape the path with `arrange_slides`;
  - vary the camera moves;
  - auto-animate the slides, then spotlight or highlight the key moments.
- In JSON operations, `update_step` accepts `builds`.

### Checksums
SHA-256 of the MSI is listed in `GlideDeck-0.1.5-x64.msi.sha256`.
