#!/usr/bin/env bash
# Build dist/steamlite-<version>.flatpak (a single-file bundle).
# Needs flatpak-builder and, from Flathub, org.gnome.Sdk//51 plus
# org.freedesktop.Sdk.Extension.rust-stable//26.08 (installed per-user).
source "$(dirname "$0")/common.sh"

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive flathub \
    org.gnome.Platform//51 org.gnome.Sdk//51 org.freedesktop.Sdk.Extension.rust-stable//26.08

manifest="$ROOT/packaging/flatpak/io.github.plaintextlab.steamlite.yml"
flatpak-builder --user --force-clean --repo="$BUILD/flatpak-repo" \
    --state-dir="$BUILD/flatpak-state" "$BUILD/flatpak-app" "$manifest"
out="$DIST/steamlite-$VERSION.flatpak"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    "$BUILD/flatpak-repo" "$out" io.github.plaintextlab.steamlite
echo "$out"
