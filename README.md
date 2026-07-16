# Pigment

**[Website](https://pigmentlab.net/)** ·
**[Documentation](https://pigmentlab.net/guide.html)** ·
**[Report a bug](https://github.com/SpeeNotPee/Pigment/issues/new/choose)**

A Roblox launcher and manager for Linux — a Bloxstrap-class front end for
[Sober](https://sober.vinegarhq.org/), the runtime that actually runs Roblox on
Linux.

Roblox Player can't run under Wine (Hyperion anti-cheat blocks it), so Sober —
which runs the Android client — is the only working path. Sober works well but
its entire configuration is a hand-edited JSON file. Pigment gives you a real
GUI on top of it: settings, profiles, FastFlags, mods, and one-click launching.

Pigment does **not** reimplement the runtime. It drives Sober as-is: reading and
safely rewriting its config, composing mods into its overlay, launching it, and
(opt-in) becoming the `roblox://` handler so it can apply your profile first.

## Features

- **Settings** — a typed UI over every Sober config key.
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

See [COMPATIBILITY.md](COMPATIBILITY.md) for a per-distribution breakdown
(Arch, CachyOS, EndeavourOS, Manjaro, SteamOS, Ubuntu, Debian, Mint, Pop!_OS).

## Install

Per-user (no root):

```sh
make install PREFIX=$HOME/.local
```

Make sure `$HOME/.local/bin` is on your `PATH`, then run `pigmentlab`.

System-wide:

```sh
sudo make install PREFIX=/usr
```

### Arch — AUR

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

### Flatpak (any distro)

For non-Arch distros — including older Ubuntu/Debian LTS and the Steam Deck —
build the Flatpak (bundles its own GTK/libadwaita, so host versions don't matter):

```sh
flatpak install flathub org.flatpak.Builder
flatpak run org.flatpak.Builder --user --install --force-clean \
  build-dir packaging/flatpak/net.pigmentlab.Pigment.yaml
flatpak run net.pigmentlab.Pigment
```

The sandbox drives the Sober Flatpak via `flatpak-spawn --host`. See
[COMPATIBILITY.md](COMPATIBILITY.md) for details.

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
