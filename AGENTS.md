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

The version is `0.1.N`: the series from `Cargo.toml` (`[workspace.package] version`, patch always 0) and N the commit count, so every commit has its own and nothing is bumped by hand; `make project` puts it into the app. `scripts/version.sh --release` dates the CHANGELOG.md section, commits and tags `v0.1.N`; pushing that tag runs `.github/workflows/release.yml`. User-visible changes get a line under **Unreleased** in `CHANGELOG.md`. See `CONTRIBUTING.md`.
