//! Local API transport and versioned specification.
pub mod error;
pub mod models;
pub mod openapi;

use axum::{Json, Router, extract::State, routing::get};
use sqlx::SqlitePool;

use error::{ApiError, ErrorCode};
use models::{Health, HealthStatus};

/// Only health and specification routes are implemented at this foundation stage.
pub fn router(pool: SqlitePool) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route(
            "/api/v1/openapi.json",
            get(|| async { Json(openapi::document()) }),
        )
        .fallback(|| async { ApiError::new(ErrorCode::NotFound) })
        .method_not_allowed_fallback(|| async {
            (
                [(axum::http::header::ALLOW, "GET, HEAD")],
                ApiError::new(ErrorCode::MethodNotAllowed),
            )
        })
        .with_state(pool)
}

async fn health(State(pool): State<SqlitePool>) -> Result<Json<Health>, ApiError> {
    let version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(&pool)
            .await
            .map_err(|_| ApiError::new(ErrorCode::StorageUnavailable))?;
    let version = version.ok_or_else(|| ApiError::new(ErrorCode::StorageUnavailable))?;
    Ok(Json(Health {
        status: HealthStatus::Ok,
        version: pipeline_autopsy_core::VERSION.into(),
        database_schema_version: version,
    }))
}
