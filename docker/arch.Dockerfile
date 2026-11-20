#arch, the reference platform. latest gtk/adw + stable rust, plus xvfb for a gui smoke test
FROM archlinux:latest
RUN pacman -Syu --noconfirm --needed base-devel gtk4 libadwaita rust git python \
        xorg-server-xvfb imagemagick appstream \
    && pacman -Scc --noconfirm
WORKDIR /src
