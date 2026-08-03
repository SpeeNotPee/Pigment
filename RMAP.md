# Roadmap

Last updated 2026-08-02, at **v0.2.0**.

Where things actually stand, what's left, and what Pigment deliberately won't do.
Ordered by what blocks what — not by ambition.

## Status

Every feature milestone from the original scope is done and shipping. The six GUI
pages (Home, Settings, FastFlags, Mods, Profiles, Activity) all run on real data;
there is no placeholder page left. 54 tests pass, clippy is clean, no open issues,
no `TODO`s in the source.

Pigment tracks Sober rather than pinning to it: Settings covers every documented
config key, and Home reports the installed *build* (not just the version, which
stays `1.7.1` across weekly refreshes) plus a warning when a newer one is out.

| | |
| --- | --- |
| Version | 0.2.0 — app id `net.pigmentlab.Pigment`, binaries `pigmentlab` + `pigment-launch` |
| Arch family | Published — [`pigment-launcher`](https://aur.archlinux.org/packages/pigment-launcher), `pigment-launcher-git` |
| Flatpak | **Dropped.** Pigment is a host-native app; it drives the Sober Flatpak but is not one |
| Everyone else | Source build (`make install PREFIX=$HOME/.local`) |
| Site | [pigmentlab.net](https://pigmentlab.net/) — rebuilt 2026-07-16, sources in `site/` |

So the work left is **distribution and verification**, not features.

## Blocked on a human — this cannot be delegated

### Remove the stray `pigment-git` AUR package

It was pushed before the `pigment` name collision was discovered and is misnamed.
Deletion needs the AUR web UI (Package Actions → Submit Request → Merge into
`pigment-launcher-git`, or Deletion); an SSH key only authorises push.

## Next up

Small, real, and unblocked.

- **Install the AppStream metainfo.** The `Makefile` installs the desktop file and
  icons but has no rule for `packaging/net.pigmentlab.Pigment.metainfo.xml`, so AUR
  and source installs ship without it and **won't appear properly in GNOME Software
  or KDE Discover**. Nothing installs it now that the Flatpak is gone, so this is the
  only route. Two lines, then a release.
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
- **Non-Arch source builds.** Everything here is built and run on Arch. Nobody has
  run `make install` on an actual Steam Deck, Ubuntu 24.04, or Mint.

The cheapest fix for the first two is one session on a real desktop with Discord open.

## Distribution reality

| Distro | Path | State |
| --- | --- | --- |
| Arch, CachyOS, EndeavourOS | AUR | Shipping |
| Manjaro | AUR | Shipping — but a stale snapshot can sit below the libadwaita 1.4 floor; full update first |
| Debian 13+, Ubuntu 24.04+, Mint 22, Pop!_OS | `packaging/install-debian.sh` | Clears the libadwaita 1.4 floor; the script handles apt deps, floor checks and rustup |
| Fedora | Source | Clears the floor; needs rustup for the 1.96 toolchain. No script yet |
| Steam Deck / SteamOS | Source, to `~/.local` | AUR can't work (immutable root, wiped on update); a per-user build survives, but the user supplies the toolchain |
| Ubuntu 22.04, Debian 12 | — | **Not supported.** libadwaita 1.2/1.1 is below the 1.4 floor, and there's no version of this that builds natively there |

Without a Flatpak, everything outside the Arch family is a source build. Packaging
that reaches those users — a `.deb`, a Copr, or a distro-native path — is the open
question, not a plan.

## Candidates, not commitments

Nothing here is owned, scheduled, or promised. Listed so the thinking isn't lost.

- **A dedicated install page.** The site is landing + guide; "which command do I run"
  is two answers today. Worth splitting out only if the packaging matrix grows.
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
- **Shipping Pigment as a Flatpak.** Dropped 2026-08-02. The manifests, the Flathub
  submission bundle, and the `flatpak-spawn --host` sandbox paths are all gone.
  Pigment runs on the host and calls the host's `flatpak` to drive Sober. This cost
  the cross-distro story — that's understood, and it's still the call.
- **Anything touching anti-cheat.** Not a gap. Not a maybe.
- **Auto-taking the `roblox://` handler.** Takeover is opt-in, labelled, reversible,
  and `pigment-launch` falls back to launching Sober directly on any failure rather
  than stranding the user.
