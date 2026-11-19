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

The API binds to `127.0.0.1:8080` only. Restarting it clears records. `create` and `destroy` change **in-memory state only**. Authentication, access control, persistence, queues, resource provisioners, test runners, artifact storage and production HTTPS are **not implemented**.

## Next tasks

1. Add tests for API endpoints and request validation.
2. Define OpenAPI and add a versioned `testora.yaml` contract.
3. Implement production authentication, organization RBAC and SQLx repository layer **in the private control-plane repository**.
4. Integrate environment execution providers **in the private platform repository**.
5. Add proper CLI login (OAuth PKCE/device flow), token storage and HTTPS.
6. Secure release process and publish binaries only after validation.

## Open-core note

The commercial architecture keeps the hosted Axum API and cloud provisioners private, while publishing the CLI, SDK, manifest and examples. **This bundle includes a small illustrative local API so the CLI can be exercised**; do not treat it as the intended public production control-plane source.

## Security

See [SECURITY.md](SECURITY.md). Never run untrusted pull-request code with privileged cloud credentials.

## License

Apache License 2.0. See [LICENSE](LICENSE). Review contributor ownership, third-party licenses and Testora trademark before publishing.
