#!/bin/sh
#
# Installs this checkout as pandemonium-preview, or, given `server`, installs
# only the headless pandemonium-server.
#
#   scripts/install-preview.sh [server]

set -eu

BIN_DIR="${PANDEMONIUM_BIN_DIR:-$HOME/.local/bin}"
APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
MAC_APPS_DIR="${PANDEMONIUM_APP_DIR:-$HOME/Applications}"
NAME="pandemonium-preview"

# Prints an error and exits.
fail() {
    echo "error: $*" >&2
    exit 1
}

# Adds the editor's icon to the user's hicolor theme, one size per picture.
install_icons() {
    pngs="$1"
    svg="$2"
    for png in "$pngs"/pandemonium-*.png; do
        size="${png##*-}"
        size="${size%.png}"
        [ "$size" -le 512 ] || continue
        mkdir -p "$ICONS_DIR/${size}x${size}/apps"
        install -m 0644 "$png" "$ICONS_DIR/${size}x${size}/apps/pandemonium.png"
    done
    mkdir -p "$ICONS_DIR/scalable/apps"
    install -m 0644 "$svg" "$ICONS_DIR/scalable/apps/pandemonium.svg"
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$ICONS_DIR" >/dev/null 2>&1 || true
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
    scripts/bundle-macos.sh target/release/pandemonium "$MAC_APPS_DIR/Pandemonium Preview.app" \
        "Pandemonium Preview" io.github.joakimcarlsson.pandemonium.preview
    echo "installed $MAC_APPS_DIR/Pandemonium Preview.app"
}

# Builds and installs the window, which carries the local server in its `--server` mode.
install_editor() {
    revision="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    echo "building pandemonium preview at $revision"
    PANDEMONIUM_CHANNEL=preview cargo build --release --locked -p pandemonium
    mkdir -p "$BIN_DIR"
    install -m 0755 target/release/pandemonium "$BIN_DIR/$NAME"
    case "$(uname -s)" in
        Linux)
            install_icons assets/brand/png assets/brand/pandemonium.svg
            install_desktop_entry
            ;;
        Darwin) install_app_bundle ;;
    esac
    echo "installed $("$BIN_DIR/$NAME" --server --version) ($revision) to $BIN_DIR/$NAME"
}

# Builds and installs the headless server alone.
install_server() {
    echo "building pandemonium-server"
    cargo build --release --locked -p pm-server
    mkdir -p "$BIN_DIR"
    install -m 0755 target/release/pandemonium-server "$BIN_DIR/pandemonium-server"
    echo "installed $("$BIN_DIR/pandemonium-server" --version) to $BIN_DIR/pandemonium-server"
}

# Installs what was asked for and warns when it is not on the PATH.
main() {
    command -v cargo >/dev/null 2>&1 || fail "'cargo' is required but was not found"
    case "${1:-editor}" in
        editor) install_editor ;;
        server) install_server ;;
        *) fail "usage: $0 [server]" ;;
    esac
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) echo "note: $BIN_DIR is not on your PATH" ;;
    esac
}

main "$@"
