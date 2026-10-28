# Development

## Setup

```sh
git clone https://github.com/taran-vijay/pipeline-autopsy.git
cd pipeline-autopsy
npm ci
cargo build --workspace --locked
```

Use stable Rust (edition 2024 support required), Node.js 22.22.2+, 24.15+, or 26+, and npm.
The Rust toolchain file selects stable and requests rustfmt and Clippy.
TypeScript 6 is used for compatibility with the ESLint TypeScript parser.
The initial local checkout is at:

```text
/Users/taranvijay/Downloads/Projects/pipeline-autopsy
```

The root Cargo workspace contains core, API, engine, tracker, contracts, analyzer,
storage, and CLI crates. See [architecture](architecture.md) for dependency direction
and [domain models](domain-models.md) for the shared data contract. Shared Rust
dependencies belong in the root Cargo manifest.
The root npm workspace contains `@pipeline-autopsy/web`; install frontend
dependencies from the repository root. Commit both `Cargo.lock` and
`package-lock.json` to keep dependency resolution reproducible.

## Run

```sh
cargo run -p pipeline-autopsy-cli
npm run dev
```

The CLI prints the core version and scaffold status. Vite prints its development
URL. In a separate terminal, start the foundation API:

```sh
cargo run -p pipeline-autopsy-api
```

It initializes SQLite and serves `/api/v1/health` and `/api/v1/openapi.json` on
`127.0.0.1:3000`. Business handlers are planned. See [storage](storage.md) and
[API contract](api.md) for configuration and schema export commands.

## Verify

```sh
cargo fmt --all -- --check
cargo lint
cargo test-workspace
npm run check
```

The frontend build includes TypeScript checking and writes to `web/dist/`.
Use `npm run preview` to serve that build locally. Rust output goes in `target/`.
The CLI startup integration test exercises the compiled executable. Core model
tests cover serialization, contract variants, evidence, and invalid input shapes.
Storage tests exercise actual migrations, database reopening, and constraints.
API tests cover the generated specification, wire types, and HTTP behavior.
The frontend startup test renders the empty runs view in jsdom with Testing Library. Add
behavior tests as pipeline features arrive.

### Rust tooling (T1.2)

- `rust-toolchain.toml` selects stable Rust with rustfmt and Clippy.
- `rustfmt.toml` sets edition 2024 formatting and four-space indentation.
- Workspace lints forbid unsafe code and enable the standard Clippy lint group.
  Each crate inherits them through `[lints] workspace = true`.
- `cargo lint` runs Clippy for the workspace and all targets, treating warnings
  as errors. `cargo test-workspace` runs workspace tests with the lockfile.

### Frontend tooling (T1.3)

Run these commands from the repository root:

| Command | Purpose |
| --- | --- |
| `npm run typecheck` | Check application, test, and Vite configuration types |
| `npm run lint` | Run ESLint with TypeScript, React Hooks, and refresh rules |
| `npm test` | Run Vitest once; failures or missing tests fail the command |
| `npm run test:watch` | Watch tests during development |
| `npm run build` | Type-check and produce a Vite build |
| `npm run check` | Run all frontend checks above, excluding watch mode |

Both TypeScript projects enable strict checking. ESLint uses a flat config in
`web/eslint.config.js`. Vitest shares the Vite configuration, uses jsdom, and loads
DOM matchers and component cleanup from `web/src/test/setup.ts`. Tests live beside
their components as `*.test.tsx`.

Tailwind uses its Vite plugin and the CSS import in `web/src/index.css`. Utility
classes can be used directly in TSX; the startup screen uses `min-h-screen`.
Existing component styles remain in place. Custom design-system work belongs to
Track 6.

## T1.6–T1.7 acceptance criteria

- SQLite opens with per-connection foreign keys and applies embedded migrations.
- Reopening preserves records; applying the same migration again is safe.
- Constraints enforce run isolation, valid states/counts, and revision integrity.
- OpenAPI is generated from Rust wire models and the committed artifact stays current.
- The local API starts after migration and serves health/specification responses.
- HTTP errors use the documented problem-details format.

## T1.4–T1.5 acceptance criteria

- All eight workspace packages build and inherit shared lint rules.
- Dependencies follow the architecture's inward direction without cycles.
- Pipeline, Stage, Run, Finding, Contract, and Evidence are serializable core types.
- JSON Schema export succeeds and the model tests pass.
- Boundary ownership and validation responsibilities are documented.

## T1.2–T1.3 acceptance criteria

- Formatting and Clippy checks pass across both Rust crates and test targets.
- `cargo test-workspace` executes the CLI integration test successfully.
- Strict TypeScript checks, ESLint, Vitest, and the production build all pass.
- The built CSS includes Tailwind utilities used by the application.

## T1.1 acceptance criteria

- The root Cargo workspace builds both the core library and CLI.
- The CLI runs and uses the shared core crate.
- The Vite frontend starts locally and builds with TypeScript checking.
- Rust formatting, Clippy, and frontend lint checks pass.
- Generated files and local secrets are ignored; dependency lockfiles are committed.

## Commit and push

After the checks pass, review the changes from the repository root:

```sh
git status
git diff
```

New files do not appear in `git diff` until staged. Stage and inspect the changes,
then commit and push:

```sh
git add .
git diff --cached --stat
git diff --cached
git commit -m "feat: add SQLite migrations and OpenAPI contract"
git push origin main
```

Keep generated run data, evidence exports, credentials, and dependency directories
out of commits. Use a focused branch and pull request for subsequent tasks.
