#!/bin/sh
#
# Installs pandemonium from a GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | sh
#
# Environment:
#   PANDEMONIUM_VERSION   release to install, e.g. 0.2.0     (default: latest)
#   PANDEMONIUM_BIN_DIR   where the binary goes              (default: ~/.local/bin)

set -eu

REPO="JoakimCarlsson/pandemonium"
VERSION="${PANDEMONIUM_VERSION:-latest}"
BIN_DIR="${PANDEMONIUM_BIN_DIR:-$HOME/.local/bin}"
APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

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
    url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" \
        || fail "could not reach the latest release of $REPO"
    tag="${url##*/tag/}"
    [ "$tag" != "$url" ] || fail "$REPO has no published release yet"
    echo "${tag#v}"
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

# Registers the editor with the desktop's application launcher.
install_desktop_entry() {
    source="$1"
    mkdir -p "$APPS_DIR"
    sed "s|^Exec=pandemonium|Exec=$BIN_DIR/pandemonium|" "$source" > "$APPS_DIR/pandemonium.desktop"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
}

# Downloads, verifies and installs the release.
main() {
    need curl
    need tar
    command -v sha256sum >/dev/null 2>&1 || need shasum

    target="$(detect_target)"
    [ "$VERSION" = "latest" ] && VERSION="$(latest_version)"
    VERSION="${VERSION#v}"

    name="pandemonium-$VERSION-$target"
    base="https://github.com/$REPO/releases/download/v$VERSION"
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT INT TERM

    echo "downloading pandemonium $VERSION for $target"
    curl -fsSL "$base/$name.tar.gz" -o "$tmp/$name.tar.gz" \
        || fail "no build of $VERSION for $target at $base/$name.tar.gz"
    curl -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" \
        || fail "could not download SHA256SUMS for $VERSION"
    verify "$tmp" "$name.tar.gz"

    tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
    mkdir -p "$BIN_DIR"
    install -m 0755 "$tmp/$name/pandemonium" "$BIN_DIR/pandemonium"
    [ -f "$tmp/$name/pandemonium.desktop" ] && install_desktop_entry "$tmp/$name/pandemonium.desktop"

    echo "installed $("$BIN_DIR/pandemonium" --version) to $BIN_DIR"
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) echo "note: $BIN_DIR is not on your PATH; add it to run 'pandemonium' from a shell" ;;
    esac
}

main
