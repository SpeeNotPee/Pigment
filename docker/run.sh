#!/usr/bin/env bash
#build + test pigment in clean containers. one cmd, all distros:
#   docker/run.sh                # arch, ubuntu-24.04, debian-13
#   docker/run.sh arch           # js one
#repo gets copied INTO the container (mounted read only), so ur host target/ is never touched.
#--network host everywhere bc docker's bridge needs iptables, which isnt always there
set -euo pipefail
cd "$(dirname "$0")/.."
REPO="$PWD"
TARGETS=("${@:-arch ubuntu-24.04 debian-13}")
# shellcheck disable=SC2206
TARGETS=(${TARGETS[*]})
results=()

#the in-container script for arch + ubuntu: fmt/clippy where avail, build, test, integration, gui smoke
read -r -d '' CHECKS <<'SH' || true
set -euo pipefail
cp -a /repo/. /work && cd /work && rm -rf target
echo "== rustc $(rustc --version)"
if cargo fmt --version >/dev/null 2>&1; then cargo fmt --all -- --check; fi
if cargo clippy --version >/dev/null 2>&1; then cargo clippy --workspace --all-targets --locked -- -D warnings; fi
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
#every symbolic icon the ui asks for has to exist in the distro's adwaita, else u get the broken-img
#placeholder. adwaita keeps deleting names (emblem-ok died in 47) so ts catches it per distro
miss=0
for i in $(grep -rhoE '"[a-z0-9.-]+-symbolic"' crates/pigment/src | tr -d '"' | sort -u); do
  find /usr/share/icons/Adwaita /usr/share/icons/hicolor -name "$i.*" | grep -q . || { echo "icon missing: $i"; miss=1; }
done
[ $miss = 0 ] && echo "  icons ok"
[ $miss = 0 ]
docker/integration.sh target/debug
#gui smoke: real window on xvfb w/ a fake home, every page, must stay alive + render non-blank
export HOME=/tmp/gh XDG_CONFIG_HOME=/tmp/gh/.config XDG_RUNTIME_DIR=/tmp/rt GDK_BACKEND=x11
mkdir -p /tmp/rt && chmod 700 /tmp/rt
mkdir -p /tmp/gh/.var/app/org.vinegarhq.Sober/config/sober
printf '// hdr\n{ "fflags": {}, "use_opengl": false, "touch_mode": "fake_off" }\n' \
  >/tmp/gh/.var/app/org.vinegarhq.Sober/config/sober/config.json
Xvfb :77 -screen 0 940x660x24 >/dev/null 2>&1 & XV=$!
sleep 1
#warmup run, throw it away. gtk 4.24 + mesa 26 llvmpipe on xvfb NEVER presents the 1st frame
#while ~/.cache/mesa_shader_cache is empty (waited 40s, still pitch black). seeding the cache fixes it.
#not our bug, real gpus dont go thru this path, but w/o this home.png was blank on arch
DISPLAY=:77 target/debug/pigmentlab >/dev/null 2>&1 & AP=$!
sleep 5; kill $AP; wait $AP 2>/dev/null || true
for page in home settings fflags mods profiles activity; do
  DISPLAY=:77 PIGMENT_START_PAGE=$page target/debug/pigmentlab >/tmp/$page.log 2>&1 & AP=$!
  sleep 5
  kill -0 $AP || { cat /tmp/$page.log; echo "gui died on $page"; exit 1; }
  DISPLAY=:77 import -window root /out/$page.png
  kill $AP; wait $AP 2>/dev/null || true
  #actually check its not blank lol. a solid black/white frame has stddev ~0, real ui is ~0.07+
  sd=$(identify -format '%[fx:standard_deviation]' /out/$page.png)
  awk -v s="$sd" 'BEGIN{exit !(s>0.02)}' || { echo "gui blank on $page (stddev $sd)"; exit 1; }
  grep -E "CRITICAL|panicked" /tmp/$page.log && { echo "gui errors on $page"; exit 1; }
  echo "  gui ok: $page"
done
kill $XV
SH

#debian: the real installer, then the integration test vs what it installed
read -r -d '' DEBIAN <<'SH' || true
set -euo pipefail
cp -a /repo/. ~/pigment && cd ~/pigment && rm -rf target
echo "== rustc $(rustc --version)"
./packaging/install-debian.sh --skip-deps </dev/null
test -x ~/.local/bin/pigmentlab && test -x ~/.local/bin/pigment-launch
test -f ~/.local/share/metainfo/net.pigmentlab.Pigment.metainfo.xml
flatpak remotes --columns=name | grep -qx flathub || { echo "flathub remote missing (R4)"; exit 1; }
echo "  installer ok, flathub remote added"
#every symbolic icon the ui asks for has to exist in the distro's adwaita, else u get the broken-img
#placeholder. adwaita keeps deleting names (emblem-ok died in 47) so ts catches it per distro
miss=0
for i in $(grep -rhoE '"[a-z0-9.-]+-symbolic"' crates/pigment/src | tr -d '"' | sort -u); do
  find /usr/share/icons/Adwaita /usr/share/icons/hicolor -name "$i.*" | grep -q . || { echo "icon missing: $i"; miss=1; }
done
[ $miss = 0 ] && echo "  icons ok"
[ $miss = 0 ]
docker/integration.sh ~/.local/bin
make uninstall PREFIX="$HOME/.local" >/dev/null
test ! -e ~/.local/bin/pigmentlab && echo "  uninstall ok"
SH

for t in "${TARGETS[@]}"; do
  echo
  echo "======== $t ========"
  out="$REPO/docker/out/$t"
  mkdir -p "$out"
  docker build --network host -t "pigment-test:$t" -f "docker/$t.Dockerfile" docker
  if [ "$t" = debian-13 ]; then script=$DEBIAN; else script=$CHECKS; fi
  if docker run --rm --network host -v "$REPO:/repo:ro" -v "$out:/out" "pigment-test:$t" bash -c "$script"; then
    results+=("PASS  $t")
  else
    results+=("FAIL  $t")
  fi
done

echo
printf '%s\n' "${results[@]}"
printf '%s\n' "${results[@]}" | grep -q FAIL && exit 1 || exit 0
