# Continuous integration (T1.8)

The `CI` workflow lives in `.github/workflows/ci.yml`. It starts on:

- Pushes to `main`.
- Pull requests.
- Manual dispatch through GitHub Actions after the workflow is on the default branch.

Rust and frontend jobs run independently on GitHub-hosted `ubuntu-latest` runners.
Both must pass. The Rust job allows 30 minutes and limits build concurrency to two
jobs for the bundled DuckDB C++ build. A newer run for the same branch/PR cancels an older in-progress
run. Jobs have explicit timeouts and a read-only repository token. Checkout does
not persist credentials, and no repository secrets are needed.

The checkout and Node setup actions are pinned to verified commit SHAs (v6 release
lines). Review and deliberately update these pins when changing action versions.
Rust uses the same stable channel as `rust-toolchain.toml`; Node uses the exact
version in `.node-version`. The Node setup step caches npm downloads keyed by the
root `package-lock.json`, not installed `node_modules`.

## Checks

| Job | Steps |
| --- | --- |
| Rust | Install stable/rustfmt/Clippy; format check; locked workspace build; Clippy; workspace tests |
| Frontend | Install pinned Node; `npm ci`; strict type check; ESLint; Vitest; production build |

The API tests check that the committed OpenAPI document equals the generated
contract and that local schema references resolve. Storage tests apply the real
embedded migration and verify reopening, checksums, constraints, and run isolation.
Engine tests exercise YAML/graph validation and real bundled DuckDB CSV ingestion.
No external database service is needed. CI does not deploy or publish anything.

Run the equivalent checks locally from the repository root:

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo lint
cargo test-workspace
npm ci
npm run typecheck
npm run lint
npm test
npm run build
```

The frontend's `npm run check` combines the four steps after dependency installation.

## First run and failures

The workflow becomes active when committed and pushed to GitHub. Open the repo's
[Actions tab](https://github.com/taran-vijay/pipeline-autopsy/actions) and select CI.
Local checks do not prove that the first hosted workflow has run. GitHub will show
its own runner logs and status after the push. Branch protection/required checks
are repository settings and are not changed by adding this file.

If a job fails, expand the failed step and reproduce that command locally. Fix the
source/configuration and push again. Do not update lockfiles or suppress a failing
check just to make CI pass. Keep the OpenAPI artifact synchronized with its source.
The current matrix checks Linux; macOS development is exercised locally. Cross-
platform release validation and Playwright workflows belong to later tracks.
