//! Pure domain types and logic — no I/O. See ADR 0004.

pub mod asset;
pub mod error;
pub mod manifest;
pub mod outcome;
pub mod plan;

pub use asset::Asset;
pub use error::{Failure, FailureKind};
pub use manifest::{
    ConnectionSettings, Entry, Layout, Manifest, ManifestError, RawEntry, RawManifest,
};
pub use outcome::{AssetOutcome, FailedAsset, RunSummary};
pub use plan::{DownloadPlan, PlannedAsset};
