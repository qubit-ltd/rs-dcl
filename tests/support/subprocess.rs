// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::ffi::OsStr;
use std::io;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const CHILD_TEST_ENV: &str = "QUBIT_DCL_BOUNDED_TEST_CHILD";
const CHILD_SUCCESS_EXIT_CODE: i32 = 73;

/// Runs a test scenario in a child test process and enforces a deadline.
///
/// The child exits with code 73 only after the scenario completes. Requiring
/// that code proves libtest matched this exact test and the scenario returned.
pub fn run_test_in_subprocess<F>(test_name: &str, timeout: Duration, scenario: F) -> Result<(), String>
where
    F: FnOnce(),
{
    if std::env::var_os(CHILD_TEST_ENV).as_deref() == Some(OsStr::new(test_name)) {
        scenario();
        std::process::exit(CHILD_SUCCESS_EXIT_CODE);
    }

    let executable =
        std::env::current_exe().map_err(|error| format!("{test_name}: could not locate test executable: {error}"))?;
    let mut child = Command::new(executable)
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_TEST_ENV, test_name)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("{test_name}: could not start child test: {error}"))?;
    let mut child = ChildGuard::new(&mut child);
    let started = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let elapsed = started.elapsed();
                if status.code() == Some(CHILD_SUCCESS_EXIT_CODE) {
                    return Ok(());
                }
                return Err(format!(
                    "{test_name}: child exited after {elapsed:?} with unexpected exit status {status}"
                ));
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let elapsed = started.elapsed();
                let cleanup = child.terminate_and_reap();
                let cleanup_detail = cleanup
                    .err()
                    .map(|error| format!("; child cleanup failed: {error}"))
                    .unwrap_or_default();
                return Err(format!(
                    "{test_name}: child timed out after {elapsed:?}{cleanup_detail}"
                ));
            }
            Err(error) => {
                let cleanup = child.terminate_and_reap();
                let cleanup_detail = cleanup
                    .err()
                    .map(|error| format!("; child cleanup failed: {error}"))
                    .unwrap_or_default();
                return Err(format!(
                    "{test_name}: failed to poll child status: {error}{cleanup_detail}"
                ));
            }
        }
    }
}

struct ChildGuard<'a> {
    child: &'a mut Child,
    reaped: bool,
}

impl<'a> ChildGuard<'a> {
    fn new(child: &'a mut Child) -> Self {
        Self { child, reaped: false }
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.reaped = true;
        }
        Ok(status)
    }

    fn terminate_and_reap(&mut self) -> io::Result<ExitStatus> {
        let poll_error = match self.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => None,
            Err(error) => Some(error),
        };

        let kill_error = self.child.kill().err();
        match self.child.wait() {
            Ok(status) => {
                self.reaped = true;
                match poll_error {
                    Some(error) => Err(io::Error::other(format!(
                        "status poll failed before child was reaped: {error}"
                    ))),
                    None => Ok(status),
                }
            }
            Err(wait_error) => {
                let mut diagnostics = Vec::new();
                if let Some(error) = poll_error {
                    diagnostics.push(format!("status poll failed: {error}"));
                }
                if let Some(error) = kill_error {
                    diagnostics.push(format!("kill failed: {error}"));
                }
                diagnostics.push(format!("wait failed: {wait_error}"));
                Err(io::Error::other(diagnostics.join("; ")))
            }
        }
    }
}

impl Drop for ChildGuard<'_> {
    fn drop(&mut self) {
        if !self.reaped
            && let Err(error) = self.terminate_and_reap()
        {
            eprintln!("failed to clean up child test process: {error}");
        }
    }
}
