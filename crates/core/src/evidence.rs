use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ColumnSchema, EvidenceId, RunId, StageId};

/// A reproducible investigation artifact. Capture and retention limits are
/// enforced by investigators/storage, not by deserialization of these models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub id: EvidenceId,
    pub run_id: RunId,
    pub stage_id: StageId,
    pub source_stage_ids: Vec<StageId>,
    pub query: Option<String>,
    pub kind: EvidenceKind,
    pub data: EvidenceData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    UnmatchedKeys,
    DuplicateKeys,
    SchemaDrift,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceData {
    /// total_count is the full number of matching records, not the sample size.
    /// rows contains only a bounded sample. Large integers/decimals that cannot
    /// be safely represented by JSON consumers should be encoded as strings.
    Rows {
        total_count: u64,
        rows: Vec<BTreeMap<String, Value>>,
    },
    SchemaDiff {
        expected: Vec<ColumnSchema>,
        observed: Vec<ColumnSchema>,
    },
}
