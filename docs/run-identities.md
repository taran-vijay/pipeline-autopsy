# Run and stage identities (T3.1)

`pipeline-autopsy-tracker::prepare_run(pool, pipeline)` allocates a random UUID v4
`RunId` and persists a pending execution record. Call it with a validated pipeline
(the CLI uses `load_pipeline`). Allocation succeeds only after SQLite commits.
Each invocation is a new run, including retries of the same pipeline revision.
IDs do not depend on timestamps, process counters, or stage ordering.

`StageId` names a stage in a pipeline definition and stays stable between runs.
`StageExecutionId { run_id, stage_id }` identifies one execution of that stage;
it matches SQLite's `(run_id, stage_id)` primary key. `StageRun::execution_id`
constructs this typed reference. CLI output displays it as `run-id/stage-id`.
This display is for inspection; consumers should retain the two typed fields.
A pending stage already has its identity before execution begins. With the current
one-attempt-per-stage model, there is no separate per-attempt identifier.

Storage registers the immutable pipeline revision, new run, pending stage records,
and contract snapshots in one transaction. Existing revisions are reused only if
their parsed JSON definitions match. Changing the definition requires a new
`version` in YAML. Duplicate run IDs or failed stage/contract inserts roll back
all new records, rather than replacing history or leaving a partial run.
SQLite primary keys enforce uniqueness, including concurrent allocations.
No migration or existing API wire-format change is needed.

## Try it

From the repository root, create the metadata directory once:

```sh
mkdir -p runs
cargo run -p pipeline-autopsy-cli -- prepare examples/sales/pipeline.yaml runs/metadata.sqlite
```

The command prints one pending run ID and four stage execution IDs, then exits.
Run it again to create a second run with different execution identities. Records
survive process exits and database reopening. The database and its sidecars are
ignored by Git. Parent directories for the explicit database path must exist.

To inspect these records with a SQLite viewer, open `runs/metadata.sqlite` and
look at `runs`, `stage_runs`, `pipeline_revisions`, and `run_contracts`.
An optional SQLite terminal query is:

```sql
SELECT run_id, stage_id, status FROM stage_runs ORDER BY run_id, stage_id;
```

Preparation does not execute CSV/SQL, set execution timestamps, or record metrics.
Records remain `pending`; unmeasured counts, schemas, and timings remain NULL.
`run` still performs an isolated, unrecorded DuckDB execution and does not consume
prepared runs yet. Connecting lifecycle/telemetry updates to these identities
belongs to the next instrumentation tasks. SQL/CSV file snapshots and fingerprints
are also later work; only the parsed definition is currently snapshotted.

## Verification

```sh
cargo test -p pipeline-autopsy-tracker -p pipeline-autopsy-cli
```

Tests cover UUID format, repeated/concurrent allocations, stage identity scope,
file-database reopening, immutable revision conflicts, duplicate-ID protection,
rollback on stage/contract failures, and separate CLI processes.
