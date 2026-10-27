//! Shared domain models, independent of SQL engines, databases, and transports.

pub mod contract;
pub mod evidence;
pub mod finding;
pub mod ids;
pub mod pipeline;
pub mod run;

pub use contract::*;
pub use evidence::*;
pub use finding::*;
pub use ids::*;
pub use pipeline::*;
pub use run::*;

/// The current engine package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Initial interchange model version; separate from package and pipeline versions.
pub const MODEL_SCHEMA_VERSION: u32 = 1;
