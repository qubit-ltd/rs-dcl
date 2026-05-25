/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Execution Logger
//!
//! Logging configuration and helpers for the double-checked lock executor.
//!

use std::fmt;

/// Logger for double-checked execution (condition unmet, prepare failures,
/// prepare commit failures, and prepare rollback failures).
///
/// Each event has its own optional [`log::Level`] and message. `None` means
/// that event does not emit logs. For prepare-style events the message is a
/// prefix formatted as `"{prefix}: {error}"`.
///
/// [`ExecutionLogger::default`] matches the previous `Option` logger unset
/// behavior: condition-unmet is silent (`None`); prepare lifecycle lines use
/// [`log::Level::Error`] with English default prefixes.
///
#[derive(Debug, Clone)]
pub struct ExecutionLogger {
    /// Log level for the condition-unmet message; `None` skips it.
    unmet_condition_level: Option<log::Level>,

    /// Message logged when the execution condition is not met.
    unmet_condition_message: String,

    /// Log level for prepare-action failure lines; `None` skips them.
    prepare_failed_level: Option<log::Level>,

    /// Prefix for prepare-failure lines, formatted as `"{prefix}: {error}"`.
    prepare_failed_message: String,

    /// Log level for prepare-commit failure lines; `None` skips them.
    prepare_commit_failed_level: Option<log::Level>,

    /// Prefix for prepare-commit failure lines, formatted as `"{prefix}: {error}"`.
    prepare_commit_failed_message: String,

    /// Log level for prepare-rollback failure lines; `None` skips them.
    prepare_rollback_failed_level: Option<log::Level>,

    /// Prefix for prepare-rollback failure lines, formatted as
    /// `"{prefix}: {error}"`.
    prepare_rollback_failed_message: String,
}

impl Default for ExecutionLogger {
    /// Returns the logger configuration used when the executor builder does not
    /// apply any logging overrides.
    ///
    /// Condition-unmet logging is disabled ([`ExecutionLogger::unmet_condition_level`]
    /// is [`None`]). Prepare lifecycle failures log at [`log::Level::Error`] with
    /// short English default prefixes (see the field defaults on [`ExecutionLogger`]).
    ///
    /// # Returns
    ///
    /// A new [`ExecutionLogger`] with the values described above.
    #[inline]
    fn default() -> Self {
        Self {
            unmet_condition_level: None,
            unmet_condition_message: String::new(),
            prepare_failed_level: Some(log::Level::Error),
            prepare_failed_message: "Prepare action failed".to_string(),
            prepare_commit_failed_level: Some(log::Level::Error),
            prepare_commit_failed_message: "Prepare commit action failed".to_string(),
            prepare_rollback_failed_level: Some(log::Level::Error),
            prepare_rollback_failed_message: "Prepare rollback action failed".to_string(),
        }
    }
}

impl ExecutionLogger {
    /// Returns the configured level for unmet-condition logging.
    ///
    /// [`None`] means the event does not emit a log line.
    ///
    /// # Returns
    ///
    /// The optional log level for unmet-condition events.
    #[inline]
    pub fn unmet_condition_level(&self) -> Option<log::Level> {
        self.unmet_condition_level
    }

    /// Returns the message used for unmet-condition logging.
    ///
    /// # Returns
    ///
    /// The stored unmet-condition log message.
    #[inline]
    pub fn unmet_condition_message(&self) -> &str {
        &self.unmet_condition_message
    }

    /// Returns the configured level for prepare-action failures.
    ///
    /// [`None`] means the event does not emit a log line.
    ///
    /// # Returns
    ///
    /// The optional log level for prepare-action failure events.
    #[inline]
    pub fn prepare_failed_level(&self) -> Option<log::Level> {
        self.prepare_failed_level
    }

    /// Returns the message prefix used for prepare-action failures.
    ///
    /// # Returns
    ///
    /// The prefix placed before prepare-action failure text.
    #[inline]
    pub fn prepare_failed_message(&self) -> &str {
        &self.prepare_failed_message
    }

    /// Returns the configured level for prepare-commit failures.
    ///
    /// [`None`] means the event does not emit a log line.
    ///
    /// # Returns
    ///
    /// The optional log level for prepare-commit failure events.
    #[inline]
    pub fn prepare_commit_failed_level(&self) -> Option<log::Level> {
        self.prepare_commit_failed_level
    }

    /// Returns the message prefix used for prepare-commit failures.
    ///
    /// # Returns
    ///
    /// The prefix placed before prepare-commit failure text.
    #[inline]
    pub fn prepare_commit_failed_message(&self) -> &str {
        &self.prepare_commit_failed_message
    }

    /// Returns the configured level for prepare-rollback failures.
    ///
    /// [`None`] means the event does not emit a log line.
    ///
    /// # Returns
    ///
    /// The optional log level for prepare-rollback failure events.
    #[inline]
    pub fn prepare_rollback_failed_level(&self) -> Option<log::Level> {
        self.prepare_rollback_failed_level
    }

    /// Returns the message prefix used for prepare-rollback failures.
    ///
    /// # Returns
    ///
    /// The prefix placed before prepare-rollback failure text.
    #[inline]
    pub fn prepare_rollback_failed_message(&self) -> &str {
        &self.prepare_rollback_failed_message
    }

    /// Updates logging for the case where the double-checked condition is not met
    /// (the tester returns `false` before or after taking the lock).
    ///
    /// When [`Self::unmet_condition_level`] is [`None`], [`Self::log_unmet_condition`]
    /// becomes a no-op. The `message` is still stored and used if the level is set
    /// to [`Some`] later.
    ///
    /// # Parameters
    ///
    /// * `level` - Optional severity for the line written through the `log` crate,
    ///   or [`None`] to disable this event.
    /// * `message` - Full line text (not a prefix); passed to [`log::log!`] as the
    ///   format argument when logging runs.
    #[inline]
    pub fn set_unmet_condition(&mut self, level: Option<log::Level>, message: impl Into<String>) {
        self.unmet_condition_level = level;
        self.unmet_condition_message = message.into();
    }

    /// Disables logging for unmet double-checked conditions.
    ///
    /// This keeps the stored message unchanged so a later call to
    /// [`Self::set_unmet_condition`] can re-enable the event with a new message.
    #[inline]
    pub fn disable_unmet_condition(&mut self) {
        self.unmet_condition_level = None;
    }

    /// Updates logging for a failed optional prepare action (before the lock is taken).
    ///
    /// When [`Self::prepare_failed_level`] is [`None`], [`Self::log_prepare_failed`]
    /// becomes a no-op.
    ///
    /// # Parameters
    ///
    /// * `level` - Optional severity for the diagnostic line, or [`None`] to disable.
    /// * `message_prefix` - Text placed before the error; the emitted line has the
    ///   form `"{prefix}: {error}"`.
    #[inline]
    pub fn set_prepare_failure(&mut self, level: Option<log::Level>, message_prefix: impl Into<String>) {
        self.prepare_failed_level = level;
        self.prepare_failed_message = message_prefix.into();
    }

    /// Disables logging for prepare-action failures.
    #[inline]
    pub fn disable_prepare_failure(&mut self) {
        self.prepare_failed_level = None;
    }

    /// Updates logging for a failed prepare commit action (after a successful task
    /// when prepare had completed).
    ///
    /// When [`Self::prepare_commit_failed_level`] is [`None`],
    /// [`Self::log_prepare_commit_failed`] becomes a no-op.
    ///
    /// # Parameters
    ///
    /// * `level` - Optional severity for the diagnostic line, or [`None`] to disable.
    /// * `message_prefix` - Text placed before the error; the emitted line has the
    ///   form `"{prefix}: {error}"`.
    #[inline]
    pub fn set_prepare_commit_failure(&mut self, level: Option<log::Level>, message_prefix: impl Into<String>) {
        self.prepare_commit_failed_level = level;
        self.prepare_commit_failed_message = message_prefix.into();
    }

    /// Disables logging for prepare-commit failures.
    #[inline]
    pub fn disable_prepare_commit_failure(&mut self) {
        self.prepare_commit_failed_level = None;
    }

    /// Updates logging for a failed prepare rollback action (after a failed second
    /// check or task when prepare had completed).
    ///
    /// When [`Self::prepare_rollback_failed_level`] is [`None`],
    /// [`Self::log_prepare_rollback_failed`] becomes a no-op.
    ///
    /// # Parameters
    ///
    /// * `level` - Optional severity for the diagnostic line, or [`None`] to disable.
    /// * `message_prefix` - Text placed before the error; the emitted line has the
    ///   form `"{prefix}: {error}"`.
    #[inline]
    pub fn set_prepare_rollback_failure(&mut self, level: Option<log::Level>, message_prefix: impl Into<String>) {
        self.prepare_rollback_failed_level = level;
        self.prepare_rollback_failed_message = message_prefix.into();
    }

    /// Disables logging for prepare-rollback failures.
    #[inline]
    pub fn disable_prepare_rollback_failure(&mut self) {
        self.prepare_rollback_failed_level = None;
    }

    /// Emits the condition-unmet log line if enabled.
    ///
    /// Does nothing when [`Self::unmet_condition_level`] is [`None`]. Otherwise
    /// writes [`Self::unmet_condition_message`] through the `log` facade at the
    /// configured level, subject to the crate-wide maximum log level (for example
    /// set via [`log::set_max_level`] or compile-time filters).
    #[inline]
    pub fn log_unmet_condition(&self) {
        let Some(level) = self.unmet_condition_level else {
            return;
        };
        log::log!(level, "{}", self.unmet_condition_message);
    }

    /// Emits a diagnostic line when the prepare action fails.
    ///
    /// Does nothing when [`Self::prepare_failed_level`] is [`None`]. Otherwise
    /// logs `"{prefix}: {err}"` at the configured level via the `log` facade,
    /// where `prefix` is [`Self::prepare_failed_message`], subject to the
    /// crate-wide maximum log level.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Displayable error or message value appended after the prefix.
    ///
    /// # Parameters
    ///
    /// * `err` - Failure to record next to the configured prefix.
    #[inline]
    pub fn log_prepare_failed<E: fmt::Display>(&self, err: E) {
        let Some(level) = self.prepare_failed_level else {
            return;
        };
        log::log!(level, "{}: {}", self.prepare_failed_message, err);
    }

    /// Emits a diagnostic line when the prepare commit action fails.
    ///
    /// Does nothing when [`Self::prepare_commit_failed_level`] is [`None`].
    /// Otherwise logs `"{prefix}: {err}"` at the configured level, where `prefix`
    /// is [`Self::prepare_commit_failed_message`], subject to the crate-wide
    /// maximum log level.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Displayable error or message value appended after the prefix.
    ///
    /// # Parameters
    ///
    /// * `err` - Commit failure to record next to the configured prefix.
    #[inline]
    pub fn log_prepare_commit_failed<E: fmt::Display>(&self, err: E) {
        let Some(level) = self.prepare_commit_failed_level else {
            return;
        };
        log::log!(level, "{}: {}", self.prepare_commit_failed_message, err);
    }

    /// Emits a diagnostic line when the prepare rollback action fails.
    ///
    /// Does nothing when [`Self::prepare_rollback_failed_level`] is [`None`].
    /// Otherwise logs `"{prefix}: {err}"` at the configured level, where `prefix`
    /// is [`Self::prepare_rollback_failed_message`], subject to the crate-wide
    /// maximum log level.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Displayable error or message value appended after the prefix.
    ///
    /// # Parameters
    ///
    /// * `err` - Rollback failure to record next to the configured prefix.
    #[inline]
    pub fn log_prepare_rollback_failed<E: fmt::Display>(&self, err: E) {
        let Some(level) = self.prepare_rollback_failed_level else {
            return;
        };
        log::log!(level, "{}: {}", self.prepare_rollback_failed_message, err);
    }
}
