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

T1.1 initializes the monorepo: a Rust Cargo workspace and a Vite frontend using
React and TypeScript. The CLI prints a startup message, and the frontend shows an
empty runs screen. Pipeline execution, diagnostics, persistence, and the API are
planned work.

## Repository layout

```text
crates/core/   Shared Rust engine library
crates/cli/    Rust command line entry point
web/           Vite + React + TypeScript application
docs/          Development instructions
```

## Quick start

Install stable Rust with Cargo and Node.js 22.12 or newer with npm.
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
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run lint
npm run build
```

See [the development guide](docs/development.md) for setup and commit instructions.

## License

[MIT](LICENSE)
