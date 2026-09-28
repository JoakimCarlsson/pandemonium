#!/bin/sh
# Installs this checkout as pandemonium-preview.

set -eu

BIN_DIR="${PANDEMONIUM_BIN_DIR:-$HOME/.local/bin}"
APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
MAC_APPS_DIR="${PANDEMONIUM_APP_DIR:-$HOME/Applications}"
NAME="pandemonium-preview"

# Prints an error and exits.
fail() {
    echo "error: $*" >&2
    exit 1
}

# Adds the launcher entry.
install_desktop_entry() {
    mkdir -p "$APPS_DIR"
    sed -e "s|^Exec=pandemonium|Exec=$BIN_DIR/$NAME|" \
        -e "s|^Name=Pandemonium|Name=Pandemonium Preview|" \
        packaging/pandemonium.desktop > "$APPS_DIR/$NAME.desktop"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
}

# Adds the app bundle.
install_app_bundle() {
    mkdir -p "$MAC_APPS_DIR"
    scripts/bundle-macos.sh target/release/pandemonium "$MAC_APPS_DIR/Pandemonium.app" \
        Pandemonium io.github.joakimcarlsson.pandemonium
    echo "installed $MAC_APPS_DIR/Pandemonium.app"
}

# Builds and installs the preview.
main() {
    command -v cargo >/dev/null 2>&1 || fail "'cargo' is required but was not found"

    revision="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    echo "building pandemonium preview at $revision"
    cargo build --release --locked -p pandemonium

    mkdir -p "$BIN_DIR"
    install -m 0755 target/release/pandemonium "$BIN_DIR/$NAME"
    case "$(uname -s)" in
        Linux) install_desktop_entry ;;
        Darwin) install_app_bundle ;;
    esac

    echo "installed $("$BIN_DIR/$NAME" --version) ($revision) to $BIN_DIR/$NAME"
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) echo "note: $BIN_DIR is not on your PATH; add it to run '$NAME' from a shell" ;;
    esac
}

main
