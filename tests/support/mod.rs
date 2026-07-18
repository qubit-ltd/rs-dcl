// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared integration-test support.

mod counting_lock;
mod panic_on_drop;

pub use counting_lock::CountingLock;
pub use panic_on_drop::PanicOnDrop;
