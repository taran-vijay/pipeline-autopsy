//! Allocate durable identities before execution starts.
//!
//! Run IDs are random UUID v4 strings; stage execution IDs are the existing
//! (run_id, stage_id) database key, distinct from reusable definition StageIds.

use pipeline_autopsy_core::{Pipeline, Run, RunId};
use pipeline_autopsy_storage::{StorageError, create_pending_run};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Allocate and persist a pending run and every stage/contract snapshot atomically.
/// Callers must validate the pipeline first. A fresh call always creates a new run;
/// no identity is returned until the database commits successfully.
pub async fn prepare_run(pool: &SqlitePool, pipeline: &Pipeline) -> Result<Run, StorageError> {
    create_pending_run(pool, RunId(Uuid::new_v4().to_string()), pipeline).await
}
