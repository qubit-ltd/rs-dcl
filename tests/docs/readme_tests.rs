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
const USER_GUIDE_EN: &str = include_str!("../../doc/user_guide.md");
const USER_GUIDE_ZH: &str = include_str!("../../doc/user_guide.zh_CN.md");

/// Verifies both README files document the supported public entry points while
/// leaving detailed lifecycle guidance to the user guides.
#[test]
fn test_readmes_document_final_public_api() {
    for readme in [README_EN, README_ZH] {
        assert!(readme.contains("DclExecutor::new"));
        assert!(!readme.contains("let executor = DclExecutor::builder"));
        assert!(readme.contains("executor.run("));
        assert!(readme.contains("LifecycleDclExecutor"));
        assert!(readme.contains("doc/user_guide"));
        assert!(!readme.contains("ExecutionReport"));
        assert!(!readme.contains("PreparationOutcome"));
        assert!(!readme.contains("ExecutionContext"));
        assert!(!readme.contains("ExecutionLogger"));
        assert!(!readme.contains("call_with"));
        assert!(!readme.contains("execute_with"));
    }
}

/// Verifies both user guides state the non-negotiable gate and lock contracts.
#[test]
fn test_user_guides_document_gate_and_lock_contracts() {
    let guide_en = USER_GUIDE_EN
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let guide_zh = USER_GUIDE_ZH
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    assert!(guide_en.contains("Acquire load paired with Release store"));
    assert!(guide_en.contains("must not acquire the coordination lock"));
    assert!(guide_en.contains(concat!(
        "`qubit_lock",
        "::Lock` represents an acquisition mode"
    )));
    assert!(guide_en.contains("`read_lock()` adapter is shared"));
    assert!(guide_en.contains("paired write mode"));
    assert!(guide_en.contains("does not require at-most-once task execution"));
    assert!(guide_en.contains("owns neither the lock nor the business data"));
    assert!(guide_en.contains("exclusive mode"));
    assert!(guide_en.contains("same underlying lock"));

    assert!(guide_zh.contains("Acquire load 配对 Release store"));
    assert!(guide_zh.contains("predicate 不得获取"));
    assert!(guide_zh.contains("协调锁"));
    assert!(guide_zh.contains(concat!("`qubit_lock", "::Lock` 表示获取模式")));
    assert!(guide_zh.contains("`read_lock()` adapter 是共享的"));
    assert!(guide_zh.contains("配套 write mode"));
    assert!(guide_zh.contains("不要求 task 至多执行一次"));
    assert!(guide_zh.contains("不拥有锁或业务数据"));
    assert!(guide_zh.contains("排他模式"));
    assert!(guide_zh.contains("同一个底层锁"));
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

/// Verifies installation snippets identify parking-lot as an optional backend.
#[test]
fn test_readme_installation_snippets_declare_parking_lot() {
    let optional_en = h2_section(README_EN, "Optional Integrations")
        .expect("English README should have optional integrations");
    let optional_zh = h2_section(README_ZH, "可选集成")
        .expect("Chinese README should have optional integrations");

    assert!(optional_en.contains("parking_lot = \"0.12\""));
    assert!(optional_zh.contains("parking_lot = \"0.12\""));
}

/// Verifies README and user guides link the related Qubit crates.
#[test]
fn test_readmes_document_qubit_ecosystem_links() {
    for document in [README_EN, README_ZH, USER_GUIDE_EN, USER_GUIDE_ZH] {
        assert!(document.contains("https://github.com/qubit-ltd/rs-atomic"));
        assert!(document.contains("https://github.com/qubit-ltd/rs-lock"));
    }
    for readme in [README_EN, README_ZH] {
        assert!(readme.contains("https://github.com/qubit-ltd/rs-dcl"));
    }
}

/// Verifies README installation sections stay minimal while optional
/// integrations remain discoverable.
#[test]
fn test_readmes_separate_minimal_and_optional_dependencies() {
    for (readme, installation, optional) in [
        (README_EN, "Installation", "Optional Integrations"),
        (README_ZH, "安装", "可选集成"),
    ] {
        let installation = h2_section(readme, installation)
            .expect("README should have an installation section");
        assert!(installation.contains("qubit-dcl = \"0.12\""));
        assert!(!installation.contains("qubit-atomic"));
        assert!(!installation.contains("qubit-lock ="));
        assert!(!installation.contains("parking_lot ="));

        let optional = h2_section(readme, optional)
            .expect("README should have an optional integrations section");
        assert!(optional.contains("qubit-atomic"));
        assert!(optional.contains("qubit-lock"));
        assert!(optional.contains("parking_lot = \"0.12\""));
    }
}

/// Verifies both user guides cover every stable outcome and panic type.
#[test]
fn test_user_guides_cover_stable_outcome_and_panic_types() {
    for guide in [USER_GUIDE_EN, USER_GUIDE_ZH] {
        for symbol in [
            "ExecutionOutcome",
            "LifecycleOutcome",
            "FinalizationOutcome",
            "RollbackCause",
            "CapturedLifecycleOutcome",
            "CapturedFinalizationOutcome",
            "PanicInfo",
            "PanicPhase",
        ] {
            assert!(guide.contains(symbol), "missing {symbol}");
        }
    }
}

/// Verifies the optional parking-lot lock implementation remains opt-in.
#[test]
fn test_manifest_exposes_parking_lot_as_an_opt_in_feature() {
    assert!(CARGO_TOML.contains("[features]\ndefault = []"));
    assert!(CARGO_TOML.contains("parking-lot = [\"qubit-lock/parking-lot\"]"));
    let dependency = find_dependency_spec(CARGO_TOML, "qubit-lock")
        .expect("Cargo.toml should declare qubit-lock");
    assert!(dependency.contains("default-features = false"));
    assert!(dependency.contains("version = \"0.13\""));
    assert!(!dependency.contains("path ="));
}

/// Verifies the manifest retains only the lock dependency required at runtime.
#[test]
fn test_manifest_uses_only_required_qubit_dependencies() {
    assert!(find_dependency_spec(CARGO_TOML, "qubit-function").is_none());

    let lock = find_dependency_spec(CARGO_TOML, "qubit-lock")
        .expect("Cargo.toml should declare qubit-lock");
    assert!(lock.contains("version = \"0.13\""));
    assert!(!lock.contains("path ="));
}

/// Verifies optional integration snippets match manifest dependency versions.
#[test]
fn test_readmes_align_lock_feature_and_dependency_versions() {
    for readme in [README_EN, README_ZH] {
        assert!(readme.contains(
            "qubit-dcl = { version = \"0.12\", features = [\"parking-lot\"] }"
        ));
        assert!(readme.contains("qubit-dcl = \"0.12\""));
    }
    for guide in [USER_GUIDE_EN, USER_GUIDE_ZH] {
        assert!(guide.contains("qubit-atomic = \"0.16\""));
        assert!(guide.contains(
            "qubit-lock = { version = \"0.13\", default-features = false }"
        ));
    }
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

/// Verifies public documentation does not retain migration guidance.
#[test]
fn test_public_docs_omit_migration_guidance() {
    for document in [README_EN, README_ZH, USER_GUIDE_EN, USER_GUIDE_ZH] {
        assert!(!document.contains("migration"));
        assert!(!document.contains("迁移"));
    }
}

/// Verifies README files route readers to the matching-language user guide and
/// both guides cover the public executor and outcome types.
#[test]
fn test_readmes_link_user_guides() {
    assert!(README_EN.contains("doc/user_guide.md"));
    assert!(README_ZH.contains("doc/user_guide.zh_CN.md"));

    for guide in [USER_GUIDE_EN, USER_GUIDE_ZH] {
        assert!(guide.contains("DclExecutor"));
        assert!(guide.contains("LifecycleDclExecutor"));
        assert!(guide.contains("ExecutionOutcome"));
        assert!(guide.contains("LifecycleOutcome"));
        assert!(guide.contains("PanicPhase"));
    }
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
fn find_dependency_spec<'a>(
    content: &'a str,
    package: &str,
) -> Option<&'a str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix(package)?.strip_prefix(" = "))
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

/// Returns the content of a named level-two Markdown section.
///
/// # Parameters
///
/// * `content` - Markdown document contents.
/// * `heading` - Heading text without the `## ` prefix.
///
/// # Returns
///
/// The section body up to the next level-two heading, or `None` when absent.
fn h2_section<'a>(content: &'a str, heading: &str) -> Option<&'a str> {
    let marker = format!("## {heading}\n");
    let start = content.find(&marker)? + marker.len();
    let remainder = &content[start..];
    let end = remainder.find("\n## ").unwrap_or(remainder.len());
    Some(&remainder[..end])
}
