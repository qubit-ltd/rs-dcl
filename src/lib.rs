// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! # Qubit DCL
//!
//! Reusable double-checked locking executors for atomic or equivalently
//! synchronized gates and generic [`qubit_lock::Lock`] implementations.
//!
//! A basic executor can use an Acquire/Release gate with one shared lock:
//!
//! ```
//! use std::sync::{
//!     Arc,
//!     Mutex,
//!     atomic::{AtomicBool, Ordering},
//! };
//!
//! use qubit_dcl::{DclExecutor, ExecutionOutcome};
//!
//! let gate = Arc::new(AtomicBool::new(true));
//! let lock = Mutex::new(());
//! let executor = DclExecutor::builder()
//!     .when({
//!         let gate = Arc::clone(&gate);
//!         move || gate.load(Ordering::Acquire)
//!     })
//!     .build();
//!
//! let outcome = executor.run(&lock, {
//!     let gate = Arc::clone(&gate);
//!     move || {
//!         gate.store(false, Ordering::Release);
//!         Ok::<(), ()>(())
//!     }
//! });
//! assert!(matches!(outcome, ExecutionOutcome::Success(())));
//! ```
//!
//! Lifecycle builder states expose only valid next steps. The following
//! incomplete configurations intentionally do not compile.
//!
//! A predicate is required before lifecycle configuration can continue:
//!
//! ```compile_fail
//! use qubit_dcl::LifecycleDclExecutor;
//!
//! let builder = LifecycleDclExecutor::builder();
//! let _executor = builder.build();
//! ```
//!
//! Prepare is required after the predicate:
//!
//! ```compile_fail
//! use qubit_dcl::LifecycleDclExecutor;
//!
//! let builder = LifecycleDclExecutor::builder()
//!         .when(|| true);
//! let _executor = builder.build();
//! ```
//!
//! Prepare must be followed by a commit/no-commit choice:
//!
//! ```compile_fail
//! use std::io;
//!
//! use qubit_dcl::LifecycleDclExecutor;
//!
//! let builder = LifecycleDclExecutor::builder()
//!         .when(|| true)
//!         .prepare(|| Ok::<(), io::Error>(()));
//! let _executor = builder.build();
//! ```
//!
//! Configured commit must be followed by rollback/no-rollback:
//!
//! ```compile_fail
//! use std::io;
//!
//! use qubit_dcl::LifecycleDclExecutor;
//!
//! let builder = LifecycleDclExecutor::builder()
//!         .when(|| true)
//!         .prepare(|| Ok::<(), io::Error>(()))
//!         .commit(|_| Ok::<(), io::Error>(()));
//! let _executor = builder.build();
//! ```
//!
//! Selecting no commit still requires rollback:
//!
//! ```compile_fail
//! use std::io;
//!
//! use qubit_dcl::LifecycleDclExecutor;
//!
//! let builder = LifecycleDclExecutor::builder()
//!         .when(|| true)
//!         .prepare(|| Ok::<(), io::Error>(()))
//!         .no_commit();
//! let _builder = builder.no_rollback();
//! ```
//!
//! Commit cannot be configured before prepare:
//!
//! ```compile_fail
//! use std::io;
//!
//! use qubit_dcl::LifecycleDclExecutor;
//!
//! let builder = LifecycleDclExecutor::builder()
//!         .when(|| true);
//! let _builder = builder.commit(|_| Ok::<(), io::Error>(()));
//! ```

pub mod double_checked;

pub use double_checked::{
    DclExecutor,
    ExecutionOutcome,
    FinalizationOutcome,
    LifecycleDclExecutor,
    LifecycleOutcome,
    PanicInfo,
    PanicPhase,
    RollbackCause,
};
