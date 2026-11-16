# Changelog

Every notable change to Bunode. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions are `0.1.N`, N being the number of commits up to the release (0.1.0 and 0.1.1 came before this). A release's notes on GitHub are its section below.

To release: `scripts/version.sh --release` turns **Unreleased** into a dated section, commits and tags it; pushing the tag makes the Release workflow build the app and publish it.

## [Unreleased]

### Added

- Open at login in Settings › General: macOS opens Bunode in the menu bar when you log in. Off until you turn it on.
- Start with Bunode on each Service: the Service gets a Run every time Bunode opens, whether you opened it or macOS did. A Service whose port is taken or whose command can't start says so on its own row, and the others still start.

### Changed

- Versions are now `0.1.N`, where N counts the commits, so every build has its own version and each release is higher than the last.

## [0.1.1] - 2026-11-16

### Added

- Every successful main-branch CI build now provides a downloadable universal macOS app.

### Fixed

- Discovered Processes refresh their command and directory after an executable change instead of keeping the launching shell's details.
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

[Unreleased]: https://github.com/imal1/bunode/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/imal1/bunode/releases/tag/v0.1.1
[0.1.0]: https://github.com/imal1/bunode/releases/tag/v0.1.0
