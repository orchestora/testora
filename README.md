# Testora

> Spin up. Test everything. Tear down.

**Early Rust starter for the Testora developer tooling and local API.** The intended GitHub home is [Orchestora](https://github.com/orchestora). This code is a **non-production local demonstration**; it does not create cloud resources or run tests.

## Structure

- `apps/api` — Axum local demo API with an in-memory environment record store.
- `apps/cli` — Clap-based `testora` CLI calling the local API over HTTP.
- `crates/contracts` — shared, versionable API models.
- `.github/workflows/ci.yml` — Rust formatting, linting, testing and build.
- `docs/ARCHITECTURE.md` — public/private boundary and environment lifecycle.

## Requirements

[rustup](https://rustup.rs/). The toolchain is pinned in `rust-toolchain.toml` and installed automatically on first `cargo` run.

## Run

Terminal 1:

```sh
cargo run -p testora-api
```

Terminal 2:

```sh
cargo run -p testora-cli -- ping
cargo run -p testora-cli -- env create --project demo --region us-east
cargo run -p testora-cli -- env list
cargo run -p testora-cli -- env status <uuid-from-create>
cargo run -p testora-cli -- env destroy <uuid-from-create>
```

The API listens on `127.0.0.1:8080` by default and refuses any non-loopback address. Restarting it clears records. `create` and `destroy` change **in-memory state only**. Authentication, access control, persistence, queues, resource provisioners, test runners, artifact storage and production HTTPS are **not implemented**.

Port 8080 already taken? Pick another loopback port for both sides:

```sh
TESTORA_API_BIND=127.0.0.1:18080 cargo run -p testora-api
TESTORA_API_URL=http://127.0.0.1:18080 cargo run -p testora-cli -- ping
```

The CLI only talks to plain-HTTP loopback URLs (`localhost`, `127.0.0.0/8`, `::1`). Project names and regions are validated by the same rules on the client and the server (`testora-contracts`).

## Test

```sh
cargo test --workspace
```

This covers the contract validation rules, every API endpoint (status codes, JSON error bodies, idempotent destroy, concurrent creates), and end-to-end runs of the real `testora` binary against an in-process API. Tests bind random ports only, so they don't clash with anything running locally.

## Next tasks

1. Define OpenAPI and a versioned `testora.yaml` manifest contract.
2. Point the CLI at the Testora platform API with a local test token.
3. Add production CLI login (OAuth device flow / PKCE), secure token storage and HTTPS.
4. Secure, signed release process; publish binaries only after validation.

Authentication, persistence and environment execution live in the private Testora platform, not in this repository.

## Open-core note

The commercial architecture keeps the hosted Axum API and cloud provisioners private, while publishing the CLI, SDK, manifest and examples. **This bundle includes a small illustrative local API so the CLI can be exercised**; do not treat it as the intended public production control-plane source.

## Security

See [SECURITY.md](SECURITY.md). Never run untrusted pull-request code with privileged cloud credentials.

## License

Apache License 2.0. See [LICENSE](LICENSE). Review contributor ownership, third-party licenses and Testora trademark before publishing.
