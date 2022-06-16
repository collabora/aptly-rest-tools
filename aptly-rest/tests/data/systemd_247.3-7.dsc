-----BEGIN PGP SIGNED MESSAGE-----
Hash: SHA256

Format: 3.0 (quilt)
Source: systemd
Binary: systemd, systemd-sysv, systemd-container, systemd-journal-remote, systemd-coredump, systemd-timesyncd, systemd-tests, libpam-systemd, libnss-myhostname, libnss-mymachines, libnss-resolve, libnss-systemd, libsystemd0, libsystemd-dev, udev, libudev1, libudev-dev, udev-udeb, libudev1-udeb
Architecture: linux-any
Version: 247.3-7
Maintainer: Debian systemd Maintainers <pkg-systemd-maintainers@lists.alioth.debian.org>
Uploaders: Michael Biebl <biebl@debian.org>, Marco d'Itri <md@linux.it>, Sjoerd Simons <sjoerd@debian.org>, Martin Pitt <mpitt@debian.org>, Felipe Sateler <fsateler@debian.org>
Homepage: https://www.freedesktop.org/wiki/Software/systemd
Standards-Version: 4.5.1
Vcs-Browser: https://salsa.debian.org/systemd-team/systemd
Vcs-Git: https://salsa.debian.org/systemd-team/systemd.git
Testsuite: autopkgtest
Testsuite-Triggers: acl, apparmor, build-essential, busybox-static, cron, cryptsetup-bin, dbus-user-session, dmeventd, dnsmasq-base, e2fsprogs, evemu-tools, fdisk, gcc, gdm3, iproute2, iputils-ping, isc-dhcp-client, kbd, less, libc-dev, libc6-dev, libcap2-bin, liblz4-tool, locales, make, net-tools, netcat-openbsd, network-manager, perl, pkg-config, plymouth, policykit-1, python3, qemu-system-arm, qemu-system-ppc, qemu-system-s390x, qemu-system-x86, quota, rsyslog, seabios, socat, squashfs-tools, strace, tree, util-linux, xserver-xorg, xserver-xorg-video-dummy, xz-utils, zstd
Build-Depends: debhelper-compat (= 13), pkg-config, xsltproc, docbook-xsl, docbook-xml, m4, meson (>= 0.52.1), gettext, gperf, gnu-efi [amd64 i386 arm64 armhf], libcap-dev (>= 1:2.24-9~), libpam0g-dev, libapparmor-dev (>= 2.13) <!stage1>, libidn2-dev <!stage1>, libiptc-dev <!stage1>, libaudit-dev <!stage1>, libdbus-1-dev (>= 1.3.2) <!nocheck> <!noinsttest>, libcryptsetup-dev (>= 2:1.6.0) <!stage1>, libselinux1-dev (>= 2.1.9), libacl1-dev, liblzma-dev, liblz4-dev (>= 0.0~r125), liblz4-tool <!nocheck>, libbz2-dev <!stage1>, zlib1g-dev <!stage1> | libz-dev <!stage1>, libcurl4-gnutls-dev <!stage1> | libcurl-dev <!stage1>, libmicrohttpd-dev <!stage1>, libgnutls28-dev <!stage1>, libpcre2-dev <!stage1>, libgcrypt20-dev, libkmod-dev (>= 15), libblkid-dev (>= 2.24), libmount-dev (>= 2.30), libseccomp-dev (>= 2.3.1) [amd64 arm64 armel armhf i386 mips mipsel mips64 mips64el x32 powerpc ppc64 ppc64el riscv64 s390x], libdw-dev (>= 0.158) <!stage1>, libpolkit-gobject-1-dev <!stage1>, libzstd-dev (>= 1.4.0), linux-base <!nocheck>, acl <!nocheck>, python3:native, python3-lxml:native, python3-pyparsing <!nocheck>, python3-evdev <!nocheck>, tzdata <!nocheck>, libcap2-bin <!nocheck>, iproute2 <!nocheck>, zstd <!nocheck>
Package-List:
 libnss-myhostname deb admin optional arch=linux-any
 libnss-mymachines deb admin optional arch=linux-any
 libnss-resolve deb admin optional arch=linux-any
 libnss-systemd deb admin standard arch=linux-any
 libpam-systemd deb admin standard arch=linux-any
 libsystemd-dev deb libdevel optional arch=linux-any
 libsystemd0 deb libs optional arch=linux-any
 libudev-dev deb libdevel optional arch=linux-any
 libudev1 deb libs optional arch=linux-any
 libudev1-udeb udeb debian-installer optional arch=linux-any profile=!noudeb
 systemd deb admin important arch=linux-any
 systemd-container deb admin optional arch=linux-any profile=!stage1
 systemd-coredump deb admin optional arch=linux-any profile=!stage1
 systemd-journal-remote deb admin optional arch=linux-any profile=!stage1
 systemd-sysv deb admin important arch=linux-any
 systemd-tests deb admin optional arch=linux-any profile=!noinsttest
 systemd-timesyncd deb admin standard arch=linux-any
 udev deb admin important arch=linux-any
 udev-udeb udeb debian-installer optional arch=linux-any profile=!noudeb
Checksums-Sha1:
 9bad8622d0198406e6570ca7c54de0eac47e468e 9895385 systemd_247.3.orig.tar.gz
 49e8715baa823f7046e710db1d99da5cd2e118c5 183052 systemd_247.3-7.debian.tar.xz
Checksums-Sha256:
 2869986e219a8dfc96cc0dffac66e0c13bb70a89e16b85a3948876c146cfa3e0 9895385 systemd_247.3.orig.tar.gz
 d80595383166aa08ed28441898880fbc124bf2468526d35fc7c85c2c09f12ca5 183052 systemd_247.3-7.debian.tar.xz
Files:
 5be57c584847b161c67577f2d997903a 9895385 systemd_247.3.orig.tar.gz
 8fa05c0e91c9d19f809afe7ee31ce352 183052 systemd_247.3-7.debian.tar.xz

-----BEGIN PGP SIGNATURE-----

iQIzBAEBCAAdFiEECbOsLssWnJBDRcxUauHfDWCPItwFAmI3jSEACgkQauHfDWCP
ItxqFA//UQmg27ZotnGYfT4NPCIsmFNWb9GsV6I7eSt6PG4A4hNTU2KBpTdPuEfG
jgmykgEYBA4Tz1hmb7Bjrf/82GPoDNf/FR7t3lkx0hH3VhZALGgzLlX47TYX+DJV
KzPngBw+QPg8+r6nYEJVl8tjPVuSXK89pDtdJ6n0CLcyNvpMAThC4tdw0QzjskeC
g12DH9wc6cb2FdHLQn+4iMLiKTO/nkklDv3XCg0TGqwsiwkWyNucYRY9Z2mYSGJ2
DHIzF8zkAXKwHpMnTAP+aL6EizGveaXyv4NwyQrhKMzLpnd36YoStbzlGyzRgtIN
7t67+4bKGwIz51wQW0hpp/CM+lKaJp3eFl8GyJqADOsh0JUZKX4DgNUhXnFRFmEB
Rqky+mN6bMibBKaSA4O7p1rKHO0c+skVJxo1v8CrqA74/CQG34g6x9d4GVxwMNAm
YS98OrEEVa5pu+g7PgFr3Zgi4fHtNXqbWgiuZ4aYZfnNRA+UCN8FW629dv3sV5os
RbzAvI3gO+/Iv5QRmlrNEDTeqlYdlCT8xmmK4U9CzFlZMZSNMOr/ttwGahHYrsjs
iqGatV3Lj0QNl8ggjeuxWmkAls9ArKXTN7qMC6PdSWusapqel88MdLXciWeNAxCK
2giWuJKhHS3vcMWtQ7ZhNfpKIJauOcZ4oqSwtNgzP8iSFqtFtjc=
=dW89
-----END PGP SIGNATURE-----
