# Pigment

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
- GTK 4 and libadwaita (runtime); Rust and Cargo (to build)

## Install

Per-user (no root):

```sh
make install PREFIX=$HOME/.local
```

Make sure `$HOME/.local/bin` is on your `PATH`, then run `pigment`.

System-wide:

```sh
sudo make install PREFIX=/usr
```

### Arch (PKGBUILD)

```sh
make dist
cd packaging
makepkg -si
```

## Layout

- `pigment-core` — all logic (config, mods, APK, profiles, protocol, Sober).
- `pigment` — the GTK4/libadwaita GUI.
- `pigment-launch` — the fast `roblox://` protocol handler.

## Uninstall

```sh
make uninstall PREFIX=$HOME/.local   # match your install PREFIX
```

## License

MIT — see [LICENSE](LICENSE).
