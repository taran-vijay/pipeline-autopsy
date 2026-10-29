use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use pipeline_autopsy_core::{
    ContractRule, MODEL_SCHEMA_VERSION, Pipeline, StageId, StageOperation,
};

use crate::{EngineError, PipelineGraph, error::invalid};

/// A validated definition and resolved local files; construction is through load_pipeline.
#[derive(Debug)]
pub struct LoadedPipeline {
    pipeline: Pipeline,
    graph: PipelineGraph,
    definition_path: PathBuf,
    base_dir: PathBuf,
    stage_paths: BTreeMap<StageId, PathBuf>,
}

impl LoadedPipeline {
    pub fn pipeline(&self) -> &Pipeline {
        &self.pipeline
    }
    pub fn graph(&self) -> &PipelineGraph {
        &self.graph
    }
    pub fn definition_path(&self) -> &Path {
        &self.definition_path
    }
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
    pub fn stage_path(&self, id: &StageId) -> Option<&Path> {
        self.stage_paths.get(id).map(PathBuf::as_path)
    }
}

fn parse_document(yaml: &str) -> Result<(Pipeline, PipelineGraph), EngineError> {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::options::DuplicateKeyPolicy::Error,
        merge_keys: serde_saphyr::options::MergeKeyPolicy::Error,
        reject_unsupported_tags: true,
    };
    let pipeline: Pipeline = serde_saphyr::from_str_with_options(yaml, options)?;
    let graph = validate_pipeline(&pipeline)?;
    Ok((pipeline, graph))
}

/// Parse one YAML document and validate definitions/references, without file I/O.
pub fn parse_pipeline(yaml: &str) -> Result<Pipeline, EngineError> {
    Ok(parse_document(yaml)?.0)
}

/// Resolve CSV/SQL paths relative to the YAML file, independent of process cwd.
/// Files must be regular local files inside the definition directory tree.
pub fn load_pipeline(path: impl AsRef<Path>) -> Result<LoadedPipeline, EngineError> {
    let definition_path = canonical(path.as_ref())?;
    let yaml = fs::read_to_string(&definition_path).map_err(|source| EngineError::Io {
        path: definition_path.clone(),
        source,
    })?;
    let (pipeline, graph) = parse_document(&yaml)?;
    let base_dir = definition_path
        .parent()
        .expect("a canonical file has a parent")
        .to_path_buf();
    let mut stage_paths = BTreeMap::new();
    for stage in &pipeline.stages {
        let path = match &stage.operation {
            StageOperation::CsvSource { path } | StageOperation::Sql { path } => path,
        };
        let resolved = canonical(&base_dir.join(path))?;
        if !resolved.starts_with(&base_dir) {
            return Err(invalid(
                format!("stages.{}.operation.path", stage.id),
                "path escapes the pipeline directory",
            ));
        }
        if !resolved.is_file() {
            return Err(invalid(
                format!("stages.{}.operation.path", stage.id),
                "expected a regular file",
            ));
        }
        let text = resolved
            .to_str()
            .ok_or_else(|| invalid("operation.path", "DuckDB paths must be UTF-8"))?;
        if text.chars().any(|c| matches!(c, '*' | '?' | '[' | ']')) {
            return Err(invalid(
                "operation.path",
                "file glob patterns are not supported",
            ));
        }
        stage_paths.insert(stage.id.clone(), resolved);
    }
    Ok(LoadedPipeline {
        pipeline,
        graph,
        definition_path,
        base_dir,
        stage_paths,
    })
}

fn canonical(path: &Path) -> Result<PathBuf, EngineError> {
    fs::canonicalize(path).map_err(|source| EngineError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn text(field: &str, value: &str) -> Result<(), EngineError> {
    if value.trim().is_empty() || value != value.trim() {
        return Err(invalid(
            field,
            "must be nonempty and have no surrounding whitespace",
        ));
    }
    Ok(())
}

fn columns(field: &str, values: &[String]) -> Result<(), EngineError> {
    if values.is_empty() {
        return Err(invalid(field, "must contain at least one column"));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        text(field, value)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(invalid(field, "duplicate column"));
        }
    }
    Ok(())
}

/// Validates the supported v1 shape and builds a dependency graph. This does not
/// inspect CSV schemas, parse SQL, or evaluate contract outcomes.
pub fn validate_pipeline(pipeline: &Pipeline) -> Result<PipelineGraph, EngineError> {
    if pipeline.schema_version != MODEL_SCHEMA_VERSION {
        return Err(invalid(
            "schema_version",
            format!("only {MODEL_SCHEMA_VERSION} is supported"),
        ));
    }
    text("id", &pipeline.id.0)?;
    text("version", &pipeline.version)?;
    text("name", &pipeline.name)?;
    if pipeline.stages.is_empty() {
        return Err(invalid("stages", "must contain at least one stage"));
    }
    for stage in &pipeline.stages {
        let id = &stage.id.0;
        let mut chars = id.chars();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(invalid(
                "stage.id",
                "use an ASCII SQL identifier: [A-Za-z_][A-Za-z0-9_]*",
            ));
        }
        text(&format!("stages.{id}.name"), &stage.name)?;
        let path = match &stage.operation {
            StageOperation::CsvSource { path } => {
                if !stage.inputs.is_empty() {
                    return Err(invalid(
                        format!("stages.{id}.inputs"),
                        "CSV sources cannot have inputs",
                    ));
                }
                path
            }
            StageOperation::Sql { path } => {
                if stage.inputs.is_empty() {
                    return Err(invalid(
                        format!("stages.{id}.inputs"),
                        "SQL stages require at least one input",
                    ));
                }
                path
            }
        };
        if path.as_os_str().is_empty() || path.is_absolute() {
            return Err(invalid(
                format!("stages.{id}.operation.path"),
                "expected a nonempty relative local file path",
            ));
        }
    }
    let graph = PipelineGraph::build(pipeline)?;
    let stages: BTreeMap<_, _> = pipeline.stages.iter().map(|s| (&s.id, s)).collect();
    let mut contract_ids = BTreeSet::new();
    for contract in &pipeline.contracts {
        text("contract.id", &contract.id.0)?;
        if !contract_ids.insert(&contract.id) {
            return Err(invalid("contracts", "duplicate contract ID"));
        }
        let target = stages.get(&contract.stage_id).ok_or_else(|| {
            invalid(
                "contract.stage_id",
                format!("unknown stage {}", contract.stage_id),
            )
        })?;
        match &contract.rule {
            ContractRule::RequiredColumns { columns: names }
            | ContractRule::NotNull { columns: names }
            | ContractRule::Unique { columns: names } => columns("contract.columns", names)?,
            ContractRule::ColumnTypes { columns: types } => {
                columns(
                    "contract.columns",
                    &types.keys().cloned().collect::<Vec<_>>(),
                )?;
                for data_type in types.values() {
                    text("contract.column_type", data_type)?;
                }
            }
            ContractRule::JoinCoverage {
                input_stage,
                reference_stage,
                keys,
            } => {
                for input in [input_stage, reference_stage] {
                    if !target.inputs.contains(input) {
                        return Err(invalid(
                            "contract.join_coverage",
                            format!("{input} must be a declared input of {}", target.id),
                        ));
                    }
                }
                if input_stage == reference_stage {
                    return Err(invalid(
                        "contract.join_coverage",
                        "input and reference stages must differ",
                    ));
                }
                columns(
                    "contract.keys.input_column",
                    &keys
                        .iter()
                        .map(|k| k.input_column.clone())
                        .collect::<Vec<_>>(),
                )?;
                columns(
                    "contract.keys.reference_column",
                    &keys
                        .iter()
                        .map(|k| k.reference_column.clone())
                        .collect::<Vec<_>>(),
                )?;
            }
            ContractRule::RowLoss {
                input_stage,
                max_loss_ratio,
            } => {
                if !target.inputs.contains(input_stage) {
                    return Err(invalid(
                        "contract.input_stage",
                        "must be a declared input of the target stage",
                    ));
                }
                if !max_loss_ratio.is_finite() || !(0.0..=1.0).contains(max_loss_ratio) {
                    return Err(invalid(
                        "contract.max_loss_ratio",
                        "must be finite and within [0, 1]",
                    ));
                }
            }
        }
    }
    Ok(graph)
}
