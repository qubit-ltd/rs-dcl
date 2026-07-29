// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! README contract and version consistency tests for qubit-dcl.

const CARGO_TOML: &str = include_str!("../../Cargo.toml");
const FEATURE_MATRIX: &str = include_str!("../../.rs-ci-cargo-matrix.json");
const README_EN: &str = include_str!("../../README.md");
const README_ZH: &str = include_str!("../../README.zh_CN.md");

/// Verifies both README files document the final executor API and lifecycle
/// choices.
#[test]
fn test_readmes_document_final_public_api() {
    for readme in [README_EN, README_ZH] {
        assert!(readme.contains("DoubleCheckedLockExecutor::builder()"));
        assert!(readme.contains("executor.run(&lock"));
        assert!(readme.contains("LifecycleDoubleCheckedLockExecutor"));
        assert!(readme.contains("run_with_token"));
        assert!(readme.contains("no_commit"));
        assert!(readme.contains("no_rollback"));
        assert!(readme.contains("ExecutionOutcome"));
        assert!(readme.contains("LifecycleOutcome"));
        assert!(readme.contains("FinalizationOutcome"));
        assert!(readme.contains("LockRelease"));
        assert!(!readme.contains("ExecutionReport"));
        assert!(!readme.contains("PreparationOutcome"));
        assert!(!readme.contains("ExecutionContext"));
        assert!(!readme.contains("ExecutionLogger"));
        assert!(!readme.contains("call_with"));
        assert!(!readme.contains("execute_with"));
    }
}

/// Verifies both README files state the three non-negotiable gate contracts.
#[test]
fn test_readmes_document_gate_and_lock_contracts() {
    let readme_en = README_EN.split_whitespace().collect::<Vec<_>>().join(" ");
    let readme_zh = README_ZH.split_whitespace().collect::<Vec<_>>().join(" ");

    assert!(readme_en.contains("Acquire load"));
    assert!(readme_en.contains("must not acquire the executor's lock"));
    assert!(
        readme_en
            .contains("`Lock` represents an acquisition mode, not necessarily an exclusive one")
    );
    assert!(readme_en.contains("shared read mode"));
    assert!(readme_en.contains("paired write mode"));
    assert!(readme_en.contains("same executor"));
    assert!(readme_en.contains("different captured data"));
    assert!(
        readme_en.contains("coordination mechanism rather than data ownership")
    );
    assert!(readme_en.contains("must use an `ExclusiveLock` mode"));
    assert!(readme_en.contains("same underlying lock"));

    assert!(readme_zh.contains("Acquire load"));
    assert!(readme_zh.contains("不得获取 executor 的同一底层锁"));
    assert!(readme_zh.contains("`Lock` 表示获取模式，并不必然表示排他锁"));
    assert!(readme_zh.contains("共享 read mode"));
    assert!(readme_zh.contains("配套的 write mode"));
    assert!(readme_zh.contains("同一个 executor"));
    assert!(readme_zh.contains("不同的捕获数据"));
    assert!(readme_zh.contains("协调机制，而不是数据所有权"));
    assert!(readme_zh.contains("必须使用 `ExclusiveLock` mode"));
    assert!(readme_zh.contains("同一底层锁"));
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

/// Verifies installation snippets declare the parking-lot backend used by the
/// default examples.
#[test]
fn test_readme_installation_snippets_declare_parking_lot() {
    for readme in [README_EN, README_ZH] {
        assert!(readme.contains("parking_lot = \"0.12\""));
    }
}

/// Verifies the optional parking-lot lock implementation remains enabled by
/// default while consumers can opt out of its transitive dependency.
#[test]
fn test_manifest_exposes_parking_lot_as_a_default_feature() {
    assert!(CARGO_TOML.contains("[features]\ndefault = [\"parking-lot\"]"));
    assert!(CARGO_TOML.contains("parking-lot = [\"qubit-lock/parking-lot\"]"));
    let dependency = find_dependency_spec(CARGO_TOML, "qubit-lock")
        .expect("Cargo.toml should declare qubit-lock");
    assert!(dependency.contains("default-features = false"));
    assert!(dependency.contains("version = \"0.12\""));
    assert!(!dependency.contains("path ="));
}

/// Verifies the CI feature matrix tests both the minimal and parking-lot lock
/// backends.
#[test]
fn test_feature_matrix_covers_minimal_and_parking_lot_backends() {
    assert!(FEATURE_MATRIX.contains("\"name\": \"base-locks\""));
    assert!(FEATURE_MATRIX.contains("\"defaultFeatures\": false"));
    assert!(FEATURE_MATRIX.contains("\"name\": \"parking-lot-locks\""));
    assert!(
        FEATURE_MATRIX
            .contains("\"features\": [\n        \"parking-lot\"\n      ]")
    );
    assert!(FEATURE_MATRIX.contains("\"name\": \"all-features\""));
    assert!(FEATURE_MATRIX.contains("\"allFeatures\": true"));
}

/// Verifies each README links to its language-specific 0.11 migration guide.
#[test]
fn test_readmes_link_migration_guides() {
    assert!(README_EN.contains("doc/user_guide_migration_0_11.md"));
    assert!(README_ZH.contains("doc/user_guide_migration_0_11.zh_CN.md"));
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

/// Finds the inline dependency specification for a package.
///
/// # Parameters
///
/// * `content` - Cargo manifest contents.
/// * `package` - Dependency package name.
///
/// # Returns
///
/// The right-hand side of the matching dependency declaration, or `None` when
/// the package is absent.
fn find_dependency_spec<'a>(content: &'a str, package: &str) -> Option<&'a str> {
    content.lines().find_map(|line| {
        line.strip_prefix(package)?
            .strip_prefix(" = ")
    })
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
