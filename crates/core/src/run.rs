use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Evidence, Finding, Pipeline, RunId, StageId};

/// A recorded run holds its definition snapshot so later edits do not rewrite history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub id: RunId,
    pub pipeline: Pipeline,
    pub status: RunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub stages: Vec<StageRun>,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Execution status is distinct from contract outcomes: a completed SQL stage
/// can still violate a contract and produce findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageRun {
    pub stage_id: StageId,
    pub status: StageStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Measured elapsed milliseconds; absent before execution or if unavailable.
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub input_rows: BTreeMap<StageId, u64>,
    /// None means unmeasured; Some(0) means an observed empty result.
    pub output_rows: Option<u64>,
    /// None means unmeasured; an empty list is an observed zero-column schema.
    pub output_schema: Option<Vec<ColumnSchema>>,
    pub error: Option<ExecutionError>,
    pub skip_reason: Option<String>,
}

impl StageRun {
    /// Resolve this run-scoped record to its durable composite identity.
    pub fn execution_id(&self, run_id: &RunId) -> crate::StageExecutionId {
        crate::StageExecutionId {
            run_id: run_id.clone(),
            stage_id: self.stage_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    /// None means nullability was not measured.
    pub nullable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionError {
    pub code: String,
    pub message: String,
}
