# Maintainer: Bitangsha Ray
# Build from a checkout of this repository:  makepkg -si
pkgname=steamlite
pkgver=0.1.0
pkgrel=1
pkgdesc="Lightweight GTK4 launcher for installed Steam games (Steam runs hidden in the background)"
arch=('x86_64')
license=('MIT')
depends=('gtk4>=4.16' 'gdk-pixbuf2' 'glib2' 'cairo' 'pango' 'glibc' 'gcc-libs')
makedepends=('cargo')
optdepends=('steam: the client steamlite launches games through')
source=()
sha256sums=()

_src() { echo "$startdir"; }

prepare() {
  cd "$(_src)"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "$(_src)"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo build --frozen --release
}

check() {
  cd "$(_src)"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo test --frozen --release
}

package() {
  cd "$(_src)"
  install -Dm755 "$srcdir/target/release/steamlite" "$pkgdir/usr/bin/steamlite"
  install -Dm644 data/steamlite.desktop "$pkgdir/usr/share/applications/steamlite.desktop"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
