#!/usr/bin/env bash
# Local fallback for the Release workflow: build a tag, package it, upload it.
#
#   packaging/release.sh v0.2.0            # build the tag and upload to its release
#   packaging/release.sh v0.2.0 --dry-run  # build and package, do not upload
#
# The build runs in a temporary git worktree checked out at the tag, so the
# checkout does not have to be clean or at the tag. This is only a fallback:
# the binary then needs the build host's glibc (openSUSE Tumbleweed has 2.44,
# so the artifacts will not start on Debian 12 or Ubuntu 22.04, while the
# GitHub runner image ubuntu-22.04 produces a glibc 2.35 baseline).
set -euo pipefail

root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
tag=
dry_run=0

usage() {
    sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

for arg in "$@"; do
    case $arg in
        --dry-run) dry_run=1 ;;
        -h|--help) usage; exit 0 ;;
        -*) echo "unknown option: $arg (try --help)" >&2; exit 2 ;;
        *)
            [ -z "$tag" ] || { echo "only one tag can be given" >&2; exit 2; }
            tag=$arg
            ;;
    esac
done

cd "$root"

if [ -z "$tag" ]; then
    version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
    tag=v$version
fi

if ! git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
    echo "==> tag $tag is not local, fetching tags"
    git fetch --tags origin
fi
git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null ||
    { echo "tag $tag does not exist" >&2; exit 1; }

if [ "$dry_run" -eq 0 ]; then
    command -v gh >/dev/null 2>&1 ||
        { echo "gh is needed to upload; use --dry-run" >&2; exit 1; }
    gh auth status >/dev/null 2>&1 ||
        { echo "gh is not authenticated; use --dry-run" >&2; exit 1; }
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/legion-monitor-release.XXXXXX")
worktree=$tmp/src
cleanup() {
    git worktree remove --force "$worktree" >/dev/null 2>&1 || true
    rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

echo "==> checking out $tag in $worktree"
git worktree add --detach "$worktree" "$tag" >/dev/null

# Tags that predate packaging/package.sh (v0.2.0) take the packaging tools from
# the current checkout; sources and assets still come from the tag.
if [ ! -f "$worktree/packaging/package.sh" ]; then
    echo "==> tag predates the packaging scripts, copying them from the checkout"
    rm -rf "$worktree/packaging"
    cp -a "$root/packaging" "$worktree/packaging"
fi

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$worktree/Cargo.toml" | head -n 1)
[ "v$version" = "$tag" ] ||
    { echo "tag $tag does not match the crate version $version" >&2; exit 1; }

echo "==> cargo build --locked --release"
(
    cd "$worktree"
    CARGO_TARGET_DIR="$root/target" cargo build --locked --release
)
binary=$root/target/release/legion-monitor
[ -x "$binary" ] || { echo "missing $binary" >&2; exit 1; }

glibc=$(
    readelf --version-info "$binary" 2>/dev/null |
        grep -oE 'GLIBC_[0-9]+\.[0-9]+' |
        sed 's/^GLIBC_//' |
        sort -V -u |
        tail -n 1 || true
)
echo "==> binary needs glibc ${glibc:-unknown} (local fallback build)"
if [ -n "$glibc" ] && [ "$(printf '%s\n2.35\n' "$glibc" | sort -V | tail -n 1)" = "$glibc" ] &&
    [ "$glibc" != 2.35 ]; then
    echo "    warning: this is newer than the 2.35 baseline the GitHub release uses;" >&2
    echo "    users on Debian 12 or Ubuntu 22.04 will not be able to run it" >&2
fi

rm -rf "$root/dist"
bash "$worktree/packaging/package.sh" \
    --version "$version" \
    --binary "$binary" \
    --out "$root/dist"

if [ "$dry_run" -eq 1 ]; then
    echo
    echo "==> dry run: packages are in $root/dist, nothing was uploaded"
    exit 0
fi

echo "==> uploading to release $tag"
if gh release view "$tag" >/dev/null 2>&1; then
    gh release upload "$tag" dist/* --clobber
else
    gh release create "$tag" dist/* \
        --verify-tag \
        --title "Legion Monitor $tag" \
        --generate-notes
fi
