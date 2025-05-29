# Maintainer: Your Name <your.email@example.com>
pkgname=rusty-socks
pkgver=0.1.0
pkgrel=1
pkgdesc="A high-performance SOCKS5 and HTTP proxy server written in Rust with journald support"
arch=('x86_64' 'i686' 'aarch64')
url="https://github.com/quintusl/rusty-socks"
license=('MIT' 'Apache')
depends=('gcc-libs' 'systemd')
makedepends=('rust' 'cargo')
backup=('etc/rusty-socks/config.yml')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "$pkgname-$pkgver"
    cargo build --release --locked
}

check() {
    cd "$pkgname-$pkgver"
    cargo test --release --locked
}

package() {
    cd "$pkgname-$pkgver"

    # Install binary
    install -Dm755 target/release/rusty-socks "$pkgdir/usr/bin/rusty-socks"

    # Install systemd service
    install -Dm644 config/rusty-socks.service "$pkgdir/usr/lib/systemd/system/rusty-socks.service"

    # Install configuration
    install -Dm644 config/config.yml.journald.example "$pkgdir/etc/rusty-socks/config.yml"
    install -Dm664 config/users.yml.example "$pkgdir/etc/rusty-socks/users.yml"

    # Create directories
    install -dm755 "$pkgdir/var/log/rusty-socks"
    install -dm755 "$pkgdir/var/run/rusty-socks"

    # Install documentation
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
    install -Dm644 LICENSE-MIT "$pkgdir/usr/share/licenses/$pkgname/LICENSE-MIT"
    install -Dm644 LICENSE-APACHE "$pkgdir/usr/share/licenses/$pkgname/LICENSE-APACHE"
}
