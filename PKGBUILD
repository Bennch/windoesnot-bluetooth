pkgname=windoesnot-bluetooth
pkgver=0.1.0
pkgrel=1
pkgdesc='WinDoesNot Bluetooth, adapted from Overskride'
arch=(x86_64)
url='https://github.com/Bennch/windoesnot-bluetooth'
license=('GPL-3.0-or-later')
depends=(gtk4 libadwaita bluez bluez-utils bluez-obex libpulse dbus)
makedepends=(rust pkgconf blueprint-compiler gettext)
options=('!lto' '!debug')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/$pkgver.tar.gz")
sha256sums=('SKIP')
prepare() {
 cd "$srcdir/$pkgname-$pkgver"
 cargo fetch --locked
}
build() {
 cd "$srcdir/$pkgname-$pkgver"
 cargo build --release --locked
}
check() {
 cd "$srcdir/$pkgname-$pkgver"
 cargo test --release --locked desktop::tests
}
package() {
 cd "$srcdir/$pkgname-$pkgver"
 install -Dm755 target/release/$pkgname "$pkgdir/usr/bin/$pkgname"
 install -Dm644 packaging/me.windoesnot.Bluetooth.desktop "$pkgdir/usr/share/applications/me.windoesnot.Bluetooth.desktop"
 install -Dm644 packaging/me.windoesnot.Bluetooth.svg "$pkgdir/usr/share/icons/hicolor/scalable/apps/me.windoesnot.Bluetooth.svg"
 install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
 install -Dm644 data/io.github.kaii_lb.Overskride.gschema.xml "$pkgdir/usr/share/glib-2.0/schemas/me.windoesnot.Bluetooth.gschema.xml"
 install -d "$pkgdir/usr/share/locale/de/LC_MESSAGES"
 msgfmt po/de.po -o "$pkgdir/usr/share/locale/de/LC_MESSAGES/windoesnot-bluetooth.mo"
}
