# PKGBUILD For liskaiso

# Contributor: Janorovic Volkov <janorovicvolkov@gmail.com>
# Maintainer: Janorovic Volkov <janorovicvolkov@gmail.com>

pkgname=liskaiso
pkgver=1
pkgrel=1
pkgdesc="Liska ISO Builder"
arch=('x86_64')
url="https://github.com/source-liskalinux/liskaiso"
license=('GPL-3.0-or-later')
depends=('lkpm' 'grub' 'libisoburn' 'mtools' 'dosfstools')
makedepends=('rust')

build() {
    echo "--> [BUILD] Compiling liskaiso...."
    cargo build --release
}

package() {
    echo "--> [PACKAGE] Installing liskaiso...."
    install -d "${pkgdir}/usr/bin"
    install -Dm755 "./target/release/liskaiso" "${pkgdir}/usr/bin/liskaiso"
    echo "--> [PACKAGE] Installing liskaiso-workspace...."
    install -d "${pkgdir}/home/liskaiso-workspace"
    cp -a "./liskaiso-workspace" "${pkgdir}/home/liskaiso-workspace"
}
