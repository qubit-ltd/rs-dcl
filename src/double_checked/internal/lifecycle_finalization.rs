// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private commit and rollback finalization policy.

use crate::double_checked::{
    CapturedFinalizationOutcome,
    FinalizationOutcome,
    PanicPhase,
    RollbackCause,
    internal::catch_phase,
};

/// Runs an optional commit callback or drops its token after successful work.
///
/// # Parameters
///
/// * `commit` - Optional callback that consumes the prepared token.
/// * `token` - Token to commit or drop.
///
/// # Returns
///
/// The finalization outcome preserving callback errors.
///
/// # Panics
///
/// Propagates a commit callback or token-drop panic.
pub(crate) fn finalize_commit<P, C, F>(
    commit: Option<&F>,
    token: P,
) -> FinalizationOutcome<C>
where
    F: Fn(P) -> Result<(), C> + ?Sized,
{
    match commit {
        Some(commit) => match commit(token) {
            Ok(()) => FinalizationOutcome::Succeeded,
            Err(error) => FinalizationOutcome::Failed(error),
        },
        None => {
            drop(token);
            FinalizationOutcome::NotRequired
        }
    }
}

/// Runs an optional commit callback or drops its token after successful work.
///
/// # Parameters
///
/// * `commit` - Optional callback that consumes the prepared token.
/// * `token` - Token to commit or drop.
///
/// # Returns
///
/// The finalization outcome preserving callback errors or captured panics.
///
/// # Panics
///
/// Propagates a commit callback or token-drop panic when unwinding.
pub(crate) fn finalize_commit_catching<P, C, F>(
    commit: Option<&F>,
    token: P,
) -> CapturedFinalizationOutcome<C>
where
    F: Fn(P) -> Result<(), C> + ?Sized,
{
    match commit {
        Some(commit) => match catch_phase(PanicPhase::Commit, || commit(token))
        {
            Ok(Ok(())) => CapturedFinalizationOutcome::Succeeded,
            Ok(Err(error)) => CapturedFinalizationOutcome::Failed(error),
            Err(panic) => CapturedFinalizationOutcome::Panicked(panic),
        },
        None => match catch_phase(PanicPhase::Commit, || drop(token)) {
            Ok(()) => CapturedFinalizationOutcome::NotRequired,
            Err(panic) => CapturedFinalizationOutcome::Panicked(panic),
        },
    }
}

/// Runs an optional rollback callback or drops its token after unsuccessful
/// work.
///
/// # Parameters
///
/// * `rollback` - Optional callback that consumes the prepared token.
/// * `token` - Token to roll back or drop.
/// * `cause` - Borrowed reason that selected rollback.
///
/// # Returns
///
/// The finalization outcome preserving callback errors.
///
/// # Panics
///
/// Propagates a rollback callback or token-drop panic.
pub(crate) fn finalize_rollback<P, C, F>(
    rollback: Option<&F>,
    token: P,
    cause: RollbackCause<'_>,
) -> FinalizationOutcome<C>
where
    F: for<'a> Fn(P, RollbackCause<'a>) -> Result<(), C> + ?Sized,
{
    match rollback {
        Some(rollback) => match rollback(token, cause) {
            Ok(()) => FinalizationOutcome::Succeeded,
            Err(error) => FinalizationOutcome::Failed(error),
        },
        None => {
            drop(token);
            FinalizationOutcome::NotRequired
        }
    }
}

/// Runs an optional rollback callback or drops its token after unsuccessful
/// work.
///
/// # Parameters
///
/// * `rollback` - Optional callback that consumes the prepared token.
/// * `token` - Token to roll back or drop.
/// * `cause` - Borrowed reason that selected rollback.
///
/// # Returns
///
/// The finalization outcome preserving callback errors or captured panics.
///
/// # Panics
///
/// Propagates a rollback callback or token-drop panic when unwinding.
pub(crate) fn finalize_rollback_catching<P, C, F>(
    rollback: Option<&F>,
    token: P,
    cause: RollbackCause<'_>,
) -> CapturedFinalizationOutcome<C>
where
    F: for<'a> Fn(P, RollbackCause<'a>) -> Result<(), C> + ?Sized,
{
    match rollback {
        Some(rollback) => {
            match catch_phase(PanicPhase::Rollback, || rollback(token, cause)) {
                Ok(Ok(())) => CapturedFinalizationOutcome::Succeeded,
                Ok(Err(error)) => CapturedFinalizationOutcome::Failed(error),
                Err(panic) => CapturedFinalizationOutcome::Panicked(panic),
            }
        }
        None => match catch_phase(PanicPhase::Rollback, || drop(token)) {
            Ok(()) => CapturedFinalizationOutcome::NotRequired,
            Err(panic) => CapturedFinalizationOutcome::Panicked(panic),
        },
    }
}
