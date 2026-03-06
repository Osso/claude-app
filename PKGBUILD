# Maintainer: Alessio Deiana <adeiana@gmail.com>
pkgname=claude-app
pkgver=0.1.0
pkgrel=1
pkgdesc="Dioxus desktop frontend for agent-orchestrator"
arch=('x86_64')
license=('MIT')
depends=('webkit2gtk-4.1')
makedepends=('cargo')
source=()

build() {
    cd "$startdir"
    cargo build --release --locked
}

package() {
    cd "$startdir"
    install -Dm755 "target/release/claude-app" "$pkgdir/usr/bin/claude-app"
}
