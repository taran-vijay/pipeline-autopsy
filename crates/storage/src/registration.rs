//! Atomic identity registration; execution telemetry repositories follow in T3.5.
use std::collections::BTreeMap;

use pipeline_autopsy_core::{
    MODEL_SCHEMA_VERSION, Pipeline, Run, RunId, RunStatus, StageRun, StageStatus,
};
use sqlx::SqlitePool;

use crate::StorageError;

/// Register an already validated definition, a fresh run, and all pending stages.
/// No execution is performed. No existing records are replaced or updated.
/// The transaction prevents callers observing a partially registered run.
pub async fn create_pending_run(
    pool: &SqlitePool,
    id: RunId,
    pipeline: &Pipeline,
) -> Result<Run, StorageError> {
    if id.0.trim().is_empty()
        || pipeline.id.0.trim().is_empty()
        || pipeline.version.trim().is_empty()
        || pipeline.stages.is_empty()
        || pipeline.schema_version != MODEL_SCHEMA_VERSION
    {
        return Err(StorageError::InvalidRegistration(
            "expected nonempty identifiers, stages, and the supported schema version".into(),
        ));
    }
    let definition = serde_json::to_value(pipeline)?;
    let mut tx = pool.begin().await?;
    // Writing first serializes concurrent registration of the same revision.
    // Only the revision-key conflict is ignored; all other constraints still fail.
    sqlx::query("INSERT INTO pipeline_revisions (pipeline_id,version,schema_version,name,definition_json) VALUES (?,?,?,?,?) ON CONFLICT(pipeline_id,version) DO NOTHING")
        .bind(&pipeline.id.0).bind(&pipeline.version).bind(pipeline.schema_version)
        .bind(&pipeline.name).bind(definition.to_string()).execute(&mut *tx).await?;
    let existing: String = sqlx::query_scalar(
        "SELECT definition_json FROM pipeline_revisions WHERE pipeline_id = ? AND version = ?",
    )
    .bind(&pipeline.id.0)
    .bind(&pipeline.version)
    .fetch_one(&mut *tx)
    .await?;
    if serde_json::from_str::<serde_json::Value>(&existing)? != definition {
        return Err(StorageError::RevisionConflict {
            pipeline_id: pipeline.id.0.clone(),
            version: pipeline.version.clone(),
        });
    }
    sqlx::query(
        "INSERT INTO runs (run_id,pipeline_id,pipeline_version,status) VALUES (?,?,?,'pending')",
    )
    .bind(&id.0)
    .bind(&pipeline.id.0)
    .bind(&pipeline.version)
    .execute(&mut *tx)
    .await?;
    let mut stages = Vec::with_capacity(pipeline.stages.len());
    for stage in &pipeline.stages {
        sqlx::query("INSERT INTO stage_runs (run_id,stage_id,status) VALUES (?,?,'pending')")
            .bind(&id.0)
            .bind(&stage.id.0)
            .execute(&mut *tx)
            .await?;
        stages.push(StageRun {
            stage_id: stage.id.clone(),
            status: StageStatus::Pending,
            started_at: None,
            finished_at: None,
            duration_ms: None,
            input_rows: BTreeMap::new(),
            output_rows: None,
            output_schema: None,
            error: None,
            skip_reason: None,
        });
    }
    for contract in &pipeline.contracts {
        // Serialization uses the shared enum's wire representation, avoiding a second mapping.
        let severity = serde_json::to_value(contract.severity)?;
        sqlx::query("INSERT INTO run_contracts (run_id,contract_id,stage_id,severity,rule_json) VALUES (?,?,?,?,?)")
            .bind(&id.0).bind(&contract.id.0).bind(&contract.stage_id.0)
            .bind(severity.as_str().expect("severity serializes as text"))
            .bind(serde_json::to_string(&contract.rule)?).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Run {
        id,
        pipeline: pipeline.clone(),
        status: RunStatus::Pending,
        started_at: None,
        finished_at: None,
        stages,
        findings: vec![],
        evidence: vec![],
    })
}
