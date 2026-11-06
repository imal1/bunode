# Bunode

A native macOS menu bar app for the Node.js and Bun dev servers you run every day: start and stop them in one click, see which port each one is on, and catch the ones that crashed. It also finds Node and Bun processes that something else started.

Bunode is at an early stage. Today it is a skeleton: the menu bar item, its popover and an empty Settings window, wired to the Rust core. See [`docs/requirements.md`](docs/requirements.md) for where it is going.

## Requirements

- macOS 26 Tahoe or later, on Apple Silicon or Intel
- Xcode 26 or later
- Rust, installed with [rustup](https://rustup.rs). The build adds the `aarch64-apple-darwin` and `x86_64-apple-darwin` targets if they are missing.
- [XcodeGen](https://github.com/yonaskolb/XcodeGen): `brew install xcodegen`

## Build

From a clean checkout:

```bash
make
```

This builds the Rust core into `build/BunodeFFI.xcframework`, generates its Swift bindings, builds the `bunode-runner` helper, generates `macos/Bunode.xcodeproj` and builds a universal `Bunode.app` into `build/DerivedData/Build/Products/Release/`. Development builds are signed ad hoc, so no Apple developer account is needed.

To build and launch it:

```bash
make run
```

Bunode has no Dock icon. Look for the hexagon in the menu bar.

Other targets:

| Command | What it does |
|---|---|
| `make test` | Runs the Rust tests and the Swift tests |
| `make lint` | `cargo fmt --check`, `cargo clippy` and the `bunode-core` dependency rule |
| `make rust` | Rebuilds only the Rust side: the XCFramework, the Swift bindings and `bunode-runner` |
| `make project` | Runs `make rust`, then generates the Xcode project |
| `make clean` | Removes `build/`, the Xcode project and Cargo's `target/` |

`make app CONFIGURATION=Debug` builds a debug app instead.

### Working in Xcode

Run `make project`, then open `macos/Bunode.xcodeproj`. The project is generated from `macos/project.yml` and is not committed, so edit the YAML and run `make project` again when you add targets or change build settings. New Swift files in `macos/Bunode/` are picked up the next time the project is generated.

Xcode does not build the Rust code. After changing anything under `crates/`, run `make rust` before building in Xcode.

## Layout

```
crates/
  bunode-core/      Supervisor and process management. No UI, FFI or Apple framework dependencies.
  bunode-ffi/       UniFFI bindings that expose bunode-core to Swift.
  bunode-runner/    Per-Run helper that owns a Run's process group (empty for now).
  uniffi-bindgen/   Build tool: generates the Swift bindings.
macos/
  project.yml       XcodeGen spec for Bunode.app and its tests.
  Bunode/           SwiftUI and AppKit sources.
  BunodeTests/      Swift tests.
scripts/            Build and check scripts used by the Makefile and CI.
docs/               Requirements, design handoff and architecture decisions.
```

Why the app is SwiftUI and AppKit over a Rust core, and why the Supervisor runs inside the app, is written down in [`docs/adr/`](docs/adr/). [`GLOSSARY.md`](GLOSSARY.md) defines Project, Service, Run and the other terms used in the code.

## License

[MIT](LICENSE)
