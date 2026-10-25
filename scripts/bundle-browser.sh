#!/bin/sh
#
# Packs the browser program and the engine files it needs at run time into the
# archive the window downloads when the reader turns the browser on, and its
# checksum: the engine's library without its debug symbols, its data, and the
# English locale.
#
#   scripts/bundle-browser.sh <cef dir> <browser program> <archive>

set -eu

[ $# -eq 3 ] || { echo "usage: $0 <cef dir> <browser program> <archive>" >&2; exit 1; }
source_dir="$1"
program="$2"
archive="$3"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/locales"
cp "$program" "$stage/pandemonium-browser"
strip -o "$stage/libcef.so" "$source_dir/libcef.so"
for file in icudtl.dat resources.pak chrome_100_percent.pak chrome_200_percent.pak \
    v8_context_snapshot.bin libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json; do
    cp "$source_dir/$file" "$stage/"
done
cp "$source_dir/locales/en-US.pak" "$stage/locales/"
tar -czf "$archive" -C "$stage" .
(cd "$(dirname "$archive")" && sha256sum "$(basename "$archive")" > "$(basename "$archive").sha256")
