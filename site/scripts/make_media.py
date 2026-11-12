"""Builds the site's pictures and clips from the app's own footage and screenshots.

    python site/scripts/make_media.py

Sources (all made from the real app):
  marketing/video/public/footage/*.mp4   recorded by crates/glidedeck/src/video_capture.rs
  marketing/video/out/glidedeck-desktop.mp4   the 70-second film
  docs/screenshots/*.png

Writes site/public/GlideDeck/media/. Needs FFmpeg and Pillow.
"""
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
FOOTAGE = ROOT / "marketing" / "video" / "public" / "footage"
SHOTS = ROOT / "docs" / "screenshots"
OUT = ROOT / "site" / "public" / "GlideDeck" / "media"
TMP = OUT / "_tmp"
OUT.mkdir(parents=True, exist_ok=True)
TMP.mkdir(exist_ok=True)


def ff(*args):
    subprocess.run(["ffmpeg", "-v", "error", "-y", *map(str, args)], check=True)


def clip(src, dst, vf, crf, start=None, frames=None):
    """A silent H.264 loop that every browser plays inline."""
    args = []
    if start is not None:
        args += ["-ss", f"{start}"]
    args += ["-i", src]
    if frames is not None:
        args += ["-frames:v", f"{frames}"]
    ff(*args, "-vf", vf + ",format=yuv420p", "-an", "-c:v", "libx264", "-preset", "veryslow", "-crf", crf,
       "-profile:v", "high", "-movflags", "+faststart", OUT / dst)


def frame(src, n, name):
    """Frame `n` of a 30 fps clip, as a PNG in the scratch folder."""
    path = TMP / f"{name}.png"
    ff("-ss", f"{n / 30:.3f}", "-i", src, "-frames:v", "1", path)
    return path


def webp(src, dst, box=None, width=None, quality=84):
    im = Image.open(src).convert("RGB")
    if box:
        im = im.crop(box)
    if width and im.width > width:
        im = im.resize((width, round(im.height * width / im.width)), Image.LANCZOS)
    im.save(OUT / dst, "WEBP", quality=quality, method=6)
    print(f"{dst}: {im.width}x{im.height}, {(OUT / dst).stat().st_size // 1024} KB")


only = set(sys.argv[1:])
want = lambda name: not only or name in only

if want("clips"):
    # Hero scrap: the portrait recording of the welcome show.
    clip(FOOTAGE / "hero-tall.mp4", "scrap.mp4", "scale=608:1080:flags=lanczos", 27)
    ff("-i", OUT / "scrap.mp4", "-frames:v", "1", TMP / "scrap.png")
    webp(TMP / "scrap.png", "scrap.webp")
    # Importing a PowerPoint and arranging it three ways.
    clip(FOOTAGE / "arrange.mp4", "arrange.mp4", "scale=1280:720:flags=lanczos", 29, frames=660)
    ff("-ss", "6.6", "-i", OUT / "arrange.mp4", "-frames:v", "1", TMP / "arrange.png")
    webp(TMP / "arrange.png", "arrange.webp")
    # A carousel show gliding over its parallax backdrop.
    clip(FOOTAGE / "carousel.mp4", "carousel.mp4", "scale=1280:720:flags=lanczos", 27)
    ff("-ss", "1.5", "-i", OUT / "carousel.mp4", "-frames:v", "1", TMP / "carousel.png")
    webp(TMP / "carousel.png", "carousel.webp")

if want("film"):
    film = ROOT / "marketing" / "video" / "out" / "glidedeck-desktop.mp4"
    (OUT / "film.mp4").write_bytes(film.read_bytes())
    ff("-ss", "17.2", "-i", film, "-frames:v", "1", TMP / "film.png")
    webp(TMP / "film.png", "film.webp", width=1600)
    # The picture shown when the page is shared.
    Image.open(TMP / "film.png").convert("RGB").resize((1200, 675), Image.LANCZOS).crop((0, 22, 1200, 652)).save(OUT / "og.jpg", quality=86)

if want("stills"):
    # Close-ups of the real interface (2560 x 1440 editor footage; 2000 x 1200 screenshots).
    arranged = frame(FOOTAGE / "arrange.mp4", 640, "arranged")
    webp(arranged, "path.webp", box=(0, 1040, 1500, 1440))
    animate = frame(FOOTAGE / "animate.mp4", 188, "animate")
    webp(animate, "animate.webp", box=(2050, 680, 2540, 1230))
    share = frame(FOOTAGE / "animate.mp4", 262, "share")
    webp(share, "share.webp", box=(1800, 10, 2300, 570))
    webp(SHOTS / "presenter.png", "presenter.webp", box=(1200, 20, 2000, 920))
    webp(SHOTS / "templates.png", "templates.webp", box=(360, 240, 1010, 510))
    webp(SHOTS / "icons.png", "icons.webp", box=(30, 205, 500, 400))

for f in TMP.iterdir():
    f.unlink()
TMP.rmdir()
for f in sorted(OUT.iterdir()):
    print(f"{f.name}: {f.stat().st_size / 1024:.0f} KB")
