# Pipeline specification v1 (T2.1–T2.7)

A pipeline is one YAML document using the existing shared `Pipeline`/`Stage`/
`Contract` models. The engine parses it with serde-saphyr, validates supported
semantics, resolves local files, and constructs a petgraph directed acyclic graph.
The runnable definition is `examples/sales/pipeline.yaml`.

## Definition

```yaml
schema_version: 1
id: orders_demo
version: "1"
name: Orders demo
stages:
  - id: orders
    name: Orders
    operation:
      kind: csv_source
      path: data/orders.csv
  - id: valid_orders
    name: Valid orders
    inputs: [orders]
    operation:
      kind: sql
      path: sql/valid_orders.sql
contracts:
  - id: unique_orders
    stage_id: orders
    severity: error
    rule:
      kind: unique
      columns: [order_id]
```

| Field | Meaning |
| --- | --- |
| `schema_version` | Integer 1; the supported format version |
| `id` | Stable, nonempty pipeline identifier |
| `version` | Nonempty user revision string; quote numeric-looking versions |
| `name` | Nonempty display name |
| `stages` | At least one CSV source or SQL transformation |
| `contracts` | Optional explicit rules; defaults to an empty list |

Each stage has `id`, `name`, optional `inputs`, and a tagged `operation`. Source
stages are themselves graph nodes: a SQL transformation lists source or upstream
SQL stage IDs in `inputs`. Every data dependency must be declared explicitly;
SQL dependency inference is not implemented.

Stage IDs must match `[A-Za-z_][A-Za-z0-9_]*` and be unique ignoring ASCII case,
matching DuckDB table-name behavior. References use the exact declared spelling.
Input edges are unique. CSV sources cannot have inputs; SQL stages require at
least one input. Declarations may appear in any order, and disconnected source
branches are allowed. Graph edges point from input to consumer. Construction
rejects missing references, duplicate names/edges, self-loops, and cycles.
Graph validation reports missing references and duplicate names/edges together,
with a declared-spelling hint for case mismatches. After references resolve,
it reports each cyclic strongly connected component, including self-loops.
Groups contain exactly the mutually reachable stages, excluding downstream
consumers that are blocked by a cycle. Group members are sorted identifiers,
not an asserted path around the cycle. Diagnostics are deterministic across YAML
declaration order. Fix reference errors before requesting cycle analysis.

The serial scheduler and materialized CSV/SQL execution are implemented.

## Files and SQL

All operation paths are relative to the YAML file's directory, regardless of the
terminal's working directory. `load_pipeline` canonicalizes them and requires
regular files inside that directory tree. Absolute paths, remote sources, globs,
and symlinks that resolve outside that tree are unsupported. An existing filename
containing DuckDB glob characters (`*`, `?`, `[` or `]`) is also rejected. Paths
passed to DuckDB must be UTF-8. Spaces, apostrophes, and Unicode are supported.

`csv_source` declares one local CSV file. `sql` declares one local query file.
`validate` and `plan` check that files exist; `run` additionally parses the SQL and
binds/executes it against the scheduled inputs. Stage IDs name the input/output
relations. Quote reserved
SQL identifiers (for example `"select"`) in SQL source. The engine always quotes
relation names when registering CSV sources.

The format does not support arbitrary Python stages, file globs, remote URLs,
inline SQL, or custom CSV options in v1. Use the tagged operations above rather
than adding fields that are not supported.

## Validation

Unknown fields, duplicate YAML keys, multiple YAML documents, unsupported operation
tags, and YAML merge keys are rejected. Errors identify the invalid field/reference
or carry YAML parse diagnostics. Basic checks also reject empty identifiers/names,
surrounding whitespace, empty stage lists, unsupported schema versions, duplicate
contract IDs, and missing contract target stages.

Contract rules retain the shared [domain format](domain-models.md). Column/key
lists must be nonempty and cannot repeat names ignoring ASCII case. Type names
must be nonempty. Join-coverage keys pair input and reference columns; both stages
must be distinct declared inputs of the contract's target stage. A row-loss rule
must reference a declared input and use a finite loss ratio in `[0, 1]`.
These checks validate declarations, not data. Contract column/type validation and
contract evaluation remain later capabilities. The `run` command parses, binds,
and executes SQL against the materialized inputs.

Library entry points in `pipeline-autopsy-engine`:

| Function/type | Behavior |
| --- | --- |
| `parse_pipeline(yaml)` | Parse and validate a definition without reading files |
| `validate_pipeline(&pipeline)` | Validate shared models and return their dependency graph |
| `load_pipeline(path)` | Parse, validate, build graph, and resolve all source/SQL files |
| `PipelineGraph` | Immutable graph plus stage lookup and sorted direct inputs/consumers |
| `GraphValidationError.issues` | Structured duplicate, missing-reference, or cycle diagnostics |
| `plan_execution(&graph)` | Deterministic topological order without execution |
| `StageScheduler` | Completion-gated, serial stage dispatch |
| `ExecutionSession::run(&loaded)` | Execute CSV/SQL stages and retain materialized outputs |
| `ExecutionSession::outputs()` | Per-input/output row counts and ordered schemas in actual execution order |
| `SourceSession::ingest(&loaded)` | Materialize CSV sources in a new in-memory DuckDB database |

## Serial scheduling (T2.6)

`StageScheduler` uses Kahn-style readiness counts. It initially releases stages
with no inputs, then releases consumers only when every input has completed.
When multiple stages are ready, the stage with the smallest ASCII identifier runs
next. This rule is independent of YAML declaration order and applies after every
completion; it is not a layer-by-layer or parallel execution policy.

`next_stage()` dispatches at most one stage. A second dispatch is rejected while
a stage is active. `complete_stage(id)` accepts only that active stage; wrong or
repeated completions return an error without changing state. The scheduler becomes
finished only after the last dispatched stage completes.

`execute_with(callback)` dispatches trusted executor callbacks and records a
completion only after the callback succeeds. If a callback fails, it returns the
stage ID and original error, leaves that stage active, and invokes no later
callbacks. It does not automatically mark failures/skips or retry: these run-level
policies belong to T2.8–T2.9. The DuckDB stage executor now uses this callback API. `ingest` remains a separate
CSV-only preparation path.

To inspect the plan:

```sh
cargo run -p pipeline-autopsy-cli --locked -- plan examples/sales/pipeline.yaml
```

Expected order:

```text
1. customers
2. orders
3. join_customers
4. daily_revenue
```

Planning performs no CSV ingestion, SQL execution, contract evaluation, or
metadata writes. The CLI loads the definition and checks local file paths first;
the library's `plan_execution` operates solely on an already validated graph.

## SQL execution (T2.7)

```sh
cargo run -p pipeline-autopsy-cli --locked -- run examples/sales/pipeline.yaml
```

The engine reads and checks every SQL file before opening the execution session.
It then opens a fresh in-memory DuckDB connection and dispatches CSV/SQL stages
through the serial scheduler. Each SQL output is a real table created with
`CREATE TABLE <stage_id> AS <query>`. Downstream stages read those tables, not lazy
views; changing an upstream table after completion cannot recompute an existing
intermediate result. Empty outputs retain their column schema.

The entire execution is one DuckDB transaction. A successful library call returns
an `ExecutionSession` with the connection and all stage output observations. Any
read, parse, ingestion, or SQL execution error stops execution; the transaction and
in-memory session are discarded. Errors retain the failing stage ID and SQL path.
Persisted history, failed/skipped stage reports, and cancellation are subsequent
tasks. There is no contract evaluation, diagnosis, or SQLite run write yet.

The supported query subset is deliberately explicit:

- One SELECT/WITH query per file; a trailing semicolon and comments are supported.
  Filters, projections, joins, grouping/aggregates, subqueries, and UNION/EXCEPT/
  INTERSECT set operations are supported when understood by both sqlparser's DuckDB
  dialect and the bundled DuckDB engine. Not all DuckDB syntax is accepted.
- Nonrecursive CTEs may read declared inputs and earlier visible CTEs. CTE names
  must be unique in their query and cannot shadow any pipeline stage ID.
- Named base relations must be declared inputs, compared using DuckDB's ASCII
  case-insensitive names. Nested queries obey the same rule. Extra declared inputs
  may remain unused; the executor does not infer or rewrite dependency edges.
- Relations are unqualified stage names; schema/catalog-qualified references and
  string/file table references are unsupported. Quote SQL keywords as identifiers.
- Table functions/external readers, recursive CTEs, scripts, DDL/DML, SELECT INTO,
  query locks/settings/output formats, and pipe queries are unsupported.

The query AST is validated and rendered before DuckDB sees it, avoiding string
splitting that would damage semicolons inside literals or comments. This also
avoids DuckDB's Rust prepare API executing earlier statements in a script. These
subset checks are not a security sandbox for untrusted SQL; broader execution
controls belong to T7.7. No arbitrary-query HTTP route is exposed.

`ExecutionSession::connection()` is available for trusted Rust inspection.
`outputs()` reports observations at completion, not live counts after callers
mutate tables. The CLI prints counts/schemas and drops the session on exit; it
does not export or persist result rows. See [stage measurements](stage-measurements.md)
for per-input count and schema semantics (T3.2–T3.3).

## CSV ingestion

Ingestion uses one isolated DuckDB connection and a transaction per session. CSV
sources become materialized tables named after stage IDs. SQL stages are not
executed. A failed import returns an error with the stage and file path; the
transaction/session is discarded. The original CSV files are never modified.
Paths are bound as SQL parameters, separate from quoted identifiers.

The v1 CSV policy is explicit:

- Comma separator, UTF-8 input, and a required header row.
- DuckDB type inference scans all records (`sample_size=-1`). Small IDs typically
  become BIGINT; text stays VARCHAR. Supply string values deliberately if leading
  zeroes or lexical numeric formatting matter. Explicit column types are future work.
- Strict parsing; malformed records are not skipped or padded. Preamble skipping
  is fixed at zero so auto-detection cannot silently discard an irregular prefix.
  Quote and escape characters are both double quotes.
- Unquoted empty fields are NULL; quoted empty strings remain empty strings.
- Quoted commas and embedded newlines are supported. A header-only file is an
  empty relation. DuckDB's reader handles header normalization and inference.

`SourceInfo` reports the row count and inferred columns for inspection. This is
not persisted Track 3 telemetry. Editing a file after ingestion does not change
that session's materialized table; a new session reads the new file contents.
Files are read sequentially, so ingestion is not a filesystem-wide atomic snapshot.
Source fingerprints and change detection belong to the instrumentation track.

DuckDB is bundled with the Rust dependency. The first native build takes longer
and needs a C++ compiler, but no separately installed DuckDB library or server.
These synchronous functions must be moved to a blocking worker when integrated
with the async API; no business API handler uses them yet.

## Try the small example

From the repository root:

```sh
cargo run -p pipeline-autopsy-cli --locked -- validate examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- ingest examples/sales/pipeline.yaml
```

Validation reports four stages and three dependency edges. Ingestion reports:

```text
customers: 2 rows, 2 columns
orders: 3 rows, 4 columns
```

Customer 999 is intentionally absent. All three orders remain in the source table.
The `ingest` command leaves all three source orders present. To execute the join
and aggregation, use `run`, which reports:

```text
customers: 2 rows, 2 columns
orders: 3 rows, 4 columns
join_customers: 2 rows, 5 columns
daily_revenue: 1 rows, 2 columns
```

The one daily aggregate has revenue 49.45, verified by the execution integration
test. The coverage contract is not evaluated yet. The CLI exits after inspection,
discarding the in-memory DuckDB session. Validation/import failures produce a nonzero exit
status and a diagnostic on stderr.

This small execution example is separate from the synthetic historical 190-order
model fixture. The full canonical execution/diagnosis demo arrives in later tracks.
