use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Contract, PipelineId, StageId};

/// A versioned definition. DAG and reference validation belongs to the planner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    pub schema_version: u32,
    pub id: PipelineId,
    /// User-controlled pipeline revision, distinct from the model schema version.
    pub version: String,
    pub name: String,
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub contracts: Vec<Contract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub id: StageId,
    pub name: String,
    #[serde(default)]
    pub inputs: Vec<StageId>,
    pub operation: StageOperation,
}

/// File paths are relative to the pipeline definition directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StageOperation {
    CsvSource { path: PathBuf },
    Sql { path: PathBuf },
}
