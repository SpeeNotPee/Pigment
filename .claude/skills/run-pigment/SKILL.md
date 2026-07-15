---
name: run-pigment
description: Build, launch, and screenshot Pigment — the GTK4/libadwaita Roblox launcher GUI (and its pigment-launch protocol handler). Use when asked to run, start, build, test, or screenshot Pigment, or to confirm a GUI change renders in the real app.
---

# Run Pigment

Pigment is a **Rust + GTK4/libadwaita desktop GUI** (a Bloxstrap-class front end
for Sober) plus a tiny GUI-less protocol-handler binary. It's a native GTK app,
so there is no browser or `chromium-cli` path — the agent path is
**`driver.sh`**, which launches the real `pigmentlab` binary on a private **Xvfb**
display and captures a screenshot with ImageMagick `import`. This never touches
the user's real Wayland/KDE session.

All paths below are relative to the repo root (`<repo>/`). The driver lives at
`.claude/skills/run-pigment/driver.sh`.

The workspace has three crates:
- `pigment-core` — all logic (config, mods, APK, profiles, protocol, Sober).
- `pigment` — the GTK4 GUI crate; builds the `pigmentlab` binary (what you screenshot).
- `pigment-launch` — the latency-critical `roblox://` handler; **launching it
  actually starts Sober** (see Gotchas).

## Prerequisites

Runtime libs + capture tools. All were already present on the dev machine
(Arch); verify with `which xvfb-run import Xvfb` and `pkg-config --exists gtk4`.

- **Arch:** `gtk4 libadwaita xorg-server-xvfb imagemagick` (plus `rust`).
- **Debian/Ubuntu equivalent** (not run here, listed for portability):
  `libgtk-4-dev libadwaita-1-dev xvfb imagemagick`.

## Build

```bash
cargo build --bin pigmentlab --bin pigment-launch
```

Fast and clean (finishes in ~1.5s incrementally). `cargo build --release
--workspace` (what the Makefile's `make build` runs) also works; the driver
prefers `target/debug/pigmentlab` and falls back to `target/release/pigmentlab`.

## Run (agent path) — driver.sh

The driver launches the GUI on Xvfb `:99`, waits for the window, screenshots the
root, then tears down Xvfb and the app.

```bash
.claude/skills/run-pigment/driver.sh                 # Home page -> screenshots/home.png
.claude/skills/run-pigment/driver.sh profiles        # Profiles page
.claude/skills/run-pigment/driver.sh about /tmp/a.png # About dialog to a custom path
```

- **First arg** (page): `home settings fflags mods profiles activity`, or the
  special `about` (opens the About window via the app's `PIGMENT_SHOW_ABOUT`
  hook). Default `home`.
- **Second arg** (optional): output PNG path. Default
  `.claude/skills/run-pigment/screenshots/<page>.png`.

**Look at the PNG afterward.** A good Home capture shows the sidebar (Home /
Settings / FastFlags / Mods / Profiles / Activity), the Pigment logo + hero, a
"Sober Runtime" card reading "Installed — 1.7.1", and the "Make Pigment the
default launcher" toggle. If Sober isn't installed on the machine, the runtime
card reads differently but the app still renders.

How it works (see the script): the app is driven to a page via the
`PIGMENT_START_PAGE` env var read in `crates/pigment/src/ui/mod.rs`; the About
dialog uses `PIGMENT_SHOW_ABOUT=1`, which opens it on a 400ms timeout — the
driver's 3s wait covers that.

## Run (human path)

On the user's real session, just:

```bash
cargo run --bin pigmentlab
```

A window opens; Ctrl-C or close it to quit. Useless headless (no display), which
is why the agent path uses Xvfb.

## Test

```bash
cargo test --workspace
```

47 tests, all in `pigment-core` (the GUI and launcher crates have no unit
tests). Fast, no display needed.

## Gotchas

- **`pigment-launch` with no/any URI actually launches Sober.** It is not a
  dry-run CLI — running `target/debug/pigment-launch` applies the active profile
  and then spawns the real Sober/Roblox runtime (you'll see `info: app:
  lifecycle:` log lines). To test it without starting Roblox, exercise the logic
  through `pigment-core`'s tests instead. If you do launch it, clean up with
  `pkill -f sober` / `flatpak kill org.vinegarhq.Sober`.
- **Xvfb screen is sized to the window (940x660), not oversized.** With no window
  manager the GTK window opens at the origin, so a matching screen crops the
  screenshot tight. A bigger screen leaves black margins on the right/bottom.
- **`GDK_BACKEND=x11` is required.** The dev machine is Wayland; without forcing
  the X11 backend, GTK tries Wayland and can't find the Xvfb display. The driver
  sets it.
- **Harmless libcurl warning.** Sober prints `libcurl.so.4: no version
  information available` — cosmetic, not a failure.
- **The About dialog overlaps the main window** in the `about` screenshot (it
  opens top-left over Home). That's expected; the dialog content is fully
  visible.

## Troubleshooting

- **`pigmentlab binary not found`** → run the Build step first
  (`cargo build --bin pigmentlab`).
- **Screenshot is all black / app exited early** → the driver prints
  `/tmp/pigment-app.log`; check it. Usually a missing GTK/adwaita lib
  (`pkg-config --exists gtk4 libadwaita-1`) or a leftover Xvfb on `:99`
  (`pkill Xvfb`).
- **`Cannot open display`** → a stale Xvfb held `:99`; `pkill Xvfb` and rerun.
