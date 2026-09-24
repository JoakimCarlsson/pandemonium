#!/bin/sh
#
# Cuts a release: bumps the workspace version, commits, tags and pushes.
# The tag triggers .github/workflows/release.yml, which builds every target
# and publishes the GitHub release once all of them have succeeded.
#
#   scripts/release.sh 0.2.0
#   scripts/release.sh 0.3.0-rc.1     (a hyphen publishes a pre-release)

set -eu

# Prints an error and exits.
fail() {
    echo "error: $*" >&2
    exit 1
}

# Exits unless the tree is clean, on main and level with origin/main.
check_repository() {
    [ -z "$(git status --porcelain)" ] || fail "the working tree has uncommitted changes"
    [ "$(git rev-parse --abbrev-ref HEAD)" = "main" ] || fail "releases are cut from main"
    git fetch --quiet --tags origin main
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || fail "main is not level with origin/main"
}

# Writes the version into the workspace manifest and the lockfile.
bump_version() {
    awk -v version="$1" '!done && /^version = "/ { $0 = "version = \"" version "\""; done = 1 } 1' \
        Cargo.toml > Cargo.toml.new
    mv Cargo.toml.new Cargo.toml
    cargo update --workspace --quiet
}

# Bumps, verifies, commits, tags and pushes the release.
main() {
    [ $# -eq 1 ] || fail "usage: $0 <version>"
    version="${1#v}"
    tag="v$version"
    echo "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$' \
        || fail "'$version' is not a semantic version"

    cd "$(git rev-parse --show-toplevel)"
    check_repository
    git rev-parse -q --verify "refs/tags/$tag" >/dev/null && fail "$tag already exists"

    bump_version "$version"
    cargo clippy --workspace --all-targets --locked -- -D warnings

    git commit --quiet -am "Release $tag"
    git tag -a "$tag" -m "$tag"

    printf 'push %s and main to origin? [y/N] ' "$tag"
    read -r answer
    case "$answer" in
        y | Y) git push --atomic origin main "$tag" ;;
        *) echo "not pushed; undo with: git tag -d $tag && git reset --hard HEAD~1"; exit 0 ;;
    esac
    echo "pushed $tag; follow the build at https://github.com/JoakimCarlsson/pandemonium/actions"
}

main "$@"
