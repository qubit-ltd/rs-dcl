// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private commit and rollback finalization policy.

use crate::double_checked::{
    FinalizationOutcome,
    PanicPhase,
    RollbackCause,
    internal::catch_phase,
};

/// Runs an optional commit callback or drops its token after successful work.
///
/// # Parameters
///
/// * `catch_panics` - Whether callback and token-drop panics become outcomes.
/// * `commit` - Optional callback that consumes the prepared token.
/// * `token` - Token to commit or drop.
///
/// # Returns
///
/// The finalization outcome preserving callback errors or captured panics.
///
/// # Panics
///
/// Propagates a commit callback or token-drop panic when `catch_panics` is
/// `false`.
pub(crate) fn finalize_commit<P, C, F>(
    catch_panics: bool,
    commit: Option<&F>,
    token: P,
) -> FinalizationOutcome<C>
where
    F: Fn(P) -> Result<(), C> + ?Sized,
{
    match commit {
        Some(commit) if catch_panics => {
            match catch_phase(PanicPhase::Commit, || commit(token)) {
                Ok(Ok(())) => FinalizationOutcome::Succeeded,
                Ok(Err(error)) => FinalizationOutcome::Failed(error),
                Err(panic) => FinalizationOutcome::Panicked(panic),
            }
        }
        Some(commit) => match commit(token) {
            Ok(()) => FinalizationOutcome::Succeeded,
            Err(error) => FinalizationOutcome::Failed(error),
        },
        None if catch_panics => {
            match catch_phase(PanicPhase::Commit, || drop(token)) {
                Ok(()) => FinalizationOutcome::NotRequired,
                Err(panic) => FinalizationOutcome::Panicked(panic),
            }
        }
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
/// * `catch_panics` - Whether callback and token-drop panics become outcomes.
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
/// Propagates a rollback callback or token-drop panic when `catch_panics` is
/// `false`.
pub(crate) fn finalize_rollback<P, C, F>(
    catch_panics: bool,
    rollback: Option<&F>,
    token: P,
    cause: RollbackCause<'_>,
) -> FinalizationOutcome<C>
where
    F: for<'a> Fn(P, RollbackCause<'a>) -> Result<(), C> + ?Sized,
{
    match rollback {
        Some(rollback) if catch_panics => {
            match catch_phase(PanicPhase::Rollback, || rollback(token, cause)) {
                Ok(Ok(())) => FinalizationOutcome::Succeeded,
                Ok(Err(error)) => FinalizationOutcome::Failed(error),
                Err(panic) => FinalizationOutcome::Panicked(panic),
            }
        }
        Some(rollback) => match rollback(token, cause) {
            Ok(()) => FinalizationOutcome::Succeeded,
            Err(error) => FinalizationOutcome::Failed(error),
        },
        None if catch_panics => {
            match catch_phase(PanicPhase::Rollback, || drop(token)) {
                Ok(()) => FinalizationOutcome::NotRequired,
                Err(panic) => FinalizationOutcome::Panicked(panic),
            }
        }
        None => {
            drop(token);
            FinalizationOutcome::NotRequired
        }
    }
}
