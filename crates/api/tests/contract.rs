use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use pipeline_autopsy_api::{
    error::{ApiError, ErrorCode, FieldError},
    models::*,
    openapi, router,
};
use pipeline_autopsy_core::{Evidence, Run};
use pipeline_autopsy_storage::in_memory_database;
use serde_json::{Value, json};
use tower::ServiceExt;

#[test]
fn checked_in_spec_matches_models_and_every_reference_resolves() {
    let generated = openapi::document();
    let checked_in: Value =
        serde_json::from_str(include_str!("../../../schemas/openapi.json")).unwrap();
    assert_eq!(
        generated, checked_in,
        "regenerate schemas/openapi.json after contract changes"
    );
    fn check_refs(value: &Value, root: &Value) {
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            assert!(
                reference.starts_with("#/"),
                "unexpected external reference {reference}"
            );
            assert!(
                root.pointer(&reference[1..]).is_some(),
                "unresolved {reference}"
            );
        }
        match value {
            Value::Object(values) => values.values().for_each(|v| check_refs(v, root)),
            Value::Array(values) => values.iter().for_each(|v| check_refs(v, root)),
            _ => {}
        }
    }
    check_refs(&generated, &generated);
    let mut ids = std::collections::HashSet::new();
    for (path, methods) in generated["paths"].as_object().unwrap() {
        for op in methods.as_object().unwrap().values() {
            assert!(ids.insert(op["operationId"].as_str().unwrap()));
            assert!(op["responses"].as_object().is_some_and(|r| !r.is_empty()));
            for segment in path.split('/') {
                if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    assert!(
                        op["parameters"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|p| p["name"] == name
                                && p["in"] == "path"
                                && p["required"] == true)
                    );
                }
            }
        }
    }
}

#[test]
fn wire_counts_and_record_values_preserve_large_integers() {
    let run: Run =
        serde_json::from_str(include_str!("../../core/tests/fixtures/sales_run.json")).unwrap();
    let mut stage = run.stages[0].clone();
    stage.output_rows = Some(u64::MAX);
    let value = serde_json::to_value(StageResponse::from(stage)).unwrap();
    assert_eq!(value["output_rows"], u64::MAX.to_string());
    assert_eq!(
        serde_json::from_value::<DecimalU64>(json!(u64::MAX.to_string()))
            .unwrap()
            .0,
        u64::MAX
    );
    for invalid in [
        json!(42),
        json!("-1"),
        json!("01"),
        json!("+1"),
        json!("18446744073709551616"),
    ] {
        assert!(serde_json::from_value::<DecimalU64>(invalid).is_err());
    }
    let mut evidence: Evidence = run.evidence[0].clone();
    evidence.data = pipeline_autopsy_core::EvidenceData::Rows {
        total_count: u64::MAX,
        rows: vec![
            [
                ("key".into(), json!(u64::MAX)),
                ("missing".into(), Value::Null),
            ]
            .into_iter()
            .collect(),
        ],
    };
    let value = serde_json::to_value(EvidenceResponse::from(evidence)).unwrap();
    assert_eq!(value["data"]["total_count"], u64::MAX.to_string());
    assert_eq!(value["data"]["rows"][0]["key"], u64::MAX.to_string());
    assert_eq!(value["data"]["rows"][0]["missing"], Value::Null);
    assert_eq!(
        json_safe(json!({"nested":[i64::MIN,42]})),
        json!({"nested":[i64::MIN.to_string(),42]})
    );
}

#[tokio::test]
async fn health_and_openapi_are_served_from_the_actual_router() {
    let pool = in_memory_database().await.unwrap();
    let app = router(pool);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["database_schema_version"], 1);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(body, openapi::document());
}

#[tokio::test]
async fn router_errors_use_problem_json_with_matching_statuses() {
    let pool = in_memory_database().await.unwrap();
    let app = router(pool);
    for (method, path, expected, code) in [
        ("GET", "/missing", 404, "not_found"),
        ("GET", "/api/v1/runs", 404, "not_found"),
        ("POST", "/api/v1/health", 405, "method_not_allowed"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
        if expected == 405 {
            assert!(response.headers().contains_key("allow"));
        }
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap())
                .unwrap();
        assert_eq!(body["status"], expected);
        assert_eq!(body["code"], code);
        assert_eq!(body["type"], "about:blank");
    }
}

#[tokio::test]
async fn unavailable_database_returns_a_sanitized_service_problem() {
    let pool = in_memory_database().await.unwrap();
    let app = router(pool.clone());
    pool.close().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(body["code"], "storage_unavailable");
    assert_eq!(
        body["detail"],
        "Metadata storage is temporarily unavailable."
    );
    assert!(!body.to_string().contains("pool"));
}

#[test]
fn validation_errors_have_field_pointers_and_server_errors_have_fixed_details() {
    let problem = ApiError::validation(vec![FieldError {
        pointer: "/stages/0/inputs".into(),
        message: "Unknown stage".into(),
    }])
    .problem();
    let encoded = serde_json::to_value(problem).unwrap();
    assert_eq!(encoded["status"], 422);
    assert_eq!(encoded["errors"][0]["pointer"], "/stages/0/inputs");
    assert!(encoded.get("instance").is_none());
    let internal = ApiError::new(ErrorCode::InternalError).problem();
    assert_eq!(internal.status, 500);
    assert_eq!(
        internal.detail,
        "An internal error prevented the request from completing."
    );
}
