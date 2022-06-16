Format: 3.0 (quilt)
Source: util-linux
Binary: util-linux, util-linux-locales, mount, bsdutils, bsdextrautils, fdisk, fdisk-udeb, libblkid1, libblkid1-udeb, libblkid-dev, libfdisk1, libfdisk1-udeb, libfdisk-dev, libmount1, libmount1-udeb, libmount-dev, libsmartcols1, libsmartcols1-udeb, libsmartcols-dev, libuuid1, uuid-runtime, libuuid1-udeb, uuid-dev, util-linux-udeb, rfkill, eject, eject-udeb
Architecture: any all
Version: 2.36.1-8+deb11u1+apertis1
Maintainer: util-linux packagers <util-linux@packages.debian.org>
Uploaders: Chris Hofstaedtler <zeha@debian.org>
Homepage: http://www.kernel.org/pub/linux/utils/util-linux/
Standards-Version: 4.5.1
Vcs-Browser: https://salsa.debian.org/debian/util-linux
Vcs-Git: https://salsa.debian.org/debian/util-linux.git
Testsuite: autopkgtest
Testsuite-Triggers: bash, bc, bsdmainutils, build-essential, dpkg, grep, pkg-config
Build-Depends: bc <!stage1 !nocheck>, bison, debhelper-compat (= 13), dh-exec, gettext, libaudit-dev [linux-any] <!stage1>, libcap-ng-dev [linux-any] <!stage1>, libncurses5-dev, libncursesw5-dev, libpam0g-dev <!stage1>, libselinux1-dev (>= 2.6-3~) [linux-any], libsystemd-dev [linux-any] <!stage1>, libtool, libudev-dev [linux-any] <!stage1>, netbase <!stage1 !nocheck>, pkg-config, po-debconf, socat <!stage1 !nocheck>, systemd [linux-any] <!stage1>, zlib1g-dev
Build-Conflicts: libcryptsetup-dev
Package-List:
 bsdextrautils deb utils optional arch=any profile=!stage1
 bsdutils deb utils required arch=any profile=!stage1 essential=yes
 eject deb utils optional arch=linux-any
 eject-udeb udeb debian-installer optional arch=linux-any
 fdisk deb utils important arch=any profile=!stage1
 fdisk-udeb udeb debian-installer optional arch=hurd-any,linux-any profile=!stage1
 libblkid-dev deb libdevel optional arch=any
 libblkid1 deb libs optional arch=any
 libblkid1-udeb udeb debian-installer optional arch=any
 libfdisk-dev deb libdevel optional arch=any
 libfdisk1 deb libs optional arch=any
 libfdisk1-udeb udeb debian-installer optional arch=any
 libmount-dev deb libdevel optional arch=linux-any
 libmount1 deb libs optional arch=any
 libmount1-udeb udeb debian-installer optional arch=linux-any
 libsmartcols-dev deb libdevel optional arch=any
 libsmartcols1 deb libs optional arch=any
 libsmartcols1-udeb udeb debian-installer optional arch=any
 libuuid1 deb libs optional arch=any
 libuuid1-udeb udeb debian-installer optional arch=any
 mount deb admin required arch=linux-any profile=!stage1
 rfkill deb utils optional arch=linux-any profile=!stage1
 util-linux deb utils required arch=any profile=!stage1 essential=yes
 util-linux-locales deb localization optional arch=all profile=!stage1
 util-linux-udeb udeb debian-installer optional arch=any profile=!stage1
 uuid-dev deb libdevel optional arch=any
 uuid-runtime deb utils optional arch=any profile=!stage1
Checksums-Sha1:
 d542136a8875ec672aa4f065c16a5fce1808ca42 5231880 util-linux_2.36.1.orig.tar.xz
 c22e11e5ec59e3349a19d59648f963b3b3d47709 107524 util-linux_2.36.1-8+deb11u1+apertis1.debian.tar.xz
Checksums-Sha256:
 09fac242172cd8ec27f0739d8d192402c69417617091d8c6e974841568f37eed 5231880 util-linux_2.36.1.orig.tar.xz
 818ed180fd3a715a4c5e8f3476474efba878c7737b85314a6c823d0e50b5a94b 107524 util-linux_2.36.1-8+deb11u1+apertis1.debian.tar.xz
Files:
 b0b702979d47043d9c4d8ba93be21e20 5231880 util-linux_2.36.1.orig.tar.xz
 054a1a2eb5e72e3847907c201a6638b5 107524 util-linux_2.36.1-8+deb11u1+apertis1.debian.tar.xz
