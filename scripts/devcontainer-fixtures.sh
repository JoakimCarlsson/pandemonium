#!/bin/sh
# Initializes the dev container fixtures as git repositories under build/devcontainer-fixtures.
#
#   scripts/devcontainer-fixtures.sh            # image and dockerfile fixtures
#   scripts/devcontainer-fixtures.sh image      # one of them
#
# Each repository is recreated from scratch; run it again to reset a fixture.

set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/crates/pm-devcontainer/fixtures"
TARGET="$ROOT/build/devcontainer-fixtures"

# Copies one fixture into the build directory and commits it.
initialize() {
    name="$1"
    [ -d "$FIXTURES/$name" ] || { echo "error: no fixture named $name" >&2; exit 1; }
    rm -rf "${TARGET:?}/$name"
    mkdir -p "$TARGET"
    cp -R "$FIXTURES/$name" "$TARGET/$name"
    (
        cd "$TARGET/$name"
        git init -q -b main
        git add -A
        git -c user.name=Fixture -c user.email=fixture@example.invalid commit -q -m "Initial fixture"
    )
    echo "$TARGET/$name"
}

if [ "$#" -eq 0 ]; then
    set -- image dockerfile
fi
for name in "$@"; do
    initialize "$name"
done
