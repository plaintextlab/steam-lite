#!/usr/bin/env bash
# steamlite installer: installs build dependencies for your distro, builds
# steamlite and installs it.
#
#   ./install.sh              install for the current user (~/.local)
#   ./install.sh --system     install to /usr/local (uses sudo)
#   ./install.sh --deps-only  only install build dependencies
#   ./install.sh --uninstall  remove what a previous run installed
#   ./install.sh --dry-run    print the commands instead of running them
#
# Supported: Void, Arch (+ Manjaro, EndeavourOS, CachyOS...), Debian/Ubuntu
# (+ Mint, Pop!_OS...), Fedora (+ Nobara, RHEL-likes), openSUSE, Alpine.
set -euo pipefail

MIN_RUST=1.83
MIN_GTK=4.16
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
OS_RELEASE=${OS_RELEASE:-/etc/os-release}   # overridable for testing

mode=install prefix="$HOME/.local" dry=0
for arg in "$@"; do
    case $arg in
        --system) prefix=/usr/local ;;
        --deps-only) mode=deps ;;
        --uninstall) mode=uninstall ;;
        --dry-run) dry=1 ;;
        -h|--help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown option: $arg (see --help)" >&2; exit 2 ;;
    esac
done

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; exit 1; }
run() {
    if [ "$dry" = 1 ]; then printf '   $ %s\n' "$*"; else "$@"; fi
}
sudo_run() {
    if [ "$(id -u)" = 0 ]; then run "$@"
    elif command -v sudo >/dev/null; then run sudo "$@"
    elif command -v doas >/dev/null; then run doas "$@"
    else die "need root to install packages: install sudo or doas, or run as root"; fi
}
# version_ge A B  ->  true if A >= B
version_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]; }

detect_distro() {
    [ -r "$OS_RELEASE" ] || die "cannot read $OS_RELEASE to detect your distro"
    # shellcheck disable=SC1090
    . "$OS_RELEASE"
    for id in ${ID:-} ${ID_LIKE:-}; do
        case $id in
            void) echo void; return ;;
            arch|archlinux|manjaro|endeavouros|cachyos|garuda|artix) echo arch; return ;;
            debian|ubuntu|linuxmint|pop|elementary|zorin|kali|raspbian) echo debian; return ;;
            fedora|rhel|centos|rocky|almalinux|nobara|ultramarine) echo fedora; return ;;
            opensuse*|suse|sles) echo suse; return ;;
            alpine|postmarketos) echo alpine; return ;;
        esac
    done
    echo "unknown:${ID:-?}"
}

install_deps() {
    local distro=$1
    say "Installing build dependencies for $distro"
    case $distro in
        void)   sudo_run xbps-install -Sy rust cargo pkg-config gcc gtk4-devel ;;
        arch)   sudo_run pacman -S --needed --noconfirm rust pkgconf gcc gtk4 ;;
        debian) sudo_run apt-get update
                sudo_run apt-get install -y cargo rustc pkg-config gcc libc6-dev libgtk-4-dev ;;
        fedora) sudo_run dnf install -y cargo rust pkgconf-pkg-config gcc gtk4-devel ;;
        suse)   sudo_run zypper --non-interactive install cargo rust pkg-config gcc gtk4-devel ;;
        alpine) sudo_run apk add cargo rust pkgconf build-base gtk4.0-dev ;;
        *)      die "don't know how to install packages on '${distro#unknown:}'.
Install these yourself, then re-run: Rust >= $MIN_RUST (cargo), pkg-config, a C compiler, GTK >= $MIN_GTK development files." ;;
    esac
}

# The distro's Rust may be older than steamlite needs (e.g. Ubuntu LTS).
ensure_rust() {
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    local have=""
    command -v cargo >/dev/null && have=$(cargo --version | awk '{print $2}')
    if [ -n "$have" ] && version_ge "$have" "$MIN_RUST"; then
        say "Rust $have OK"
        return
    fi
    if [ "$dry" = 1 ]; then
        say "would install Rust via rustup if the distro's cargo (${have:-none}) is older than $MIN_RUST"
        return
    fi
    warn "cargo ${have:-not found} is older than $MIN_RUST; installing a current toolchain with rustup (into ~/.cargo)"
    command -v curl >/dev/null || die "curl is needed to fetch rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    . "$HOME/.cargo/env"
}

check_gtk() {
    [ "$dry" = 1 ] && return
    local v
    v=$(pkg-config --modversion gtk4 2>/dev/null) || die "GTK 4 development files still not found by pkg-config"
    version_ge "$v" "$MIN_GTK" || die "GTK $v is too old; steamlite needs GTK >= $MIN_GTK.
Use the Flatpak build instead (see README), or upgrade your distro."
    say "GTK $v OK"
}

files=(
    bin/steamlite
    share/applications/steamlite.desktop
    share/icons/hicolor/scalable/apps/steamlite.svg
    share/icons/hicolor/256x256/apps/steamlite.png
)

as_owner() {  # write into $prefix: plain for ~/.local, via sudo for /usr/local
    if [ "$prefix" = "$HOME/.local" ]; then run "$@"; else sudo_run "$@"; fi
}

install_files() {
    say "Installing to $prefix"
    as_owner install -Dm755 "$ROOT/target/release/steamlite" "$prefix/bin/steamlite"
    as_owner install -Dm644 "$ROOT/data/steamlite.desktop" "$prefix/share/applications/steamlite.desktop"
    as_owner install -Dm644 "$ROOT/data/steamlite.svg" "$prefix/share/icons/hicolor/scalable/apps/steamlite.svg"
    as_owner install -Dm644 "$ROOT/data/steamlite.png" "$prefix/share/icons/hicolor/256x256/apps/steamlite.png"
    command -v update-desktop-database >/dev/null && run update-desktop-database -q "$prefix/share/applications" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null && run gtk-update-icon-cache -qtf "$prefix/share/icons/hicolor" 2>/dev/null || true
    case ":$PATH:" in
        *":$prefix/bin:"*) ;;
        *) warn "$prefix/bin is not on your PATH; add it, or run $prefix/bin/steamlite" ;;
    esac
}

uninstall() {
    say "Removing steamlite from $prefix (your ~/.config/steamlite is kept)"
    for f in "${files[@]}"; do as_owner rm -f "$prefix/$f"; done
}

if [ "$mode" = uninstall ]; then uninstall; exit 0; fi

distro=$(detect_distro)
install_deps "$distro"
ensure_rust
check_gtk
[ "$mode" = deps ] && { say "Dependencies installed."; exit 0; }

say "Building steamlite (release)"
run cargo build --release --locked --manifest-path "$ROOT/Cargo.toml"
install_files
say "Done. Start it with: steamlite"
if ! command -v steam >/dev/null 2>&1 && [ ! -d "$HOME/.var/app/com.valvesoftware.Steam" ]; then
    warn "no 'steam' command found. If Steam runs elsewhere (e.g. you're in a distrobox), set
   steam_command = \"distrobox-host-exec steam\" in ~/.config/steamlite/config.toml"
fi
