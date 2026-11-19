# Architecture boundary

Testora is open core. This repository holds the public developer tooling.

## Public (this repo, Apache-2.0)

- `apps/cli` — the `testora` CLI.
- `crates/contracts` — shared, versioned API contracts.
- Future: manifest schema, runner protocol spec, SDK.

## Private (future repositories)

- Authenticated Axum control plane, SQLx/PostgreSQL persistence and job orchestration.
- Hosted execution providers, billing, quotas, tenancy and production infrastructure.

The public CLI talks only to the authenticated Testora Cloud HTTPS API, never to a cloud provider directly.

## Local demo API

`apps/api` in this repo is a **localhost-only mock** with in-memory state. It exists so the CLI can be exercised end to end. It is not the production control plane and must not be deployed.

## Environment lifecycle

`requested → provisioning → initializing → ready → destroy_requested → destroying → destroyed`, with `failed` / `cleanup_failed` handled by reconciliation. Test results are tracked independently of environment state.
