//! Validated pipeline definitions and scheduled, materialized DuckDB execution.
pub mod error;
pub mod execution;
pub mod graph;
pub mod scheduler;
pub mod sources;
pub mod spec;
mod sql;

pub use error::{EngineError, GraphIssue, GraphValidationError};
pub use execution::{ExecutionSession, StageOutput};
pub use graph::PipelineGraph;
pub use scheduler::{DispatchError, ScheduleError, StageScheduler, plan_execution};
pub use sources::{SourceInfo, SourceSession};
pub use spec::{LoadedPipeline, load_pipeline, parse_pipeline, validate_pipeline};
