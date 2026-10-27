use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ContractId, EvidenceId, FindingId, RunId, StageId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Information,
    Warning,
    Error,
}

/// Confidence in the stated explanation; confirmed evidence is not proof of
/// the ultimate upstream business cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    ConfirmedEvidence,
    SuspectedCause,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: FindingId,
    pub run_id: RunId,
    pub stage_id: StageId,
    pub contract_id: Option<ContractId>,
    pub severity: Severity,
    pub conclusion: Conclusion,
    pub summary: String,
    pub explanation: String,
    pub expected: Value,
    pub observed: Value,
    #[serde(default)]
    pub evidence_ids: Vec<EvidenceId>,
    /// Potentially impacted outputs, not automatically additional root causes.
    #[serde(default)]
    pub affected_stage_ids: Vec<StageId>,
}
