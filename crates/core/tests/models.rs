use pipeline_autopsy_core::*;
use serde_json::{Value, json};

fn sales_run() -> Run {
    serde_json::from_str(include_str!("fixtures/sales_run.json")).unwrap()
}

#[test]
fn recorded_investigation_round_trips_without_losing_counts_or_references() {
    let run = sales_run();
    let encoded = serde_json::to_string(&run).unwrap();
    assert_eq!(serde_json::from_str::<Run>(&encoded).unwrap(), run);
    let wire: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(wire["evidence"][0]["data"]["total_count"], 190);
    assert_eq!(
        wire["evidence"][0]["data"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(run.findings[0].evidence_ids[0], run.evidence[0].id);
    assert_eq!(run.findings[0].run_id, run.id);
    assert_eq!(wire["stages"][2]["input_rows"]["orders"], 950);
    assert_eq!(wire["stages"][2]["output_rows"], 760);
    // SQL completion must remain distinct from a contract violation.
    assert_eq!(run.status, RunStatus::Completed);
    assert_eq!(run.findings[0].severity, Severity::Error);
}

#[test]
fn absent_metrics_remain_distinct_from_observed_zero() {
    let mut stage = sales_run().stages.remove(0);
    stage.output_rows = None;
    stage.output_schema = None;
    let missing = serde_json::to_value(&stage).unwrap();
    stage.output_rows = Some(0);
    stage.output_schema = Some(vec![]);
    let empty = serde_json::to_value(&stage).unwrap();
    assert_eq!(missing["output_rows"], Value::Null);
    assert_eq!(empty["output_rows"], 0);
    assert_eq!(missing["output_schema"], Value::Null);
    assert_eq!(empty["output_schema"], json!([]));
}

#[test]
fn all_contract_families_have_an_explicit_wire_tag() {
    let rules = [
        json!({"kind":"required_columns","columns":["order_id"]}),
        json!({"kind":"column_types","columns":{"order_id":"BIGINT"}}),
        json!({"kind":"not_null","columns":["order_id"]}),
        json!({"kind":"unique","columns":["order_id","customer_id"]}),
        json!({"kind":"join_coverage","input_stage":"orders","reference_stage":"customers",
            "keys":[{"input_column":"customer_id","reference_column":"id"}]}),
        json!({"kind":"row_loss","input_stage":"orders","max_loss_ratio":0.05}),
    ];
    for wire in rules {
        let rule: ContractRule = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(rule).unwrap(), wire);
    }
}

#[test]
fn execution_failure_and_upstream_skip_survive_serialization() {
    let mut run = sales_run();
    run.status = RunStatus::Failed;
    run.stages[2].status = StageStatus::Failed;
    run.stages[2].output_rows = None;
    run.stages[2].error = Some(ExecutionError {
        code: "sql_execution_failed".into(),
        message: "Missing column: customer_id".into(),
    });
    run.stages[3].status = StageStatus::Skipped;
    run.stages[3].output_rows = None;
    run.stages[3].skip_reason = Some("Upstream stage join_customers failed".into());
    let encoded = serde_json::to_value(&run).unwrap();
    assert_eq!(encoded["stages"][2]["status"], "failed");
    assert_eq!(encoded["stages"][3]["status"], "skipped");
    assert_eq!(serde_json::from_value::<Run>(encoded).unwrap(), run);
}

#[test]
fn schema_drift_and_uncertain_conclusions_round_trip() {
    let mut run = sales_run();
    run.findings[0].conclusion = Conclusion::Inconclusive;
    run.evidence[0].kind = EvidenceKind::SchemaDrift;
    run.evidence[0].data = EvidenceData::SchemaDiff {
        expected: vec![ColumnSchema {
            name: "customer_id".into(),
            data_type: "BIGINT".into(),
            nullable: Some(false),
        }],
        observed: vec![ColumnSchema {
            name: "customer_id".into(),
            data_type: "VARCHAR".into(),
            nullable: None,
        }],
    };
    let encoded = serde_json::to_value(&run).unwrap();
    assert_eq!(encoded["findings"][0]["conclusion"], "inconclusive");
    assert_eq!(encoded["evidence"][0]["data"]["kind"], "schema_diff");
    assert_eq!(serde_json::from_value::<Run>(encoded).unwrap(), run);
}

#[test]
fn malformed_definitions_and_invalid_states_are_rejected() {
    let mut pipeline = serde_json::to_value(sales_run().pipeline).unwrap();
    pipeline["stages"][0]["operation"]["paht"] = json!("typo.csv");
    assert!(serde_json::from_value::<Pipeline>(pipeline).is_err());
    assert!(
        serde_json::from_value::<StageOperation>(json!({"kind":"python","path":"a.py"})).is_err()
    );
    assert!(serde_json::from_value::<RunStatus>(json!("successful")).is_err());
    let mut run = serde_json::to_value(sales_run()).unwrap();
    run["started_at"] = json!("not-a-timestamp");
    assert!(serde_json::from_value::<Run>(run).is_err());
}
