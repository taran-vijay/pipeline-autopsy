# Small sales fixture

This fixture exercises YAML loading, graph construction, and scheduled CSV/SQL execution in DuckDB.
It contains three orders and two customers; customer 999 is deliberately missing.
All data is synthetic.

From the repository root:

```sh
cargo run -p pipeline-autopsy-cli --locked -- validate examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- plan examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- ingest examples/sales/pipeline.yaml
cargo run -p pipeline-autopsy-cli --locked -- run examples/sales/pipeline.yaml
```

Expected: four graph nodes, three edges, three orders, two customers. Planning
prints `customers → orders → join_customers → daily_revenue` without executing it. The `run` command materializes all stages: the join keeps two orders and the
aggregate produces one day with revenue 49.45. The CLI reports stage counts, while
integration tests verify the revenue value. No contract result or finding is
produced yet. The session is discarded when the command exits.
See [pipeline format](../../docs/pipeline-spec.md) for semantics and limitations.
