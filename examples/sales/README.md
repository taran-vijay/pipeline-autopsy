# Small sales fixture

This fixture exercises YAML loading, graph construction, and DuckDB CSV ingestion.
It contains three orders and two customers; customer 999 is deliberately missing.
All data is synthetic.

From the repository root:

```sh
cargo run -p pipeline-autopsy-cli --locked -- validate examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- ingest examples/sales/pipeline.yaml
```

Expected: four graph nodes, three edges, three orders, two customers. SQL stage
execution is not implemented yet, so no join/aggregate result or finding is produced.
The SQL files describe a future join that retains two orders and one day's revenue
of 49.45; those results are not produced by the current ingestion command.
See [pipeline format](../../docs/pipeline-spec.md) for semantics and limitations.
