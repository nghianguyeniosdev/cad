use std::collections::HashMap;
use std::sync::Mutex;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::domain::{AssetOutcome, RunSummary};
use crate::ports::ProgressReporter;

/// A TTY reporter: one line per downloading Asset that updates in place and
/// then settles into a `✓`/`✗` done state on the same line, plus an overall
/// file counter (`Total N/M files`) pinned at the bottom.
pub struct IndicatifReporter {
    multi: MultiProgress,
    overall: Mutex<Option<ProgressBar>>,
    bars: Mutex<HashMap<usize, (ProgressBar, u64)>>,
}

impl IndicatifReporter {
    pub fn new() -> Self {
        Self {
            multi: MultiProgress::new(),
            overall: Mutex::new(None),
            bars: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for IndicatifReporter {
    fn default() -> Self {
        Self::new()
    }
}

fn asset_style() -> ProgressStyle {
    ProgressStyle::with_template("  {msg:26} [{bar:28}] {bytes}/{total_bytes}")
        .unwrap()
        .progress_chars("=> ")
}

/// Style for a finished line — no bar, just the settled message (in place).
fn done_style() -> ProgressStyle {
    ProgressStyle::with_template("  {msg}").unwrap()
}

fn overall_style() -> ProgressStyle {
    // The overall line is a file counter, not a bytes bar.
    ProgressStyle::with_template("Total {pos}/{len} files").unwrap()
}

impl ProgressReporter for IndicatifReporter {
    fn start(&self, total_files: usize, _total_bytes: u64) {
        // Overall line counts completed files (not bytes).
        let bar = self.multi.add(ProgressBar::new(total_files as u64));
        bar.set_style(overall_style());
        *self.overall.lock().unwrap() = Some(bar);
    }

    fn asset_started(&self, index: usize, name: &str, size: u64) {
        // Add to the MultiProgress FIRST (binds the bar to the group's draw
        // target), THEN style it — otherwise the bar draws standalone and every
        // update lands on a new line.
        let bar = match self.overall.lock().unwrap().as_ref() {
            Some(overall) => self.multi.insert_before(overall, ProgressBar::new(size)),
            None => self.multi.add(ProgressBar::new(size)),
        };
        bar.set_style(asset_style());
        bar.set_message(name.to_string());
        self.bars.lock().unwrap().insert(index, (bar, size));
    }

    fn asset_advanced(&self, index: usize, bytes: u64) {
        // Only the per-file bar tracks bytes; the overall line counts files.
        if let Some((bar, _)) = self.bars.lock().unwrap().get(&index) {
            bar.inc(bytes);
        }
    }

    fn asset_finished(&self, index: usize, name: &str, outcome: &AssetOutcome) {
        let bar = self.bars.lock().unwrap().remove(&index);
        match (bar, outcome) {
            // A downloading Asset: settle its own line into the done state.
            (Some((bar, size)), AssetOutcome::Downloaded(_)) => {
                bar.set_style(done_style());
                bar.finish_with_message(format!("✓ {name} ({size} bytes) md5 ok"));
            }
            (Some((bar, _)), AssetOutcome::Failed(failure)) => {
                bar.set_style(done_style());
                bar.finish_with_message(format!("✗ {name}: {}", failure.message));
            }
            (Some((bar, _)), AssetOutcome::Cached) => {
                bar.set_style(done_style());
                bar.finish_with_message(format!("✓ {name} (cached)"));
            }
            // Cached Asset never started a bar — print a one-off done line.
            (None, AssetOutcome::Cached) => {
                let _ = self.multi.println(format!("  ✓ {name} (cached)"));
            }
            (None, AssetOutcome::Downloaded(_)) => {
                let _ = self.multi.println(format!("  ✓ {name} md5 ok"));
            }
            (None, AssetOutcome::Failed(failure)) => {
                let _ = self
                    .multi
                    .println(format!("  ✗ {name}: {}", failure.message));
            }
        }

        // Advance the overall file counter for an obtained (downloaded/cached) Asset.
        if matches!(outcome, AssetOutcome::Downloaded(_) | AssetOutcome::Cached) {
            if let Some(overall) = self.overall.lock().unwrap().as_ref() {
                overall.inc(1);
            }
        }
    }

    fn finish(&self, _summary: &RunSummary) {
        if let Some(overall) = self.overall.lock().unwrap().take() {
            overall.finish_and_clear();
        }
    }
}
