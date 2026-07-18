// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Test token whose destructor always panics.

/// Panics when dropped so tests can verify panic-priority contracts.
pub struct PanicOnDrop;

impl Drop for PanicOnDrop {
    /// Raises the secondary panic represented by this test token.
    fn drop(&mut self) {
        panic!("secondary token drop panic");
    }
}
