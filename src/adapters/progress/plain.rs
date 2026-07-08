use crate::domain::{AssetOutcome, RunSummary};
use crate::ports::ProgressReporter;

/// A `ProgressReporter` for non-TTY output (CI, pipes): plain log lines, no bars
/// or escape sequences. Only terminal per-Asset events are logged.
pub struct PlainReporter;

impl ProgressReporter for PlainReporter {
    fn start(&self, _total_files: usize, _total_bytes: u64) {}

    fn asset_started(&self, _index: usize, _name: &str, _size: u64) {}

    fn asset_advanced(&self, _index: usize, _bytes: u64) {}

    fn asset_finished(&self, _index: usize, name: &str, outcome: &AssetOutcome) {
        match outcome {
            AssetOutcome::Downloaded(bytes) => eprintln!("  ✓ {name} ({bytes} bytes) md5 ok"),
            AssetOutcome::Cached => eprintln!("  ✓ {name} (cached)"),
            AssetOutcome::Failed(failure) => eprintln!("  ✗ {name}: {}", failure.message),
        }
    }

    fn finish(&self, _summary: &RunSummary) {}
}
