#oldest supported floor: gtk 4.14 + libadwaita 1.5, built w/ exactly rust 1.92 (the rust-version floor).
#if this breaks, either a dep raised its msrv or i used an api newer than the floors
#librsvg2-common = svg pixbuf loader. --no-install-recommends skips it n gtk 4.14 then cant draw
#adwaita svg icons at all (real ubuntu desktops always have it)
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config libgtk-4-dev libadwaita-1-dev curl ca-certificates git python3 \
        xvfb xauth imagemagick librsvg2-common \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain 1.92 --no-modify-path
WORKDIR /src
