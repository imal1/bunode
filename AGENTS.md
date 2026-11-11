# Bunode

macOS native dev-server manager for Node.js & Bun. Requirements: `docs/requirements.md`.

## Agent skills

### Issue tracker

GitHub Issues on imal1/bunode, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five roles; each label string equals its role name. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`. See `docs/agents/domain.md`.

## Build and test

Run from the repo root (macOS 26+, Xcode 26+, rustup, XcodeGen; see `README.md`):

- `make`: Rust core → `build/BunodeFFI.xcframework` + Swift bindings + `build/runner/bunode-runner` → `macos/Bunode.xcodeproj` (XcodeGen, not committed) → `Bunode.app`
- `make test`: `cargo test --workspace` plus the Swift tests in `macos/BunodeTests`. The Rust integration tests (`crates/bunode-runner/tests/`) start real node, bun, npm, pnpm and python3 processes from `fixtures/`, so those tools must be on PATH.
- `make lint`: `cargo fmt --check`, `cargo clippy -D warnings`, and `scripts/check-core-deps.sh` (`bunode-core` must not depend on UniFFI or Apple frameworks)

On Linux only the Rust side builds and tests; CI (`.github/workflows/ci.yml`, macOS runner) runs all of it.

Never launch a development build while the user's own Bunode is running: at launch the Supervisor ends the leftover Runs recorded in `~/Library/Application Support/Bunode`, which includes theirs.

## Versions and releases

One version, in `Cargo.toml` (`[workspace.package] version`); `make project` puts it and the commit count into the app. `scripts/version.sh --set X.Y.Z` bumps it and dates the CHANGELOG.md section; pushing the tag `vX.Y.Z` runs `.github/workflows/release.yml`. User-visible changes get a line under **Unreleased** in `CHANGELOG.md`. See `CONTRIBUTING.md`.
