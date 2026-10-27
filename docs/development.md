# Development

## Setup

```sh
git clone https://github.com/taran-vijay/pipeline-autopsy.git
cd pipeline-autopsy
npm ci
cargo build --workspace --locked
```

Use stable Rust (edition 2024 support required), Node.js 22.12 or newer, and npm.
The initial local checkout is at:

```text
/Users/taranvijay/Downloads/Projects/pipeline-autopsy
```

The root Cargo workspace contains `pipeline-autopsy-core` and
`pipeline-autopsy-cli`. Shared Rust dependencies belong in the root Cargo manifest.
The root npm workspace contains `@pipeline-autopsy/web`; install frontend
dependencies from the repository root. Commit both `Cargo.lock` and
`package-lock.json` to keep dependency resolution reproducible.

## Run

```sh
cargo run -p pipeline-autopsy-cli
npm run dev
```

The CLI currently prints the core version and scaffold status. Vite prints its
local development URL. There is no backend HTTP service yet.

## Verify

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run lint
npm run build
```

The frontend build includes TypeScript checking and writes to `web/dist/`.
Use `npm run preview` to serve that build locally. Rust output goes in `target/`.
No behavior tests exist yet; add meaningful tests with the pipeline features.

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

New files do not appear in `git diff` until staged. Stage and inspect the scaffold,
then commit and push:

```sh
git add .
git diff --cached --stat
git diff --cached
git commit -m "chore: initialize Rust and Vite monorepo"
git push origin main
```

Keep generated run data, evidence exports, credentials, and dependency directories
out of commits. Use a focused branch and pull request for subsequent tasks.
