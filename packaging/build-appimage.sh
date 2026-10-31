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
# AppStream wants a reverse-DNS id whose desktop file has the same name.
id=io.github.plaintextlab.steamlite
mv "$appdir/usr/share/applications/steamlite.desktop" "$appdir/usr/share/applications/$id.desktop"
install -Dm644 "$ROOT/data/$id.metainfo.xml" "$appdir/usr/share/metainfo/$id.metainfo.xml"

export PATH="$tools:$PATH" APPIMAGE_EXTRACT_AND_RUN=1 DEPLOY_GTK_VERSION=4
export LDAI_OUTPUT="$DIST/steamlite-$VERSION-x86_64.AppImage" LINUXDEPLOY_OUTPUT_VERSION="$VERSION"
cd "$BUILD"
linuxdeploy-x86_64.AppImage --appdir "$appdir" \
    --executable "$appdir/usr/bin/steamlite" \
    --desktop-file "$appdir/usr/share/applications/$id.desktop" \
    --icon-file "$ROOT/data/steamlite.svg" \
    --plugin gtk --output appimage
echo "$LDAI_OUTPUT"
