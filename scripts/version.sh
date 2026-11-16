#!/usr/bin/env bash
# Bunode's version is MAJOR.MINOR.N. MAJOR.MINOR is the series, from
# `[workspace.package] version` in Cargo.toml (its patch number is always
# 0); N is the number of commits up to the one being built. So every
# commit on main has its own version, each higher than the last, and a
# release is a tag on one of them: nothing is bumped by hand.
#
#   scripts/version.sh            prints the version, e.g. 0.1.74
#   scripts/version.sh --build    prints the build number: the commit count, 74
#   scripts/version.sh --tag      prints the tag a release of this commit gets, v0.1.74
#   scripts/version.sh --release  dates the Unreleased notes in CHANGELOG.md as the
#                                 next version, commits that and tags the commit
#   scripts/version.sh --set 0.2  starts a new series (rare: N keeps counting)
#
# The Makefile hands the version and the build number to XcodeGen, so
# Bunode.app's CFBundleShortVersionString and CFBundleVersion follow them.
set -euo pipefail

cd "$(dirname "$0")/.."

series() {
  sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([0-9][0-9]*\.[0-9][0-9]*\)\..*"/\1/p' Cargo.toml
}

# 0 where there is no history to count, such as a source archive.
build() {
  git rev-list --count HEAD 2>/dev/null || echo 0
}

# A shallow clone counts only the commits it has.
need_full_history() {
  if [ "$(git rev-parse --is-shallow-repository 2>/dev/null)" != "false" ]; then
    echo "error: needs the full history to count commits (git fetch --unshallow)" >&2
    exit 1
  fi
}

case "${1:-}" in
  "")
    echo "$(series).$(build)"
    ;;
  --build)
    build
    ;;
  --tag)
    need_full_history
    echo "v$(series).$(build)"
    ;;
  --release)
    need_full_history
    if ! git diff --quiet || ! git diff --cached --quiet; then
      echo "error: commit or stash your changes first" >&2
      exit 1
    fi
    # The release commit made below is the one that gets the version.
    version="$(series).$(($(build) + 1))"
    if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
      echo "error: the tag v$version already exists" >&2
      exit 1
    fi
    # The Unreleased notes become the new version's.
    VERSION="$version" TODAY="$(date +%Y-%m-%d)" perl -pi -e '
      s/^## \[Unreleased\]$/## [Unreleased]\n\n## [$ENV{VERSION}] - $ENV{TODAY}/;
      s{^\[Unreleased\]: .*}{[Unreleased]: https://github.com/imal1/bunode/compare/v$ENV{VERSION}...HEAD\n[$ENV{VERSION}]: https://github.com/imal1/bunode/releases/tag/v$ENV{VERSION}};
    ' CHANGELOG.md
    if git diff --quiet -- CHANGELOG.md; then
      echo "error: CHANGELOG.md has no Unreleased section to date" >&2
      exit 1
    fi
    git commit --quiet -m "chore: release $version" -- CHANGELOG.md
    git tag "v$version"
    echo "Tagged v$version. Publish it with: git push origin $(git branch --show-current) v$version"
    ;;
  --set)
    new="${2:?usage: scripts/version.sh --set X.Y}"
    if ! [[ "$new" =~ ^[0-9]+\.[0-9]+$ ]]; then
      echo "error: $new is not X.Y" >&2
      exit 1
    fi
    old=$(series)
    NEW="$new" perl -pi -e 's/^version = "[^"]*"/version = "$ENV{NEW}.0"/ if /^\[workspace\.package\]/ ... /^\[(?!workspace\.package)/' Cargo.toml
    cargo update --workspace --quiet
    echo "$old -> $new: the next commit is $new.$(($(build) + 1)). Commit Cargo.toml and Cargo.lock."
    ;;
  *)
    echo "usage: scripts/version.sh [--build | --tag | --release | --set X.Y]" >&2
    exit 1
    ;;
esac
