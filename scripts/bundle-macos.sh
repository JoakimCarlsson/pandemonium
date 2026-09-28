#!/bin/sh
#
# Wraps a pandemonium binary in a signed macOS app bundle.
#
#   scripts/bundle-macos.sh <binary> <app> <name> <identifier>

set -eu

# Prints an error and exits.
fail() {
    echo "error: $*" >&2
    exit 1
}

# Prints the workspace version from the root manifest.
workspace_version() {
    sed -n 's/^version = "\(.*\)"/\1/p' "$1/Cargo.toml" | head -n1
}

# Renders the source icon into an .icns.
build_icon() {
    source="$1"
    icns="$2"
    iconset="$(mktemp -d)/AppIcon.iconset"
    mkdir -p "$iconset"
    for size in 16 32 128 256 512; do
        sips -z "$size" "$size" "$source" --out "$iconset/icon_${size}x${size}.png" >/dev/null
        double=$((size * 2))
        sips -z "$double" "$double" "$source" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
    done
    iconutil -c icns "$iconset" -o "$icns"
    rm -rf "$(dirname "$iconset")"
}

# Lays out, fills in and signs the bundle.
main() {
    [ $# -eq 4 ] || fail "usage: $0 <binary> <app> <name> <identifier>"
    binary="$1"
    app="$2"
    name="$3"
    identifier="$4"
    [ -x "$binary" ] || fail "$binary is not an executable"
    root="$(cd "$(dirname "$0")/.." && pwd)"
    version="$(workspace_version "$root")"

    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    install -m 0755 "$binary" "$app/Contents/MacOS/pandemonium"
    sed -e "s|@NAME@|$name|g" \
        -e "s|@IDENTIFIER@|$identifier|g" \
        -e "s|@VERSION@|$version|g" \
        "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
    build_icon "$root/packaging/macos/icon.png" "$app/Contents/Resources/AppIcon.icns"

    codesign --force --sign - "$app"
}

main "$@"
