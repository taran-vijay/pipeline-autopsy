# Pipeline Autopsy

Debug data pipelines with stage-level checks, evidence-backed root-cause analysis,
and an interactive lineage dashboard.

## The problem

A pipeline can finish successfully while silently losing records. Pipeline Autopsy
will show where an expectation first failed, explain the evidence, and identify
potentially affected downstream outputs. In the canonical sales example, 950 valid
orders become 760 after a customer join: the investigation should explain the 190
unmatched orders while treating deliberate filtering and aggregation as expected.

## Status

T1.1–T1.7 establish the workspace, tooling, backend boundaries, shared models,
SQLite migrations, and a versioned API contract. The local API serves health and
OpenAPI endpoints; the CLI and frontend still show their startup states.
Pipeline execution, diagnostic services, and business API handlers are planned work.

See [architecture](docs/architecture.md), [domain models](docs/domain-models.md),
[storage](docs/storage.md), and [API contract](docs/api.md) for the current foundations.

## Repository layout

```text
crates/core/        Shared domain models
crates/api/         Local API boundary
crates/engine/      Execution and planning boundary
crates/tracker/     Instrumentation boundary
crates/contracts/   Contract evaluation boundary
crates/analyzer/    Investigation boundary
crates/storage/     Persistence boundary
crates/cli/         Command line entry point
web/                Vite + React + TypeScript application
docs/               Architecture, model, and development guides
```

## Quick start

Install stable Rust with Cargo and Node.js 22.22.2+, 24.15+, or 26+ with npm.
From the repository root:

```sh
cargo run -p pipeline-autopsy-cli
# In a separate terminal, start the local API:
cargo run -p pipeline-autopsy-api
# In another terminal, start the frontend:
npm ci
npm run dev
```

Open the local URL printed by Vite. The frontend is not yet connected to the API.
The API health endpoint is `http://127.0.0.1:3000/api/v1/health`, and the OpenAPI
artifact is served at `/api/v1/openapi.json`. SQLite metadata is created under
`runs/metadata.sqlite` and is excluded from Git.

## Checks

```sh
cargo fmt --all -- --check
cargo lint
cargo test-workspace
npm run check
```

See [the development guide](docs/development.md) for setup and commit instructions.

## License

[MIT](LICENSE)
