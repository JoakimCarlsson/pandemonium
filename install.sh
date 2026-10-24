#!/bin/sh
#
# Installs pandemonium from a GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | sh
#
# Environment:
#   PANDEMONIUM_CHANNEL   stable or preview                  (default: stable; nightly is the old name of preview)
#   PANDEMONIUM_VERSION   release to install, e.g. 0.2.0     (default: the latest release of the channel)
#   PANDEMONIUM_COMPONENT editor or server                    (default: editor)
#   PANDEMONIUM_BIN_DIR   where the binary goes              (default: ~/.local/bin)
#   PANDEMONIUM_APP_DIR   where the macOS app goes           (default: ~/Applications)

set -eu

REPO="JoakimCarlsson/pandemonium"
CHANNEL="${PANDEMONIUM_CHANNEL:-stable}"
[ "$CHANNEL" = "nightly" ] && CHANNEL="preview"
VERSION="${PANDEMONIUM_VERSION:-}"
COMPONENT="${PANDEMONIUM_COMPONENT:-editor}"
BIN_DIR="${PANDEMONIUM_BIN_DIR:-$HOME/.local/bin}"
APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
MAC_APPS_DIR="${PANDEMONIUM_APP_DIR:-$HOME/Applications}"

# Sets the names the chosen channel installs under, so that preview sits beside the stable app.
choose_channel() {
    case "$CHANNEL" in
        stable)
            slug="pandemonium"
            label="Pandemonium"
            VERSION="${VERSION:-latest}"
            ;;
        preview)
            slug="pandemonium-preview"
            label="Pandemonium Preview"
            VERSION="${VERSION:-latest}"
            ;;
        *) fail "PANDEMONIUM_CHANNEL must be stable or preview" ;;
    esac
}

# Prints an error and exits.
fail() {
    echo "error: $*" >&2
    exit 1
}

# Exits unless the named command is on PATH.
need() {
    command -v "$1" >/dev/null 2>&1 || fail "'$1' is required but was not found"
}

# Prints the Rust target triple the release was built for on this machine.
detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$arch" in
        x86_64 | amd64) arch="x86_64" ;;
        arm64 | aarch64) arch="aarch64" ;;
        *) fail "unsupported architecture: $arch" ;;
    esac
    case "$os" in
        Linux) echo "$arch-unknown-linux-gnu" ;;
        Darwin) echo "$arch-apple-darwin" ;;
        MINGW* | MSYS* | CYGWIN*) fail "on Windows, use install.ps1 instead" ;;
        *) fail "unsupported OS: $os" ;;
    esac
}

# Prints the version the latest release is tagged with, without the leading v.
latest_version() {
    url="$(curl --connect-timeout 10 --max-time 120 -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" \
        || fail "could not reach the latest release of $REPO"
    tag="${url##*/tag/}"
    [ "$tag" != "$url" ] || fail "$REPO has no published release yet"
    echo "${tag#v}"
}

# Prints the version the newest preview release is tagged with, without the leading v.
latest_preview_version() {
    listing="$(curl --connect-timeout 10 --max-time 120 -fsSL "https://api.github.com/repos/$REPO/releases?per_page=100")" \
        || fail "could not reach the releases of $REPO"
    tag="$(echo "$listing" | grep -o '"tag_name": *"[^"]*-[^"]*"' | head -n 1 | sed 's/.*: *"\(.*\)"/\1/')"
    [ -n "$tag" ] || fail "$REPO has no preview release yet"
    echo "${tag#v}"
}

# Prints the folder the desktop keeps an app's data folders in.
data_root() {
    case "$(uname -s)" in
        Darwin) echo "$HOME/Library/Application Support" ;;
        *) echo "${XDG_DATA_HOME:-$HOME/.local/share}" ;;
    esac
}

# Stops the server that the nightly install left running, so that its data folder can be moved.
stop_nightly_server() {
    credential="$1/server.json"
    [ -f "$credential" ] || return 0
    pid="$(sed -n 's/.*"pid" *: *\([0-9][0-9]*\).*/\1/p' "$credential" | head -n 1)"
    [ -n "$pid" ] || return 0
    kill -TERM "$pid" 2>/dev/null || return 0
    waited=0
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt 40 ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
}

# Moves what nightly kept in the data folder `$1` into the one preview already has, `$2`: the projects, sessions and the rest replace preview's, and the settings preview has already are left as they are.
merge_nightly_data() {
    for entry in "$1"/* "$1"/.[!.]*; do
        [ -e "$entry" ] || continue
        name="$(basename "$entry")"
        case "$name" in
            server.json) continue ;;
            window-settings.json | editor-settings.json | machine-settings)
                [ -e "$2/$name" ] && continue
                ;;
        esac
        rm -rf "${2:?}/$name"
        mv "$entry" "$2/$name"
    done
    rm -rf "$1"
}

# Moves a nightly install over to preview: its server is stopped, its data folder and so its settings, projects and sessions are carried across, and the files it installed are removed.
migrate_nightly() {
    [ "$CHANNEL" = "preview" ] || return 0
    old="pandemonium-nightly"
    root="$(data_root)"
    [ -e "$BIN_DIR/$old" ] || [ -e "$BIN_DIR/$old-server" ] || [ -d "$root/$old" ] || [ -d "$MAC_APPS_DIR/Pandemonium Nightly.app" ] || return 0
    echo "moving your nightly install over to preview"
    stop_nightly_server "$root/$old"
    if [ -d "$root/$old" ]; then
        if [ -e "$root/$slug" ]; then
            merge_nightly_data "$root/$old" "$root/$slug" && echo "moved your projects and sessions from $root/$old to $root/$slug"
        else
            mv "$root/$old" "$root/$slug" && echo "moved $root/$old to $root/$slug"
        fi
    fi
    rm -f "$BIN_DIR/$old" "$BIN_DIR/$old-server" "$APPS_DIR/$old.desktop"
    for icon in "$ICONS_DIR"/*/apps/"$old".png "$ICONS_DIR"/scalable/apps/"$old".svg; do
        [ -e "$icon" ] && rm -f "$icon"
    done
    rm -rf "$MAC_APPS_DIR/Pandemonium Nightly.app"
}

# Checks the downloaded archive against the release's SHA256SUMS.
verify() {
    dir="$1"
    archive="$2"
    line="$(grep " $archive\$" "$dir/SHA256SUMS")" || fail "$archive is not listed in SHA256SUMS"
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$dir" && echo "$line" | sha256sum -c - >/dev/null)
    else
        (cd "$dir" && echo "$line" | shasum -a 256 -c - >/dev/null)
    fi || fail "checksum mismatch for $archive"
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
        install -m 0644 "$png" "$ICONS_DIR/${size}x${size}/apps/$slug.png"
    done
    mkdir -p "$ICONS_DIR/scalable/apps"
    install -m 0644 "$svg" "$ICONS_DIR/scalable/apps/$slug.svg"
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$ICONS_DIR" >/dev/null 2>&1 || true
}

# Registers the editor with the desktop's application launcher.
install_desktop_entry() {
    source="$1"
    mkdir -p "$APPS_DIR"
    sed "s|^Exec=.*|Exec=$BIN_DIR/$slug|" "$source" > "$APPS_DIR/$slug.desktop"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
}

# Downloads, verifies and installs the macOS app bundle.
install_app_bundle() {
    base="$1"
    name="$2"
    tmp="$3"
    if ! curl --connect-timeout 10 --max-time 120 -fsSL "$base/$name.app.zip" -o "$tmp/$name.app.zip"; then
        echo "note: this release has no macOS app; only the binary was installed"
        return
    fi
    verify "$tmp" "$name.app.zip"
    ditto -x -k "$tmp/$name.app.zip" "$tmp/app"
    mkdir -p "$MAC_APPS_DIR"
    rm -rf "$MAC_APPS_DIR/$label.app"
    mv "$tmp/app/$label.app" "$MAC_APPS_DIR/$label.app"
    xattr -dr com.apple.quarantine "$MAC_APPS_DIR/$label.app" 2>/dev/null || true
    echo "installed $MAC_APPS_DIR/$label.app"
}

# Downloads, verifies and installs the release.
main() {
    need curl
    need tar
    command -v sha256sum >/dev/null 2>&1 || need shasum
    choose_channel
    case "$COMPONENT" in
        editor) binary="$slug" ;;
        server) binary="$slug-server" ;;
        *) fail "PANDEMONIUM_COMPONENT must be editor or server" ;;
    esac

    target="$(detect_target)"
    if [ "$VERSION" = "latest" ]; then
        if [ "$CHANNEL" = "preview" ]; then
            VERSION="$(latest_preview_version)"
        else
            VERSION="$(latest_version)"
        fi
    fi
    VERSION="${VERSION#v}"

    name="$binary-$VERSION-$target"
    tag="v$VERSION"
    base="https://github.com/$REPO/releases/download/$tag"
    scratch="${XDG_CACHE_HOME:-$HOME/.cache}/scratch/pandemonium"
    mkdir -p "$scratch"
    tmp="$(mktemp -d "$scratch/install.XXXXXXXX")"
    trap 'rm -rf "$tmp"' EXIT INT TERM

    echo "downloading $binary $VERSION for $target"
    curl --connect-timeout 10 --max-time 120 -fsSL "$base/$name.tar.gz" -o "$tmp/$name.tar.gz" \
        || fail "no build of $VERSION for $target at $base/$name.tar.gz"
    curl --connect-timeout 10 --max-time 120 -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" \
        || fail "could not download SHA256SUMS for $VERSION"
    verify "$tmp" "$name.tar.gz"
    migrate_nightly

    tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
    mkdir -p "$BIN_DIR"
    install -m 0755 "$tmp/$name/$binary" "$BIN_DIR/$binary"
    [ "$(uname -s)" = "Darwin" ] && xattr -d com.apple.quarantine "$BIN_DIR/$binary" 2>/dev/null || true
    if [ "$COMPONENT" = "editor" ]; then
        [ -d "$tmp/$name/icons" ] && [ "$(uname -s)" = "Linux" ] && install_icons "$tmp/$name/icons" "$tmp/$name/icons/pandemonium.svg"
        [ -f "$tmp/$name/$slug.desktop" ] && install_desktop_entry "$tmp/$name/$slug.desktop"
        [ "$(uname -s)" = "Darwin" ] && install_app_bundle "$base" "$name" "$tmp"
    fi

    echo "installed $("$BIN_DIR/$binary" --version) to $BIN_DIR"
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) echo "note: $BIN_DIR is not on your PATH; add it to run '$binary' from a shell" ;;
    esac
}

main
