# PKGBUILD For Liska ISO

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

package() {
    install -d "${pkgdir}/usr/bin"
    install -Dm755 "${srcdir}/../target/release/liskaiso" "${pkgdir}/usr/bin/liskaiso"
    install -Dm755 "${srcdir}/../src/liskaiso-workspace/packages" "${pkgdir}/home/liskaiso-workspace/packages"
}
