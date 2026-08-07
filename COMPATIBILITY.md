# Distribution compatibility

Pigment is a GTK4/libadwaita app. Whether it runs on your distribution comes
down to three version floors:

| Component | Minimum | Why |
|---|---|---|
| **GTK 4** | **≥ 4.12** | The GUI is built against the GTK 4.12 API (`v4_12`). |
| **libadwaita** | **≥ 1.4** | Uses libadwaita 1.4 widgets (e.g. `AboutWindow`). |
| **Rust** | **≥ 1.96** | Needed only to *build* from source. |
| **Flatpak** | any | Runtime dependency only — Pigment is not a Flatpak, but it drives the Sober one. |

> **Rust note:** most distributions' packaged `rustc` is older than 1.96.
> If your build fails on the toolchain version, install Rust with
> [rustup](https://rustup.rs/) (`rustup default stable`) and build again.

If GTK or libadwaita is below the floor, the build fails at **compile time**
with a "version too old" error — it does not build a broken binary.

## Arch-based

The AUR packages (`pigment-launcher`, `pigment-launcher-git`) work here.
(The base name `pigment` on the AUR is an unrelated image color-palette app.)

| Distro | Status | Notes |
|---|---|---|
| **Arch Linux** | ✅ Works | Reference platform. |
| **CachyOS** | ✅ Works | Ships the Arch libraries; AUR support is built into Pamac. |
| **EndeavourOS** | ✅ Works | Uses the Arch repositories; install an AUR helper (e.g. `yay`) as usual. |
| **Manjaro** | ✅ Works | AUR is opt-in in Pamac. **Run a full update first** (`sudo pacman -Syu`) — a stale snapshot can carry a libadwaita below the 1.4 floor and fail the build. |
| **SteamOS / Steam Deck** | ⚠️ Not via AUR | The read-only, A/B system image disables `pacman` and wipes system-level installs on OS updates. A per-user build (`make install PREFIX=$HOME/.local`) survives updates, but you must supply the Rust toolchain yourself via [rustup](https://rustup.rs/). |

## Debian-based

There is **no `.deb`**, and the AUR packages do not apply here — so this means
building from source. [`packaging/install-debian.sh`](packaging/install-debian.sh)
automates it end to end (apt dependencies, floor checks, rustup, install), and
refuses up front on the releases below. Only releases new enough to meet the
libadwaita 1.4 floor can run it:

| Distro / release | GTK 4 | libadwaita | Status |
|---|---|---|---|
| **Ubuntu 26.04 LTS** (Resolute Raccoon) | 4.22.2 | 1.9.0 | ✅ Works |
| **Ubuntu 24.04 LTS** (Noble Numbat) | 4.14.2 | 1.5.0 | ✅ Works |
| **Ubuntu 22.04 LTS** | 4.6 | 1.1 | ❌ Too old |
| **Debian 13 (Trixie)** and newer | 4.18.6 | 1.7.6 | ✅ Works — verified on real hardware |
| **Debian 12 (Bookworm)** | 4.8 | 1.2 | ❌ Too old |
| **Linux Mint 22** (Ubuntu 24.04 base) | 4.14 | 1.5 | ✅ Works |
| **Linux Mint 21.x** (Ubuntu 22.04 base) | 4.6 | 1.1 | ❌ Too old |
| **Pop!_OS (COSMIC / 24.04 base)** | 4.14 | 1.5 | ✅ Works |
| **Pop!_OS 22.04** | 4.6 | 1.1 | ❌ Too old |

*Ubuntu and Debian rows verified 2026-08-03 against `packages.ubuntu.com` and
`packages.debian.org`; Mint and Pop!_OS inherit their Ubuntu base. Debian 13
additionally verified end-to-end on real hardware 2026-08-03 —
`install-debian.sh` full run, build, install, and app launch.*

**Rule of thumb:** anything on an **Ubuntu 24.04 base or newer**, or **Debian 13
or newer**, works. The 22.04 / Debian 12 generation does not.

> **No Debian or Ubuntu release packages a new enough `rustc`** — not even the
> newest. Verified 2026-08-03: Debian 13 ships **1.85.0**, Ubuntu 24.04 **1.75.0**,
> Ubuntu 26.04 **1.93.1**, all below the **1.96** floor. So the rustup step is
> universal here, not an edge case. `rustup` itself *is* packaged
> (`apt install rustup`), and the install script prefers that over piping
> `sh.rustup.rs` into a shell.

## Distributions below the floor

Pigment is **not** packaged as a Flatpak, so there is no bundled-runtime escape
hatch: the "too old" rows above (Ubuntu 22.04, Debian 12, Mint 21, Pop!_OS
22.04) cannot run it. Upgrading to a release on the Ubuntu 24.04 / Debian 13
generation is the supported path.

Pigment runs unsandboxed on the host and calls the host's `flatpak` directly to
drive Sober, read `~/.var/app/org.vinegarhq.Sober`, and register the `roblox://`
handler.

---

*Versions verified July 2026. If your distribution isn't listed, check its GTK 4
and libadwaita versions against the floors above:*
`pkg-config --modversion gtk4 libadwaita-1`
