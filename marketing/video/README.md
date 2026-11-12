# The GlideDeck marketing video

A 70-second video in two cuts, built in code so it can be re-rendered whenever the app or
the script changes:

| File | For | Format |
|---|---|---|
| `out/glidedeck-desktop.mp4` | GitHub (README, release notes), a website, YouTube | 16:9, 1920 × 1080, H.264 + AAC, under 10 MB |
| `out/glidedeck-mobile.mp4` | Reels, Shorts, TikTok, LinkedIn | 9:16, 1080 × 1920, H.264 + AAC, burned-in captions |

The research, the message and the script are in **[SCRIPT.md](SCRIPT.md)**.

## Putting it on GitHub

GitHub plays a video in a README only when it is uploaded through the editor, not when it is
a file in the repository. Edit the README on github.com, drag `out/glidedeck-desktop.mp4`
into the text box, and GitHub inserts a `https://github.com/user-attachments/...` link that
renders as a player. The file is kept under 10 MB, the limit for free accounts.

## How it is made

- **The app footage is real.** `crates/glidedeck/src/video_capture.rs` drives the actual
  editor and show through `egui_kittest` on a GPU device and pipes every frame to FFmpeg, so
  the clips are exact and repeatable. It writes each clip's named moments and pointer track
  to a `.json` file, which the video uses to cut on the action and to draw the cursor.
- **The deck that gets imported is a real PowerPoint file**, `assets/launch-plan.pptx`,
  written by `scripts/make_pptx.py`.
- **The video is a [Remotion](https://www.remotion.dev) project** (`src/`). Both cuts are the
  same composition at two sizes; each scene lays itself out for wide or tall.
- **The narration is ElevenLabs** (`scripts/voice.mjs`), one take per line in
  `src/beats.json`. The script trims long pauses and saves word timings to `src/vo.json`.
  Scenes, captions and footage are all timed from those words, so a new take or a changed
  line re-times the whole video.
- **Music and sound effects**: `scripts/sound.mjs` asks ElevenLabs for a music bed.
  `scripts/beat.py` and `scripts/sfx.py` synthesize a fallback beat and the whoosh, click and
  pop sounds locally.

## Rebuilding

You need Node 20+, FFmpeg, Python 3 with `numpy` and `python-pptx`, and Rust (for the footage).

```bash
npm install
```

1. **Footage** (only when the app changes). From the repository root:

   ```bash
   GLIDEDECK_VIDEO_OUT=/tmp/gd-footage cargo test -p glidedeck video_footage -- --ignored --nocapture
   ```

   Copy the `.mp4` files to `public/footage/`, the `.json` files to `src/footage/`, and save
   the first frame of `hero.mp4` and `hero-tall.mp4` as `public/footage/hero-poster.png` and
   `hero-tall-poster.png`.

2. **Voice.** Put `ELEVENLABS_API_KEY` and `ELEVENLABS_VOICE_ID` in `.env` (not committed), then:

   ```bash
   npm run voice
   ```

   `npm run voice -- hook close` re-records only those lines. `npm run voice -- --retime`
   re-trims the takes already on disk without calling the API.

3. **Music.** With an API key that has the *Music Generation* permission:

   ```bash
   npm run music
   ```

   Without it, `npm run music:synth` writes the fallback beat.

4. **Render and encode.**

   ```bash
   npm run build
   ```

`npm run studio` opens the Remotion studio for scrubbing and editing with live preview.

## Licences

Remotion is free for individuals and for companies of up to three people; larger companies
need a [company licence](https://www.remotion.dev/license). Audio made with ElevenLabs is
covered by the plan of the account that made it. Fonts are Bricolage Grotesque and Inter
(SIL Open Font License), loaded from Google Fonts at render time.
