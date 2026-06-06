// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

const SOURCES: &[(&str, &str)] = &[
    (
        "double_checked_lock.rs",
        include_str!("../../src/double_checked/double_checked_lock.rs"),
    ),
    (
        "double_checked_lock_builder.rs",
        include_str!("../../src/double_checked/double_checked_lock_builder.rs"),
    ),
    (
        "double_checked_lock_executor.rs",
        include_str!(
            "../../src/double_checked/double_checked_lock_executor.rs"
        ),
    ),
    (
        "double_checked_lock_ready_builder.rs",
        include_str!(
            "../../src/double_checked/double_checked_lock_ready_builder.rs"
        ),
    ),
    (
        "executor_builder.rs",
        include_str!("../../src/double_checked/executor_builder.rs"),
    ),
    (
        "executor_lock_builder.rs",
        include_str!("../../src/double_checked/executor_lock_builder.rs"),
    ),
    (
        "executor_ready_builder.rs",
        include_str!("../../src/double_checked/executor_ready_builder.rs"),
    ),
];

const MUST_USE_RETURN_TYPES: &[&str] = &[
    "-> Self",
    "-> ExecutorBuilder",
    "-> ExecutorLockBuilder",
    "-> ExecutorReadyBuilder",
    "-> DoubleCheckedLockBuilder",
    "-> DoubleCheckedLockReadyBuilder",
    "-> DoubleCheckedLockExecutor",
];

#[test]
fn test_builder_value_returning_methods_are_must_use() {
    let missing = find_missing_must_use_methods();

    assert!(
        missing.is_empty(),
        "builder value-returning methods must be #[must_use]:\n{}",
        missing.join("\n")
    );
}

fn find_missing_must_use_methods() -> Vec<String> {
    SOURCES
        .iter()
        .flat_map(|(file_name, source)| {
            find_missing_must_use_methods_in_source(file_name, source)
        })
        .collect()
}

fn find_missing_must_use_methods_in_source(
    file_name: &str,
    source: &str,
) -> Vec<String> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut missing = Vec::new();

    for (line_index, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with("pub fn ") {
            continue;
        }
        let signature = collect_signature(&lines, line_index);
        if !returns_must_use_value(&signature)
            || has_must_use_attr(&lines, line_index)
        {
            continue;
        }
        missing.push(format!(
            "{}:{}::{}",
            file_name,
            line_index + 1,
            function_name(&signature)
        ));
    }
    missing
}

fn collect_signature(lines: &[&str], start_index: usize) -> String {
    let mut signature = String::new();
    for line in &lines[start_index..] {
        signature.push_str(line.trim());
        signature.push(' ');
        if line.contains('{') {
            break;
        }
    }
    signature
}

fn returns_must_use_value(signature: &str) -> bool {
    MUST_USE_RETURN_TYPES
        .iter()
        .any(|return_type| signature.contains(return_type))
}

fn has_must_use_attr(lines: &[&str], function_line: usize) -> bool {
    let mut index = function_line;
    while index > 0 {
        index -= 1;
        let line = lines[index].trim();
        if line.starts_with("#[must_use") {
            return true;
        }
        if line.is_empty() || line.starts_with("///") || line.starts_with("#[")
        {
            continue;
        }
        break;
    }
    false
}

fn function_name(signature: &str) -> &str {
    let Some(name_start) = signature.find("pub fn ") else {
        return "<unknown>";
    };
    let name = &signature[name_start + "pub fn ".len()..];
    let name_end = name
        .find(|ch: char| ch == '(' || ch == '<' || ch.is_whitespace())
        .unwrap_or(name.len());
    &name[..name_end]
}
