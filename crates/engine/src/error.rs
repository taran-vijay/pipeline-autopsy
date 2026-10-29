use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid pipeline YAML: {0}")]
    Yaml(#[from] serde_saphyr::Error),
    #[error("invalid {field}: {message}")]
    Validation { field: String, message: String },
    #[error(transparent)]
    Graph(#[from] GraphValidationError),
    #[error("DuckDB operation failed: {0}")]
    Database(#[from] duckdb::Error),
    #[error("CSV ingestion failed for stage {stage} ({path}): {source}")]
    Csv {
        stage: String,
        path: PathBuf,
        #[source]
        source: duckdb::Error,
    },
}

pub(crate) fn invalid(field: impl Into<String>, message: impl Into<String>) -> EngineError {
    EngineError::Validation {
        field: field.into(),
        message: message.into(),
    }
}

/// Structured graph diagnostics; stages in a cycle are component members, not
/// an asserted path through that component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphIssue {
    EmptyGraph,
    DuplicateStage {
        ids: Vec<pipeline_autopsy_core::StageId>,
    },
    DuplicateInput {
        stage: pipeline_autopsy_core::StageId,
        input: pipeline_autopsy_core::StageId,
    },
    MissingInput {
        stage: pipeline_autopsy_core::StageId,
        input: pipeline_autopsy_core::StageId,
        declared_spelling: Option<pipeline_autopsy_core::StageId>,
    },
    Cycle {
        stages: Vec<pipeline_autopsy_core::StageId>,
    },
}

impl std::fmt::Display for GraphIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyGraph => write!(f, "the graph must contain at least one stage"),
            Self::DuplicateStage { ids } => write!(
                f,
                "duplicate stage names (case-insensitive): {}",
                ids.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::DuplicateInput { stage, input } => {
                write!(f, "stage '{stage}' repeats input '{input}'")
            }
            Self::MissingInput {
                stage,
                input,
                declared_spelling,
            } => {
                write!(f, "stage '{stage}' references missing input '{input}'")?;
                if let Some(declared) = declared_spelling {
                    write!(f, "; use the declared spelling '{declared}'")?;
                }
                Ok(())
            }
            Self::Cycle { stages } => write!(
                f,
                "cycle component: {}",
                stages
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphValidationError {
    pub issues: Vec<GraphIssue>,
}
impl std::fmt::Display for GraphValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dependency graph validation failed")?;
        for issue in &self.issues {
            write!(f, "\n- {issue}")?;
        }
        Ok(())
    }
}
impl std::error::Error for GraphValidationError {}
