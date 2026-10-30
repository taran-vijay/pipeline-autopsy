# Stage counts and schemas (T3.2–T3.3)

Successful `ExecutionSession::run` calls expose a `StageOutput` for each CSV and
SQL stage through `outputs()`, in actual execution order. These are immutable
observations of the completed materialized tables, not live queries.

| Field | Meaning |
| --- | --- |
| `stage_id` | Stage identifier in the loaded pipeline |
| `input_rows` | One `u64` count per declared upstream stage, keyed by its ID |
| `row_count` | Observed output row count, including zero |
| `columns` | Ordered column names, DuckDB type strings, and catalog nullability |

The engine measures each output using `COUNT(*)` after materialization. Before
executing a consumer, it records the counts of its completed inputs. Inputs are
kept separate: a join's two inputs are not summed into a misleading single count.
A CSV source has an empty input map because its file is not an upstream stage.
Self-joins count a declared input once. Counts describe upstream table cardinality,
not query scan counts, filtered rows, distinct business keys, or join matches.
Every declared input is recorded, even if a query does not reference it.

Schemas come from DuckDB `DESCRIBE` on the materialized table. They preserve
column order, output aliases, exact type strings (including decimal precision and
nested types), and empty-result schemas. CSV types follow the engine's full-file
inference settings; SQL output types reflect the resulting table. Nullability is
catalog metadata, not a measurement of NULL frequency or a contract result.

## Inspect an execution

From the repository root:

```sh
cargo run -p pipeline-autopsy-cli -- run examples/sales/pipeline.yaml
```

The join reports inputs `customers: 2` and `orders: 3`, with 2 output rows.
`daily_revenue` reports `join_customers: 2` as its input and 1 output row.
The CLI also prints every column's name, type, and catalog nullability.
An empty output reports zero rows with its schema; it is not an unmeasured result.

Measurements are snapshots: editing source files or mutating the session through
the trusted `connection()` accessor does not retroactively change the observations.
A new execution captures new observations in its own isolated session.

T3.1's `prepare` command still creates only pending SQLite records. These new
measurements are available in memory and in CLI output; they are not yet written
to those records. SQLite telemetry repositories arrive in T3.5. Timing/failure
instrumentation is T3.4. The existing executor returns an error without a session
on failure, so partial or failed-run telemetry is not exposed yet. There is no
new API route or contract evaluation in these tasks.

Tests cover separate join inputs, row multiplication, filtering to zero followed
by aggregation, header-only sources, ordered aliases and rich types, snapshot
stability, and repeated execution with changed data/schema.
