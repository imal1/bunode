#!/usr/bin/env bash
# Builds bunode-ffi for Apple Silicon and Intel, generates its Swift bindings
# and packs both into build/BunodeFFI.xcframework.
#
# Outputs (all under build/, which is git-ignored):
#   build/BunodeFFI.xcframework   universal static library + C header/module map
#   build/ffi/swift/bunode_ffi.swift   Swift bindings, compiled into Bunode.app
#
# Run it again after changing any Rust code that the app links.
set -euo pipefail

cd "$(dirname "$0")/.."

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
LIB=libbunode_ffi.a
OUT=build/ffi
XCFRAMEWORK=build/BunodeFFI.xcframework

if [[ "$(uname -s)" != Darwin ]]; then
  echo "error: the XCFramework can only be built on macOS" >&2
  exit 1
fi

# Install the Rust targets on first use, so a clean checkout builds in one go.
if command -v rustup >/dev/null; then
  installed=$(rustup target list --installed)
  for target in "${TARGETS[@]}"; do
    grep -qx "$target" <<<"$installed" || rustup target add "$target"
  done
fi

for target in "${TARGETS[@]}"; do
  cargo build --package bunode-ffi --lib --release --target "$target"
done

rm -rf "$OUT" "$XCFRAMEWORK"
mkdir -p "$OUT/lib" "$OUT/include" "$OUT/swift" "$OUT/generated"

lipo -create \
  "target/aarch64-apple-darwin/release/$LIB" \
  "target/x86_64-apple-darwin/release/$LIB" \
  -output "$OUT/lib/$LIB"

# UniFFI reads the interface from the library's metadata, so the bindings
# always match the code that gets linked.
cargo run --quiet --package uniffi-bindgen -- generate \
  "target/aarch64-apple-darwin/release/$LIB" \
  --language swift \
  --no-format \
  --out-dir "$OUT/generated"

mv "$OUT/generated/bunode_ffi.swift" "$OUT/swift/"
mv "$OUT/generated/bunode_ffiFFI.h" "$OUT/include/"
# Xcode looks for a file named module.modulemap next to the headers.
mv "$OUT/generated/bunode_ffiFFI.modulemap" "$OUT/include/module.modulemap"
rmdir "$OUT/generated"

xcodebuild -create-xcframework \
  -library "$OUT/lib/$LIB" \
  -headers "$OUT/include" \
  -output "$XCFRAMEWORK"

echo "Built $XCFRAMEWORK and $OUT/swift/bunode_ffi.swift"
