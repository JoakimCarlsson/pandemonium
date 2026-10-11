#!/bin/sh
# Builds a matching server beside the editor, statically linked on Linux.

set -eu

# Builds the server for the requested profile and installs it beside the editor.
main() {
    profile="${1:-debug}"
    case "$profile" in
        debug) cargo_profile="dev" ;;
        release) cargo_profile="release" ;;
        *) echo "error: expected debug or release" >&2; exit 1 ;;
    esac
    host="$(rustc -vV | sed -n 's/^host: //p')"
    output="${CARGO_TARGET_DIR:-target}/$profile"
    case "$host" in
        *-unknown-linux-gnu | *-unknown-linux-musl)
            architecture="${host%%-*}"
            target="$architecture-unknown-linux-musl"
            rustup target add "$target"
            cargo build --locked -p pm-server --profile "$cargo_profile" --target "$target"
            mkdir -p "$output"
            install -m 0755 "${CARGO_TARGET_DIR:-target}/$target/$profile/pandemonium-server" "$output/pandemonium-server"
            ;;
        *) cargo build --locked -p pm-server --profile "$cargo_profile" ;;
    esac
}

main "$@"

