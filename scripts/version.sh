#!/usr/bin/env bash
# Bunode's version, from one place: `[workspace.package] version` in Cargo.toml.
#
#   scripts/version.sh            prints the version, e.g. 0.1.0
#   scripts/version.sh --build    prints the build number: the commit count
#   scripts/version.sh --set 0.2.0
#                                 sets the version in Cargo.toml and Cargo.lock
#                                 and opens a CHANGELOG.md section for it
#
# The Makefile hands both numbers to XcodeGen, so Bunode.app's
# CFBundleShortVersionString and CFBundleVersion follow them.
set -euo pipefail

cd "$(dirname "$0")/.."

current() {
  sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml
}

case "${1:-}" in
  "")
    current
    ;;
  --build)
    git rev-list --count HEAD 2>/dev/null || echo 1
    ;;
  --set)
    new="${2:?usage: scripts/version.sh --set X.Y.Z}"
    if ! [[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      echo "error: $new is not X.Y.Z" >&2
      exit 1
    fi
    old=$(current)
    sed -i '' "/^\[workspace.package\]/,/^\[/s/^version = \"$old\"/version = \"$new\"/" Cargo.toml
    cargo update --workspace --quiet
    if ! grep -q "^## \[$new\]" CHANGELOG.md; then
      # The Unreleased notes become the new version's.
      sed -i '' "s/^## \[Unreleased\]$/## [Unreleased]\\
\\
## [$new] - $(date +%Y-%m-%d)/" CHANGELOG.md
      sed -i '' "s|^\[Unreleased\]: .*|[Unreleased]: https://github.com/imal1/bunode/compare/v$new...HEAD\\
[$new]: https://github.com/imal1/bunode/releases/tag/v$new|" CHANGELOG.md
    fi
    echo "$old -> $new. Review CHANGELOG.md, commit, then: git tag v$new && git push origin v$new"
    ;;
  *)
    echo "usage: scripts/version.sh [--build | --set X.Y.Z]" >&2
    exit 1
    ;;
esac
