//! Unified logging and progress UI.
//!
//! [`Notifier`] wraps `env_logger` (text logs) and `indicatif` (spinners/bars) under a single
//! verbosity switch:
//! - [`VerbosityLevel::Quiet`] → no text logs; shows a live spinner and optional progress bars.
//! - [`VerbosityLevel::Info`]/[`VerbosityLevel::Debug`]/[`VerbosityLevel::Trace`] → standard logs.
//!
//! What you get:
//! - [`Notifier::info`]/[`Notifier::debug`]/[`Notifier::warn`]/[`Notifier::trace`] — emit logs
//!   (or update the Quiet-mode spinner message for `info`).
//! - [`Notifier::create_progress_bar`] — add a pretty progress bar (Quiet mode only).
//! - [`Notifier::progress`] — periodic textual progress for non-Quiet modes.
//! - [`Notifier::use_beautiful_progress`] — check if UI bars are active.
//! - [`Notifier::verbosity_level`] — read the current level.
//!
//! Levels map to `env_logger` filters; Quiet suppresses logs (≥ Warn) while rendering
//! spinners/bars via an internal `MultiProgress`.

use env_logger::Env;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{Level, LevelFilter, Log, Record};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerbosityLevel {
    Quiet = 0, // Beautiful progress, no text logs
    Info = 1,  // Text logs at info level
    Debug = 2, // Text logs at debug level
    Trace = 3, // Text logs at trace level
}

impl From<u8> for VerbosityLevel {
    fn from(level: u8) -> Self {
        match level {
            0 => VerbosityLevel::Quiet,
            1 => VerbosityLevel::Info,
            2 => VerbosityLevel::Debug,
            _ => VerbosityLevel::Trace,
        }
    }
}

impl VerbosityLevel {
    fn to_log_level(self) -> LevelFilter {
        match self {
            VerbosityLevel::Quiet => LevelFilter::Warn,
            VerbosityLevel::Info => LevelFilter::Info,
            VerbosityLevel::Debug => LevelFilter::Debug,
            VerbosityLevel::Trace => LevelFilter::Trace,
        }
    }
}

pub struct Notifier {
    verbosity: VerbosityLevel,
    logger: env_logger::Logger,
    multi_progress: Option<Arc<MultiProgress>>,
    active_spinner: RefCell<Option<ProgressBar>>,
}

impl Notifier {
    pub fn new(verbosity_level: u8) -> Self {
        let verbosity = VerbosityLevel::from(verbosity_level);

        // Create logger instance
        let logger = env_logger::Builder::from_env(Env::default())
            .filter_level(verbosity.to_log_level())
            .build();

        let multi_progress = if verbosity == VerbosityLevel::Quiet {
            Some(Arc::new(MultiProgress::new()))
        } else {
            None
        };

        Self {
            verbosity,
            logger,
            multi_progress,
            active_spinner: RefCell::new(None),
        }
    }

    pub fn info(&self, message: &str) {
        match self.verbosity {
            VerbosityLevel::Quiet => {
                // Lazy initialize spinner on first info call
                if self.active_spinner.borrow().is_none() {
                    if let Some(multi_progress) = &self.multi_progress {
                        let spinner_style = ProgressStyle::default_spinner()
                            .template("{spinner:.green} {msg}")
                            .unwrap();

                        let spinner = multi_progress.add(ProgressBar::new_spinner());
                        spinner.set_style(spinner_style);
                        spinner.enable_steady_tick(Duration::from_millis(100));

                        *self.active_spinner.borrow_mut() = Some(spinner);
                    }
                }

                // Update spinner message
                if let Some(spinner) = self.active_spinner.borrow().as_ref() {
                    spinner.set_message(message.to_string());
                }
            }
            _ => {
                self.logger.log(
                    &Record::builder()
                        .args(format_args!("{}", message))
                        .level(Level::Info)
                        .target(module_path!())
                        .build(),
                );
            }
        }
    }

    pub fn debug(&self, message: &str) {
        if self.verbosity != VerbosityLevel::Quiet {
            self.logger.log(
                &Record::builder()
                    .args(format_args!("{}", message))
                    .level(Level::Debug)
                    .target(module_path!())
                    .build(),
            );
        }
    }

    pub fn warn(&self, message: &str) {
        if self.verbosity != VerbosityLevel::Quiet {
            self.logger.log(
                &Record::builder()
                    .args(format_args!("{}", message))
                    .level(Level::Warn)
                    .target(module_path!())
                    .build(),
            );
        }
    }

    pub fn trace(&self, message: &str) {
        if self.verbosity != VerbosityLevel::Quiet {
            self.logger.log(
                &Record::builder()
                    .args(format_args!("{}", message))
                    .level(Level::Trace)
                    .target(module_path!())
                    .build(),
            );
        }
    }

    pub fn create_progress_bar(&self, length: u64, message: &str) -> Option<ProgressBar> {
        if self.verbosity == VerbosityLevel::Quiet {
            if let Some(multi_progress) = &self.multi_progress {
                let progress_style = ProgressStyle::default_bar()
                    .template(
                        "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
                    )
                    .unwrap()
                    .progress_chars("=> ");

                let progress_bar = multi_progress.add(ProgressBar::new(length));
                progress_bar.set_style(progress_style);
                progress_bar.set_message(message.to_string());
                return Some(progress_bar);
            }
        }
        None
    }

    pub fn progress(&self, current: u64, total: u64, message: &str) {
        if self.verbosity != VerbosityLevel::Quiet && (current % 100 == 0 || current == total) {
            self.info(&format!("{}: {}/{}", message, current, total));
        }
    }

    pub fn use_beautiful_progress(&self) -> bool {
        self.verbosity == VerbosityLevel::Quiet
    }

    pub fn verbosity_level(&self) -> VerbosityLevel {
        self.verbosity
    }
}
