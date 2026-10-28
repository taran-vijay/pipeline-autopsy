//! API response types. Counts use decimal strings to preserve u64 precision in JS.
use std::{borrow::Cow, collections::BTreeMap};

use chrono::{DateTime, Utc};
use pipeline_autopsy_core::*;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecimalU64(pub u64);

impl Serialize for DecimalU64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DecimalU64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let value: u64 = text.parse().map_err(serde::de::Error::custom)?;
        if text != value.to_string() {
            return Err(serde::de::Error::custom(
                "expected a canonical unsigned decimal string",
            ));
        }
        Ok(Self(value))
    }
}

impl JsonSchema for DecimalU64 {
    fn schema_name() -> Cow<'static, str> {
        "DecimalU64".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "string", "pattern": "^(0|[1-9][0-9]{0,19})$",
            "description": "Unsigned decimal in [0, 18446744073709551615]; returned as a string for lossless JavaScript handling."
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub status: HealthStatus,
    pub version: String,
    pub database_schema_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Opaque continuation token. Null marks the end of the result set.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub id: RunId,
    pub pipeline_id: PipelineId,
    pub pipeline_version: String,
    pub status: RunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRunRequest {
    /// Run the exact registered revision; never silently substitute a newer one.
    #[schemars(length(min = 1))]
    pub pipeline_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageResponse {
    pub stage_id: StageId,
    pub status: StageStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<DecimalU64>,
    pub input_rows: BTreeMap<StageId, DecimalU64>,
    pub output_rows: Option<DecimalU64>,
    pub output_schema: Option<Vec<ColumnSchema>>,
    pub error: Option<ExecutionError>,
    pub skip_reason: Option<String>,
}

impl From<StageRun> for StageResponse {
    fn from(stage: StageRun) -> Self {
        Self {
            stage_id: stage.stage_id,
            status: stage.status,
            started_at: stage.started_at,
            finished_at: stage.finished_at,
            duration_ms: stage.duration_ms.map(DecimalU64),
            input_rows: stage
                .input_rows
                .into_iter()
                .map(|(id, count)| (id, DecimalU64(count)))
                .collect(),
            output_rows: stage.output_rows.map(DecimalU64),
            output_schema: stage.output_schema,
            error: stage.error,
            skip_reason: stage.skip_reason,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunDetail {
    pub summary: RunSummary,
    pub pipeline: Pipeline,
    pub stages: Vec<StageResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceResponse {
    pub id: EvidenceId,
    pub run_id: RunId,
    pub stage_id: StageId,
    pub source_stage_ids: Vec<StageId>,
    pub query: Option<String>,
    pub kind: EvidenceKind,
    pub data: EvidenceResponseData,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceResponseData {
    Rows {
        total_count: DecimalU64,
        rows: Vec<BTreeMap<String, Value>>,
    },
    SchemaDiff {
        expected: Vec<ColumnSchema>,
        observed: Vec<ColumnSchema>,
    },
}

/// Normalize arbitrary JSON record values before exposing them to JS clients.
/// Exact decimal values must already be supplied by producers as strings.
pub fn json_safe(value: Value) -> Value {
    const MAX_SAFE: u64 = 9_007_199_254_740_991;
    match value {
        Value::Number(ref n)
            if n.as_u64().is_some_and(|v| v > MAX_SAFE)
                || n.as_i64().is_some_and(|v| v.unsigned_abs() > MAX_SAFE) =>
        {
            Value::String(n.to_string())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(json_safe).collect()),
        Value::Object(items) => {
            Value::Object(items.into_iter().map(|(k, v)| (k, json_safe(v))).collect())
        }
        value => value,
    }
}

impl From<Evidence> for EvidenceResponse {
    fn from(evidence: Evidence) -> Self {
        let data = match evidence.data {
            EvidenceData::Rows { total_count, rows } => EvidenceResponseData::Rows {
                total_count: DecimalU64(total_count),
                rows: rows
                    .into_iter()
                    .map(|row| row.into_iter().map(|(k, v)| (k, json_safe(v))).collect())
                    .collect(),
            },
            EvidenceData::SchemaDiff { expected, observed } => {
                EvidenceResponseData::SchemaDiff { expected, observed }
            }
        };
        Self {
            id: evidence.id,
            run_id: evidence.run_id,
            stage_id: evidence.stage_id,
            source_stage_ids: evidence.source_stage_ids,
            query: evidence.query,
            kind: evidence.kind,
            data,
        }
    }
}

/// Apply this adapter to diagnostic JSON before returning a Finding response.
pub fn finding_response(mut finding: Finding) -> Finding {
    finding.expected = json_safe(finding.expected);
    finding.observed = json_safe(finding.observed);
    finding
}
