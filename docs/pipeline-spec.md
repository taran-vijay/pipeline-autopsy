# Pipeline specification v1 (T2.1–T2.4)

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
rejects missing references, duplicate names/edges, self-loops, and cycles. Scheduling
and SQL execution are later tasks.

## Files and SQL

All operation paths are relative to the YAML file's directory, regardless of the
terminal's working directory. `load_pipeline` canonicalizes them and requires
regular files inside that directory tree. Absolute paths, remote sources, globs,
and symlinks that resolve outside that tree are unsupported. An existing filename
containing DuckDB glob characters (`*`, `?`, `[` or `]`) is also rejected. Paths
passed to DuckDB must be UTF-8. Spaces, apostrophes, and Unicode are supported.

`csv_source` declares one local CSV file. `sql` declares one local SQL file. This
step checks that SQL files exist, not their syntax or referenced table names. In
future SQL execution, stage IDs name the input/output relations. Quote reserved
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
These checks validate declarations, not data. Column existence, actual types,
contract evaluation, and SQL parsing/execution are later capabilities.

Library entry points in `pipeline-autopsy-engine`:

| Function/type | Behavior |
| --- | --- |
| `parse_pipeline(yaml)` | Parse and validate a definition without reading files |
| `validate_pipeline(&pipeline)` | Validate shared models and return their dependency graph |
| `load_pipeline(path)` | Parse, validate, build graph, and resolve all source/SQL files |
| `PipelineGraph` | Immutable graph plus stage lookup and sorted direct inputs/consumers |
| `SourceSession::ingest(&loaded)` | Materialize CSV sources in a new in-memory DuckDB database |

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
The join and daily aggregate SQL files are future execution inputs, and the
coverage contract is not evaluated yet. The CLI exits after inspection, discarding
the in-memory DuckDB session. Validation/import failures produce a nonzero exit
status and a diagnostic on stderr.

This small ingestion example is separate from the synthetic historical 190-order
model fixture. The full canonical execution/diagnosis demo arrives in later tracks.
