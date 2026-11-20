#debian 13 runs the REAL packaging/install-debian.sh. deps + rustup r preinstalled so the
#script runs w/o a tty, n flatpak is there w/o a flathub remote to exercise that fix
FROM debian:13
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config libgtk-4-dev libadwaita-1-dev curl ca-certificates git python3 \
        desktop-file-utils flatpak rustup sudo \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m tester && echo 'tester ALL=(ALL) NOPASSWD:ALL' >/etc/sudoers.d/tester
USER tester
RUN rustup default stable
ENV PATH=/home/tester/.cargo/bin:$PATH
WORKDIR /home/tester
