# Changelog

Every notable change to Bunode. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/). A release's notes on GitHub are its section below.

To release: `scripts/version.sh --set X.Y.Z` turns **Unreleased** into a dated section, then commit and push the tag `vX.Y.Z`; the Release workflow builds the app and publishes it.

## [Unreleased]

### Fixed

- A Service whose command starts its server in a new process group, like `pnpm dev` with pnpm 12, now shows the server's port, and Stop, Force Stop, a crash of Bunode or a killed runner end the server too ([ADR-0010](docs/adr/0010-a-run-is-its-group-and-its-descendants.md)).

## [0.1.0] - 2026-11-11

The first release.

### Added

- Menu bar popover with every Service, grouped flat or by Project, each with its status, port and one-click Start, Stop and Restart.
- Projects registered in Settings, with aliases. Services from `package.json` scripts, monorepo packages included, or any command.
- Package manager detection for npm, pnpm, Yarn and Bun, with a Picker when a Project has several lockfiles.
- Runs that never outlive Bunode: each one has its own process group behind `bunode-runner`, so Stop, Quit, a crash or a force quit end every process it started.
- Crashed, Failed and Not responding states, and short-lived Services told apart from long-running ones.
- Live logs with colors, search and export, a log file per Run, opened in Console.
- The ports each Run listens on, with a link to open them, and a check before Start that the port is free.
- Other processes: Node.js and Bun processes started outside Bunode, with where they came from, and Agent Servers that Claude starts in a Project's root.
- Services run with the login shell's environment.
- Keyboard and VoiceOver support in the popover.
- English and Simplified Chinese, switched in Settings › General.
- App icon in light and dark.
- The app menu bar, with About Bunode, while the Settings window is open.

[Unreleased]: https://github.com/imal1/bunode/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/imal1/bunode/releases/tag/v0.1.0
