#!/usr/bin/env bash
# Build dist/steamlite-<version>-x86_64.AppImage with linuxdeploy and its GTK
# plugin (bundles GTK 4 and its dependencies; needs host glibc >= the build
# host's). Tools are downloaded once into packaging/.build/tools.
source "$(dirname "$0")/common.sh"
build_release

tools="$BUILD/tools"
mkdir -p "$tools"
[ -x "$tools/linuxdeploy-x86_64.AppImage" ] || curl -fsSLo "$tools/linuxdeploy-x86_64.AppImage" \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
[ -x "$tools/linuxdeploy-plugin-gtk.sh" ] || curl -fsSLo "$tools/linuxdeploy-plugin-gtk.sh" \
    https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh
chmod +x "$tools"/*

appdir="$BUILD/AppDir"
rm -rf "$appdir"
stage_files "$appdir"
install -Dm644 "$ROOT/data/io.github.plaintextlab.steamlite.metainfo.xml" \
    "$appdir/usr/share/metainfo/steamlite.appdata.xml"
# appstream wants the metainfo id to match the desktop file name.
sed -i 's/io.github.plaintextlab.steamlite.desktop/steamlite.desktop/; s#<id>io.github.plaintextlab.steamlite</id>#<id>steamlite</id>#' \
    "$appdir/usr/share/metainfo/steamlite.appdata.xml"

export PATH="$tools:$PATH" APPIMAGE_EXTRACT_AND_RUN=1 DEPLOY_GTK_VERSION=4
export LDAI_OUTPUT="$DIST/steamlite-$VERSION-x86_64.AppImage" LINUXDEPLOY_OUTPUT_VERSION="$VERSION"
cd "$BUILD"
linuxdeploy-x86_64.AppImage --appdir "$appdir" \
    --executable "$appdir/usr/bin/steamlite" \
    --desktop-file "$appdir/usr/share/applications/steamlite.desktop" \
    --icon-file "$ROOT/data/steamlite.svg" \
    --plugin gtk --output appimage
echo "$LDAI_OUTPUT"
