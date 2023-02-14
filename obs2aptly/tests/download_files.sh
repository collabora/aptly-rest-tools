#!/bin/bash

# download_files.sh:
# Takes an output directory + 1 or more links to Debian .changes files, then
# downloads the changes files, as well as the control files from the .debs
# contained within.

set -eo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <output directory> <urls>..." >&2
  exit 1
fi

outdir="$1"
shift

for url in "$@"; do
  urlbase="${url%/*}"
  urlfile="${url##*/}"

  echo "***** $urlfile"
  curl -Lo "$outdir/$urlfile" "$url"

  files=($(awk \
    '/Checksums-Sha256/{p=1; next} /^[^ ]/{if(p)exit} p{print $3;}' \
    "$outdir/$urlfile"))
  for file in "${files[@]}"; do
    stem="${file%.*}"
    ext="${file##*.}"
    case "$ext" in
    deb)
      controlfile="$stem.control"
      echo "***** $file -> $controlfile"
      curl -L "$urlbase/$file" \
        | bsdtar -Oxf - control.tar.xz \
        | bsdtar -Oxf - ./control \
        > "$outdir/$controlfile"
      ;;
    *)
      echo "***** skipping unknown file: $file"
    esac
  done
done
