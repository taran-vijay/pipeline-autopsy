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

T1.1–T1.5 establish the Rust workspace, frontend tooling, backend crate boundaries,
and shared domain models. The CLI prints a startup message, and the frontend shows
an empty runs screen. Execution, diagnostic services, persistence, and API serving
are planned work.

See [backend architecture](docs/architecture.md) and
[shared domain models](docs/domain-models.md) for the current boundaries and types.

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
npm ci
npm run dev
```

Open the local URL printed by Vite. The CLI and frontend currently run independently.

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
