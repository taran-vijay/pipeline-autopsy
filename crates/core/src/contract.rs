use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ContractId, Severity, StageId};

/// An explicit expectation attached to a stage's output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub id: ContractId,
    pub stage_id: StageId,
    pub severity: Severity,
    pub rule: ContractRule,
}

/// Rule definitions only; evaluation and semantic validation arrive in Track 4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContractRule {
    RequiredColumns {
        columns: Vec<String>,
    },
    /// Column names mapped to expected engine type names, e.g. BIGINT.
    ColumnTypes {
        columns: BTreeMap<String, String>,
    },
    NotNull {
        columns: Vec<String>,
    },
    /// The combination of columns must be unique.
    Unique {
        columns: Vec<String>,
    },
    /// Evaluate coverage of an input against a referenced stage, including rows
    /// that a lossy join may have removed from this contract's output stage.
    JoinCoverage {
        input_stage: StageId,
        reference_stage: StageId,
        keys: Vec<JoinKey>,
    },
    /// (input rows - output rows) / input rows, clamped at zero.
    /// Must be finite and within [0, 1]. Empty inputs pass. Evaluator enforces it.
    RowLoss {
        input_stage: StageId,
        max_loss_ratio: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinKey {
    pub input_column: String,
    pub reference_column: String,
}
