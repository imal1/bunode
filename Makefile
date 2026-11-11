# Build entry points. `make` builds everything from a clean checkout.
# Needs macOS 26+, Xcode 26+, Rust (rustup) and XcodeGen; see README.md.

CONFIGURATION ?= Release
DERIVED_DATA  := build/DerivedData
PROJECT       := macos/Bunode.xcodeproj
APP           := $(DERIVED_DATA)/Build/Products/$(CONFIGURATION)/Bunode.app
XCODEBUILD    := xcodebuild -project $(PROJECT) -scheme Bunode -derivedDataPath $(DERIVED_DATA)
# One version for the crates and the app: Cargo.toml (scripts/version.sh).
VERSION       := $(shell scripts/version.sh)
BUILD         := $(shell scripts/version.sh --build)
ZIP           := build/Bunode-$(VERSION).zip

.PHONY: all app rust project run dist version test test-rust test-app soak lint clean

all: app

## rust: build build/BunodeFFI.xcframework, its Swift bindings and build/runner/bunode-runner
rust:
	scripts/build-rust.sh

## project: generate macos/Bunode.xcodeproj (needs the Rust build)
project: rust
	BUNODE_VERSION=$(VERSION) BUNODE_BUILD=$(BUILD) xcodegen generate --spec macos/project.yml

## app: build Bunode.app
app: project
	$(XCODEBUILD) -quiet -configuration $(CONFIGURATION) -destination 'generic/platform=macOS' build
	@echo "Built $(APP)"

## run: build and launch Bunode.app
run: app
	open "$(APP)"

## dist: zip Bunode.app for a GitHub release
dist: app
	ditto -c -k --keepParent "$(APP)" "$(ZIP)"
	@echo "Built $(ZIP)"

## version: print the version and build number
version:
	@echo "$(VERSION) ($(BUILD))"

## test: run the Rust and the Swift tests
test: test-rust test-app

test-rust:
	cargo test --workspace

# Not -quiet, so the log shows which tests ran. The tests expect English
# whatever the Mac's language.
test-app: project
	$(XCODEBUILD) -configuration Debug -destination 'platform=macOS,arch=$(shell uname -m)' -testLanguage en -testRegion US test

## soak: the long stability tests (docs/stability.md); BUNODE_SOAK_SECS sets their length
soak:
	cargo test --release -p bunode-runner --test stability -- --ignored --nocapture

## lint: formatting, clippy and the bunode-core dependency rule
lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	scripts/check-core-deps.sh

clean:
	rm -rf build $(PROJECT)
	cargo clean
