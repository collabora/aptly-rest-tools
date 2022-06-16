Format: 3.0 (quilt)
Source: rustc
Binary: rustc, libstd-rust-1.48, libstd-rust-dev, libstd-rust-dev-windows, libstd-rust-dev-wasm32, rust-gdb, rust-lldb, rust-doc, rust-src
Architecture: any all
Version: 1.48.0+dfsg1-2co1
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Uploaders:  Ximin Luo <infinity0@debian.org>, Sylvestre Ledru <sylvestre@debian.org>
Homepage: http://www.rust-lang.org/
Standards-Version: 4.2.1
Vcs-Browser: https://salsa.debian.org/rust-team/rust
Vcs-Git: https://salsa.debian.org/rust-team/rust.git
Build-Depends: debhelper (>= 9), debhelper-compat (= 12), dpkg-dev (>= 1.17.14), python3:native, cargo:native (>= 0.40.0) <!pkg.rustc.dlstage0>, rustc:native (>= 1.47.0+dfsg) <!pkg.rustc.dlstage0>, rustc:native (<= 1.48.0++) <!pkg.rustc.dlstage0>, llvm-11-dev:native, llvm-11-tools:native, libllvm11, cmake (>= 3.0) | cmake3, pkg-config, zlib1g-dev:native, zlib1g-dev, liblzma-dev:native, binutils (>= 2.26) <!nocheck> | binutils-2.26 <!nocheck>, git <!nocheck>, procps <!nocheck>, gdb (>= 7.12) <!nocheck>, curl <pkg.rustc.dlstage0>, ca-certificates <pkg.rustc.dlstage0>
Build-Depends-Indep: wasi-libc (>= 0.0~git20200731.215adc8~~) <!nowasm>, wasi-libc (<= 0.0~git20200731.215adc8++) <!nowasm>, clang-11:native
Build-Conflicts: gdb-minimal <!nocheck>
Package-List:
 libstd-rust-1.48 deb libs optional arch=any
 libstd-rust-dev deb libdevel optional arch=any
 libstd-rust-dev-wasm32 deb libdevel optional arch=all profile=!nowasm
 libstd-rust-dev-windows deb libdevel optional arch=amd64 profile=!nowindows
 rust-doc deb doc optional arch=all profile=!nodoc
 rust-gdb deb devel optional arch=all
 rust-lldb deb devel optional arch=all
 rust-src deb devel optional arch=all
 rustc deb devel optional arch=any
Checksums-Sha1:
 7d2c6a2c01f86107eb1a40ecdbe59c79da2bbd79 22048320 rustc_1.48.0+dfsg1.orig.tar.xz
 9e7ea4c424933e96f6a5d5d661b57827b73100af 82528 rustc_1.48.0+dfsg1-2co1.debian.tar.xz
Checksums-Sha256:
 f39dd5901feb713bc8876a042c3105bf654177878d8bcc71962c8dcc041af367 22048320 rustc_1.48.0+dfsg1.orig.tar.xz
 3a5e7c085587e91abbcea8bec12313ae6c7a4f0dd9f873fbeeeb5365acc655d3 82528 rustc_1.48.0+dfsg1-2co1.debian.tar.xz
Files:
 a429436119d1d92c53524836c3017f63 22048320 rustc_1.48.0+dfsg1.orig.tar.xz
 76c22632b467b7cea27939bebcc0bf38 82528 rustc_1.48.0+dfsg1-2co1.debian.tar.xz
