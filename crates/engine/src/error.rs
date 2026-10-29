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
    #[error("duplicate stage ID (DuckDB names are case-insensitive): {stage}")]
    DuplicateStage { stage: String },
    #[error("stage {stage} references missing input {input}")]
    MissingInput { stage: String, input: String },
    #[error("dependency cycle includes stage {stage}")]
    Cycle { stage: String },
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
