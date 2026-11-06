#!/usr/bin/env bash
# Builds the Rust side of Bunode.app for Apple Silicon and Intel:
# bunode-ffi with its Swift bindings, packed into an XCFramework, and the
# bunode-runner helper that the app bundles.
#
# Outputs (all under build/, which is git-ignored):
#   build/BunodeFFI.xcframework        universal static library + C header/module map
#   build/ffi/swift/bunode_ffi.swift   Swift bindings, compiled into Bunode.app
#   build/runner/bunode-runner         universal helper, copied into Contents/MacOS
#
# Run it again after changing any Rust code.
set -euo pipefail

cd "$(dirname "$0")/.."

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
LIB=libbunode_ffi.a
OUT=build/ffi
XCFRAMEWORK=build/BunodeFFI.xcframework
RUNNER=build/runner/bunode-runner

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
  cargo build --package bunode-ffi --lib --package bunode-runner --bin bunode-runner \
    --release --target "$target"
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

mkdir -p "$(dirname "$RUNNER")"
lipo -create \
  target/aarch64-apple-darwin/release/bunode-runner \
  target/x86_64-apple-darwin/release/bunode-runner \
  -output "$RUNNER"
# Only the arm64 slice comes out of the linker signed.
codesign --force --sign - "$RUNNER"

echo "Built $XCFRAMEWORK, $OUT/swift/bunode_ffi.swift and $RUNNER"
