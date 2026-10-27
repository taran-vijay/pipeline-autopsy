//! Distinct identifier types prevent accidentally mixing record references.
//!
//! IDs are opaque strings. Allocation and persistence belong to the tracker and
//! storage layers; constructing an ID does not validate it or allocate a record.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! identifier {
    ($name:ident) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

identifier!(PipelineId);
identifier!(StageId);
identifier!(RunId);
identifier!(ContractId);
identifier!(FindingId);
identifier!(EvidenceId);
