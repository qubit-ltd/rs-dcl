// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Panic phase classification for double-checked execution.

/// Identifies the lifecycle phase in which a captured panic occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanicPhase {
    /// The lock-free condition check panicked.
    InitialConditionCheck,
    /// The prepare callback panicked.
    Prepare,
    /// Acquiring or entering the executor lock panicked.
    LockAcquisition,
    /// The condition check performed while holding the lock panicked.
    SecondConditionCheck,
    /// The task panicked while holding the executor lock.
    Task,
    /// The commit callback panicked after the lock was released.
    Commit,
    /// The rollback callback panicked after the lock was released.
    Rollback,
}
