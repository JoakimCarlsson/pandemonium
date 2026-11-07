#!/bin/sh
#
# Cuts a release: bumps the workspace version, commits, tags and pushes.
# The tag triggers .github/workflows/release.yml, which builds every target
# and publishes the GitHub release once all of them have succeeded.
#
#   scripts/release.sh 0.2.0
#   scripts/release.sh 0.3.0-rc.1     (a hyphen publishes a preview release)
#
# Stable versions are cut from main, or from a release/X.Y branch for a patch.
# Preview releases are cut from main or from next.

set -eu

# Prints an error and exits.
fail() {
    echo "error: $*" >&2
    exit 1
}

# Exits unless the branch may release this version.
check_branch() {
    branch="$1"
    version="$2"
    case "$version" in
        *-*)
            case "$branch" in main | next) return ;; esac
            fail "preview releases are cut from main or next, not $branch"
            ;;
        *)
            case "$branch" in main | release/*) return ;; esac
            fail "stable releases are cut from main or a release/ branch, not $branch"
            ;;
    esac
}

# Exits unless the tree is clean, the branch may release the version and the
# branch is level with its remote.
check_repository() {
    [ -z "$(git status --porcelain)" ] || fail "the working tree has uncommitted changes"
    check_branch "$1" "$2"
    git fetch --quiet --tags origin "$1"
    [ "$(git rev-parse HEAD)" = "$(git rev-parse "origin/$1")" ] || fail "$1 is not level with origin/$1"
}

# Writes the version into the workspace manifest and the lockfile.
bump_version() {
    awk -v version="$1" '!done && /^version = "/ { $0 = "version = \"" version "\""; done = 1 } 1' \
        Cargo.toml > Cargo.toml.new
    mv Cargo.toml.new Cargo.toml
    cargo update --workspace --quiet
}

CRAB_REPO="https://github.com/JoakimCarlsson/oh-my-crab"

# Prints the oh-my-crab tag the crates depend on, and exits unless every crate
# names the same one.
crab_tag() {
    tags="$(grep -h "$CRAB_REPO" crates/*/Cargo.toml | sed -n 's/.*tag = "\([^"]*\)".*/\1/p' | sort -u)"
    [ -n "$tags" ] || fail "no crate depends on an oh-my-crab tag"
    [ "$(echo "$tags" | wc -l)" -eq 1 ] || fail "the crates depend on different oh-my-crab tags: $(echo $tags)"
    echo "$tags"
}

# Prints the commit an oh-my-crab tag points at on GitHub, and exits when the
# tag is not published.
#   $1  the tag
crab_commit() {
    refs="$(git ls-remote --tags "$CRAB_REPO" "refs/tags/$1" "refs/tags/$1^{}")" \
        || fail "could not reach $CRAB_REPO"
    [ -n "$refs" ] || fail "oh-my-crab $1 is not published; release oh-my-crab first"
    echo "$refs" | awk '{ commit = $1 } $2 ~ /\^\{\}$/ { peeled = $1 } END { print (peeled != "" ? peeled : commit) }'
}

# Exits unless every omc-* package in Cargo.lock comes from the published
# oh-my-crab tag, so a release never builds against a local checkout or a
# stale lock.
#   $1  the tag
#   $2  the commit the tag points at
check_crab_lock() {
    expected="git+$CRAB_REPO?tag=$1#$2"
    wrong="$(awk -v expected="$expected" '
        /^\[\[package\]\]/ { if (name ~ /^omc-/ && source != expected) print name; name = ""; source = "" }
        /^name = / { name = $3; gsub(/"/, "", name) }
        /^source = / { source = $3; gsub(/"/, "", source) }
        END { if (name ~ /^omc-/ && source != expected) print name }
    ' Cargo.lock)"
    [ -z "$wrong" ] || fail "Cargo.lock does not take $(echo $wrong) from oh-my-crab $1; remove the local override in .cargo/config.toml and run cargo update"
}

# Exits unless the release builds against a published oh-my-crab tag.
check_crab() {
    crab_version="$(crab_tag)"
    crab_revision="$(crab_commit "$crab_version")"
    check_crab_lock "$crab_version" "$crab_revision"
}

# Bumps, verifies, commits, tags and pushes the release.
main() {
    [ $# -eq 1 ] || fail "usage: $0 <version>"
    version="${1#v}"
    tag="$version"
    echo "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$' \
        || fail "'$version' is not a semantic version"

    cd "$(git rev-parse --show-toplevel)"
    branch="$(git rev-parse --abbrev-ref HEAD)"
    check_repository "$branch" "$version"
    git rev-parse -q --verify "refs/tags/$tag" >/dev/null && fail "$tag already exists"

    check_crab
    bump_version "$version"
    check_crab
    echo "building against oh-my-crab $crab_version"
    cargo clippy --workspace --all-targets --locked -- -D warnings

    git commit --quiet -am "Release $tag"
    git tag -a "$tag" -m "$tag"

    printf 'push %s and %s to origin? [y/N] ' "$tag" "$branch"
    read -r answer
    case "$answer" in
        y | Y) git push --atomic origin "$branch" "$tag" ;;
        *) echo "not pushed; undo with: git tag -d $tag && git reset --hard HEAD~1"; exit 0 ;;
    esac
    echo "pushed $tag; follow the build at https://github.com/JoakimCarlsson/pandemonium/actions"
}

main "$@"
