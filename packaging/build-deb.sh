#!/usr/bin/env bash
# Build dist/steamlite_<version>_amd64.deb (Debian 13+, Ubuntu 25.04+).
source "$(dirname "$0")/common.sh"
build_release

d="$BUILD/deb"
rm -rf "$d"
stage_files "$d"
# Debian keeps the license as doc/<pkg>/copyright.
mv "$d/usr/share/doc/steamlite/LICENSE" "$d/usr/share/doc/steamlite/copyright"

size=$(du -sk "$d/usr" | cut -f1)
mkdir -p "$d/DEBIAN"
cat > "$d/DEBIAN/control" <<EOF
Package: steamlite
Version: $VERSION-1
Architecture: amd64
Maintainer: Bitangsha Ray <bitangsha2014@gmail.com>
Installed-Size: $size
Depends: libgtk-4-1 (>= 4.16), libglib2.0-0t64 | libglib2.0-0, libgdk-pixbuf-2.0-0, libpango-1.0-0, libgraphene-1.0-0, libc6 (>= 2.39), libgcc-s1
Recommends: steam-installer | steam-launcher | steam
Section: games
Priority: optional
Homepage: https://github.com/plaintextlab/steam-lite
Description: lightweight launcher for installed Steam games
 steamlite shows installed Steam games as a keyboard-driven grid of covers
 and launches them through the Steam client running hidden in the
 background (steam -silent), so Steam's heavy UI never has to open.
EOF

out="$DIST/steamlite_${VERSION}_amd64.deb"
dpkg-deb --root-owner-group -Zxz --build "$d" "$out" >/dev/null
echo "$out"
