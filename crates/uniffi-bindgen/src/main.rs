//! Generates the Swift bindings for `bunode-ffi`.
//!
//! Living in the workspace pins the generator to the same UniFFI version as
//! the scaffolding, so the bindings and the library always agree.
//! See `scripts/build-xcframework.sh`.

fn main() {
    uniffi::uniffi_bindgen_main()
}
