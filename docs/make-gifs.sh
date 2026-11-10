#!/usr/bin/env bash
# Turns the frame folders written by the `readme_gif_frames` test into the
# README GIFs in docs/gifs/. Needs FFmpeg.
#
#   GLIDEDECK_GIF_FRAMES=/tmp/frames cargo test -p glidedeck readme_gif_frames -- --ignored
#   docs/make-gifs.sh /tmp/frames
set -euo pipefail
frames="${1:?usage: docs/make-gifs.sh <frames dir>}"
out="$(cd "$(dirname "$0")" && pwd)/gifs"
mkdir -p "$out"
make() { # name width
  ffmpeg -v error -y -framerate 15 -i "$frames/$1/%04d.png" \
    -vf "scale=$2:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
    "$out/$1.gif"
}
make tour 800
make zoom 800
make editor 1000
ls -la "$out"
