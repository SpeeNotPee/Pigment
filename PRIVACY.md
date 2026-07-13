# Privacy Notice

_Last updated: 13 July 2026_

Pigment is a local desktop application. It has **no servers, no accounts, and no
analytics or telemetry**. The developers do not collect, receive, or store any of
your data.

## What stays on your device

- Pigment reads and writes Sober's configuration, its asset overlay, and its logs
  on your machine.
- Pigment stores your profiles and mod library under `~/.config/pigment/`.

## Network activity

Pigment makes network requests only for the features below, sending only the data
described:

- **Game names (Activity page).** To show readable game names, Pigment sends a
  Roblox *universe ID* — a public game identifier taken from your Sober logs — to
  Roblox's public web API (`games.roblox.com`). No personal information is sent.
  This is best-effort; if you are offline, place IDs are shown instead.
- **Discord Rich Presence (optional, off by default).** If you enable it, Pigment
  connects to your **local** Discord client over its IPC socket and sends the
  current game name and status. Discord — not Pigment — then shares that according
  to your Discord settings. Pigment makes no other connection to Discord.

Pigment does not "phone home", and it does not transmit your configuration,
FastFlags, mods, or logs anywhere.

## Third parties

The Sober runtime, the Roblox platform, and Discord are independent and have their
own privacy policies. Pigment does not control what they collect.

## Contact

Questions: open an issue at <https://github.com/SpeeNotPee/Pigment/issues>.

---

_This notice is provided for transparency and is not legal advice._
