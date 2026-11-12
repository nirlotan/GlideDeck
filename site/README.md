# The GlideDeck site

Live at **https://nirlotan.com/GlideDeck/**. Plain HTML, CSS and JavaScript in
`public/GlideDeck/`, served by a Cloudflare Worker (`glidedeck-site`) with static assets.

## Publish

From this folder, with Wrangler logged in to the Cloudflare account that holds `nirlotan.com`:

```bash
npx wrangler deploy
```

`wrangler.jsonc` puts the Worker on `nirlotan.com/GlideDeck*` (and the lowercase spelling,
which redirects). The rest of nirlotan.com is a different Worker and is not touched.
`worker.js` only handles what plain file serving cannot: byte ranges for the videos
(Safari needs them), the spelling redirect and the 404 page.

## Preview

```bash
npx wrangler dev
```

Then open http://localhost:8787/GlideDeck/.

## Refresh the content

- **The live show** in "Fly it yourself" is GlideDeck's own web page export of the welcome
  canvas. After the app changes, from the repository root:

  ```bash
  cargo run --release -p glidedeck -- new welcome.glide.json --demo --force
  cargo run --release -p glidedeck -- export-html welcome.glide.json -o site/public/GlideDeck/show/index.html
  ```

  If the stops on its path change, update the list in `index.html` (`#stops`) to match.

- **Pictures and clips** come from the app footage and screenshots:

  ```bash
  python site/scripts/make_media.py
  ```

  It needs FFmpeg and Pillow, and the footage in `marketing/video/public/footage/`
  (see `marketing/video/README.md`).

Fonts are Bricolage Grotesque and Inter (SIL Open Font License), served from `fonts/`.
