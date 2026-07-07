//! Pure domain types and logic — no I/O. See ADR 0004.

pub mod asset;
pub mod error;
pub mod manifest;
pub mod outcome;
pub mod plan;

pub use asset::Asset;
pub use error::FailureKind;
pub use manifest::{ConnectionSettings, Entry, Manifest, ManifestError};
pub use outcome::RunSummary;
pub use plan::{DownloadPlan, PlannedAsset};
