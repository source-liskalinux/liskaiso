# PKGBUILD For Liska ISO

# Contributor: Janorovic Volkov <janorovicvolkov@gmail.com>
# Maintainer: Janorovic Volkov <janorovicvolkov@gmail.com>

pkgname=liskaiso
pkgver=1.0.0
pkgrel=1
pkgdesc="Liska ISO Builder"
arch=('x86_64')
license=('General Public License v3 or Later')
depends=('busybox' 'cpio')
package() {
    install -d "${pkgdir}/usr/bin"
    install -Dm755 "${srcdir}/../target/debug/liskaiso" "${pkgdir}/usr/bin/liskaiso"
    install -Dm755 "${srcdir}/../src/liskaiso-workspace/packages" "${pkgdir}/home/liskaiso-workspace/packages"
}
