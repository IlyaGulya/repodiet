//! Progress reporting abstraction
//!
//! Decouples scanning logic from UI concerns (indicatif).

use indicatif::{ProgressBar, ProgressStyle};

/// A handle to an active progress bar
pub trait ProgressHandle: Send + Sync {
    fn inc(&self, n: u64);
    fn finish(&self);
}

/// Factory for creating progress handles
pub trait ProgressReporter: Send + Sync {
    fn start(&self, label: &str, total: u64) -> Box<dyn ProgressHandle>;
}

/// Indicatif-based progress reporter for CLI usage
pub struct IndicatifProgress;

impl ProgressReporter for IndicatifProgress {
    fn start(&self, label: &str, total: u64) -> Box<dyn ProgressHandle> {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "{{spinner:.green}} {}: [{{bar:50.cyan/blue}}] {{pos}}/{{len}} ({{per_sec}})",
                    label
                ))
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("=>-"),
        );
        Box::new(IndicatifHandle(pb))
    }
}

struct IndicatifHandle(ProgressBar);

impl ProgressHandle for IndicatifHandle {
    fn inc(&self, n: u64) {
        self.0.inc(n);
    }

    fn finish(&self) {
        self.0.finish_and_clear();
    }
}

/// No-op progress reporter for benchmarks and quiet mode
pub struct NoopProgress;

impl ProgressReporter for NoopProgress {
    fn start(&self, _label: &str, _total: u64) -> Box<dyn ProgressHandle> {
        Box::new(NoopHandle)
    }
}

struct NoopHandle;

impl ProgressHandle for NoopHandle {
    fn inc(&self, _n: u64) {}
    fn finish(&self) {}
}

/// Progress reporter that only shows output when verbose
pub struct VerboseProgress {
    verbose: bool,
}

impl VerboseProgress {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl ProgressReporter for VerboseProgress {
    fn start(&self, label: &str, total: u64) -> Box<dyn ProgressHandle> {
        if self.verbose {
            IndicatifProgress.start(label, total)
        } else {
            NoopProgress.start(label, total)
        }
    }
}
