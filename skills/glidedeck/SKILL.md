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
`export_pptx`, `save_as`, `open_in_editor`, `search_icons` and
`edit_presentation`. Prefer them.

Follow this workflow:

1. **Plan the story first.** Decide the 5–12 slides: one idea per slide, short
   titles, at most about 30 words of body text per slide.
2. **`catalog`.** Pick the template whose structure fits the story:
   - `pitch-deck`, `roadmap`, `showcase` or `branches` for business;
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
   separate bullet paragraphs.
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
6. **Check your work.** Call `render_slide` for each slide (step 1..N) and look
   at the image. Fix overflowing or cramped text, leftover placeholders and
   off-topic icons. Shorten text rather than shrinking it.
7. **Finish.** Tell the user where the project file is. Offer `export_pptx`
   for PowerPoint and `open_in_editor` to open it in GlideDeck. Use `save_as`
   to keep a separate copy or version.

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
```

To change text with `apply`, use a `set_kind` operation with the element's
full kind. `show` lists the ids. The operation schema is documented in
GlideDeck's RUNNING.md under "JSON operations".

To connect the MCP server instead, run `glidedeck agent-setup` in the project
folder, or see the GlideDeck README section "Use with Claude and GitHub Copilot".
