// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
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
/// Ensures README dependency snippets stay in sync with Cargo.toml (`x.y` in
/// README may abbreviate any `x.y.z…` package version with the same leading
/// `x.y`).
fn test_readme_dependency_version_matches_cargo_toml() {
    let cargo_version = extract_package_version(CARGO_TOML)
        .expect("Failed to extract version from Cargo.toml");
    let readme_en_version =
        extract_readme_dependency_version(README_EN, "qubit-dcl")
            .expect("Failed to extract qubit-dcl version from README.md");
    let readme_zh_version =
        extract_readme_dependency_version(README_ZH, "qubit-dcl")
            .expect("Failed to extract qubit-dcl version from README.zh_CN.md");
    assert!(
        dotted_numeric_version_compatible(readme_en_version, cargo_version),
        "README.md: qubit-dcl version {readme_en_version:?} is not compatible with package version {cargo_version:?} in Cargo.toml"
    );
    assert!(
        dotted_numeric_version_compatible(readme_zh_version, cargo_version),
        "README.zh_CN.md: qubit-dcl version {readme_zh_version:?} is not compatible with package version {cargo_version:?} in Cargo.toml"
    );
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
fn extract_readme_dependency_version<'a>(
    content: &'a str,
    crate_name: &str,
) -> Option<&'a str> {
    let prefix = format!("{} = \"", crate_name);
    for line in content.lines() {
        if let Some(value) = line.trim().strip_prefix(&prefix) {
            return value.strip_suffix('"');
        }
    }
    None
}

/// Compares a README dependency version string with `[package] version` from
/// Cargo.toml.
///
/// When the README uses a two-component `x.y` form, it is accepted if the
/// package version has more components and starts with the same `x.y` (so any
/// `x.y.z`, `x.y.z.w`, … matches). Otherwise the dotted numeric segments must
/// match exactly (same length, same numbers). Non-numeric segments fall back to
/// full string equality.
fn dotted_numeric_version_compatible(readme: &str, package: &str) -> bool {
    fn parse_dotted(v: &str) -> Option<Vec<u32>> {
        let mut out = Vec::new();
        for s in v.split('.') {
            out.push(s.parse().ok()?);
        }
        Some(out)
    }
    match (parse_dotted(readme), parse_dotted(package)) {
        (Some(r), Some(p)) if r.len() == 2 && p.len() > 2 => {
            r[0] == p[0] && r[1] == p[1]
        }
        (Some(r), Some(p)) => r == p,
        _ => readme == package,
    }
}

#[cfg(test)]
mod dotted_numeric_version_compatible_tests {
    use super::dotted_numeric_version_compatible;

    #[test]
    fn two_component_readme_matches_any_longer_package_with_same_prefix() {
        assert!(dotted_numeric_version_compatible("0.3", "0.3.0"));
        assert!(dotted_numeric_version_compatible("0.3", "0.3.1"));
        assert!(dotted_numeric_version_compatible("0.3", "0.3.99"));
        assert!(dotted_numeric_version_compatible("0.3", "0.3.0.0"));
    }

    #[test]
    fn two_component_exact_or_mismatch() {
        assert!(dotted_numeric_version_compatible("0.3", "0.3"));
        assert!(!dotted_numeric_version_compatible("0.3", "0.30"));
        assert!(!dotted_numeric_version_compatible("0.3", "0.4.0"));
    }

    #[test]
    fn full_three_part_must_match_exactly() {
        assert!(dotted_numeric_version_compatible("0.3.0", "0.3.0"));
        assert!(!dotted_numeric_version_compatible("0.3.0", "0.3.1"));
    }
}
