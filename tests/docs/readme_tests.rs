/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! README consistency tests for qubit-dcl.

const CARGO_TOML: &str = include_str!("../../Cargo.toml");
const README_EN: &str = include_str!("../../README.md");
const README_ZH: &str = include_str!("../../README.zh_CN.md");

#[test]
/// Ensures README files reference the current crate and primary type.
fn test_readme_references_current_crate_and_api() {
    assert!(README_EN.contains("qubit-dcl"));
    assert!(README_ZH.contains("qubit-dcl"));
    assert!(README_EN.contains("DoubleCheckedLockExecutor"));
    assert!(README_ZH.contains("DoubleCheckedLockExecutor"));
}

#[test]
/// Ensures README dependency snippets stay in sync with Cargo.toml.
fn test_readme_dependency_version_matches_cargo_toml() {
    let cargo_version =
        extract_package_version(CARGO_TOML).expect("Failed to extract version from Cargo.toml");
    let readme_en_version = extract_readme_dependency_version(README_EN, "qubit-dcl")
        .expect("Failed to extract qubit-dcl version from README.md");
    let readme_zh_version = extract_readme_dependency_version(README_ZH, "qubit-dcl")
        .expect("Failed to extract qubit-dcl version from README.zh_CN.md");
    assert_eq!(readme_en_version, cargo_version);
    assert_eq!(readme_zh_version, cargo_version);
}

/// Extracts the first package version entry from Cargo.toml content.
fn extract_package_version(content: &str) -> Option<&str> {
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("version = \"") {
            return value.strip_suffix('"');
        }
    }
    None
}

/// Extracts the dependency version for the specified crate from a README file.
fn extract_readme_dependency_version<'a>(content: &'a str, crate_name: &str) -> Option<&'a str> {
    let prefix = format!("{} = \"", crate_name);
    for line in content.lines() {
        if let Some(value) = line.trim().strip_prefix(&prefix) {
            return value.strip_suffix('"');
        }
    }
    None
}
