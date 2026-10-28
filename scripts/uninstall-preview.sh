#!/bin/sh
#
# Removes pandemonium-preview and everything it keeps, so that the next
# `make install` starts from a clean slate. The stable pandemonium, its data and
# its icons are left alone. A running preview and its server are stopped first.
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

# Prints the numbers of the processes started from the installed preview: its window and its server, and not the stable app, which is another program.
running() {
    command -v pgrep >/dev/null 2>&1 || return 0
    pgrep -f "^$BIN_DIR/$NAME( |\$)" 2>/dev/null | grep -vx "$$" || true
}

# Stops the preview and its server, asking them to quit first and ending what is left after a few seconds, so that what it keeps can be removed.
stop_running() {
    pids="$(running)"
    [ -n "$pids" ] || return 0
    echo "stopping $NAME: $(echo $pids | tr '\n' ' ')"
    kill $pids 2>/dev/null || true
    waited=0
    while [ "$waited" -lt 10 ] && [ -n "$(running)" ]; do
        sleep 0.5
        waited=$((waited + 1))
    done
    left="$(running)"
    [ -z "$left" ] || kill -9 $left 2>/dev/null || true
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

# Stops the preview and removes it.
main() {
    stop_running
    case "$(uname -s)" in
        Linux) uninstall_linux ;;
        Darwin) uninstall_macos ;;
        *) fail "unsupported system" ;;
    esac
    echo "removed $NAME; the stable pandemonium was not touched"
}

main "$@"
