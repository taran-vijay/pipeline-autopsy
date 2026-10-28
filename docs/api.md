# Local API contract (T1.7)

The versioned contract is [OpenAPI 3.1.1](https://spec.openapis.org/oas/v3.1.1.html).
The committed artifact is `schemas/openapi.json`. Rust wire types and route metadata
in `crates/api/src/` generate it, and a test rejects a stale artifact or unresolved
references. The schema uses JSON Schema 2020-12 with components derived by schemars.

## Current host

```sh
cargo run -p pipeline-autopsy-api
```

The host initializes/migrates SQLite before binding to `127.0.0.1:3000`. Override
the port with `PIPELINE_AUTOPSY_PORT`. It remains bound to loopback. Ctrl-C shuts
down serving gracefully and closes the connection pool. `PIPELINE_AUTOPSY_DATABASE`
controls the metadata path (default `runs/metadata.sqlite`).

Implemented endpoints:

| Method and path | Result |
| --- | --- |
| `GET /api/v1/health` | Version and migrated database version; 503 if metadata is unavailable |
| `GET /api/v1/openapi.json` | The generated specification |

Other paths currently return the same problem-details 404 response. Unsupported
methods on existing routes return 405 with an Allow header. No CORS policy is
configured; browser frontend API integration is later work.

## Planned operations

The specification marks each operation with `x-implementation-status`. Only health
and specification retrieval are implemented. Planned entries define the future
contract, not available functionality, and do not have registered handlers yet.

| Method and path | Designed behavior |
| --- | --- |
| `GET /api/v1/pipelines` | Page of registered revision definitions |
| `POST /api/v1/pipelines` | Register a Pipeline as an immutable revision; 201 or 409 |
| `GET /api/v1/pipelines/{pipeline_id}/versions/{pipeline_version}` | Exact revision definition |
| `POST /api/v1/pipelines/{pipeline_id}/runs` | Queue the requested revision; 202 with RunSummary and Location |
| `GET /api/v1/runs` | Page of run summaries, optionally filtered by pipeline/status |
| `GET /api/v1/runs/{run_id}` | Pipeline snapshot, run summary, and stage metadata |
| `GET /api/v1/runs/{run_id}/stages/{stage_id}` | A stage's execution metrics |
| `GET /api/v1/runs/{run_id}/findings` | Page of run-scoped findings |
| `GET /api/v1/runs/{run_id}/evidence/{evidence_id}` | Captured evidence, with full count and stored sample |

Page responses contain `items` and nullable `next_cursor`. `limit` defaults to 50
and ranges from 1 to 100. Cursors are opaque and scoped to the endpoint and filters.
Future repositories should use stable created-time/ID keyset ordering, with a
snapshot boundary in the cursor so new inserts cannot shift an existing traversal.
Do not accept a cursor from a different scope. Cursor encoding/validation will be
implemented alongside the first list handler.

Creation requests require an exact registered `pipeline_version`. They must not
silently substitute the newest revision. Execution is asynchronous; acceptance
returns a run resource to poll. A completed run may contain failed contracts.
No destructive operations or arbitrary SQL execution endpoints are specified.

## Wire representation

Core definitions and identifiers keep their snake_case fields. API-specific stage
and evidence DTOs serialize all unsigned 64-bit counts and durations as decimal
strings, preserving precision in JavaScript. `null` still means unmeasured, while
`"0"` means an observed zero. For example:

```json
{"input_rows":{"orders":"950"},"output_rows":"760","duration_ms":"12"}
```

`DecimalU64` rejects noncanonical or overflowing strings. Consumers can display
these values directly or use BigInt for arithmetic; do not coerce large totals
into JavaScript Number. Evidence total counts are independent of sample size.
The conversion helpers stringify integers outside JavaScript's safe range in
arbitrary evidence records and finding expected/observed values. Exact decimals
must already be strings at capture time. Future handlers must use these wire
adapters instead of returning raw core StageRun/Evidence objects.

Run detail excludes embedded findings/evidence; retrieve those through their
separate endpoints. That avoids returning all diagnostic samples as part of a run
history request. Evidence sample limits and access/retention controls are later
instrumentation/release work; the current DTO preserves a producer's captured sample.

## Errors

All HTTP errors use [RFC 9457 problem details](https://www.rfc-editor.org/rfc/rfc9457.html)
with `Content-Type: application/problem+json`. `ApiError` keeps the HTTP status and
body status aligned. `type` is `about:blank`, and `title` is the HTTP status phrase.
`code` is the stable application extension used for programmatic handling.

```json
{
  "type":"about:blank",
  "title":"Unprocessable Entity",
  "status":422,
  "detail":"The request violates one or more validation rules.",
  "code":"validation_failed",
  "errors":[{"pointer":"/stages/0/inputs","message":"Unknown stage"}]
}
```

| Status | Code | Meaning |
| --- | --- | --- |
| 400 | `invalid_request` | Malformed request/parameters/cursor |
| 404 | `not_found` | Missing resource, including wrong run scope |
| 405 | `method_not_allowed` | Existing route does not accept the method |
| 409 | `conflict` | Duplicate revision or conflicting state |
| 422 | `validation_failed` | Decoded input violates domain rules |
| 500 | `internal_error` | Unexpected failure with fixed public detail |
| 501 | `not_implemented` | Reserved for handlers that explicitly reject a planned operation |
| 503 | `storage_unavailable` | Metadata storage temporarily unavailable |

`instance` is omitted until an occurrence URI is allocated. Field errors use JSON
pointers and are omitted when empty. Future JSON/query extractors must map their
rejections through ApiError as well. SQL, filesystem paths, and internal error
messages must not be placed in public server/storage problem details; diagnostics
can log private context when tracing is implemented.

## Regenerate and verify

```sh
cargo run -p pipeline-autopsy-api --example export_openapi > schemas/openapi.json
cargo test -p pipeline-autopsy-api
```

Tests cover checked-in schema equality, local references, operation identifiers,
required path parameters, actual health/spec HTTP responses, 404/405 errors, 503
storage failure, field validation errors, and large-number conversion.
