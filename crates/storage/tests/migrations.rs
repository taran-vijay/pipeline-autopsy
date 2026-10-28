use pipeline_autopsy_storage::{MIGRATOR, in_memory_database, open_database};
use serde_json::{Value, json};
use sqlx::SqlitePool;

async fn seed(pool: &SqlitePool, run_id: &str) {
    let fixture: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/sales_run.json")).unwrap();
    sqlx::query("INSERT OR IGNORE INTO pipeline_revisions (pipeline_id,version,schema_version,name,definition_json) VALUES ('sales','1',1,'Sales',?)")
        .bind(fixture["pipeline"].to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO runs (run_id,pipeline_id,pipeline_version,status) VALUES (?,'sales','1','completed')")
        .bind(run_id).execute(pool).await.unwrap();
    for id in ["orders", "customers", "join_customers", "daily_revenue"] {
        sqlx::query("INSERT INTO stage_runs (run_id,stage_id,status) VALUES (?,?,'completed')")
            .bind(run_id)
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO run_contracts (run_id,contract_id,stage_id,severity,rule_json) VALUES (?,'coverage','join_customers','error',?)")
        .bind(run_id).bind(fixture["pipeline"]["contracts"][0]["rule"].to_string())
        .execute(pool).await.unwrap();
}

#[tokio::test]
async fn migrations_are_repeatable_and_records_survive_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.sqlite");
    let pool = open_database(&path).await.unwrap();
    seed(&pool, "run-1").await;
    // Verify PRAGMA configuration on independently held pooled connections.
    let mut first = pool.acquire().await.unwrap();
    let mut second = pool.acquire().await.unwrap();
    for connection in [&mut first, &mut second] {
        let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut **connection)
            .await
            .unwrap();
        assert_eq!(enabled, 1);
    }
    drop(first);
    drop(second);
    MIGRATOR.run(&pool).await.unwrap();
    pool.close().await;
    let reopened = open_database(&path).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs WHERE run_id = 'run-1'")
        .fetch_one(&reopened)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let migrations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(&reopened)
            .await
            .unwrap();
    assert_eq!(migrations, 1);
    reopened.close().await;
}

#[tokio::test]
async fn checksum_changes_are_rejected() {
    let pool = in_memory_database().await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum = X'01'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(MIGRATOR.run(&pool).await.is_err());
}

#[tokio::test]
async fn revisions_are_immutable_and_json_identity_must_match() {
    let pool = in_memory_database().await.unwrap();
    seed(&pool, "run-1").await;
    assert!(
        sqlx::query("UPDATE pipeline_revisions SET name = 'changed'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM pipeline_revisions")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("INSERT INTO pipeline_revisions (pipeline_id,version,schema_version,name,definition_json) VALUES ('wrong','1',1,'x',?)")
        .bind(json!({"id":"sales","version":"1","schema_version":1}).to_string())
        .execute(&pool).await.is_err());
    assert!(sqlx::query("INSERT INTO runs (run_id,pipeline_id,pipeline_version,status) VALUES ('orphan','missing','1','pending')")
        .execute(&pool).await.is_err());
}

#[tokio::test]
async fn counts_preserve_u64_and_distinguish_zero_from_unmeasured() {
    let pool = in_memory_database().await.unwrap();
    seed(&pool, "run-1").await;
    for value in ["0", "18446744073709551615"] {
        sqlx::query("UPDATE stage_runs SET output_rows = ? WHERE stage_id = 'orders'")
            .bind(value)
            .execute(&pool)
            .await
            .unwrap();
        let stored: String =
            sqlx::query_scalar("SELECT output_rows FROM stage_runs WHERE stage_id = 'orders'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, value);
        assert!(stored.parse::<u64>().is_ok());
    }
    for invalid in [
        "-1",
        "01",
        "1.5",
        "",
        "18446744073709551616",
        "100000000000000000000",
    ] {
        assert!(
            sqlx::query("UPDATE stage_runs SET output_rows = ? WHERE stage_id = 'orders'")
                .bind(invalid)
                .execute(&pool)
                .await
                .is_err(),
            "accepted {invalid}"
        );
    }
    let missing: Option<String> =
        sqlx::query_scalar("SELECT output_rows FROM stage_runs WHERE stage_id = 'customers'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(missing, None);
    assert!(
        sqlx::query("UPDATE stage_runs SET status = 'success'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE stage_runs SET output_schema_json = '{}'")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn evidence_links_cannot_cross_runs_and_deleting_a_run_cleans_children() {
    let pool = in_memory_database().await.unwrap();
    seed(&pool, "run-1").await;
    seed(&pool, "run-2").await;
    sqlx::query("INSERT INTO findings (run_id,finding_id,stage_id,contract_id,severity,conclusion,summary,explanation,expected_json,observed_json) VALUES ('run-1','finding','join_customers','coverage','error','confirmed_evidence','Missing orders','Anti-join evidence','0','190')")
        .execute(&pool).await.unwrap();
    let data = json!({"kind":"rows","total_count":190,"rows":[{"customer_id":null}]});
    for (run_id, evidence_id) in [("run-1", "evidence-1"), ("run-2", "evidence-2")] {
        sqlx::query("INSERT INTO evidence (run_id,evidence_id,stage_id,kind,data_json) VALUES (?,?,'join_customers','unmatched_keys',?)")
            .bind(run_id).bind(evidence_id).bind(data.to_string()).execute(&pool).await.unwrap();
    }
    assert!(
        sqlx::query("INSERT INTO finding_evidence VALUES ('run-1','finding','evidence-2')")
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO finding_evidence VALUES ('run-1','finding','evidence-1')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO evidence_sources VALUES ('run-1','evidence-1','orders')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO finding_impacts VALUES ('run-1','finding','daily_revenue')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO stage_inputs VALUES ('run-1','join_customers','orders','950')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO check_results VALUES ('run-1','coverage','join_customers','failed','0','190','Unmatched orders')").execute(&pool).await.unwrap();
    let stored: String =
        sqlx::query_scalar("SELECT data_json FROM evidence WHERE run_id = 'run-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&stored).unwrap(), data);
    sqlx::query("DELETE FROM runs WHERE run_id = 'run-1'")
        .execute(&pool)
        .await
        .unwrap();
    for (table, query) in [
        (
            "stage_runs",
            "SELECT COUNT(*) FROM stage_runs WHERE run_id = ?",
        ),
        (
            "stage_inputs",
            "SELECT COUNT(*) FROM stage_inputs WHERE run_id = ?",
        ),
        (
            "run_contracts",
            "SELECT COUNT(*) FROM run_contracts WHERE run_id = ?",
        ),
        (
            "check_results",
            "SELECT COUNT(*) FROM check_results WHERE run_id = ?",
        ),
        ("findings", "SELECT COUNT(*) FROM findings WHERE run_id = ?"),
        ("evidence", "SELECT COUNT(*) FROM evidence WHERE run_id = ?"),
        (
            "evidence_sources",
            "SELECT COUNT(*) FROM evidence_sources WHERE run_id = ?",
        ),
        (
            "finding_evidence",
            "SELECT COUNT(*) FROM finding_evidence WHERE run_id = ?",
        ),
        (
            "finding_impacts",
            "SELECT COUNT(*) FROM finding_impacts WHERE run_id = ?",
        ),
    ] {
        let count: i64 = sqlx::query_scalar(query)
            .bind("run-1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "orphan rows in {table}");
    }
    let other: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs WHERE run_id = 'run-2'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(other, 1);
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(violations.is_empty());
}
