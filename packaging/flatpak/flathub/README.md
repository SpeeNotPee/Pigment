# Flathub submission

This directory holds the manifest for submitting Pigment to
[Flathub](https://flathub.org/), kept separate from the development manifest
(`../org.pigment.Pigment.yaml`, which builds the local checkout).

- **`org.pigment.Pigment.yaml`** — builds a pinned release tarball (`type: archive`),
  as Flathub requires. Currently pinned to **v0.1.2**.
- **`cargo-sources.json`** — the offline Rust crate sources, generated from
  `Cargo.lock`. Must sit next to the manifest.

## Local test build

```sh
flatpak install flathub org.flatpak.Builder      # one-time
cd packaging/flatpak/flathub
flatpak run org.flatpak.Builder --user --install --force-clean build-dir \
  org.pigment.Pigment.yaml
flatpak run org.pigment.Pigment
```

## Submitting

1. Fork [`flathub/flathub`](https://github.com/flathub/flathub) and create a
   branch named `org.pigment.Pigment` (branch = the app id).
2. Copy **`org.pigment.Pigment.yaml`** and **`cargo-sources.json`** into the
   repository root of that branch.
3. Open a pull request against `flathub/flathub`. The
   [flatpak-external-data-checker] bot and reviewers build and vet it; expect to
   respond to review comments.
4. Once merged, Flathub builds and publishes the app.

## When cutting a new release

1. Bump the version and cut a `vX.Y.Z` GitHub release (see the repo's release
   flow).
2. Regenerate `cargo-sources.json` if `Cargo.lock` changed:
   `flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json`
   then copy it here.
3. Update `url` + `sha256` in `org.pigment.Pigment.yaml` to the new tarball, and
   add a `<release>` entry to `packaging/org.pigment.Pigment.metainfo.xml`.

[flatpak-external-data-checker]: https://github.com/flathub/flatpak-external-data-checker
