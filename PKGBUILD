pkgname=kws
pkgver=1.0.0
pkgrel=1
pkgdesc="Abre workspaces de terminal no Konsole (tabs, splits e comandos) descritos em um arquivo TOML"
arch=('x86_64')
url="https://github.com/caio-couto/kws"
license=('custom')
depends=('konsole' 'dbus')
makedepends=('cargo')
options=('!lto')
source=()
sha256sums=()

build() {
  cd "$startdir"
  cargo build --release --locked --target-dir "$srcdir/target"
}

check() {
  cd "$startdir"
  cargo test --release --locked --target-dir "$srcdir/target"
}

package() {
  install -Dm755 "$srcdir/target/release/kws" "$pkgdir/usr/bin/kws"
  install -Dm644 "$startdir/README.md" "$pkgdir/usr/share/doc/$pkgname/README.md"
}
