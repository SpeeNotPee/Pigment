#!/usr/bin/env bash
#end to end smoke test of the real binaries. fake HOME, fake flatpak, fake xdg-mime,
#so it never launches sober or touches a real handler. runs native or inside docker
#usage: docker/integration.sh [target-dir]   (defaults to ./target/debug)
set -euo pipefail

BIN_DIR="$(cd "${1:-target/debug}" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export HOME="$WORK/home" XDG_CONFIG_HOME="$WORK/home/.config" XDG_DATA_HOME="$WORK/home/.local/share"
FAKEBIN="$WORK/bin"
mkdir -p "$FAKEBIN" "$XDG_DATA_HOME/applications"
export PATH="$FAKEBIN:$PATH"
pass=0
ok()   { echo "  ok   $*"; pass=$((pass + 1)); }
fail() { echo "  FAIL $*" >&2; exit 1; }

#fake flatpak: logs argv, pretends sober 1.8 is installed
cat >"$FAKEBIN/flatpak" <<'FP'
#!/usr/bin/env bash
echo "$*" >>"$HOME/flatpak.calls"
case "$1" in
  info) printf '       Version: 1.8.0\n        Origin: flathub\n        Commit: abc\n       Subject: Update Sober to 2026-11-05_c107424 (1.8.0 refresh) (01a28a7acd44)\n' ;;
  run) exit 0 ;;
esac
FP
#fake xdg-mime: a tiny key=value db so query/default round trip like the real one
cat >"$FAKEBIN/xdg-mime" <<'XM'
#!/usr/bin/env bash
db="$HOME/mime.db"; touch "$db"
case "$1" in
  query) grep "^$3=" "$db" | tail -1 | cut -d= -f2- ;;
  default) grep -v "^$3=" "$db" >"$db.tmp" || true; echo "$3=$2" >>"$db.tmp"; mv "$db.tmp" "$db" ;;
esac
XM
chmod +x "$FAKEBIN"/*

#sober config + an active profile w/ a flag n a setting
SOBER="$HOME/.var/app/org.vinegarhq.Sober"
mkdir -p "$SOBER/config/sober" "$XDG_CONFIG_HOME/pigment/profiles"
cat >"$SOBER/config/sober/config.json" <<'CFG'
// !!! STOP !!!
{
    "fflags": {},
    "use_opengl": false,
    "some_future_key": 7
}
CFG
cat >"$XDG_CONFIG_HOME/pigment/profiles/Ranked.json" <<'PR'
{ "name": "Ranked", "settings": { "use_opengl": true }, "fflags": { "FFlagFromProfile": true }, "mods": ["../../escape", "missing-mod"] }
PR
echo '{"active_profile":"Ranked"}' >"$XDG_CONFIG_HOME/pigment/state.json"
#the previous handler before pigment, a non-sober app, to prove switch back is honest
echo "x-scheme-handler/roblox=other-launcher.desktop" >"$HOME/mime.db"
echo "x-scheme-handler/roblox-player=other-launcher.desktop" >>"$HOME/mime.db"
touch "$XDG_DATA_HOME/applications/other-launcher.desktop"

echo "pigment-launch applies the active profile then hands off to sober"
"$BIN_DIR/pigment-launch" "roblox://experiences/start?placeId=1" 2>"$WORK/launch.err" || fail "launch exited nonzero"
grep -q '"FFlagFromProfile": true' "$SOBER/config/sober/config.json" && ok "profile flag written" || fail "flag missing"
grep -q '"use_opengl": true' "$SOBER/config/sober/config.json" && ok "profile setting written" || fail "setting missing"
grep -q '"some_future_key": 7' "$SOBER/config/sober/config.json" && ok "unknown sober key kept" || fail "unknown key dropped"
grep -q '^// !!! STOP' "$SOBER/config/sober/config.json" && ok "comment header kept" || fail "header lost"
[ -f "$SOBER/config/sober/config.json.pigment.orig" ] && ok "one-time original backup made" || fail "no .orig"
grep -q 'run org.vinegarhq.Sober roblox://experiences/start?placeId=1' "$HOME/flatpak.calls" && ok "uri forwarded to sober" || fail "uri not forwarded"
grep -q 'missing mods' "$WORK/launch.err" && ok "bad/missing mod names reported, not followed" || fail "no missing-mod warning"
[ ! -e "$SOBER/data/sober/asset_overlay/escape" ] && ok "traversal mod name copied nothing" || fail "traversal copied"

echo "pigment-launch survives a non-utf8 argument"
"$BIN_DIR/pigment-launch" "$(printf 'roblox://\xff')" 2>/dev/null && ok "no panic on non-utf8 arg" || fail "non-utf8 arg failed"

echo "--unregister restores the previous handler"
#register the way the gui does it: write the desktop file + defaults. done thru the binary's
#own restore path by first faking a registered state
cat >"$XDG_DATA_HOME/applications/net.pigmentlab.Pigment.Launcher.desktop" <<'D'
[Desktop Entry]
Type=Application
D
python3 - "$XDG_CONFIG_HOME/pigment/state.json" <<'PY'
import json,sys
p=sys.argv[1]; s=json.load(open(p))
s["previous_handlers"]={"x-scheme-handler/roblox":"other-launcher.desktop","x-scheme-handler/roblox-player":"other-launcher.desktop"}
s["future_key_from_newer_pigment"]=1
json.dump(s,open(p,"w"))
PY
xdg-mime default net.pigmentlab.Pigment.Launcher.desktop x-scheme-handler/roblox
xdg-mime default net.pigmentlab.Pigment.Launcher.desktop x-scheme-handler/roblox-player
"$BIN_DIR/pigment-launch" --unregister >/dev/null || fail "--unregister failed"
[ "$(xdg-mime query default x-scheme-handler/roblox)" = other-launcher.desktop ] && ok "roblox:// back to the previous app" || fail "roblox not restored: $(xdg-mime query default x-scheme-handler/roblox)"
[ "$(xdg-mime query default x-scheme-handler/roblox-player)" = other-launcher.desktop ] && ok "roblox-player:// back too" || fail "roblox-player not restored"
[ ! -e "$XDG_DATA_HOME/applications/net.pigmentlab.Pigment.Launcher.desktop" ] && ok "generated desktop file removed" || fail "desktop file left"
grep -q '"active_profile": "Ranked"' "$XDG_CONFIG_HOME/pigment/state.json" && ok "active profile kept in state" || fail "state clobbered"
grep -q future_key_from_newer_pigment "$XDG_CONFIG_HOME/pigment/state.json" && ok "unknown state keys kept" || fail "state extra lost"

echo "--unregister falls back to sober when the old handler is gone"
rm "$XDG_DATA_HOME/applications/other-launcher.desktop"
python3 - "$XDG_CONFIG_HOME/pigment/state.json" <<'PY'
import json,sys
p=sys.argv[1]; s=json.load(open(p)); s["previous_handlers"]={"x-scheme-handler/roblox":"other-launcher.desktop"}; json.dump(s,open(p,"w"))
PY
"$BIN_DIR/pigment-launch" --unregister >/dev/null
[ "$(xdg-mime query default x-scheme-handler/roblox)" = org.vinegarhq.Sober.desktop ] && ok "fell back to sober" || fail "no sober fallback"

echo
echo "integration: $pass checks passed"
