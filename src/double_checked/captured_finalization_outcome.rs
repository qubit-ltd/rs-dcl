// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Commit and rollback finalization outcomes captured from panic-aware
//! lifecycle APIs.

use crate::double_checked::PanicInfo;

/// Describes how a lifecycle token was finalized after locked execution when
/// panic capture is enabled.
///
/// # Type Parameters
///
/// * `C` - Error type returned by the commit or rollback callback.
#[derive(Debug)]
#[must_use = "the captured finalization outcome must be inspected"]
pub enum CapturedFinalizationOutcome<C> {
    /// No callback was required and the token was dropped normally.
    NotRequired,
    /// The configured commit or rollback callback returned successfully.
    Succeeded,
    /// The configured callback returned its original lifecycle error.
    Failed(C),
    /// The callback or token destructor panicked during finalization.
    Panicked(PanicInfo),
}
