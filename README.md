# Pipeline Autopsy

Debug data pipelines with stage-level checks, evidence-backed root-cause analysis,
and an interactive lineage workbench.

A pipeline can finish successfully while silently losing records. Pipeline Autopsy
will show where an expectation first failed, explain the evidence, and identify
potentially affected downstream outputs. The canonical example follows 950 valid
orders through a customer join that leaves 760: an investigation should identify
the 190 unmatched orders while treating expected filtering and aggregation normally.

## Current status

Track 1 (T1.1–T1.9) establishes the project foundation:

- Eight Rust workspace crates and shared domain models.
- SQLite metadata schema and automatic, embedded migrations.
- An Axum API serving health and the generated OpenAPI specification.
- A React/TypeScript/Vite frontend with Tailwind, ESLint, and Vitest.
- GitHub Actions checks for Rust and frontend builds/tests.

T2.1–T2.7 add YAML parsing/validation, detailed graph diagnostics, deterministic
serial scheduling, and isolated DuckDB CSV ingestion. The CLI can validate,
plan, ingest, and run a pipeline. SQL stages execute in dependency order and
materialize tables for downstream use. Diagnostics, repository save/load methods,
and business API handlers remain planned. The frontend still shows
an empty runs screen. The 190-order investigation is currently a synthetic model
test fixture. It is not yet an executable demo.

Development currently runs locally in a browser alongside a Rust service. Tauri
packaging is a future desktop option. No hosted service or cloud account is needed.

## Prerequisites

- Stable Rust installed through [rustup](https://rustup.rs/), including a platform
  C/C++ compiler and linker. `rust-toolchain.toml` selects stable, rustfmt, and Clippy.
- Node.js **26.10.0**, matching `.node-version`, with npm. Compatible alternatives
  are listed in `package.json`, but CI uses the pinned version.
- Git. A separate SQLite installation and SQLx CLI are not required.

See [local setup](docs/development.md) for platform requirements and troubleshooting.

## Setup and run

```sh
git clone https://github.com/taran-vijay/pipeline-autopsy.git
cd pipeline-autopsy
npm ci
cargo build --workspace --locked
```

Start the API in one terminal:

```sh
cargo run -p pipeline-autopsy-api --locked
```

It migrates `runs/metadata.sqlite` automatically and serves:

- [Health](http://127.0.0.1:3000/api/v1/health)
- [OpenAPI specification](http://127.0.0.1:3000/api/v1/openapi.json)

Start the frontend in another terminal, from the same repository root:

```sh
npm run dev
```

Open the URL Vite prints (normally `http://localhost:5173`). The frontend is not yet
connected to the API. Ctrl-C stops either service. Validate and ingest the small sales fixture:

```sh
cargo run -p pipeline-autopsy-cli --locked -- validate examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- plan examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- ingest examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- run examples/sales/pipeline.yaml
```

## Verify

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo lint
cargo test-workspace
npm run check
```

CI runs these checks on pull requests and pushes to `main`. The first hosted run
starts after the workflow is pushed; see [CI details](docs/ci.md).

## Repository guide

```text
crates/core/        Shared domain models
crates/api/         Local API host and versioned contract
crates/storage/     SQLite connections, schema, and migrations
crates/engine/      YAML, scheduling, and materialized DuckDB execution
crates/tracker/     Instrumentation boundary
crates/contracts/   Contract evaluation boundary
crates/analyzer/    Investigation boundary
crates/cli/         Command line entry point
web/                React + TypeScript application
examples/sales/     Small YAML/CSV/SQL execution example
schemas/            Generated OpenAPI artifact
.github/workflows/  Continuous integration
docs/               Architecture and contributor documentation
```

| Guide | Contents |
| --- | --- |
| [Development](docs/development.md) | Setup, running services, checks, troubleshooting |
| [Architecture](docs/architecture.md) | Boundaries, dependency direction, runtime flow |
| [Domain models](docs/domain-models.md) | Shared records, serialization, semantics |
| [Pipeline format](docs/pipeline-spec.md) | YAML, paths, dependencies, CSV policy, CLI example |
| [Storage](docs/storage.md) | Database schema, migration rules, integrity |
| [API](docs/api.md) | Implemented/planned routes, wire types, errors |
| [CI](docs/ci.md) | Triggers, jobs, local equivalents, first-run expectations |
| [Contributing](CONTRIBUTING.md) | Change and review workflow |

## License

[MIT](LICENSE)
