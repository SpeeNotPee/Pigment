#!/usr/bin/env bash
# driver.sh — launch the Pigment GTK4 GUI on a throwaway Xvfb display and
# screenshot it. This never touches the user's real (Wayland/KDE) session.
#
# Usage:
#   driver.sh [PAGE] [OUT_PNG]
#     PAGE    one of: home settings fflags mods profiles activity  (default: home)
#             the special value "about" opens the About window instead
#     OUT_PNG output path (default: .claude/skills/run-pigment/screenshots/<page>.png)
#
# Examples:
#   driver.sh                       # Home page -> screenshots/home.png
#   driver.sh profiles              # Profiles page
#   driver.sh about /tmp/about.png  # About dialog to a custom path
#
# Requires: an already-built ./target/debug/pigmentlab (run `cargo build --bin pigmentlab`).
set -euo pipefail

# Resolve repo root: this script lives at <root>/.claude/skills/run-pigment/driver.sh
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

PAGE="${1:-home}"
DISPLAY_NUM=99
SCR_DIR="$ROOT/.claude/skills/run-pigment/screenshots"
mkdir -p "$SCR_DIR"

BIN="$ROOT/target/debug/pigmentlab"
[ -x "$BIN" ] || BIN="$ROOT/target/release/pigmentlab"
if [ ! -x "$BIN" ]; then
  echo "pigmentlab binary not found. Run: cargo build --bin pigmentlab" >&2
  exit 1
fi

# Env that drives the app to a specific page (see crates/pigment/src/ui/mod.rs).
declare -a APP_ENV=(GDK_BACKEND=x11 "DISPLAY=:$DISPLAY_NUM")
if [ "$PAGE" = "about" ]; then
  OUT="${2:-$SCR_DIR/about.png}"
  APP_ENV+=(PIGMENT_SHOW_ABOUT=1)
else
  OUT="${2:-$SCR_DIR/$PAGE.png}"
  APP_ENV+=("PIGMENT_START_PAGE=$PAGE")
fi

# Start a private X server sized to the window. The window opens at 940x660
# (main window) at 0,0 with no window manager, so a matching screen crops tight.
# The About dialog is smaller and also centers at the origin, so this size fits.
Xvfb ":$DISPLAY_NUM" -screen 0 940x660x24 -nolisten tcp >/tmp/pigment-xvfb.log 2>&1 &
XVFB_PID=$!
cleanup() { kill "$APP_PID" 2>/dev/null || true; kill "$XVFB_PID" 2>/dev/null || true; }
trap cleanup EXIT
sleep 1  # let Xvfb come up

# Launch the GUI in the background.
env "${APP_ENV[@]}" "$BIN" >/tmp/pigment-app.log 2>&1 &
APP_PID=$!

# Give GTK time to realize the window (+400ms for the About timeout hook).
sleep 3

if ! kill -0 "$APP_PID" 2>/dev/null; then
  echo "pigment exited early. Log:" >&2
  cat /tmp/pigment-app.log >&2
  exit 1
fi

# Capture the whole root window (import -window root grabs the full screen).
DISPLAY=":$DISPLAY_NUM" import -window root "$OUT"
echo "Wrote $OUT"
