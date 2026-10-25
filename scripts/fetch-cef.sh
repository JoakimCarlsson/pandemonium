#!/bin/sh
#
# Downloads the embedded browser's engine into a directory unless it is
# already there, at the version the lockfile pins.
#
#   scripts/fetch-cef.sh [dir]      (default: $CEF_PATH, else ~/.cache/cef)

set -eu

DIR="${1:-${CEF_PATH:-$HOME/.cache/cef}}"
[ -e "$DIR/libcef.so" ] && exit 0

cd "$(git rev-parse --show-toplevel)"
version="$(awk '/^name = "cef"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
[ -n "$version" ] || { echo "error: the lockfile has no cef package" >&2; exit 1; }

echo "downloading the embedded browser engine $version to $DIR"
cargo install --locked --quiet --root build/cef-tool export-cef-dir --version "=${version%%+*}"
build/cef-tool/bin/export-cef-dir --force "$DIR"
