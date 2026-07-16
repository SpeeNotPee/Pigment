# Flathub submission

This directory holds the manifest for submitting Pigment to
[Flathub](https://flathub.org/), kept separate from the development manifest
(`../net.pigmentlab.Pigment.yaml`, which builds the local checkout).

- **`net.pigmentlab.Pigment.yaml`** — builds a pinned release tarball (`type: archive`),
  as Flathub requires. Currently pinned to **v0.2.0**.
- **`cargo-sources.json`** — the offline Rust crate sources, generated from
  `Cargo.lock`. Must sit next to the manifest.

## Why the app id is `net.pigmentlab.Pigment`

Flathub requires the reverse-DNS app id to use a domain the project actually
controls: *"the author or the developer or the project must have control over
the domain."* We control `pigmentlab.net`; we do **not** control `pigment.org`,
so the original `org.pigment.Pigment` id was not eligible.

Flathub may ask us to prove ownership by serving a token at
`https://pigmentlab.net/.well-known/org.flathub.VerifiedApps.txt`. That also
earns the verified badge on the app's Flathub page.

## Local test build

```sh
flatpak install flathub org.flatpak.Builder      # one-time
cd packaging/flatpak/flathub
flatpak run org.flatpak.Builder --user --install --force-clean build-dir \
  net.pigmentlab.Pigment.yaml
flatpak run net.pigmentlab.Pigment
```

## Lint before submitting

Flathub gates submissions on `flatpak-builder-lint`, which has two separate
checks — the manifest, and the built repo:

```sh
cd packaging/flatpak/flathub
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest \
  net.pigmentlab.Pigment.yaml

# The repo check needs a build with --repo:
flatpak run org.flatpak.Builder --user --force-clean \
  --repo=$HOME/.cache/pigment-lint-repo build-dir net.pigmentlab.Pigment.yaml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo \
  $HOME/.cache/pigment-lint-repo
```

### Screenshot lint errors are a local artifact — ignore them

A plain local `repo` lint also reports `appstream-external-screenshot-url` and
`appstream-screenshots-not-mirrored-in-ostree`. **These are expected locally** —
Flathub's buildbot mirrors screenshots itself for manifest-based submissions like
ours. Verified 2026-07-16 by reproducing what the buildbot does:

```sh
rm -rf .flatpak-builder    # the mirroring runs in the `cleanup` stage; a cache
                           # hit silently skips it and the flags do nothing
flatpak run org.flatpak.Builder --user --force-clean \
  --compose-url-policy=full --mirror-screenshots-url=https://dl.flathub.org/media \
  --repo=$HOME/.cache/pigment-mirror-repo build-dir net.pigmentlab.Pigment.yaml
```

That commits a `screenshots/x86_64` ostree ref (all 4 screenshots fetched and
resized into Flathub's thumbnail set) and **both screenshot errors disappear**,
leaving only the two architectural errors below.

### Known lint errors — an exception request is required

After the screenshot noise above is accounted for, exactly two linter errors
remain. **Both are inherent to what Pigment is** (a front end that drives Sober,
which lives on the host), so neither can be "fixed" without gutting the app:

| Error | Why we need it |
| --- | --- |
| `finish-args-flatpak-spawn-access` | `--talk-name=org.freedesktop.Flatpak` — Pigment shells out via `flatpak-spawn --host` to launch Sober and query its version. |
| `finish-args-flatpak-appdata-folder-org.vinegarhq.Sober-create-access` | `--filesystem=~/.var/app/org.vinegarhq.Sober:create` — Pigment reads and writes Sober's config, FastFlags, mods and logs. |

Flathub's policy for both is *"granted on sufficient explanation being provided"*,
via a pull request to their exception file. So publishing requires that exception
request in addition to the submission PR.

> **Write the exception request yourself.** Flathub's linter docs state: *"Please
> do not use LLMs in any way to handle exceptions PRs. The exception can be
> permanently denied in that case."* The per-permission rationale in
> `net.pigmentlab.Pigment.yaml`'s comments is there as background — do not paste
> generated prose into the exception PR.

## Submitting

**Note:** Flathub's requirements state that submission pull requests *"must not
be generated, opened, or automated using AI tools or agents."* A human maintainer
opens this PR.

1. Fork [`flathub/flathub`](https://github.com/flathub/flathub/fork). Leave
   *"Copy the master branch only"* **unchecked** — the `new-pr` branch is needed.
2. Branch from `new-pr` (**not** `master`):
   ```sh
   git checkout new-pr
   git checkout -b net.pigmentlab.Pigment new-pr
   ```
3. Copy **`net.pigmentlab.Pigment.yaml`** and **`cargo-sources.json`** into the
   repository root of that branch. The manifest must be at the top level and
   named after the app id. Do not include source code or build artifacts.
4. Commit, push, and open a pull request **against the `new-pr` base branch** —
   not `master`. Title it `Add net.pigmentlab.Pigment`.
5. Respond to the review. Once merged, Flathub builds and publishes the app.

## When cutting a new release

1. Bump the version and cut a `vX.Y.Z` GitHub release (see the repo's release
   flow).
2. Regenerate `cargo-sources.json` only if `Cargo.lock` gained or changed
   **external** crates — a workspace-only version bump does not affect it:
   `flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json`
   then copy it here.
3. Update `url` + `sha256` in `net.pigmentlab.Pigment.yaml` to the new tarball, and
   add a `<release>` entry to `packaging/net.pigmentlab.Pigment.metainfo.xml`.
