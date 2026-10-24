---
name: glidedeck
description: Create, edit and export zooming presentations with GlideDeck. Use when the user asks for a presentation, slide deck, pitch deck, talk, lecture, workshop slides, or a Prezi-style zooming story, or wants slides exported to PowerPoint (.pptx). Produces a .glide.json GlideDeck project the user can open and keep editing in the GlideDeck app.
---

# GlideDeck presentations

GlideDeck is a local, native zooming-presentation app. A presentation is a
**project file** (`<name>.glide.json`, plus an `assets/` folder for images)
holding frames on an infinite canvas. The show flies the camera from frame to
frame. Everything you build is saved in that project, so the user can open it in
the GlideDeck editor later to edit and present it, or export it to PowerPoint.

## Use the GlideDeck MCP tools

If the `glidedeck` MCP server is connected, its tools are available: `catalog`,
`create_presentation`, `get_presentation`, `add_slide`, `update_elements`,
`add_text`, `add_image`, `set_theme`, `remove_slide`, `render_slide`,
`export_pptx`, `export_pdf`, `export_html`, `import_pptx`, `arrange_slides`, `animate`,
`auto_animate`, `set_transitions`, `set_notes`, `clear_animations`, `save_as`,
`open_in_editor`, `search_icons`, `list_fonts`, `add_font` and
`edit_presentation`. Prefer them.

**Fonts:** any font installed on the user's computer works. Find exact family
names with `list_fonts` (e.g. `query: "serif"`), then set one per text box with
`update_elements` (`font`). If the user gives you a font file, `add_font` copies
it into the project so it travels with the presentation.

**Starting from an existing PowerPoint file?** Call `import_pptx`. It turns
every slide into a frame with its text, pictures and notes, laid out as a plain
grid. Then call `arrange_slides` to give it a zooming path that suits the story:
- `carousel` to keep it linear but sleek: slides glide past over a parallax backdrop;
- `spiral` or `journey` for a narrative;
- `orbits` or `sunflower` for one central idea;
- `tree` for a hierarchy;
- `staircase` for growth;
- `zoom-tunnel` for "going deeper".

Render step 1 (the overview) to check it, then improve the content.

Follow this workflow:

1. **Plan the story first.** Decide the 5–12 slides: one idea per slide, short
   titles, at most about 30 words of body text per slide.
2. **`catalog`.** Pick the template whose structure fits the story:
   - `carousel`, `pitch-deck`, `roadmap`, `showcase` or `branches` for business
     (`carousel` comes with glass cards, a parallax backdrop and animations built in);
   - `journey`, `summit`, `spiral` or `deep-dive` for a narrative;
   - `big-picture`, `hive`, `orbit` or `mood-board` for ideas;
   - `lecture`, `chapters`, `workshop` or `cycle` for teaching;
   - `blank` to build slide by slide.

   Pick a theme too if the user cares about the look.
3. **`create_presentation`.** Pass a descriptive path, e.g.
   `q3-results.glide.json` in the user's working folder, plus the template and
   a title. The result lists every slide with element ids and placeholder text.
4. **Replace every placeholder.** Use `update_elements` (batch many updates
   into one call). Templates ship with sample text such as "Your big idea",
   "Section title" or "Avery Chen", and none of it may remain. Use `\n` to
   separate bullet paragraphs. To emphasise a word, pass `markdown: true` and
   write `**bold**`, `*italic*`, `__underline__` or `~~strike~~`. Use emphasis
   sparingly.
5. **`add_slide` for anything more.** Choose a layout and fill its
   `text_fields` and `icon_slots` (names come from `catalog`; repeated names
   take an array). Good matches:
   - `comparison` for A vs B;
   - `quadrants` for SWOT;
   - `dashboard` for KPIs;
   - `process` or `staircase` for steps;
   - `timeline` or `winding-road` for history;
   - `hub-spokes` or `venn` for relationships;
   - `team` for people;
   - `big-number` and `quote` for emphasis.

   Use `search_icons` for icon names.
6. **Make it outstanding: path, motion and builds.** This step is what makes a
   GlideDeck talk memorable instead of a slideshow. Do all three unless the
   user asks for a plain deck (see "Making it outstanding" below):
   - `arrange_slides` with a layout that fits the story;
   - `set_transitions` with a varied mix of camera moves;
   - `auto_animate`, then `animate` the two or three key moments.
   Then write **speaker notes** for every slide with `set_notes`: what to say, the
   number to stress, where to pause. The presenter sees them in presenter view.
7. **Check your work.** Call `render_slide` for each slide (step 1..N) and look
   at the image. Fix overflowing or cramped text, leftover placeholders and
   off-topic icons. Shorten text rather than shrinking it. (Renders show each
   slide fully built: animations don't hide anything there.)
8. **Finish.** Tell the user where the project file is. Mention they can
   present it with F5; with a second display, presenter view shows the speaker
   notes (Alt+F5 rehearses with them on one screen). Offer `export_pptx`
   for PowerPoint, `export_pdf` for a PDF, `export_html` for a web page that
   plays the whole show in a browser with every effect intact (the best way to
   share it with people who don't have GlideDeck), and `open_in_editor` to open it in
   GlideDeck. Use `save_as`
   to keep a separate copy or version.

## Making it outstanding

Use the layout features and in-slide animations generously and with intent.
Every choice should support the story.

**1. Shape the journey with `arrange_slides`.** A straight row of slides wastes
the zooming canvas. Pick the arrangement that mirrors the structure of the talk:

| Story | Arrangement |
|---|---|
| One central idea and its parts | `orbits`, `sunflower` |
| A hierarchy or agenda with sub-topics | `tree` |
| A narrative from A to B | `journey`, `wave`, `spiral` |
| A sleek linear deck, e.g. an imported PowerPoint | `carousel` |
| Growth, progress, levelling up | `staircase`, `spiral` |
| Drilling down / "going deeper" | `zoom-tunnel` |
| A cyclical process | `ring` |
| A dense catalogue of items | `honeycomb`, `snake` |

Keep `overview: true` so the talk opens and closes on the big picture, and
`trail: true` so the audience sees the road. Render step 1 to check.

**2. Vary the camera with `set_transitions`.** Mix moves instead of using one
everywhere, e.g. `styles: ["smooth", "swoop", "punch"]` cycles them:
- `smooth`: zoom out, travel, zoom in; best into and out of overviews;
- `swoop`: a sideways arc with a gentle roll, for moving between neighbours;
- `punch`: overshoots and settles, for a bold statement or big number;
- `spin`: a barrel roll, for a change of topic (use once or twice);
- `fade`: a dip through the background, for a new chapter;
- `direct` / `cut`: plain moves.

**3. Animate inside the slides.** Builds play in the show in order. `on_click`
waits for a click; `with_previous` starts together with the one before;
`after_previous` starts when it ends. A slide whose first build isn't on
click plays it as soon as the camera arrives.
- Start with `auto_animate` (all slides). Each slide's content cascades in, in
  reading order: titles `focus`, text `rise`, numbers `count_up`, icons `pop`,
  pictures `zoom`.
- Then `animate` the key moments with ids from `get_presentation`:
  - `spotlight` dims everything but the one number, chart or phrase that
    matters; `highlight` sweeps a marker behind a phrase; `underline` draws a
    line under it; `color_pop` turns it the accent color; `pulse`, `glow` and
    `wiggle` give a brief nudge.
  - `count_up` makes metrics ("$4.2M", "87%") count up from zero.
  - `words` or `typewriter` reveal a quote or a key sentence.
  - Use on-click `rise` or `fly` (with `direction`) to reveal list items one by
    one as the speaker talks about them.
  - `fade_out`, `sink`, `zoom_out`, `fly_out` or `shrink` clear an object
    before the punchline replaces it in the same spot.
- Taste: one or two emphasis effects per slide, entrances under a second, and
  not every slide needs clicks. Call `get_presentation` to review each slide's
  `animations` and `transition`.

Example: spotlight the KPI on a click after the slide's cascade, then count it
up:

```json
{"path": "q3.glide.json", "animations": [
  {"id": "text-kpi", "effect": "spotlight"},
  {"id": "text-kpi", "effect": "count_up", "trigger": "with_previous"}
]}
```

**Parallax.** `update_elements` with `parallax` gives any object depth in the show:
0.3–0.7 for a far backdrop that drifts slower than the slides, negative for near objects
that move faster. Keep parallax objects decorative (glows, shapes), never text to read.

Notes:
- Slide coordinates for `add_text` and `add_image` are 1600 × 900 units with
  (0, 0) at the slide's center.
- Steps are 1-based. A template's first and last step is often an "Overview"
  frame that shows the whole canvas; leave it in place.
- Do not delete the `assets/` folder next to the project; images live there.

## Without the MCP server: the `glidedeck` CLI

When the tools are not available, but a terminal is, the same operations
exist as commands. `glidedeck` is on PATH after installing GlideDeck:

```bash
glidedeck new talk.glide.json --template roadmap   # create from a template
glidedeck show talk.glide.json                       # list element ids and names
glidedeck apply talk.glide.json ops.json             # atomic batch of JSON edit operations
glidedeck render talk.glide.json -o s1.png --step 1  # render a slide to check it
glidedeck export-pptx talk.glide.json -o talk.pptx   # PowerPoint with Morph transitions
glidedeck export-html talk.glide.json -o talk.html   # web page that plays the show exactly
```

Animations live on each path step as `builds`. With `apply`, set them with
`{"op":"update_step","index":0,"builds":[{"element":"text-1","effect":"rise","duration":0.6}]}`
(`trigger`: `on_click` (default), `with_previous` or `after_previous`; optional
`delay` and `direction`).

To change text with `apply`, use a `set_kind` operation with the element's
full kind. `show` lists the ids. The operation schema is documented in
GlideDeck's RUNNING.md under "JSON operations".

To connect the MCP server instead, run `glidedeck agent-setup` in the project
folder, or see the GlideDeck README section "Use with Claude and GitHub Copilot".
