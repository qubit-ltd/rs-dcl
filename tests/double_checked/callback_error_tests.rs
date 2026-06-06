// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow explicit-imports
#[cfg(test)]
mod tests {
    use qubit_dcl::double_checked::CallbackError;

    mod test_callback_error {
        use super::*;

        #[test]
        fn test_from_display_is_untyped() {
            let error = CallbackError::from_display("callback failed");

            assert_eq!(error.message(), "callback failed");
            assert_eq!(error.callback_type(), None);
            assert!(!error.is_typed());
        }

        #[test]
        fn test_with_callback_type_records_callback_type() {
            let error =
                CallbackError::with_callback_type("prepare", "callback failed");

            assert_eq!(error.message(), "callback failed");
            assert_eq!(error.callback_type(), Some("prepare"));
            assert!(error.is_typed());
        }
    }
}
