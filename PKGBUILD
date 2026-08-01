# PKGBUILD for liskaiso

# Contributor: Janorovic Volkov <janorovicvolkov@gmail.com>
# Maintainer: Janorovic Volkov <janorovicvolkov@gmail.com>

pkgname=liskaiso
pkgver=1.0.0
pkgrel=1
pkgdesc="Liska ISO Builder"
arch=('x86_64')
license=('GPL-3.0-or-later')
depends=('lkinit' 'lkpm' 'grub' 'libisoburn' 'mtools' 'dosfstools')
makedepends=('rust')

build() {
    echo "--> [BUILD] Compiling...."
    cargo build --release
}

package() {
    echo "--> [PACKAGE] Installing liskaiso...."
    install -d "${pkgdir}/usr/bin"
    install -Dm755 "${srcdir}/../target/release/liskaiso" "${pkgdir}/usr/bin/liskaiso"
    echo "--> [PACKAGE] Installing packages...."
    install -Dm755 "${srcdir}/../src/liskaiso-workspace/packages" "${pkgdir}/home/liskaiso-workspace/packages"
}
