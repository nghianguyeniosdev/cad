use crate::domain::Failure;

/// What happened to a single Asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetOutcome {
    /// Fetched, verified, and committed; carries the byte count.
    Downloaded(u64),
    /// Already present with a matching MD5 (Verify-and-Skip).
    Cached,
    /// Failed after any retries; carries the cause.
    Failed(Failure),
}

/// A failed Asset in the Run Summary: its name and the human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedAsset {
    pub name: String,
    pub reason: String,
}

/// The end-of-run tally that determines the exit code.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub downloaded: usize,
    pub cached: usize,
    pub failed: usize,
    /// Package Versions whose archive was unzipped in the Extract Phase.
    pub extracted: usize,
    pub bytes: u64,
    /// The Assets that failed, each with a reason (for the failed-Asset list).
    pub failed_assets: Vec<FailedAsset>,
}

impl RunSummary {
    /// Exit code: `1` if any Asset failed, else `0`.
    pub fn exit_code(&self) -> i32 {
        if self.failed > 0 {
            1
        } else {
            0
        }
    }
}
