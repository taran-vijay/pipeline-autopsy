# Shared domain models (T1.5)

The Rust types in `crates/core/src/` are the shared representation for execution,
storage, diagnostics, and future transport adapters. Every record derives Serde
serialization/deserialization and schemars JSON Schema generation. This is model
schema version 1, separate from package version and the user's pipeline revision.

| Model | Purpose |
| --- | --- |
| `Pipeline` | Schema version, stable ID, user revision, stage definitions, declared contracts |
| `Stage` | Stable ID, name, input stage IDs, CSV-source or SQL-file operation |
| `Run` | Run ID, pipeline definition snapshot, execution state, stage records, findings, evidence |
| `Contract` | Stable rule ID, target stage, severity, explicit rule definition |
| `Finding` | Run/stage/contract references, expected and observed values, explanation, confidence, evidence and affected-stage IDs |
| `Evidence` | Run/stage/source references, investigation query, evidence category, sampled rows or schema difference |

## Representation

- Record fields and enum variants use `snake_case`. Payload enums carry a `kind`
  tag (`csv_source`, `sql`, `join_coverage`, `rows`, and so on).
- IDs use distinct transparent string newtypes. A `StageId` cannot be accidentally
  passed as a `RunId` in Rust. They serialize as strings. ID allocation belongs to
  Track 3; constructors do not validate nonempty/unique identifiers.
- Unknown record fields and unsupported enum variants are rejected so typos do not
  silently change the meaning of definitions. Optional collections default to empty.
- Timestamps use UTC date-time values serialized as RFC 3339 strings. Durations are
  measured integer milliseconds. A missing measurement is `null`, distinct from an
  observed row count of zero or an observed empty schema.
- `StageRun.input_rows` records counts by input stage, avoiding ambiguous summed
  counts for joins. Output schemas store column name, engine type name, and
  optional nullability.
- Paths resolve relative to the pipeline definition directory. The Run embeds the
  pipeline definition so a later edit does not change recorded stage declarations.
  This does not snapshot file contents: SQL/input fingerprints and source capture
  belong to the instrumentation track.

## Execution and checks

Run states are `pending`, `running`, `completed`, `failed`, and `cancelled`.
Stage states also include `skipped` for stages blocked by upstream failure.
Execution errors have a code and message; a skipped stage can record a reason.

These are representations, not an implemented state machine. The engine and
tracker will enforce legal transitions, timestamps, and measurement consistency.
A `completed` run or stage means execution completed; it can still contain
contract failures. Contract evaluation outcomes will be implemented in Track 4.

The contract variants cover required columns, expected column types, non-null
columns, composite uniqueness, join coverage, and allowed row-loss ratios.
Join coverage explicitly references an input stage, reference stage, and paired
key columns, so an investigator can find records removed by an inner join.
Row loss compares a named input's count with the target output's count. The
ratio is clamped at zero for growth and empty inputs pass; allowed loss must be a
finite number from zero to one. Not-null expectations are separate contracts.

The model only describes these rules. The planner/evaluator must validate IDs,
references, graph shape, nonempty keys/columns, supported schema version, and
threshold ranges before execution. Deserialization alone does not do this.

## Evidence and uncertainty

Findings use `information`, `warning`, or `error` severity. Conclusions distinguish
`confirmed_evidence`, `suspected_cause`, and `inconclusive`. A confirmed anti-join
result can establish which orders disappeared without establishing why upstream
customer data is missing. Affected-stage IDs represent potentially impacted
outputs; they do not designate additional root causes.

Evidence categories are `unmatched_keys`, `duplicate_keys`, and `schema_drift`.
Row evidence stores a full `total_count` separately from sampled `rows`. The
investigator is responsible for bounded capture and storage for retention.
Deserialization does not impose a sample-size cap or cross-record consistency.
Schema evidence contains expected and observed column definitions. JSON values
support nulls; encode large integers and decimals as strings when clients cannot
represent their precision safely. API DTOs use decimal strings for unsigned counts/durations and normalize unsafe
record integers; see [the API contract](api.md).

## Example and schema export

`crates/core/tests/fixtures/sales_run.json` is synthetic model-test data, not an
executed pipeline. It records the 950 → 760 customer join, an observed total of
190 unmatched orders, two sample records, and downstream daily revenue impact.

Generate the full Run schema (including all nested domain definitions):

```sh
cargo run -p pipeline-autopsy-core --example export_schema
```

The generated schema is useful for inspecting the serialized contract. It is not
the OpenAPI specification or a semantic pipeline validator. Tests cover complete
investigation round trips, all contract families, missing versus zero metrics,
failed/skipped stages, schema evidence, uncertainty, malformed timestamps,
unknown fields, and invalid enum tags.
