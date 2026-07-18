// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! README contract and version consistency tests for qubit-dcl.

const CARGO_TOML: &str = include_str!("../../Cargo.toml");
const README_EN: &str = include_str!("../../README.md");
const README_ZH: &str = include_str!("../../README.zh_CN.md");

/// Verifies both README files document the final executor API and lifecycle
/// choices.
#[test]
fn test_readmes_document_final_public_api() {
    for readme in [README_EN, README_ZH] {
        assert!(readme.contains("DoubleCheckedLockExecutor::builder(lock)"));
        assert!(readme.contains("LifecycleDoubleCheckedLockExecutor"));
        assert!(readme.contains("run_with_token"));
        assert!(readme.contains("no_commit"));
        assert!(readme.contains("no_rollback"));
        assert!(readme.contains("ExecutionOutcome"));
        assert!(readme.contains("PreparationOutcome"));
        assert!(!readme.contains("ExecutionContext"));
        assert!(!readme.contains("ExecutionLogger"));
        assert!(!readme.contains("call_with"));
        assert!(!readme.contains("execute_with"));
    }
}

/// Verifies both README files state the three non-negotiable gate contracts.
#[test]
fn test_readmes_document_gate_and_lock_contracts() {
    assert!(README_EN.contains("Acquire load"));
    assert!(README_EN.contains("must not acquire the executor's lock"));
    assert!(README_EN.contains("inside the executor lock"));
    assert!(README_EN.contains("same underlying lock"));

    assert!(README_ZH.contains("Acquire load"));
    assert!(README_ZH.contains("不得获取 executor 的同一底层锁"));
    assert!(README_ZH.contains("executor 锁内"));
    assert!(README_ZH.contains("同一底层锁"));
}

/// Verifies installation snippets use the package's current major/minor
/// version.
#[test]
fn test_readme_dependency_versions_match_package_version() {
    let package_version = extract_package_version(CARGO_TOML)
        .expect("Cargo.toml should contain a package version");
    let expected = major_minor(package_version)
        .expect("package version should contain major and minor components");

    for readme in [README_EN, README_ZH] {
        let dependency_version = extract_dependency_version(readme)
            .expect("README should contain a qubit-dcl dependency snippet");
        assert_eq!(dependency_version, expected);
    }
}

/// Verifies each README links to its language-specific 0.10 migration guide.
#[test]
fn test_readmes_link_migration_guides() {
    assert!(README_EN.contains("doc/user_guide_migration_0_10.md"));
    assert!(README_ZH.contains("doc/user_guide_migration_0_10.zh_CN.md"));
}

/// Verifies the required final four H2 sections are the last H2 headings in
/// both README files.
#[test]
fn test_readmes_end_with_required_sections() {
    assert_eq!(
        final_h2_headings(README_EN),
        ["Testing", "License", "Contributing", "Author"]
    );
    assert_eq!(
        final_h2_headings(README_ZH),
        ["测试", "许可证", "贡献", "作者"]
    );
}

/// Extracts the package version from the `[package]` table.
///
/// # Parameters
///
/// * `content` - Cargo manifest contents.
///
/// # Returns
///
/// The first quoted package version, or `None` when absent.
fn extract_package_version(content: &str) -> Option<&str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("version = \"")?.strip_suffix('"'))
}

/// Extracts the qubit-dcl version from a README dependency snippet.
///
/// # Parameters
///
/// * `content` - README contents.
///
/// # Returns
///
/// The quoted dependency version, or `None` when absent.
fn extract_dependency_version(content: &str) -> Option<&str> {
    content.lines().find_map(|line| {
        line.trim()
            .strip_prefix("qubit-dcl = \"")?
            .strip_suffix('"')
    })
}

/// Returns the `major.minor` prefix of a dotted version.
///
/// # Parameters
///
/// * `version` - Dotted semantic version.
///
/// # Returns
///
/// The borrowed prefix through the minor component, or `None` when the input
/// has fewer than two components.
fn major_minor(version: &str) -> Option<&str> {
    let first_dot = version.find('.')?;
    let remaining = &version[first_dot + 1..];
    let second_dot = remaining.find('.')? + first_dot + 1;
    Some(&version[..second_dot])
}

/// Returns the final four level-two Markdown headings.
///
/// # Parameters
///
/// * `content` - README contents.
///
/// # Returns
///
/// Four headings in document order.
fn final_h2_headings(content: &str) -> [&str; 4] {
    let headings = content
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .collect::<Vec<_>>();
    headings[headings.len() - 4..]
        .try_into()
        .expect("README should contain at least four H2 headings")
}
