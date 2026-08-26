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
backup=('etc/liskaiso')

build() {
    echo "--> [BUILD] Compiling liskaiso...."
    cargo build --release
}

package() {
    echo "--> [PACKAGE] Installing liskaiso...."
    install -d "${pkgdir}/usr/bin"
    install -Dm 755 "./target/release/liskaiso" "${pkgdir}/usr/bin/liskaiso"
    install -d "${pkgdir}/etc/liskaiso"
    install -d "${pkgdir}/etc/liskaiso/airootfs"
    install -d "${pkgdir}/etc/liskaiso/iso_root/boot/grub"
    cp -a ./workspace/packages "${pkgdir}/etc/liskaiso/packages"
    cp -a ./workspace/grub.cfg "${pkgdir}/etc/liskaiso/iso_root/boot/grub/grub.cfg"
    install -d 750 "${pkgdir}/etc/liskaiso/airootfs/root"
    chmod 750 "${pkgdir}/etc/liskaiso/airootfs/root"
    chown root:root "${pkgdir}/etc/liskaiso/airootfs/root"
    cp -a ./workspace/.zshrc "${pkgdir}/etc/liskaiso/root/.zshrc"
    chmod 600 "${pkgdir}/etc/liskaiso/airootfs/root/.zshrc"
    chown root:root "${pkgdir}/etc/liskaiso/airootfs/root/.zshrc"
}
