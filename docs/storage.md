# SQLite metadata storage (T1.6, T3.1)

The storage crate embeds `migrations/0001_metadata.sql` with SQLx. Both
`open_database(path)` and `in_memory_database()` apply missing migrations before
returning a pool. SQLx tracks versions/checksums in `_sqlx_migrations`; migrations
must remain unchanged after use. Add a new numbered migration for schema changes.
`build.rs` makes Cargo rebuild embedded migrations when that directory changes.

File databases use WAL, a five-second busy timeout, and at most five pooled
connections. Foreign keys are enabled per connection. The in-memory test pool uses
one connection. Connection helpers close the pool if migration startup fails.

## Schema v1

| Table | Identity and purpose |
| --- | --- |
| `pipeline_revisions` | `(pipeline_id, version)`; immutable JSON definition snapshot |
| `runs` | `run_id`; revision reference, execution state, timestamps |
| `stage_runs` | `(run_id, stage_id)`; status, duration, output count/schema, error or skip reason |
| `stage_inputs` | `(run_id, stage_id, input_stage_id)`; per-input row count |
| `run_contracts` | `(run_id, contract_id)`; run-scoped rule snapshot and target stage |
| `check_results` | `(run_id, contract_id)`; passed/failed/error outcome, expected/observed JSON |
| `findings` | `(run_id, finding_id)`; explanation, severity, confidence, optional contract |
| `evidence` | `(run_id, evidence_id)`; investigator category, query, sampled data/schema JSON |
| `evidence_sources` | Evidence-to-source-stage references scoped to the same run |
| `finding_evidence` | Finding-to-evidence references scoped to the same run |
| `finding_impacts` | Finding-to-affected-stage references scoped to the same run |

Run references prevent deleting a pipeline revision while history uses it. An
update trigger rejects changes to revision records. Create a new revision instead.
Pipeline JSON must agree with the revision ID, version, and schema version columns.
A revision's JSON is the authoritative definition, including declared dependencies
and contracts; SQL/input file contents and fingerprints are later Track 3 work.

Stage/input/contract/finding/evidence foreign keys use composite run-scoped keys.
A contract-linked finding must target that contract's stage. Deleting a run
cascades through its owned records and association tables; other runs remain.
Indexes support pipeline/status run-history queries and stage investigations.

## Representation and constraints

- IDs and revisions are text. States, severity, conclusions, and evidence categories
  have explicit CHECK constraints.
- Counts and durations use canonical unsigned-decimal TEXT, supporting the entire
  Rust `u64` range even though SQLite INTEGER is signed. Constraints reject negative,
  noncanonical, or overflowing values. Parse them as `u64` when reading. SQL numeric
  ordering/arithmetic requires deliberate handling; do not cast large values to
  signed INTEGER or floating-point for diagnostic totals.
- Unmeasured output counts and schemas are SQL NULL; observed zero is `'0'` and an
  observed empty schema is JSON `[]`.
- JSON columns validate their basic shape. Evidence JSON retains full totals and
  bounded samples separately, matching the core `EvidenceData` representation.
- Timestamp strings use UTC RFC 3339. Application code must validate chronology,
  lifecycle transitions, required fields, and definition/reference semantics beyond
  the relational constraints. DDL checks are not a complete domain validator.
- Sample limits, evidence redaction/retention, SQL/input fingerprints, and full telemetry repository
  save/load methods are later tasks. This migration defines their storage boundary.

`create_pending_run` now initializes a run and its stage/contract snapshots in one
transaction. A conflicting definition for the same pipeline revision is rejected;
a repeated run of the same definition gets its own record. See
[run identities](run-identities.md). Future repositories will update related
telemetry/findings/evidence atomically. Deletion
and retention must be explicit actions; the foundation API exposes no delete route.

## Start and test

```sh
cargo run -p pipeline-autopsy-api
cargo test -p pipeline-autopsy-storage
```

The API creates `runs/metadata.sqlite` by default and migrates it on startup.
Override it with `PIPELINE_AUTOPSY_DATABASE=/path/to/metadata.sqlite`. The connection
helper expects parent directories to exist; the API host creates them.
SQLite database files and WAL/SHM sidecars are ignored by Git.

Tests use temporary file databases or isolated in-memory databases, covering repeat
migration startup, persisted records after reopening, checksums, foreign keys on
pooled connections, immutable revisions, unsigned counts, invalid states/JSON,
run isolation, and cascading cleanup.
