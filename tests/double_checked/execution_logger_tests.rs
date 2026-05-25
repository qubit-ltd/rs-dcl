/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
// qubit-style: allow explicit-imports
#[cfg(test)]
mod tests {
    use qubit_dcl::double_checked::ExecutionLogger;

    mod test_execution_logger {
        use super::*;

        #[test]
        fn test_execution_logger_creation() {
            let mut logger = ExecutionLogger::default();
            logger.set_unmet_condition(Some(log::Level::Info), "Test message");

            assert_eq!(logger.unmet_condition_level(), Some(log::Level::Info));
            assert_eq!(logger.unmet_condition_message(), "Test message");
            assert_eq!(logger.prepare_failed_message(), "Prepare action failed");
            assert_eq!(logger.prepare_commit_failed_message(), "Prepare commit action failed");
            assert_eq!(
                logger.prepare_rollback_failed_message(),
                "Prepare rollback action failed"
            );
        }

        #[test]
        fn test_execution_logger_debug() {
            let mut logger = ExecutionLogger::default();
            logger.set_unmet_condition(Some(log::Level::Warn), "Warning message");

            let debug_str = format!("{:?}", logger);
            assert!(debug_str.contains("ExecutionLogger"));
            assert!(debug_str.contains("Warn"));
            assert!(debug_str.contains("Warning message"));
        }

        #[test]
        fn test_execution_logger_clone() {
            let mut logger = ExecutionLogger::default();
            logger.set_unmet_condition(Some(log::Level::Error), "Error occurred");

            let cloned = logger.clone();
            assert_eq!(cloned.unmet_condition_level(), logger.unmet_condition_level());
            assert_eq!(cloned.unmet_condition_message(), logger.unmet_condition_message());
        }

        #[test]
        fn test_execution_logger_with_empty_message() {
            let mut logger = ExecutionLogger::default();
            logger.set_unmet_condition(Some(log::Level::Debug), "");

            assert_eq!(logger.unmet_condition_level(), Some(log::Level::Debug));
            assert!(logger.unmet_condition_message().is_empty());
        }

        #[test]
        fn test_execution_logger_with_unicode_message() {
            let mut logger = ExecutionLogger::default();
            logger.set_unmet_condition(Some(log::Level::Info), "测试消息 🚀");

            assert_eq!(logger.unmet_condition_message(), "测试消息 🚀");
        }

        #[test]
        fn test_disabled_execution_logger_skips_all_log_methods() {
            let mut logger = ExecutionLogger::default();
            logger.disable_unmet_condition();
            logger.disable_prepare_failure();
            logger.disable_prepare_commit_failure();
            logger.disable_prepare_rollback_failure();

            logger.log_unmet_condition();
            logger.log_prepare_failed("prepare");
            logger.log_prepare_commit_failed("commit");
            logger.log_prepare_rollback_failed("rollback");

            assert_eq!(logger.unmet_condition_level(), None);
            assert_eq!(logger.prepare_failed_level(), None);
            assert_eq!(logger.prepare_commit_failed_level(), None);
            assert_eq!(logger.prepare_rollback_failed_level(), None);
        }

        #[test]
        fn test_enabled_execution_logger_logs_all_methods() {
            let mut logger = ExecutionLogger::default();
            logger.set_unmet_condition(Some(log::Level::Info), "enabled");

            logger.log_unmet_condition();
            logger.log_prepare_failed("prepare");
            logger.log_prepare_commit_failed("commit");
            logger.log_prepare_rollback_failed("rollback");
        }
    }
}
