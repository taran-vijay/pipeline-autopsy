use std::collections::BTreeSet;

use pipeline_autopsy_core::{Pipeline, RunId, RunStatus, StageStatus};
use pipeline_autopsy_storage::{
    StorageError, create_pending_run, in_memory_database, open_database,
};
use pipeline_autopsy_tracker::prepare_run;
use sqlx::SqlitePool;

fn pipeline() -> Pipeline {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/sales_run.json")).unwrap();
    serde_json::from_value(fixture["pipeline"].clone()).unwrap()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    let query = match table {
        "pipeline_revisions" => "SELECT COUNT(*) FROM pipeline_revisions",
        "runs" => "SELECT COUNT(*) FROM runs",
        "stage_runs" => "SELECT COUNT(*) FROM stage_runs",
        "run_contracts" => "SELECT COUNT(*) FROM run_contracts",
        _ => panic!("unexpected test table"),
    };
    sqlx::query_scalar(query).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn repeated_runs_have_distinct_ids_and_unmeasured_pending_stages() {
    let pool = in_memory_database().await.unwrap();
    let pipeline = pipeline();
    let first = prepare_run(&pool, &pipeline).await.unwrap();
    let second = prepare_run(&pool, &pipeline).await.unwrap();
    assert_ne!(first.id, second.id);
    for run in [&first, &second] {
        let uuid = uuid::Uuid::parse_str(&run.id.0).unwrap();
        assert_eq!(uuid.get_version_num(), 4);
        assert_eq!(uuid.to_string(), run.id.0);
        assert_eq!(run.pipeline, pipeline);
        assert_eq!(run.status, RunStatus::Pending);
        assert!(run.started_at.is_none() && run.finished_at.is_none());
        assert_eq!(run.stages.len(), pipeline.stages.len());
        for stage in &run.stages {
            assert_eq!(stage.status, StageStatus::Pending);
            assert!(stage.output_rows.is_none() && stage.output_schema.is_none());
            assert!(stage.started_at.is_none() && stage.finished_at.is_none());
            assert!(stage.duration_ms.is_none() && stage.error.is_none());
            assert!(stage.input_rows.is_empty() && stage.skip_reason.is_none());
        }
    }
    let first_stage = first.stages[0].execution_id(&first.id);
    let second_stage = second.stages[0].execution_id(&second.id);
    assert_eq!(first_stage.stage_id, second_stage.stage_id);
    assert_ne!(first_stage, second_stage);
    assert_eq!(
        serde_json::from_str::<pipeline_autopsy_core::StageExecutionId>(
            &serde_json::to_string(&first_stage).unwrap()
        )
        .unwrap(),
        first_stage
    );
    assert_eq!(count(&pool, "pipeline_revisions").await, 1);
    assert_eq!(count(&pool, "runs").await, 2);
    assert_eq!(
        count(&pool, "stage_runs").await,
        2 * pipeline.stages.len() as i64
    );
    assert_eq!(
        count(&pool, "run_contracts").await,
        2 * pipeline.contracts.len() as i64
    );
}

#[tokio::test]
async fn records_and_definition_survive_reopening_and_new_process_allocation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite");
    let pool = open_database(&path).await.unwrap();
    let first = prepare_run(&pool, &pipeline()).await.unwrap();
    pool.close().await;
    let reopened = open_database(&path).await.unwrap();
    let stored: (String, String, Option<String>) =
        sqlx::query_as("SELECT run_id,status,started_at FROM runs WHERE run_id = ?")
            .bind(&first.id.0)
            .fetch_one(&reopened)
            .await
            .unwrap();
    assert_eq!(stored, (first.id.0.clone(), "pending".into(), None));
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT stage_id FROM stage_runs WHERE run_id = ? ORDER BY stage_id")
            .bind(&first.id.0)
            .fetch_all(&reopened)
            .await
            .unwrap();
    assert_eq!(
        ids.into_iter().collect::<BTreeSet<_>>(),
        first.stages.iter().map(|s| s.stage_id.0.clone()).collect()
    );
    let json: String = sqlx::query_scalar("SELECT definition_json FROM pipeline_revisions")
        .fetch_one(&reopened)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Pipeline>(&json).unwrap(),
        first.pipeline
    );
    let second = prepare_run(&reopened, &pipeline()).await.unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(count(&reopened, "runs").await, 2);
    reopened.close().await;
}

#[tokio::test]
async fn concurrent_allocations_share_revision_but_never_execution_ids() {
    let temp = tempfile::tempdir().unwrap();
    let pool = open_database(temp.path().join("parallel.sqlite"))
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..12 {
        let pool = pool.clone();
        tasks.spawn(async move { prepare_run(&pool, &pipeline()).await.unwrap().id });
    }
    let mut ids = BTreeSet::new();
    while let Some(result) = tasks.join_next().await {
        assert!(ids.insert(result.unwrap()));
    }
    assert_eq!(ids.len(), 12);
    assert_eq!(count(&pool, "pipeline_revisions").await, 1);
    assert_eq!(count(&pool, "runs").await, 12);
    assert_eq!(
        count(&pool, "stage_runs").await,
        12 * pipeline().stages.len() as i64
    );
    pool.close().await;
}

#[tokio::test]
async fn changed_revision_is_rejected_without_rewriting_history() {
    let pool = in_memory_database().await.unwrap();
    let original = pipeline();
    prepare_run(&pool, &original).await.unwrap();
    // JSON formatting/key order are irrelevant to revision equality.
    sqlx::query("DROP TRIGGER pipeline_revisions_immutable")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE pipeline_revisions SET definition_json = ?")
        .bind(serde_json::to_string_pretty(&original).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    prepare_run(&pool, &original).await.unwrap();
    let mut changed = original.clone();
    changed.stages[0].name = "changed".into();
    assert!(matches!(
        prepare_run(&pool, &changed).await,
        Err(StorageError::RevisionConflict { .. })
    ));
    assert_eq!(count(&pool, "runs").await, 2);
    changed.version = "2".into();
    prepare_run(&pool, &changed).await.unwrap();
    assert_eq!(count(&pool, "pipeline_revisions").await, 2);
    assert_eq!(count(&pool, "runs").await, 3);
}

#[tokio::test]
async fn partial_registration_rolls_back_and_duplicate_run_id_never_overwrites() {
    let pool = in_memory_database().await.unwrap();
    let mut invalid = pipeline();
    invalid.stages.push(invalid.stages[0].clone());
    assert!(prepare_run(&pool, &invalid).await.is_err());
    for table in ["pipeline_revisions", "runs", "stage_runs", "run_contracts"] {
        assert_eq!(count(&pool, table).await, 0, "partial {table}");
    }
    invalid = pipeline();
    invalid.contracts[0].stage_id = "missing".into();
    assert!(prepare_run(&pool, &invalid).await.is_err());
    assert_eq!(count(&pool, "runs").await, 0);
    assert_eq!(count(&pool, "stage_runs").await, 0);
    let first = prepare_run(&pool, &pipeline()).await.unwrap();
    assert!(
        create_pending_run(&pool, first.id, &pipeline())
            .await
            .is_err()
    );
    assert_eq!(count(&pool, "runs").await, 1);
    assert_eq!(
        count(&pool, "stage_runs").await,
        pipeline().stages.len() as i64
    );
    assert!(
        create_pending_run(&pool, RunId("".into()), &pipeline())
            .await
            .is_err()
    );
    assert_eq!(count(&pool, "runs").await, 1);
}
