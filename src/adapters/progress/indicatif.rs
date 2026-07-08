use std::collections::HashMap;
use std::sync::Mutex;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::domain::{AssetOutcome, RunSummary};
use crate::ports::ProgressReporter;

/// A hybrid apt-style reporter (TTY): up to `concurrency` live per-Asset bars
/// plus an overall bytes bar, with a persistent `✓`/`✗` line logged as each
/// Asset finishes.
pub struct IndicatifReporter {
    multi: MultiProgress,
    overall: Mutex<Option<ProgressBar>>,
    bars: Mutex<HashMap<usize, (ProgressBar, String, u64)>>,
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
    ProgressStyle::with_template("  {msg:24} [{bar:30}] {bytes}/{total_bytes}")
        .unwrap()
        .progress_chars("=> ")
}

fn overall_style() -> ProgressStyle {
    ProgressStyle::with_template("Total [{bar:30}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("=> ")
}

impl ProgressReporter for IndicatifReporter {
    fn start(&self, _total_files: usize, total_bytes: u64) {
        let bar = self.multi.add(ProgressBar::new(total_bytes));
        bar.set_style(overall_style());
        *self.overall.lock().unwrap() = Some(bar);
    }

    fn asset_started(&self, index: usize, name: &str, size: u64) {
        let bar = ProgressBar::new(size);
        bar.set_style(asset_style());
        bar.set_message(name.to_string());
        // Insert per-Asset bars above the overall bar so the overall stays pinned
        // at the bottom (apt-style).
        let bar = match self.overall.lock().unwrap().as_ref() {
            Some(overall) => self.multi.insert_before(overall, bar),
            None => self.multi.add(bar),
        };
        self.bars
            .lock()
            .unwrap()
            .insert(index, (bar, name.to_string(), size));
    }

    fn asset_advanced(&self, index: usize, bytes: u64) {
        if let Some((bar, _, _)) = self.bars.lock().unwrap().get(&index) {
            bar.inc(bytes);
        }
        if let Some(overall) = self.overall.lock().unwrap().as_ref() {
            overall.inc(bytes);
        }
    }

    fn asset_finished(&self, index: usize, outcome: &AssetOutcome) {
        let Some((bar, name, size)) = self.bars.lock().unwrap().remove(&index) else {
            return;
        };
        bar.finish_and_clear();
        let line = match outcome {
            AssetOutcome::Downloaded(_) => format!("✓ {name} ({size} bytes) md5 ok"),
            AssetOutcome::Cached => format!("✓ {name} (cached)"),
            AssetOutcome::Failed(failure) => format!("✗ {name}: {}", failure.message),
        };
        let _ = self.multi.println(line);
    }

    fn finish(&self, _summary: &RunSummary) {
        if let Some(overall) = self.overall.lock().unwrap().take() {
            overall.finish_and_clear();
        }
    }
}
