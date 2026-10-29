use std::collections::BTreeMap;

use duckdb::Connection;
use pipeline_autopsy_core::{ColumnSchema, StageId, StageOperation};

use crate::{EngineError, LoadedPipeline};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo {
    pub stage_id: StageId,
    pub row_count: u64,
    pub columns: Vec<ColumnSchema>,
}

/// One isolated in-memory DuckDB connection per ingestion. Source tables are
/// materialized so later file edits cannot change an already ingested source.
pub struct SourceSession {
    connection: Connection,
    sources: BTreeMap<StageId, SourceInfo>,
}

pub(crate) fn identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

impl SourceSession {
    /// Ingest only CSV sources. SQL stage files are deliberately not executed.
    pub fn ingest(loaded: &LoadedPipeline) -> Result<Self, EngineError> {
        let mut connection = Connection::open_in_memory()?;
        let mut sources = BTreeMap::new();
        let transaction = connection.transaction()?;
        for stage in &loaded.pipeline().stages {
            if !matches!(stage.operation, StageOperation::CsvSource { .. }) {
                continue;
            }
            let path = loaded
                .stage_path(&stage.id)
                .expect("loaded stages have resolved paths");
            let source = import_csv(&transaction, &stage.id, path)?;
            sources.insert(stage.id.clone(), source);
        }
        transaction.commit()?;
        Ok(Self {
            connection,
            sources,
        })
    }

    pub fn sources(&self) -> &BTreeMap<StageId, SourceInfo> {
        &self.sources
    }
    /// For trusted engine code and inspection; not exposed as an HTTP SQL endpoint.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

pub(crate) fn import_csv(
    connection: &Connection,
    stage: &StageId,
    path: &std::path::Path,
) -> Result<SourceInfo, EngineError> {
    let result = || -> Result<SourceInfo, duckdb::Error> {
        let table = identifier(&stage.0);
        connection.execute(
            &format!("CREATE TABLE {table} AS SELECT * FROM read_csv(?, header=true, skip=0, delim=',', quote='\"', escape='\"', sample_size=-1, strict_mode=true, ignore_errors=false, null_padding=false, nullstr='', allow_quoted_nulls=false)"),
            [path.to_str().expect("loader validates UTF-8")],
        )?;
        inspect_table(connection, stage)
    };
    result().map_err(|source| EngineError::Csv {
        stage: stage.0.clone(),
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn inspect_table(
    connection: &Connection,
    stage: &StageId,
) -> Result<SourceInfo, duckdb::Error> {
    let table = identifier(&stage.0);
    let row_count: u64 =
        connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?;
    let mut statement = connection.prepare(&format!("DESCRIBE {table}"))?;
    let columns = statement
        .query_map([], |row| {
            let nullable: String = row.get(2)?;
            Ok(ColumnSchema {
                name: row.get(0)?,
                data_type: row.get(1)?,
                nullable: match nullable.as_str() {
                    "YES" => Some(true),
                    "NO" => Some(false),
                    _ => None,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SourceInfo {
        stage_id: stage.clone(),
        row_count,
        columns,
    })
}
