#!/bin/sh
#
# Packs the browser helper into the archive the window downloads when the reader
# turns the browser on, and its checksum: Electron, which holds the engine, with
# English as its only locale where the system allows it, and the helper's own
# files. It packs for the system it runs on; set npm_config_arch to pack another
# architecture's Electron.
#
#   scripts/bundle-browser.sh <archive>

set -eu

[ $# -eq 1 ] || { echo "usage: $0 <archive>" >&2; exit 1; }
command -v npm >/dev/null 2>&1 || { echo "error: 'npm' is required but was not found" >&2; exit 1; }
archive="$1"
helper="$(CDPATH= cd -- "$(dirname -- "$0")/../helpers/electron-browser" && pwd)"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/app" "$stage/work" "$stage/out"
cp "$helper/package.json" "$helper/package-lock.json" "$stage/work/"
(
    cd "$stage/work"
    npm ci --omit=dev --no-audit --no-fund >/dev/null
    node node_modules/electron/install.js
)
cp -r "$helper/main.js" "$helper/src" "$helper/package.json" "$stage/app/"
dist="$stage/work/node_modules/electron/dist"
case "$(uname -s)" in
    Linux)
        find "$dist/locales" -type f ! -name 'en-US.pak' -delete
        cp -r "$dist" "$stage/out/electron"
        cp -r "$stage/app" "$stage/out/app"
        cat > "$stage/out/pandemonium-browser" <<'LAUNCHER'
#!/bin/sh
here=$(CDPATH= cd -- "$(dirname -- "$(readlink -f -- "$0")")" && pwd)
exec "$here/electron/electron" --ozone-platform-hint=auto "$here/app/main.js" "$@"
LAUNCHER
        chmod 0755 "$stage/out/pandemonium-browser"
        ;;
    Darwin)
        cp -R "$dist" "$stage/out/electron"
        cp -r "$stage/app" "$stage/out/app"
        cat > "$stage/out/pandemonium-browser" <<'LAUNCHER'
#!/bin/sh
here=$(CDPATH= cd -- "$(dirname -- "$(readlink -f -- "$0")")" && pwd)
exec "$here/electron/Electron.app/Contents/MacOS/Electron" "$here/app/main.js" "$@"
LAUNCHER
        chmod 0755 "$stage/out/pandemonium-browser"
        ;;
    MINGW* | MSYS* | CYGWIN*)
        find "$dist/locales" -type f ! -name 'en-US.pak' -delete
        cp -r "$dist/." "$stage/out/"
        mv "$stage/out/electron.exe" "$stage/out/pandemonium-browser.exe"
        cp -r "$stage/app" "$stage/out/resources/app"
        ;;
    *)
        echo "error: no browser bundle for $(uname -s)" >&2
        exit 1
        ;;
esac
mkdir -p "$(dirname "$archive")"
tar -czf "$archive" -C "$stage/out" .
(cd "$(dirname "$archive")" && sha256sum "$(basename "$archive")" > "$(basename "$archive").sha256")
