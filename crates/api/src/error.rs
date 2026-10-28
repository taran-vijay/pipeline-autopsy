//! RFC 9457 problem details shared by all HTTP failures.
use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    ValidationFailed,
    NotFound,
    Conflict,
    MethodNotAllowed,
    NotImplemented,
    StorageUnavailable,
    InternalError,
}

impl ErrorCode {
    pub fn status(self) -> StatusCode {
        match self {
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::ValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::NotImplemented => StatusCode::NOT_IMPLEMENTED,
            Self::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldError {
    /// JSON pointer into the request, e.g. /stages/0/inputs.
    pub pointer: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub code: ErrorCode,
    /// URI reference identifying an occurrence; omitted if none was allocated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<FieldError>,
}

#[derive(Debug)]
pub struct ApiError {
    code: ErrorCode,
    errors: Vec<FieldError>,
}

impl ApiError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            errors: vec![],
        }
    }
    pub fn validation(errors: Vec<FieldError>) -> Self {
        Self {
            code: ErrorCode::ValidationFailed,
            errors,
        }
    }
    pub fn problem(&self) -> Problem {
        let status = self.code.status();
        // Server/storage failures intentionally have fixed public messages.
        // Callers must log internal errors privately instead of serializing them.
        let detail = match self.code {
            ErrorCode::InvalidRequest => "The request could not be decoded.",
            ErrorCode::ValidationFailed => "The request violates one or more validation rules.",
            ErrorCode::NotFound => "The requested resource was not found.",
            ErrorCode::Conflict => "The request conflicts with existing resource state.",
            ErrorCode::MethodNotAllowed => "This resource does not support the requested method.",
            ErrorCode::NotImplemented => "This operation is planned but not implemented.",
            ErrorCode::StorageUnavailable => "Metadata storage is temporarily unavailable.",
            ErrorCode::InternalError => "An internal error prevented the request from completing.",
        };
        Problem {
            type_uri: "about:blank".into(),
            title: status.canonical_reason().unwrap_or("Error").into(),
            status: status.as_u16(),
            detail: detail.into(),
            code: self.code,
            instance: None,
            errors: self.errors.clone(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let problem = self.problem();
        (
            self.code.status(),
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(problem),
        )
            .into_response()
    }
}
