//! Scheduled, materialized CSV and SQL stage execution in a fresh DuckDB database.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use duckdb::Connection;
use pipeline_autopsy_core::{ColumnSchema, StageId, StageOperation};

use crate::{
    DispatchError, EngineError, LoadedPipeline, SourceInfo, StageScheduler,
    sources::{identifier, import_csv, inspect_table},
    sql::compile_query,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageOutput {
    pub stage_id: StageId,
    pub row_count: u64,
    pub columns: Vec<ColumnSchema>,
}
impl From<SourceInfo> for StageOutput {
    fn from(info: SourceInfo) -> Self {
        Self {
            stage_id: info.stage_id,
            row_count: info.row_count,
            columns: info.columns,
        }
    }
}

pub struct ExecutionSession {
    connection: Connection,
    outputs: Vec<StageOutput>,
}

impl ExecutionSession {
    /// Preflight query files, then execute all stages serially in one transaction.
    /// On any error no session is returned; all in-memory outputs are discarded.
    pub fn run(loaded: &LoadedPipeline) -> Result<Self, EngineError> {
        let stages: BTreeMap<_, _> = loaded
            .pipeline()
            .stages
            .iter()
            .map(|stage| (&stage.id, stage))
            .collect();
        let all_stage_ids: BTreeSet<_> =
            stages.keys().map(|id| id.0.to_ascii_lowercase()).collect();
        let mut queries = BTreeMap::new();
        for stage in stages.values() {
            if matches!(stage.operation, StageOperation::Sql { .. }) {
                let path = loaded.stage_path(&stage.id).expect("loaded paths exist");
                let source = fs::read_to_string(path).map_err(|source| EngineError::SqlRead {
                    stage: stage.id.0.clone(),
                    path: path.to_path_buf(),
                    source,
                })?;
                let query = compile_query(&source, stage, &all_stage_ids).map_err(|message| {
                    EngineError::SqlDefinition {
                        stage: stage.id.0.clone(),
                        path: path.to_path_buf(),
                        message,
                    }
                })?;
                queries.insert(stage.id.clone(), query);
            }
        }
        let mut connection = Connection::open_in_memory()?;
        let transaction = connection.transaction()?;
        let mut outputs = Vec::new();
        let mut scheduler = StageScheduler::new(loaded.graph());
        let result = scheduler.execute_with(|id| {
            let stage = stages[id];
            let path = loaded.stage_path(id).expect("loaded paths exist");
            let output = match stage.operation {
                StageOperation::CsvSource { .. } => import_csv(&transaction, id, path)?,
                StageOperation::Sql { .. } => {
                    let execute = || -> Result<SourceInfo, duckdb::Error> {
                        transaction.execute(
                            &format!("CREATE TABLE {} AS {}", identifier(&id.0), queries[id]),
                            [],
                        )?;
                        inspect_table(&transaction, id)
                    };
                    execute().map_err(|source| EngineError::Sql {
                        stage: id.0.clone(),
                        path: path.to_path_buf(),
                        source,
                    })?
                }
            };
            outputs.push(StageOutput::from(output));
            Ok::<_, EngineError>(())
        });
        match result {
            Ok(()) => {}
            Err(DispatchError::Scheduler(error)) => return Err(error.into()),
            Err(DispatchError::Executor { source, .. }) => return Err(source),
        }
        transaction.commit()?;
        Ok(Self {
            connection,
            outputs,
        })
    }

    /// Stage outputs in actual execution order, including zero-row results.
    pub fn outputs(&self) -> &[StageOutput] {
        &self.outputs
    }
    /// Trusted Rust inspection only; no arbitrary-query HTTP route is provided.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
