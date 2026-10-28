#!/bin/sh
#
# Removes pandemonium-preview and everything it keeps, so that the next
# `make install` starts from a clean slate. The stable pandemonium, its data and
# its icons are left alone.
#
#   scripts/uninstall-preview.sh

set -eu

BIN_DIR="${PANDEMONIUM_BIN_DIR:-$HOME/.local/bin}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
MAC_APPS_DIR="${PANDEMONIUM_APP_DIR:-$HOME/Applications}"
NAME="pandemonium-preview"

# Prints an error and exits.
fail() {
    echo "error: $*" >&2
    exit 1
}

# Removes `path` when it is there, saying so.
remove() {
    if [ -e "$1" ] || [ -L "$1" ]; then
        rm -rf "$1"
        echo "removed $1"
    fi
}

# Refuses while the preview is running, since what it keeps would be taken from under it.
check_not_running() {
    if command -v pgrep >/dev/null 2>&1 && pgrep -f "$BIN_DIR/$NAME" >/dev/null 2>&1; then
        fail "$NAME is running; quit it, and its server, first"
    fi
}

# Removes the files of the preview on Linux.
uninstall_linux() {
    remove "$BIN_DIR/$NAME"
    remove "$DATA_HOME/applications/$NAME.desktop"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA_HOME/applications" >/dev/null 2>&1 || true
    remove "$DATA_HOME/$NAME"
    remove "$CACHE_HOME/$NAME"
}

# Removes the files of the preview on macOS.
uninstall_macos() {
    remove "$BIN_DIR/$NAME"
    remove "$MAC_APPS_DIR/Pandemonium Preview.app"
    remove "$HOME/Library/Application Support/$NAME"
    remove "$HOME/Library/Caches/$NAME"
}

# Removes the preview.
main() {
    check_not_running
    case "$(uname -s)" in
        Linux) uninstall_linux ;;
        Darwin) uninstall_macos ;;
        *) fail "unsupported system" ;;
    esac
    echo "removed $NAME; the stable pandemonium was not touched"
}

main "$@"
