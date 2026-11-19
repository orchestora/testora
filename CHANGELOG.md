# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cargo workspace: `testora` CLI, localhost-only demo API, shared contracts crate.
- Environment lifecycle states in contracts.
- Rust CI: fmt, clippy, test, release build with pinned actions.
- Shared request validation in `testora-contracts` (project name and region rules), used by both API and CLI.
- Tests for every demo API endpoint, validation and concurrency, plus end-to-end tests of the CLI binary.
- `TESTORA_API_BIND` to choose the demo API's loopback port.

### Changed

- Demo API returns JSON error bodies for every failure (bad JSON, wrong content type, malformed IDs, unknown routes).
- CLI shows the API's error message, fails fast when the API is unreachable, and validates arguments before sending requests.

### Fixed

- CLI accepted URLs like `http://127.0.0.1:8080@evil.example`, which pass a "starts with 127.0.0.1" check but actually target a remote host. The URL is now parsed and must be plain HTTP to a loopback host without credentials.
