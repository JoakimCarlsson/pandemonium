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
RUSTLE_REPO="https://github.com/JoakimCarlsson/rustle"

# Prints the tag of a repository the crates depend on, and exits unless every
# crate names the same one.
#   $1  the repository's URL
#   $2  the repository's name, for messages
dependency_tag() {
    tags="$(grep -h "$1\"" crates/*/Cargo.toml | sed -n 's/.*tag = "\([^"]*\)".*/\1/p' | sort -u)"
    [ -n "$tags" ] || fail "no crate depends on a $2 tag"
    [ "$(echo "$tags" | wc -l)" -eq 1 ] || fail "the crates depend on different $2 tags: $(echo $tags)"
    echo "$tags"
}

# Prints the commit a tag of a repository points at on GitHub, and exits when
# the tag is not published.
#   $1  the repository's URL
#   $2  the repository's name, for messages
#   $3  the tag
dependency_commit() {
    refs="$(git ls-remote --tags "$1" "refs/tags/$3" "refs/tags/$3^{}")" \
        || fail "could not reach $1"
    [ -n "$refs" ] || fail "$2 $3 is not published; release $2 first"
    echo "$refs" | awk '{ commit = $1 } $2 ~ /\^\{\}$/ { peeled = $1 } END { print (peeled != "" ? peeled : commit) }'
}

# Exits unless every package of a repository in Cargo.lock comes from its
# published tag, so a release never builds against a local checkout or a
# stale lock.
#   $1  the repository's URL
#   $2  the repository's name, for messages
#   $3  the pattern the names of its packages match
#   $4  the tag
#   $5  the commit the tag points at
check_dependency_lock() {
    expected="git+$1?tag=$4#$5"
    wrong="$(awk -v expected="$expected" -v named="$3" '
        /^\[\[package\]\]/ { if (name ~ named && source != expected) print name; name = ""; source = "" }
        /^name = / { name = $3; gsub(/"/, "", name) }
        /^source = / { source = $3; gsub(/"/, "", source) }
        END { if (name ~ named && source != expected) print name }
    ' Cargo.lock)"
    [ -z "$wrong" ] || fail "Cargo.lock does not take $(echo $wrong) from $2 $4; remove the local override in .cargo/config.toml and run cargo update"
}

# Exits unless the release builds against a published oh-my-crab tag.
check_crab() {
    crab_version="$(dependency_tag "$CRAB_REPO" oh-my-crab)"
    crab_revision="$(dependency_commit "$CRAB_REPO" oh-my-crab "$crab_version")"
    check_dependency_lock "$CRAB_REPO" oh-my-crab '^omc-' "$crab_version" "$crab_revision"
}

# Exits unless the release builds against a published rustle tag.
check_rustle() {
    rustle_version="$(dependency_tag "$RUSTLE_REPO" rustle)"
    rustle_revision="$(dependency_commit "$RUSTLE_REPO" rustle "$rustle_version")"
    check_dependency_lock "$RUSTLE_REPO" rustle '^rustle(-|$)' "$rustle_version" "$rustle_revision"
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
    check_rustle
    bump_version "$version"
    check_crab
    check_rustle
    echo "building against oh-my-crab $crab_version and rustle $rustle_version"
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
