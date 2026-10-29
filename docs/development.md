# Development

## Prerequisites

Install Git, stable Rust through [rustup](https://rustup.rs/), and Node.js 26.10.0
with npm. `.node-version` records the Node version used in CI. Use your preferred
Node version manager, or install that version directly. Verify `node --version`
before installing dependencies. Other supported runtime ranges are recorded in
`package.json`; dependency compatibility is checked by npm.

The workspace uses Rust edition 2024. `rust-toolchain.toml` selects stable and
requests rustfmt and Clippy. Because stable advances, build output can change after
a toolchain update even while `Cargo.lock` stays fixed. CI prints the installed
toolchain during setup and checks the same channel.

A native C/C++ build toolchain is needed for Rust linking and bundled SQLite/DuckDB:

| Platform | Requirement |
| --- | --- |
| macOS | Xcode Command Line Tools (`xcode-select --install`) |
| Ubuntu/Debian | C/C++ compilers, linker, and make (`build-essential`) |
| Windows | Rust MSVC toolchain and Visual Studio C++ Build Tools with Windows SDK |

These are platform prerequisites, not extra project services. SQLite is bundled by
SQLx; no database server, SQLite command line program, or SQLx CLI is required.
The first DuckDB C++ build may take several minutes; later builds reuse its artifacts.
The application is developed on macOS; CI currently verifies Linux. Windows and
additional platforms are not yet part of the test matrix.

## Clean setup

```sh
git clone https://github.com/taran-vijay/pipeline-autopsy.git
cd pipeline-autopsy
npm ci
cargo build --workspace --locked
```

Run all commands below from the repository root. Both dependency lockfiles are
committed. `npm ci` installs the frontend from the root npm workspace and fails if
package manifests and lockfile disagree. Rust builds use `--locked` to reject
unexpected lockfile changes. No secrets or environment file are needed.

The eight Cargo packages share dependencies and lint rules from `Cargo.toml`.
The npm workspace contains `@pipeline-autopsy/web`. Add frontend dependencies from
the root, for example `npm install <package> --workspace @pipeline-autopsy/web`,
and review the resulting manifest and lockfile changes.

## Run locally

In the API terminal:

```sh
cargo run -p pipeline-autopsy-api --locked
```

Startup creates the metadata directory, opens SQLite, validates/applies embedded
migrations, and binds to `127.0.0.1:3000`. It prints its address and database path.
A migration or connection failure stops startup. The implemented endpoints are:

```text
GET http://127.0.0.1:3000/api/v1/health
GET http://127.0.0.1:3000/api/v1/openapi.json
```

A healthy response includes `status: "ok"`, the application version, and
`database_schema_version: 1`. You can open the URLs in your browser, or verify from
a terminal with:

```sh
curl --fail http://127.0.0.1:3000/api/v1/health
```

In another terminal, from the same root:

```sh
npm run dev
```

Open the URL printed by Vite; its default port is 5173. The empty pipeline-runs
screen is expected. It does not yet request API data. The API and UI are separate
processes; use Ctrl-C in each terminal to stop them.

The command line scaffold is independent:

```sh
cargo run -p pipeline-autopsy-cli --locked
```

With no arguments it prints the core version and command guidance. Validate and
ingest the small example with:

```sh
cargo run -p pipeline-autopsy-cli --locked -- validate examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- plan examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- ingest examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- run examples/sales/pipeline.yaml
```

The plan command prints `customers → orders → join_customers → daily_revenue`
without executing those stages. The library scheduler dispatches executor callbacks
and waits for explicit completion before releasing dependents.
The ingest command reports three orders and two customers without executing SQL.
The run command executes the join (two orders) and aggregation (one revenue row).
Neither command persists run history or evaluates contracts. See [the pipeline format](pipeline-spec.md).

## Configuration and generated files

| Setting/path | Default and behavior |
| --- | --- |
| `PIPELINE_AUTOPSY_DATABASE` | `runs/metadata.sqlite`, relative to the working directory |
| `PIPELINE_AUTOPSY_PORT` | `3000`; listener stays on IPv4 loopback |
| `runs/` | Ignored local metadata; do not delete if you need the run history |
| `target/` | Ignored Rust build output |
| `node_modules/`, `web/node_modules/` | Ignored installed dependencies and tool caches |
| `web/dist/` | Ignored production frontend build |

Environment overrides must be set in the terminal launching the API. For example,
in macOS/Linux shells:

```sh
PIPELINE_AUTOPSY_PORT=3001 cargo run -p pipeline-autopsy-api --locked
```

In PowerShell:

```powershell
$env:PIPELINE_AUTOPSY_PORT = "3001"
cargo run -p pipeline-autopsy-api --locked
```

The API does not load `.env` files automatically. A database path with spaces is
supported when the environment variable is quoted. Migrations are embedded and
run on startup; do not edit an applied migration to resolve a checksum error.

## Verify changes

The local equivalents of the CI jobs are:

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo lint
cargo test-workspace
npm run check
```

`cargo lint` runs workspace Clippy for all targets with warnings treated as errors.
`cargo test-workspace` runs tests with the lockfile. Those tests cover the CLI,
shared models, YAML/graph validation, deterministic scheduling, real DuckDB CSV ingestion, SQLite
migrations/constraints, HTTP responses, and the
checked-in OpenAPI artifact. `npm run check` type-checks, lints, runs Vitest once,
and produces a frontend build. See [CI](ci.md) for individual steps and triggers.

For frontend watch mode, use `npm run test:watch`. `npm run preview` serves the
production build after `npm run build`; preview is not the API server.

TypeScript uses strict checks; TypeScript 6 remains selected for ESLint parser
compatibility. ESLint's flat config is `web/eslint.config.js`. Vitest uses jsdom,
DOM matchers, and automatic component cleanup from `web/src/test/setup.ts`.
Tests live beside components as `*.test.tsx`. Tailwind is connected through its
Vite plugin and the CSS import in `web/src/index.css`.

## Model, schema, and migration changes

Regenerate the API artifact after changing wire types or route definitions:

```sh
cargo run -p pipeline-autopsy-api --example export_openapi --locked > schemas/openapi.json
cargo test -p pipeline-autopsy-api --locked
```

The core domain schema can be printed independently:

```sh
cargo run -p pipeline-autopsy-core --example export_schema --locked
```

Add a new numbered SQL file under `crates/storage/migrations/` for a database
change. Never rewrite a migration that has been applied. Include a migration test
and update [storage documentation](storage.md). Schema artifact changes should be
reviewed with the Rust model changes that generated them.

## Troubleshooting

| Symptom | Next step |
| --- | --- |
| Node engine or native frontend dependency error | Confirm Node matches `.node-version`, then run `npm ci` from the root |
| Missing C compiler/linker | Install the native build tools listed above |
| API address already in use | Stop the existing API or set `PIPELINE_AUTOPSY_PORT` |
| API 404 for pipelines/runs | Expected: business handlers are planned; health and OpenAPI are the implemented routes |
| Browser cannot retrieve API data | The current frontend has no API integration; no CORS policy is configured yet |
| SQLite path/permission error | Check the configured directory and access rights; the host creates parent directories |
| Migration checksum mismatch | Restore the original migration and add a new version; preserve the database |
| OpenAPI equality test fails | Regenerate `schemas/openapi.json`, inspect its changes, then rerun the API tests |
| Formatting failure | Run `cargo fmt --all`, inspect the diff, and repeat the check |

## Track 1 completion

| Tasks | Foundation delivered |
| --- | --- |
| T1.1 | Cargo/npm workspaces and runnable CLI/frontend scaffolds |
| T1.2–T1.3 | Rust and frontend linting, formatting, builds, and tests |
| T1.4–T1.5 | Eight crate boundaries, shared serializable domain models, JSON Schema export |
| T1.6–T1.7 | SQLite migrations and minimal local API with generated contract/errors |
| T1.8–T1.9 | GitHub Actions workflow, architecture overview, and clean setup documentation |

This completes the foundation scope. T2.1–T2.7 implement parsing, source ingestion,
detailed graph validation, a serial scheduler, and materialized SQL execution.
The next task adds failed/skipped/completed stage reporting. Read
[architecture](architecture.md) before changing ownership or crate dependencies,
and [contributing](../CONTRIBUTING.md) before sending changes.
