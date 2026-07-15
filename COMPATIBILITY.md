# Distribution compatibility

Pigment is a GTK4/libadwaita app. Whether it runs on your distribution comes
down to three version floors:

| Component | Minimum | Why |
|---|---|---|
| **GTK 4** | **≥ 4.12** | The GUI is built against the GTK 4.12 API (`v4_12`). |
| **libadwaita** | **≥ 1.4** | Uses libadwaita 1.4 widgets (e.g. `AboutWindow`). |
| **Rust** | **≥ 1.96** | Needed only to *build* from source. |
| **Flatpak** | any | Runtime dependency — Pigment drives the Sober Flatpak. |

> **Rust note:** most distributions' packaged `rustc` is older than 1.96.
> If your build fails on the toolchain version, install Rust with
> [rustup](https://rustup.rs/) (`rustup default stable`) and build again.

If GTK or libadwaita is below the floor, the build fails at **compile time**
with a "version too old" error — it does not build a broken binary.

## Arch-based

The AUR packages (`pigment`, `pigment-git`) work here.

| Distro | Status | Notes |
|---|---|---|
| **Arch Linux** | ✅ Works | Reference platform. |
| **CachyOS** | ✅ Works | Ships the Arch libraries; AUR support is built into Pamac. |
| **EndeavourOS** | ✅ Works | Uses the Arch repositories; install an AUR helper (e.g. `yay`) as usual. |
| **Manjaro** | ✅ Works | AUR is opt-in in Pamac. **Run a full update first** (`sudo pacman -Syu`) — a stale snapshot can carry a libadwaita below the 1.4 floor and fail the build. |
| **SteamOS / Steam Deck** | ⚠️ Not via AUR | The read-only, A/B system image disables `pacman` and wipes system-level installs on OS updates. Use a Flatpak instead (planned — see below). |

## Debian-based

There is **no `.deb`** yet, and the AUR packages do not apply here — so today
this means building from source. Only releases new enough to meet the
libadwaita 1.4 floor can run it:

| Distro / release | libadwaita | Status |
|---|---|---|
| **Ubuntu 24.04 LTS** and newer | 1.5+ | ✅ Works |
| **Ubuntu 22.04 LTS** | 1.1 | ❌ Too old |
| **Debian 13 (Trixie)** and newer | 1.7+ | ✅ Works |
| **Debian 12 (Bookworm)** | 1.2 | ❌ Too old |
| **Linux Mint 22** (Ubuntu 24.04 base) | 1.5 | ✅ Works |
| **Linux Mint 21.x** (Ubuntu 22.04 base) | 1.1 | ❌ Too old |
| **Pop!_OS (COSMIC / 24.04 base)** | 1.5 | ✅ Works |
| **Pop!_OS 22.04** | 1.1 | ❌ Too old |

**Rule of thumb:** anything on an **Ubuntu 24.04 base or newer**, or **Debian 13
or newer**, works. The 22.04 / Debian 12 generation does not.

## Coming: Flatpak

A Flatpak is the planned way to reach the distributions above that fall below
the library floor (older Ubuntu/Debian LTS) and the Steam Deck. A Flatpak bundles
its own GNOME runtime, so the host's GTK/libadwaita versions no longer matter and
no Rust toolchain is needed to install. It is not yet published.

---

*Versions verified July 2026. If your distribution isn't listed, check its GTK 4
and libadwaita versions against the floors above:*
`pkg-config --modversion gtk4 libadwaita-1`
