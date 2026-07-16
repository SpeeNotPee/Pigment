# Roadmap

Last updated 2026-07-16, at **v0.2.0**.

Where things actually stand, what's left, and what Pigment deliberately won't do.
Ordered by what blocks what — not by ambition.

## Status

Every feature milestone from the original scope is done and shipping. The six GUI
pages (Home, Settings, FastFlags, Mods, Profiles, Activity) all run on real data;
there is no placeholder page left. 48 tests pass, clippy is clean, no open issues,
no `TODO`s in the source.

| | |
| --- | --- |
| Version | 0.2.0 — app id `net.pigmentlab.Pigment`, binaries `pigmentlab` + `pigment-launch` |
| Arch family | Published — [`pigment-launcher`](https://aur.archlinux.org/packages/pigment-launcher), `pigment-launcher-git` |
| Flatpak | Builds, runs, drives Sober from inside the sandbox — **not yet on Flathub** |
| Everyone else | Source build only, until Flathub lands |
| Site | [pigmentlab.net](https://pigmentlab.net/) — rebuilt 2026-07-16, sources in `site/` |

So the work left is **distribution and verification**, not features.

## Blocked on a human — these cannot be delegated

Flathub's rules are explicit that submissions "must not be generated, opened, or
automated using AI tools or agents", and that using an LLM to handle an exception
request "can permanently deny" it. These three are yours alone.

### 1. Open the Flathub submission PR

Everything is prepared and test-built in `packaging/flatpak/flathub/`. The full
procedure — including the two easy-to-miss parts — is in that directory's README.
The short version:

- Fork `flathub/flathub` with **"Copy the master branch only" unchecked**.
- Branch **from `new-pr`**, and target the PR at **`new-pr`** — *not* `master`.
- Copy the manifest and `cargo-sources.json` to the **repo root**.
- Title it `Add net.pigmentlab.Pigment`.

### 2. Write the lint exception request

`flatpak-builder-lint` reports exactly two errors, and both are inherent to what
Pigment *is* — neither can be fixed without gutting the app:

| Error | Why Pigment needs it |
| --- | --- |
| `finish-args-flatpak-spawn-access` | `--talk-name=org.freedesktop.Flatpak` — Pigment shells out via `flatpak-spawn --host` to launch Sober and read its version. |
| `finish-args-flatpak-appdata-folder-org.vinegarhq.Sober-create-access` | `--filesystem=~/.var/app/org.vinegarhq.Sober:create` — Pigment reads and writes Sober's config, FastFlags, mods and logs. |

Flathub's policy for both is *"granted on sufficient explanation being provided"*,
via a separate PR to their exception file. Write it in your own words: Pigment is a
front end for Sober, Sober is a separate Flatpak on the host, so Pigment has to
launch it and read the config it manages. Do not paste generated prose — including
the rationale comments in the manifest.

### 3. Remove the stray `pigment-git` AUR package

It was pushed before the `pigment` name collision was discovered and is misnamed.
Deletion needs the AUR web UI (Package Actions → Submit Request → Merge into
`pigment-launcher-git`, or Deletion); an SSH key only authorises push.

## Next up

Small, real, and unblocked.

- **Serve the Flathub domain token.** `https://pigmentlab.net/.well-known/org.flathub.VerifiedApps.txt`
  proves you control `pigmentlab.net` — which is *why* the app id is
  `net.pigmentlab.Pigment` — and earns the verified badge. Drop it in `docs/`.
- **Install the AppStream metainfo.** The `Makefile` installs the desktop file and
  icons but has no rule for `packaging/net.pigmentlab.Pigment.metainfo.xml`, so AUR
  and source installs ship without it and **won't appear properly in GNOME Software
  or KDE Discover**. The Flatpak already installs it. Two lines, then a release.
- **Upload the Discord Rich Presence asset.** The presence wiring is tested, but the
  large image needs an asset keyed `pigment` uploaded to Discord app
  `1526262789927075950` or it silently shows nothing.
- **Decide the site `<title>`.** It currently leads with the outcome
  ("Roblox on Linux, done right"), matching the chosen positioning. If Bloxstrap
  matters for search, it belongs in the title too — one line in
  `site/index.template.html`.

## Verification gaps

Known-untested, and honest about why. None are known to be broken; they simply
cannot be exercised from a headless container.

- **The real click path.** Browser → "Play" → `roblox://` → `pigment-launch` →
  profile applied → Sober. Every piece is unit-tested and the handler registration
  is verified end-to-end, but nobody has clicked Play in a browser on a real desktop
  session and watched it work.
- **Live Discord presence.** Verified against a mock IPC socket only; Discord has
  never been running during a test.
- **Flatpak on real hardware.** It builds and runs here on Arch. Nobody has run it
  on an actual Steam Deck, Ubuntu 24.04, or Mint.

The cheapest fix for all three is one session on a real desktop with Discord open.

## Distribution reality

| Distro | Path | State |
| --- | --- | --- |
| Arch, CachyOS, EndeavourOS | AUR | Shipping |
| Manjaro | AUR | Shipping — but a stale snapshot can sit below the libadwaita 1.4 floor; full update first |
| Steam Deck / SteamOS | Flatpak | Needs Flathub — AUR can't work (immutable root, wiped on update) |
| Ubuntu 24.04+, Mint 22, Pop!_OS | Flatpak | Needs Flathub |
| Fedora | Flatpak | Needs Flathub |
| Ubuntu 22.04, Debian 12 | — | **Not supported.** libadwaita 1.2/1.1 is below the 1.4 floor. Flatpak is the only answer; there's no version of this that builds natively there |

Flathub isn't polish — it's the only route to most of that table.

## Candidates, not commitments

Nothing here is owned, scheduled, or promised. Listed so the thinking isn't lost.

- **A dedicated install page.** The site is landing + guide; "which command do I run"
  is now genuinely three answers. Worth splitting out if Flathub lands and the
  matrix grows.
- **Light theme for the site.** The redesign is dark-only *by choice* — it's a Linux
  desktop at night, and a light variant is a different design, not a recolour. If it's
  wanted, it's real work.
- **A game library page.** The original scope listed one; Activity currently covers
  recent games from Sober's logs. Whether that's the same thing, or a gap, is an open
  question rather than a plan.
- **Changelog page.** There are real releases now (0.1.0 → 0.2.0) and the metainfo
  already carries `<release>` entries to generate from.

## Non-goals

These are decided, not pending. Reopen only with a reason.

- **Simultaneous multi-instance.** Requires defeating single-instance in a closed
  binary — precisely the abuse VinegarHQ closed-sourced Sober to prevent, and
  Bloxstrap doesn't do it either. Profiles with sequential sessions are the
  replacement.
- **Reimplementing the runtime.** Pigment drives Sober as-is and always will. It
  reads and writes the same config Sober already uses, so you can stop using Pigment
  at any time and nothing breaks.
- **Anything touching anti-cheat.** Not a gap. Not a maybe.
- **Auto-taking the `roblox://` handler.** Takeover is opt-in, labelled, reversible,
  and `pigment-launch` falls back to launching Sober directly on any failure rather
  than stranding the user.
