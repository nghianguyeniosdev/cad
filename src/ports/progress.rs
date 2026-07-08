use crate::domain::{AssetOutcome, RunSummary};

/// The terminal-UI seam. The Download Phase drives these lifecycle hooks;
/// adapters render them (an `indicatif` multi-bar on a TTY, plain log lines
/// otherwise). Methods take `&self` and run concurrently across Asset tasks.
pub trait ProgressReporter: Send + Sync {
    /// Called once before downloading, with the plan's totals.
    fn start(&self, total_files: usize, total_bytes: u64);

    /// An Asset (identified by its plan `index`) has begun downloading. Not
    /// called for cached Assets (which never download).
    fn asset_started(&self, index: usize, name: &str, size: u64);

    /// `bytes` more bytes of the Asset have been received.
    fn asset_advanced(&self, index: usize, bytes: u64);

    /// The Asset reached a terminal outcome. Always called (incl. cached), so
    /// `name` is passed explicitly.
    fn asset_finished(&self, index: usize, name: &str, outcome: &AssetOutcome);

    /// Called once after all Assets, with the final summary.
    fn finish(&self, summary: &RunSummary);
}

/// A reporter that does nothing — the default when no UI is wired.
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn start(&self, _total_files: usize, _total_bytes: u64) {}
    fn asset_started(&self, _index: usize, _name: &str, _size: u64) {}
    fn asset_advanced(&self, _index: usize, _bytes: u64) {}
    fn asset_finished(&self, _index: usize, _name: &str, _outcome: &AssetOutcome) {}
    fn finish(&self, _summary: &RunSummary) {}
}
