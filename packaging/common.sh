# Shared by the packaging scripts. Source it from the repo root.
set -euo pipefail
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
DIST="$ROOT/dist"
BUILD="$ROOT/packaging/.build"
mkdir -p "$DIST" "$BUILD"

build_release() {
    (cd "$ROOT" && cargo build --release --locked)
}

# Install the files every package ships into a staging prefix ($1 = DESTDIR).
stage_files() {
    local d=$1
    install -Dm755 "$ROOT/target/release/steamlite" "$d/usr/bin/steamlite"
    install -Dm644 "$ROOT/data/steamlite.desktop" "$d/usr/share/applications/steamlite.desktop"
    install -Dm644 "$ROOT/data/steamlite.svg" "$d/usr/share/icons/hicolor/scalable/apps/steamlite.svg"
    install -Dm644 "$ROOT/data/steamlite.png" "$d/usr/share/icons/hicolor/256x256/apps/steamlite.png"
    install -Dm644 "$ROOT/README.md" "$d/usr/share/doc/steamlite/README.md"
    install -Dm644 "$ROOT/LICENSE" "$d/usr/share/doc/steamlite/LICENSE"
}
