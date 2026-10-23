# GlideDeck's MCP server in a container (used by Glama's automated checks).
#
#   docker build -t glidedeck-mcp .
#   docker run -i --rm -v "$PWD:/work" glidedeck-mcp
#
# The server speaks MCP over stdio and works on .glide.json projects in /work.
# Rendering tools (render_slide, export_pdf) need a GPU; everything else,
# including PowerPoint export, runs headless.

FROM rust:1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libasound2-dev libxkbcommon-dev libwayland-dev libx11-dev \
        libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libudev-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p glidedeck

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        libasound2 libxkbcommon0 libwayland-client0 libx11-6 libxcursor1 libxrandr2 libxi6 \
        libgl1 libegl1 libvulkan1 mesa-vulkan-drivers libudev1 fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/glidedeck /usr/local/bin/glidedeck
WORKDIR /work
ENTRYPOINT ["glidedeck", "mcp"]
