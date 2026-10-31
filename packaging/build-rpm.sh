#!/usr/bin/env bash
# Build dist/steamlite-<version>-1.x86_64.rpm (Fedora 41+, openSUSE Tumbleweed).
source "$(dirname "$0")/common.sh"
build_release

top="$BUILD/rpm"
rm -rf "$top"
mkdir -p "$top"/{SPECS,BUILD,RPMS,SOURCES,BUILDROOT}
stage_files "$top/stage"

cat > "$top/SPECS/steamlite.spec" <<EOF
Name:           steamlite
Version:        $VERSION
Release:        1
Summary:        Lightweight launcher for installed Steam games
License:        MIT
URL:            https://github.com/plaintextlab/steam-lite
Requires:       gtk4 >= 4.16
Recommends:     steam

%description
steamlite shows installed Steam games as a keyboard-driven grid of covers
and launches them through the Steam client running hidden in the
background (steam -silent), so Steam's heavy UI never has to open.

%install
cp -a $top/stage/. %{buildroot}/

%files
%license /usr/share/doc/steamlite/LICENSE
%doc /usr/share/doc/steamlite/README.md
/usr/bin/steamlite
/usr/share/applications/steamlite.desktop
/usr/share/icons/hicolor/scalable/apps/steamlite.svg
/usr/share/icons/hicolor/256x256/apps/steamlite.png
EOF

rpmbuild --quiet -bb \
    --define "_topdir $top" \
    --define "_build_id_links none" \
    --define "debug_package %{nil}" \
    "$top/SPECS/steamlite.spec"
out="$DIST/steamlite-$VERSION-1.x86_64.rpm"
cp "$top/RPMS/x86_64/steamlite-$VERSION-1.x86_64.rpm" "$out"
echo "$out"
