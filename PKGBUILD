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

prepare() {
    cargo check --release --all-targets
}

build() {
    cargo build --release
}

check() {
    cargo test --release
}

package() {
    install -d "${pkgdir}/usr/bin"
    install -Dm 755 "./target/release/liskaiso" "${pkgdir}/usr/bin/liskaiso"
    install -d "${pkgdir}/etc/liskaiso.d"
    install -D ./workspace/packages "${pkgdir}/etc/liskaiso.d/packages"
    install -D ./workspace/grub.cfg "${pkgdir}/etc/liskaiso.d/iso_root/boot/grub/grub.cfg"
    install -D ./workspace/zprofile "${pkgdir}/etc/liskaiso.d/airootfs/etc/zsh/zprofile"
    install -D ./workspace/hostname "${pkgdir}/etc/liskaiso.d/airootfs/etc/hostname"
}
