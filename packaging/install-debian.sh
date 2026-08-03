#!/usr/bin/env bash
# pigment installer for debian n its derivatives. source build only, theres no .deb yet.
#
# usage:
#   ./packaging/install-debian.sh                    # per user, into ~/.local
#   ./packaging/install-debian.sh --prefix /usr      # system wide, will sudo
#   ./packaging/install-debian.sh --yes              # dont ask me anything js do it
#   ./packaging/install-debian.sh --skip-deps        # i already got the apt stuff
#
# ts checks the gtk/libadwaita floors BEFORE building, bc bookworm fails ~4 min
# into cargo w/ a wall of sys crate errors and thats a horrible way to find out.

set -euo pipefail

PREFIX="${HOME}/.local"
ASSUME_YES=0
SKIP_DEPS=0

# floors. gtk n adw are compile time feature gates in crates/pigment/Cargo.toml,
# so being under them is a build error not a broken binary. rust one gets read
# out the workspace manifest below so it cant drift.
GTK_MIN="4.12"
ADW_MIN="1.4"
RUST_MIN="1.96"

APT_DEPS=(build-essential pkg-config libgtk-4-dev libadwaita-1-dev git curl)

# ---- yapping helpers ---------------------------------------------------------

bold=$'\033[1m'; red=$'\033[31m'; yellow=$'\033[33m'; green=$'\033[32m'; off=$'\033[0m'
say()  { printf '%s==>%s %s\n' "$bold" "$off" "$*"; }
warn() { printf '%s==>%s %s\n' "$yellow" "$off" "$*" >&2; }
die()  { printf '%serror:%s %s\n' "$red" "$off" "$*" >&2; exit 1; }
ok()   { printf '%s  ok%s %s\n' "$green" "$off" "$*"; }

ask() {
    # returns 0 for yes. --yes short circuits, n so does a non tty so ts doesnt
    # hang forever in ci or when someone pipes it into bash
    [ "$ASSUME_YES" = 1 ] && return 0
    [ -t 0 ] || return 1
    local reply
    read -r -p "    $1 [y/N] " reply
    [[ "$reply" =~ ^[Yy] ]]
}

# ---- args --------------------------------------------------------------------

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)    PREFIX="${2:?--prefix needs a path}"; shift 2 ;;
        --prefix=*)  PREFIX="${1#*=}"; shift ;;
        -y|--yes)    ASSUME_YES=1; shift ;;
        --skip-deps) SKIP_DEPS=1; shift ;;
        -h|--help)   sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)           die "unknown arg: $1 . try --help" ;;
    esac
done

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[ -f "$REPO/Cargo.toml" ] || die "cant find the repo root from $REPO"

# sudo only when we actually need it. per user installs into ~/.local shouldnt
# be asking for a password n if it does thats a bug
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    SUDO="sudo"
    command -v sudo >/dev/null || die "need root or sudo to install packages"
fi

# ---- 0. is ts even debian ----------------------------------------------------

command -v apt-get >/dev/null || die "no apt-get here. ts script is debian/ubuntu only, see COMPATIBILITY.md"

distro="debian-ish"
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    distro="$(. /etc/os-release && echo "${PRETTY_NAME:-$ID}")"
fi
say "installing pigment on $distro, prefix $PREFIX"

# ---- 1. apt deps -------------------------------------------------------------

if [ "$SKIP_DEPS" = 1 ]; then
    say "skipping apt deps like u asked"
else
    say "installing build deps"
    echo "    ${APT_DEPS[*]}"
    # no libssl-dev on purpose, ureq resolves to rustls in Cargo.lock so theres
    # no openssl anywhere in the tree
    $SUDO apt-get update
    $SUDO apt-get install -y "${APT_DEPS[@]}"
fi

# ---- 2. the floors, ts is the whole reason the script exists ------------------

say "checking version floors"
command -v pkg-config >/dev/null || die "pkg-config missing, rerun without --skip-deps"

check_floor() {
    local mod="$1" min="$2" label="$3" have
    have="$(pkg-config --modversion "$mod" 2>/dev/null)" \
        || die "$label not found. rerun without --skip-deps"
    if dpkg --compare-versions "$have" ge "$min"; then
        ok "$label $have  (needs >= $min)"
    else
        printf '\n'
        warn "$label is $have, pigment needs >= $min"
        cat >&2 <<EOF

    ts release is below the floor n theres nothing to do abt it locally. gtk n
    libadwaita are pinned as compile time features so cargo would js fail later
    w/ a version too old error, it wont build u a broken binary.

    known bad:  debian 12 bookworm   gtk 4.8 / libadwaita 1.2
                ubuntu 22.04 jammy   gtk 4.6 / libadwaita 1.1
    known good: debian 13 trixie     gtk 4.18.6 / libadwaita 1.7.6
                ubuntu 24.04 noble   gtk 4.14.2 / libadwaita 1.5.0
                ubuntu 26.04 resolute gtk 4.22.2 / libadwaita 1.9.0

    there used to be a flatpak that dodged ts by bundling its own runtime.
    that got dropped, so upgrading the distro is the only path now.
    see COMPATIBILITY.md
EOF
        exit 1
    fi
}

check_floor gtk4 "$GTK_MIN" "gtk4"
check_floor libadwaita-1 "$ADW_MIN" "libadwaita"

# ---- 3. rust -----------------------------------------------------------------

# read the real floor out the workspace so ts cant go stale when it gets bumped
manifest_rust="$(grep -m1 '^rust-version' "$REPO/Cargo.toml" | cut -d'"' -f2 || true)"
[ -n "$manifest_rust" ] && RUST_MIN="$manifest_rust"

rust_have() { command -v cargo >/dev/null && cargo --version | awk '{print $2}' || echo 0; }
rust_ok()   { dpkg --compare-versions "$(rust_have)" ge "$RUST_MIN"; }

# rustup drops its shims in ~/.cargo/bin n neither the apt pkg nor --no-modify-path
# touches ur PATH, so we gotta put it in front of /usr/bin ourselves or wed js keep
# seeing the old apt cargo
use_rustup_toolchain() {
    export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
    rustup default stable >/dev/null 2>&1 || rustup toolchain install stable
    export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
}

if rust_ok; then
    ok "cargo $(rust_have)  (needs >= $RUST_MIN)"
else
    # ts is not an edge case, NO debian or ubuntu release ships a new enough rustc.
    # verified 2026-08-03: trixie 1.85.0, ubuntu 24.04 1.75, ubuntu 26.04 1.93.1.
    # so everyone lands here n rustup is basically mandatory
    echo
    if [ "$(rust_have)" = 0 ]; then
        warn "no cargo on PATH"
    else
        warn "cargo is $(rust_have), pigment needs >= $RUST_MIN"
    fi
    echo "    no debian/ubuntu release packages rust that new, so ts needs rustup."
    echo

    if ask "install rust via rustup now?"; then
        # apt first, its in trixie n resolute n noble. beats piping curl into sh.
        # its in universe on ubuntu tho so it can js not be there
        if $SUDO apt-get install -y rustup 2>/dev/null && command -v rustup >/dev/null; then
            ok "rustup from apt"
        else
            warn "no rustup in apt, falling back to the upstream installer"
            echo "    ts downloads n runs https://sh.rustup.rs"
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
        fi
        use_rustup_toolchain
        rust_ok || die "rustup ran but cargo is still $(rust_have). open a new shell n rerun"
        ok "cargo $(rust_have)"
    else
        die "cant build without rust >= $RUST_MIN. see https://rustup.rs then rerun"
    fi
fi

# ---- 4. build n install ------------------------------------------------------

say "building, ts takes a few min on first run"
make -C "$REPO" build

# do we need root to drop files in there. if PREFIX exists check it directly,
# else check the parent bc we gotta mkdir into it
install_sudo=""
if [ -d "$PREFIX" ]; then
    [ -w "$PREFIX" ] || install_sudo="$SUDO"
else
    [ -w "$(dirname "$PREFIX")" ] || install_sudo="$SUDO"
fi

say "installing to $PREFIX"
# CARGO=true makes the makefiles build dep a no op. matters bc we already built
# as the user n rustup lives in ~/.cargo, root wouldnt even find cargo n would
# leave root owned junk in target/ if it did
if [ -n "$install_sudo" ]; then
    $install_sudo make -C "$REPO" install PREFIX="$PREFIX" CARGO=true
else
    make -C "$REPO" install PREFIX="$PREFIX"
fi

# desktop entry wont show up til the db is rebuilt. best effort, dont care if
# it aint there
if command -v update-desktop-database >/dev/null; then
    $install_sudo update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
fi

# ---- 5. sober, the actual runtime --------------------------------------------

echo
say "checking sober"
if ! command -v flatpak >/dev/null; then
    warn "flatpak isnt installed. pigment is js a frontend, sober is what runs roblox"
    if ask "apt install flatpak now?"; then
        $SUDO apt-get install -y flatpak
        $SUDO flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
    fi
fi

if command -v flatpak >/dev/null; then
    if flatpak info org.vinegarhq.Sober >/dev/null 2>&1; then
        ok "sober $(flatpak info org.vinegarhq.Sober 2>/dev/null | awk '/^ *Version:/{print $2}')"
    else
        warn "sober isnt installed"
        if ask "install sober from flathub now?"; then
            flatpak install -y flathub org.vinegarhq.Sober
        else
            echo "    later:  flatpak install flathub org.vinegarhq.Sober"
        fi
    fi
fi

# ---- 6. done -----------------------------------------------------------------

echo
say "done"
echo "    installed pigmentlab n pigment-launch into $PREFIX/bin"

case ":$PATH:" in
    *":$PREFIX/bin:"*) echo "    run:  pigmentlab" ;;
    *)
        echo
        warn "$PREFIX/bin aint on ur PATH. add ts to ~/.bashrc or ~/.zshrc:"
        echo "        export PATH=\"$PREFIX/bin:\$PATH\""
        echo "    or run it directly:  $PREFIX/bin/pigmentlab"
        ;;
esac
echo
echo "    launch roblox thru sober once so it writes its config, then pigments"
echo "    settings page has smth to edit. uninstall w/ make uninstall PREFIX=$PREFIX"
