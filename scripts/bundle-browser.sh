#!/bin/sh
#
# Packs the browser helper into the archive the window downloads when the reader
# turns the browser on, and its checksum: Electron, which holds the engine, with
# English as its only locale, and the helper's own files.
#
#   scripts/bundle-browser.sh <archive>

set -eu

[ $# -eq 1 ] || { echo "usage: $0 <archive>" >&2; exit 1; }
command -v npm >/dev/null 2>&1 || { echo "error: 'npm' is required but was not found" >&2; exit 1; }
archive="$1"
helper="$(CDPATH= cd -- "$(dirname -- "$0")/../helpers/electron-browser" && pwd)"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/app" "$stage/work"
cp "$helper/package.json" "$helper/package-lock.json" "$stage/work/"
(
    cd "$stage/work"
    npm ci --omit=dev --no-audit --no-fund >/dev/null
    node node_modules/electron/install.js
)
cp -r "$stage/work/node_modules/electron/dist" "$stage/electron"
find "$stage/electron/locales" -type f ! -name 'en-US.pak' -delete
cp -r "$helper/main.js" "$helper/src" "$helper/package.json" "$stage/app/"
cat > "$stage/pandemonium-browser" <<'LAUNCHER'
#!/bin/sh
here=$(CDPATH= cd -- "$(dirname -- "$(readlink -f -- "$0")")" && pwd)
exec "$here/electron/electron" --ozone-platform-hint=auto "$here/app/main.js" "$@"
LAUNCHER
chmod 0755 "$stage/pandemonium-browser"
rm -rf "$stage/work"
mkdir -p "$(dirname "$archive")"
tar -czf "$archive" -C "$stage" .
(cd "$(dirname "$archive")" && sha256sum "$(basename "$archive")" > "$(basename "$archive").sha256")
