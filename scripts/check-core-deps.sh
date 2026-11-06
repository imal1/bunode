#!/usr/bin/env bash
# bunode-core must stay independent of the FFI layer and of Apple frameworks,
# so it can later be hosted by a standalone Supervisor daemon unchanged
# (ADR-0001, ADR-0002). Fails if any such crate shows up in its dependency
# tree, on any target.
set -euo pipefail

cd "$(dirname "$0")/.."

banned='^(uniffi|uniffi_[a-z_]+|objc|objc2|objc2-[a-z0-9-]+|block2|dispatch2|cocoa|cocoa-[a-z-]+|core-foundation|core-foundation-sys|core-graphics|core-graphics-types|security-framework|security-framework-sys)$'

deps=$(cargo tree --package bunode-core --target all --edges normal,build \
  --prefix none --format '{p}' | awk '{print $1}' | sort -u)

if offenders=$(grep -E "$banned" <<<"$deps"); then
  echo "error: bunode-core must not depend on UniFFI or Apple frameworks, but pulls in:" >&2
  sed 's/^/  /' <<<"$offenders" >&2
  echo "Keep FFI code in bunode-ffi and UI code in the app (ADR-0001, ADR-0002)." >&2
  exit 1
fi

echo "bunode-core dependencies OK"
