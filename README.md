# Pigment

**[Website](https://pigmentlab.net/)** ·
**[Documentation](https://pigmentlab.net/guide.html)** ·
**[Report a bug](https://github.com/SpeeNotPee/Pigment/issues/new/choose)**

A Roblox launcher and manager for Linux, a Bloxstrap front end for
[Sober](https://sober.vinegarhq.org/), the runtime that actually runs Roblox on
Linux.

Roblox Player can't run under Wine (Hyperion anti-cheat blocks it), so Sober is the only working method. Sober works well but
its entire configuration is a hand edited JSON file. Pigment gives you a real
GUI on top of it: settings, profiles, FastFlags, mods, and one-click launching.

Pigment does **not** reimplement the runtime. It drives Sober as it is: reading and
safely rewriting its config, composing mods into its overlay, launching it, and
(opt in) becoming the `roblox://` handler so it can apply your profile first.

## Features

- **Settings** — a typed UI over every Sober config key.
- **Runtime status** — the installed Sober build, the Roblox client version it
  fetched, and a warning when a newer Sober refresh is out.
- **Profiles** — named main/alt/testing setups, applied one at a time; the active
  one is applied automatically when you launch.
- **FastFlags** — a validating JSON editor in Bloxstrap's exact format, so Windows
  presets paste straight in.
- **Mods** — file overlays via Sober's sanctioned `asset_overlay`, validated
  against the real Roblox APK asset tree.
- **Default launcher** — opt-in, reversible takeover of the `roblox://` handler.

## Requirements

- The Sober Flatpak: `flatpak install flathub org.vinegarhq.Sober`
- GTK 4 (**≥ 4.12**) and libadwaita (**≥ 1.4**) at runtime; Rust **≥ 1.96** and
  Cargo to build.

> **Keep Sober updated.** VinegarHQ republishes Sober as periodic *refreshes*
> that keep the same version number (1.7.1 has been rebuilt roughly weekly), and
> Roblox rejects clients that fall behind. Pigment's Home page tells you when
> your build is stale; `flatpak update org.vinegarhq.Sober` fixes it.

See [COMPATIBILITY.md](COMPATIBILITY.md) for a per-distribution breakdown
(Arch, CachyOS, EndeavourOS, Manjaro, SteamOS, Ubuntu, Debian, Mint, Pop!_OS).

## Install

Per user (no root):

```sh
make install PREFIX=$HOME/.local
```

Make sure `$HOME/.local/bin` is on your `PATH`, then run `pigmentlab`.

System wide:

```sh
sudo make install PREFIX=/usr
```

### Arch - AUR

Install with your AUR helper:

```sh
yay -S pigment-launcher        # latest release
yay -S pigment-launcher-git    # builds the latest git main
```

Works on Arch and its derivatives (CachyOS, EndeavourOS, Manjaro). See
[COMPATIBILITY.md](COMPATIBILITY.md) for per-distro notes.

### Arch — from source (PKGBUILD)

```sh
make dist
cd packaging
makepkg -si
```

### Debian, Ubuntu, Mint, Pop!_OS

There is no `.deb`, but there's a script that does the whole source build:

```sh
./packaging/install-debian.sh                 # per-user, into ~/.local
./packaging/install-debian.sh --prefix /usr   # system-wide
```

It installs the apt build dependencies, **checks the GTK/libadwaita floors before
building** (so an unsupported release fails in seconds instead of minutes into
`cargo`), sets up rustup, then installs Pigment and optionally Sober. `--help`
lists the flags.

Needs **Debian 13 (Trixie)**, **Ubuntu 24.04**, or newer — verified working on
Debian 13 and Ubuntu 26.04. Expect the rustup step every time: no Debian or
Ubuntu release packages a `rustc` new enough for the 1.96 floor, not even 26.04
(1.93.1). See [COMPATIBILITY.md](COMPATIBILITY.md).

### Other distributions

No Flatpak, no `.deb`. Build from source with `make install` above; you need
GTK 4.12+, libadwaita 1.4+, and Rust 1.96+ on the host.

## Layout

- `pigment-core` — all logic (config, mods, APK, profiles, protocol, Sober).
- `pigment` — the GTK4/libadwaita GUI (installs the `pigmentlab` binary).
- `pigment-launch` — the fast `roblox://` protocol handler.

## Uninstall

```sh
make uninstall PREFIX=$HOME/.local   # match your install PREFIX
```


## Feedback & bug reports

Found a bug or have an idea? Please [open an issue](https://github.com/SpeeNotPee/Pigment/issues/new/choose).
There are templates for **bug reports** and **feature requests**. From inside the
app you can also use **Menu ▸ Report a Bug**, or **Menu ▸ About Pigment ▸ Report
an Issue**.

When reporting a bug, include your Pigment version (Menu ▸ About Pigment), your
Sober version, and your distro/desktop.

## Legal

- Pigment is **unofficial** and is not affiliated with, endorsed by, or sponsored
  by Roblox Corporation or VinegarHQ. All trademarks belong to their respective
  owners.
- Using unofficial clients is at your own risk; you are responsible for complying
  with Roblox's Terms of Use.
- [Terms of Use](TERMS.md) · [Privacy Notice](PRIVACY.md) · [License (MIT)](LICENSE)

## License

MIT — see [LICENSE](LICENSE).
