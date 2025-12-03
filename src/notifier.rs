//! Unified logging and progress UI.
//!
//! [`SimpleNotifier`] wraps `env_logger` (text logs) and `indicatif` (spinners/bars) under a single
//! verbosity switch:
//! - [`VerbosityLevel::Quiet`] → no text logs; shows a live spinner_style and optional progress bars.
//! - [`VerbosityLevel::Info`]/[`VerbosityLevel::Debug`]/[`VerbosityLevel::Trace`] → standard logs.
//!
//! What you get:
//! - [`SimpleNotifier::info`]/[`SimpleNotifier::debug`]/[`SimpleNotifier::warn`]/[`SimpleNotifier::trace`] — emit logs
//!   (or update the Quiet-mode spinner_style message for `info`).
//! - [`SimpleNotifier::create_progress_bar`] — add a pretty progress bar (Quiet mode only).
//! - [`SimpleNotifier::progress`] — periodic textual progress for non-Quiet modes.
//! - [`SimpleNotifier::use_beautiful_progress`] — check if UI bars are active.
//! - [`SimpleNotifier::verbosity_level`] — read the current level.
//!
//! Levels map to `env_logger` filters; Quiet suppresses logs (≥ Warn) while rendering
//! spinners/bars via an internal `MultiProgress`.

use atty::Stream;
use env_logger::Env;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use log::{Level, LevelFilter, Log, Record};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, sync::Mutex};

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
#[derive(Clone, Copy)]
pub enum NotifierFlavor {
    Simple,
    Enhanced,
}

pub enum AnyNotifier {
    Simple(SimpleNotifier),
    Enhanced(EnhancedNotifier),
}

pub struct SimpleNotifier {
    verbosity: VerbosityLevel,
    logger: env_logger::Logger,
    multi_progress: Option<Arc<MultiProgress>>,
    active_spinner: RefCell<Option<ProgressBar>>,
}
pub struct EnhancedNotifier {
    verbosity: VerbosityLevel,
    logger: env_logger::Logger,
    multi_progress: MultiProgress,
    bars: Mutex<HashMap<String, ProgressBar>>,
}
pub trait Notifier: Send + Sync {
    fn info(&self, msg: &str);
    fn debug(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
}

impl AnyNotifier {
    pub fn new(flavor: NotifierFlavor, verbosity: u8) -> Self {
        match flavor {
            NotifierFlavor::Simple => AnyNotifier::Simple(SimpleNotifier::new(verbosity)),
            NotifierFlavor::Enhanced => AnyNotifier::Enhanced(EnhancedNotifier::new(verbosity)),
        }
    }

    pub fn finish_spinner(&self) {
        if let AnyNotifier::Simple(n) = self {
            n.finish_spinner();
        }
    }
    pub fn create_progress_bar(&self, length: u64, name: &str) -> Option<ProgressBar> {
        match self {
            // Enhanced expects (name, Option<u64>): use `message` as the name/prefix
            AnyNotifier::Simple(n) => n.create_progress_bar(length, name),
            AnyNotifier::Enhanced(n) => n.create_progress_bar_enhanced(length, name),
        }
    }
    pub fn progress(&self, current: u64, total: u64, message: &str) {
        match self {
            AnyNotifier::Simple(n) => n.progress(current, total, message),
            AnyNotifier::Enhanced(_) => {}
        }
    }
    pub fn info(&self, msg: &str) {
        match self {
            AnyNotifier::Simple(n) => n.info(msg),
            AnyNotifier::Enhanced(n) => n.info(msg),
        }
    }
    pub fn debug(&self, msg: &str) {
        match self {
            AnyNotifier::Simple(n) => n.debug(msg),
            AnyNotifier::Enhanced(n) => n.debug(msg),
        }
    }

    pub fn println_above(&self, msg: String) {
        match self {
            AnyNotifier::Enhanced(n) => n.println_above(msg),
            AnyNotifier::Simple(_) => {}
        }
    }

    pub fn warn(&self, msg: &str) {
        match self {
            AnyNotifier::Simple(n) => n.warn(msg),
            AnyNotifier::Enhanced(n) => n.warn(msg),
        }
    }

    pub fn error(&self, msg: &str) {
        match self {
            AnyNotifier::Simple(_) => {}
            AnyNotifier::Enhanced(n) => n.error(msg),
        }
    }
}

impl SimpleNotifier {
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

    pub fn finish_spinner(&self) {
        if let Some(spinner) = self.active_spinner.borrow_mut().take() {
            spinner.finish_and_clear();
        }
    }

    pub fn info(&self, message: &str) {
        match self.verbosity {
            VerbosityLevel::Quiet => {
                // Lazy initialize spinner_style on first info call
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

                // Update spinner_style message
                if let Some(spinner) = self.active_spinner.borrow().as_ref() {
                    spinner.set_message(message.to_string());
                }
            }
            _ => {
                self.logger.log(
                    &Record::builder()
                        .args(format_args!("{message}"))
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
                    .args(format_args!("{message}"))
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
                    .args(format_args!("{message}"))
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
                    .args(format_args!("{message}"))
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
                        "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} [{eta}]",
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
            self.info(&format!("{message}: {current}/{total}"));
        }
    }

    pub fn use_beautiful_progress(&self) -> bool {
        self.verbosity == VerbosityLevel::Quiet
    }

    pub fn verbosity_level(&self) -> VerbosityLevel {
        self.verbosity
    }
}

impl EnhancedNotifier {
    pub fn new(verbosity_level: u8) -> Self {
        let verbosity = VerbosityLevel::from(verbosity_level);
        let logger = env_logger::Builder::from_env(Env::default())
            .filter_level(verbosity.to_log_level())
            .build();
        let mp = if atty::is(Stream::Stderr) {
            MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(15))
        } else {
            MultiProgress::with_draw_target(ProgressDrawTarget::hidden())
        };

        Self {
            verbosity,
            logger,
            multi_progress: mp,
            bars: Mutex::new(HashMap::new()),
        }
    }

    pub fn finish_progress_bar(&self, human_label: &str) {
        let key = format!("files:{}", human_label);
        let mut map = self.bars.lock().unwrap();
        if let Some(pb) = map.remove(&key) {
            pb.finish_and_clear();
        }
    }

    pub fn create_progress_bar_enhanced(&self, length: u64, message: &str) -> Option<ProgressBar> {
        if self.verbosity == VerbosityLevel::Quiet {
            let multi_progress = &self.multi_progress;
            let progress_style = ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {prefix} [{eta}]")
                .unwrap()
                .progress_chars("=> ");

            let progress_bar = multi_progress.add(ProgressBar::new(length));
            progress_bar.set_style(progress_style);
            progress_bar.set_prefix(message.to_string()); // label goes in {prefix}
            progress_bar.set_message(""); // keep {msg} free/empty
            progress_bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(30));
            return Some(progress_bar);
        }
        None
    }

    fn level_enabled(&self, level: LevelFilter) -> bool {
        // LevelFilter can be compared to Level directly (Info <= Debug, etc.)
        // so we can reuse your VerbosityLevel mapping.
        self.verbosity.to_log_level() >= level
    }
    pub fn println_above(&self, msg: impl AsRef<str>) {
        // prints a line above all bars without creating a new bar
        println!("{}", msg.as_ref())
    }

    pub fn shutdown(&self) {
        let mut map = self.bars.lock().unwrap();
        for pb in map.values() {
            if !pb.is_finished() {
                pb.finish_and_clear();
            }
        }
        self.multi_progress
            .set_draw_target(ProgressDrawTarget::hidden());
        map.clear();
    }

    pub fn suspend<F: FnOnce() -> R, R>(&self, f: F) -> R {
        // temporarily suspends drawing, runs `f`, then resumes
        self.multi_progress.suspend(f)
    }
}

impl Notifier for EnhancedNotifier {
    fn info(&self, msg: &str) {
        // show info only when enabled (i.e., -v or higher)
        if self.level_enabled(LevelFilter::Info) {
            // use MultiProgress::println so bars don’t jump
            self.multi_progress.println(msg).ok();
        }
    }

    fn debug(&self, msg: &str) {
        // debug only when -vv or higher
        if self.level_enabled(LevelFilter::Debug) {
            self.logger.log(
                &Record::builder()
                    .args(format_args!("{}", msg))
                    .level(Level::Debug)
                    .target(module_path!())
                    .build(),
            );
        }
    }
    fn warn(&self, msg: &str) {
        self.multi_progress.println(format!("⚠️ {msg}")).ok();
    }
    fn error(&self, msg: &str) {
        self.multi_progress.println(format!("❌ {msg}")).ok();
    }
}
