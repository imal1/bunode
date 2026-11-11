# Contributing to Bunode

Thanks for helping. Bug fixes can go straight to a pull request; for a new feature or a change in behavior, open an [issue](https://github.com/imal1/bunode/issues) first so we can agree on the shape. [`docs/requirements.md`](docs/requirements.md) says what is in and out of scope.

## Setup

- macOS 26 Tahoe or later, on Apple Silicon or Intel
- Xcode 26 or later
- Rust, installed with [rustup](https://rustup.rs). The build adds the `aarch64-apple-darwin` and `x86_64-apple-darwin` targets if they are missing.
- [XcodeGen](https://github.com/yonaskolb/XcodeGen): `brew install xcodegen`
- For the integration tests: `node`, `bun`, `npm`, `pnpm` and `python3` on `PATH`

## Build and run

```bash
make        # build Bunode.app
make run    # build and open it
```

`make` builds the Rust core into `build/BunodeFFI.xcframework`, generates its Swift bindings, builds the `bunode-runner` helper, generates `macos/Bunode.xcodeproj` and builds a universal `Bunode.app` into `build/DerivedData/Build/Products/Release/`. Development builds are signed ad hoc.

| Command | What it does |
|---|---|
| `make test` | Runs the Rust tests and the Swift tests |
| `make lint` | `cargo fmt --check`, `cargo clippy -D warnings` and the `bunode-core` dependency rule |
| `make soak` | Runs the long stability soaks; see [`docs/stability.md`](docs/stability.md) |
| `make rust` | Rebuilds only the Rust side: the XCFramework, the Swift bindings and `bunode-runner` |
| `make project` | Runs `make rust`, then generates the Xcode project |
| `make dist` | Builds the app and zips it into `build/Bunode-<version>.zip` |
| `make version` | Prints the version and build number |
| `make clean` | Removes `build/`, the Xcode project and Cargo's `target/` |

`make app CONFIGURATION=Debug` builds a debug app instead.

> [!WARNING]
> Don't run a development build next to the Bunode you use every day. At launch the Supervisor ends the leftover Runs it finds in `~/Library/Application Support/Bunode`, which includes the other copy's. Quit the installed Bunode first.

### Working in Xcode

Run `make project`, then open `macos/Bunode.xcodeproj`. The project is generated from [`macos/project.yml`](macos/project.yml) and is not committed: edit the YAML and run `make project` again when you add targets or change build settings. New Swift files in `macos/Bunode/` are picked up the next time the project is generated. Generate it with `make`, not `xcodegen` alone, since make fills in the version.

Xcode does not build the Rust code. After changing anything under `crates/`, run `make rust` before building in Xcode.

## Code layout

```
crates/
  bunode-core/      Supervisor and process management. No UI, FFI or Apple framework dependencies.
  bunode-ffi/       UniFFI bindings that expose bunode-core to Swift.
  bunode-runner/    Per-Run helper that owns a Run's process group. Its tests start real processes.
  uniffi-bindgen/   Build tool: generates the Swift bindings.
macos/
  project.yml       XcodeGen spec for Bunode.app and its tests.
  Bunode/           SwiftUI and AppKit sources, and Localizable.xcstrings.
  BunodeTests/      Swift tests.
design/             App icon sources.
docs/               Requirements, design handoff, architecture decisions, images.
fixtures/           Small Node, Bun, npm and pnpm projects the tests run.
scripts/            Build, version and check scripts used by the Makefile and CI.
```

Read [`GLOSSARY.md`](GLOSSARY.md) before naming things, and the ADRs in [`docs/adr/`](docs/adr/) that touch the area you are changing. A change that reverses a decision gets a new ADR.

## Pull requests

- `make lint` and `make test` pass. CI runs both on a macOS runner.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `refactor:`…
- Every new user-facing string goes through `String(localized:)` or a SwiftUI `Text` and gets a Simplified Chinese entry in `Localizable.xcstrings`; a test fails otherwise.
- Add a line under **Unreleased** in [`CHANGELOG.md`](CHANGELOG.md) for anything a user would notice.

## Versions and releases

There is one version for the crates and the app: `[workspace.package] version` in `Cargo.toml`. `make project` writes it into the app's `CFBundleShortVersionString`, with the commit count as `CFBundleVersion`. The popover footer and About Bunode show both.

To release `X.Y.Z`:

```bash
scripts/version.sh --set X.Y.Z   # bumps Cargo.toml and Cargo.lock, dates the Unreleased notes
git commit -am "chore: release X.Y.Z"
git tag vX.Y.Z
git push origin main vX.Y.Z
```

The [Release workflow](.github/workflows/release.yml) checks that the tag matches `Cargo.toml`, builds the universal app, zips it and publishes a GitHub release with that version's CHANGELOG section as the notes. Releases are signed ad hoc and not notarized yet.
