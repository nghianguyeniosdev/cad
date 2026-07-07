use crate::domain::FailureKind;

/// What happened to a single Asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetOutcome {
    /// Fetched, verified, and committed; carries the byte count.
    Downloaded(u64),
    /// Already present with a matching MD5 (Verify-and-Skip).
    Cached,
    /// Failed after any retries.
    Failed(FailureKind),
}

/// The end-of-run tally that determines the exit code.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub downloaded: usize,
    pub cached: usize,
    pub failed: usize,
    pub bytes: u64,
    /// Names of Assets that failed (for the failed-Asset list).
    pub failed_assets: Vec<String>,
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
