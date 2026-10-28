-- Metadata schema v1. DuckDB holds execution data; SQLite holds metadata.
-- JSON payloads preserve domain shapes; relational keys isolate each run.
CREATE TABLE pipeline_revisions (
    pipeline_id TEXT NOT NULL CHECK (length(pipeline_id) > 0),
    version TEXT NOT NULL CHECK (length(version) > 0),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    name TEXT NOT NULL,
    definition_json TEXT NOT NULL CHECK (
        json_valid(definition_json) AND json_type(definition_json) = 'object'
        AND COALESCE(json_extract(definition_json, '$.id') = pipeline_id, 0)
        AND COALESCE(json_extract(definition_json, '$.version') = version, 0)
        AND COALESCE(json_extract(definition_json, '$.schema_version') = schema_version, 0)
    ),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (pipeline_id, version)
);

CREATE TRIGGER pipeline_revisions_immutable
BEFORE UPDATE ON pipeline_revisions
BEGIN
    SELECT RAISE(ABORT, 'pipeline revisions are immutable; create a new revision');
END;

CREATE TABLE runs (
    run_id TEXT PRIMARY KEY NOT NULL CHECK (length(run_id) > 0),
    pipeline_id TEXT NOT NULL,
    pipeline_version TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending','running','completed','failed','cancelled')),
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (pipeline_id, pipeline_version)
        REFERENCES pipeline_revisions(pipeline_id, version) ON DELETE RESTRICT
);
CREATE INDEX runs_by_pipeline ON runs(pipeline_id, created_at DESC, run_id);
CREATE INDEX runs_by_status ON runs(status, created_at DESC, run_id);

CREATE TABLE stage_runs (
    run_id TEXT NOT NULL,
    stage_id TEXT NOT NULL CHECK (length(stage_id) > 0),
    status TEXT NOT NULL CHECK (status IN ('pending','running','completed','failed','skipped','cancelled')),
    started_at TEXT,
    finished_at TEXT,
    duration_ms TEXT CHECK (duration_ms IS NULL OR (typeof(duration_ms) = 'text' AND length(duration_ms) BETWEEN 1 AND 20 AND duration_ms NOT GLOB '*[^0-9]*' AND (duration_ms = '0' OR substr(duration_ms, 1, 1) != '0') AND (length(duration_ms) < 20 OR duration_ms <= '18446744073709551615'))),
    output_rows TEXT CHECK (output_rows IS NULL OR (typeof(output_rows) = 'text' AND length(output_rows) BETWEEN 1 AND 20 AND output_rows NOT GLOB '*[^0-9]*' AND (output_rows = '0' OR substr(output_rows, 1, 1) != '0') AND (length(output_rows) < 20 OR output_rows <= '18446744073709551615'))),
    output_schema_json TEXT CHECK (output_schema_json IS NULL OR
        (json_valid(output_schema_json) AND json_type(output_schema_json) = 'array')),
    error_json TEXT CHECK (error_json IS NULL OR
        (json_valid(error_json) AND json_type(error_json) = 'object')),
    skip_reason TEXT,
    PRIMARY KEY (run_id, stage_id),
    FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
);

CREATE TABLE stage_inputs (
    run_id TEXT NOT NULL,
    stage_id TEXT NOT NULL,
    input_stage_id TEXT NOT NULL,
    row_count TEXT NOT NULL CHECK (typeof(row_count) = 'text' AND length(row_count) BETWEEN 1 AND 20 AND row_count NOT GLOB '*[^0-9]*' AND (row_count = '0' OR substr(row_count, 1, 1) != '0') AND (length(row_count) < 20 OR row_count <= '18446744073709551615')),
    PRIMARY KEY (run_id, stage_id, input_stage_id),
    FOREIGN KEY (run_id, stage_id) REFERENCES stage_runs(run_id, stage_id) ON DELETE CASCADE,
    FOREIGN KEY (run_id, input_stage_id) REFERENCES stage_runs(run_id, stage_id) ON DELETE CASCADE
);

-- Contract snapshots are run-scoped so finding/check references cannot cross runs.
CREATE TABLE run_contracts (
    run_id TEXT NOT NULL,
    contract_id TEXT NOT NULL CHECK (length(contract_id) > 0),
    stage_id TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('information','warning','error')),
    rule_json TEXT NOT NULL CHECK (json_valid(rule_json) AND json_type(rule_json) = 'object'),
    PRIMARY KEY (run_id, contract_id),
    UNIQUE (run_id, contract_id, stage_id),
    FOREIGN KEY (run_id, stage_id) REFERENCES stage_runs(run_id, stage_id) ON DELETE CASCADE
);

CREATE TABLE check_results (
    run_id TEXT NOT NULL,
    contract_id TEXT NOT NULL,
    stage_id TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('passed','failed','error')),
    expected_json TEXT NOT NULL CHECK (json_valid(expected_json)),
    observed_json TEXT NOT NULL CHECK (json_valid(observed_json)),
    explanation TEXT NOT NULL,
    PRIMARY KEY (run_id, contract_id),
    FOREIGN KEY (run_id, contract_id, stage_id)
        REFERENCES run_contracts(run_id, contract_id, stage_id) ON DELETE CASCADE
);

CREATE TABLE findings (
    run_id TEXT NOT NULL,
    finding_id TEXT NOT NULL CHECK (length(finding_id) > 0),
    stage_id TEXT NOT NULL,
    contract_id TEXT,
    severity TEXT NOT NULL CHECK (severity IN ('information','warning','error')),
    conclusion TEXT NOT NULL CHECK (conclusion IN ('confirmed_evidence','suspected_cause','inconclusive')),
    summary TEXT NOT NULL,
    explanation TEXT NOT NULL,
    expected_json TEXT NOT NULL CHECK (json_valid(expected_json)),
    observed_json TEXT NOT NULL CHECK (json_valid(observed_json)),
    PRIMARY KEY (run_id, finding_id),
    FOREIGN KEY (run_id, stage_id) REFERENCES stage_runs(run_id, stage_id) ON DELETE CASCADE,
    FOREIGN KEY (run_id, contract_id, stage_id)
        REFERENCES run_contracts(run_id, contract_id, stage_id) ON DELETE CASCADE
);
CREATE INDEX findings_by_stage ON findings(run_id, stage_id, severity);

CREATE TABLE evidence (
    run_id TEXT NOT NULL,
    evidence_id TEXT NOT NULL CHECK (length(evidence_id) > 0),
    stage_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('unmatched_keys','duplicate_keys','schema_drift')),
    query TEXT,
    data_json TEXT NOT NULL CHECK (json_valid(data_json) AND json_type(data_json) = 'object'
        AND COALESCE(json_extract(data_json, '$.kind') IN ('rows','schema_diff'), 0)),
    PRIMARY KEY (run_id, evidence_id),
    FOREIGN KEY (run_id, stage_id) REFERENCES stage_runs(run_id, stage_id) ON DELETE CASCADE
);
CREATE INDEX evidence_by_stage ON evidence(run_id, stage_id);

CREATE TABLE evidence_sources (
    run_id TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    source_stage_id TEXT NOT NULL,
    PRIMARY KEY (run_id, evidence_id, source_stage_id),
    FOREIGN KEY (run_id, evidence_id) REFERENCES evidence(run_id, evidence_id) ON DELETE CASCADE,
    FOREIGN KEY (run_id, source_stage_id) REFERENCES stage_runs(run_id, stage_id) ON DELETE CASCADE
);

CREATE TABLE finding_evidence (
    run_id TEXT NOT NULL,
    finding_id TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    PRIMARY KEY (run_id, finding_id, evidence_id),
    FOREIGN KEY (run_id, finding_id) REFERENCES findings(run_id, finding_id) ON DELETE CASCADE,
    FOREIGN KEY (run_id, evidence_id) REFERENCES evidence(run_id, evidence_id) ON DELETE CASCADE
);

CREATE TABLE finding_impacts (
    run_id TEXT NOT NULL,
    finding_id TEXT NOT NULL,
    stage_id TEXT NOT NULL,
    PRIMARY KEY (run_id, finding_id, stage_id),
    FOREIGN KEY (run_id, finding_id) REFERENCES findings(run_id, finding_id) ON DELETE CASCADE,
    FOREIGN KEY (run_id, stage_id) REFERENCES stage_runs(run_id, stage_id) ON DELETE CASCADE
);
