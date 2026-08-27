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
depends=('lkpm' 'grub' 'libisoburn' 'mtools' 'dosfstools' 'squashfs-tools' 'xorriso')
makedepends=('rustup')
backup=('etc/liskaiso.d')

build() {
    echo "--> [BUILD] Compiling liskaiso...."
    cargo build --release
}

package() {
    echo "--> [PACKAGE] Installing liskaiso...."
    install -d "${pkgdir}/usr/bin"
    install -Dm 755 "./target/release/liskaiso" "${pkgdir}/usr/bin/liskaiso"
    install -d "${pkgdir}/etc/liskaiso.d"
    install -Dm 644 ./workspace/packages "${pkgdir}/etc/liskaiso.d/packages"
    install -Dm 644 ./workspace/grub.cfg "${pkgdir}/etc/liskaiso.d/iso_root/boot/grub/grub.cfg"
    install -dm 750 "${pkgdir}/etc/liskaiso.d/airootfs/root"
    chmod 750 "${pkgdir}/etc/liskaiso.d/airootfs/root"
    chown root:root "${pkgdir}/etc/liskaiso.d/airootfs/root"
    install -Dm 600 ./workspace/zshrc "${pkgdir}/etc/liskaiso.d/airootfs/root/.zshrc"
    chmod 600 "${pkgdir}/etc/liskaiso.d/airootfs/root/.zshrc"
    chown root:root "${pkgdir}/etc/liskaiso.d/airootfs/root/.zshrc"
}
