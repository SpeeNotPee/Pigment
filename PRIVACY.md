# Privacy Notice

_Last updated: 20 November 2026_

Pigment is a local desktop application. It has **no servers, no accounts, and no
analytics or telemetry**. Pigment does not collect, receive, or store any of
your data.

## What stays on your device

- Pigment reads and writes Sober configuration, its asset overlay, and its logs
  on your machine.
- Pigment stores your profiles and mod library under `~/.config/pigment/`.

## Network activity

Pigment makes network requests only for the features below, sending only the data
described. Nothing is contacted when Pigment starts except the Flathub check on the
Home page; everything else happens when you open the page that needs it.

- **Sober update check (Home page).** Pigment runs `flatpak remote-info` so Flatpak
  asks Flathub (`dl.flathub.org`) for the newest Sober build, to warn you when
  your copy is out of date. Flatpak sends the same request it sends for
  `flatpak update`.
- **Game names (Activity page, and Discord presence if enabled).** To show
  readable game names, Pigment sends a Roblox *universe ID* — a public game
  identifier taken from your Sober logs — to Roblox's public web API
  (`games.roblox.com`). No personal information is sent. This is best-effort; if
  you are offline, place IDs are shown instead.
- **Preset catalog (Mods page).** The first time you open the Mods page in a
  session, Pigment downloads the list of downloadable presets from GitHub
  (`raw.githubusercontent.com/SpeeNotPee/Pigment/…/catalog/presets.json`). No
  data is sent beyond a normal web request.
- **Preset downloads (only when you click Install).** Installing a downloadable
  preset fetches its files from the HTTPS address listed in the catalog, which
  may be a third-party site (for example a community archive on GitHub). Each
  file is checked against a pinned SHA-256 checksum before it is installed.
- **Discord Rich Presence (optional, off by default).** If you enable it, Pigment
  connects to your **local** Discord client over its IPC socket and sends the
  current game name and status. Discord then shares that according
  to your Discord settings. Pigment makes no other connection to Discord.

Pigment does not "phone home", and it does not transmit your configuration,
FastFlags, mods, or logs anywhere.

## Third parties

The Sober runtime, the Roblox platform, and Discord are independent and have their
own privacy policies. Pigment does not control what they collect.

## Contact

Questions: open an issue at <https://github.com/SpeeNotPee/Pigment/issues>.

---

_I'm not a fucking lawyer but I'll just put here incase you motherfuckers decide to sue or some shit._
