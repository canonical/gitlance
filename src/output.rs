// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

/// GitHub Actions annotation levels
#[derive(Debug, Clone, Copy)]
pub enum AnnotationLevel {
    Notice,
    Error,
}

impl AnnotationLevel {
    fn as_str(&self) -> &'static str {
        match self {
            AnnotationLevel::Notice => "notice",
            AnnotationLevel::Error => "error",
        }
    }
}

/// Outputs a GitHub Actions annotation
/// Format: ::level::message
pub fn annotate(level: AnnotationLevel, message: &str) {
    println!(
        "::{level}::{message}",
        level = level.as_str(),
        message = message
    );
}

/// Outputs an error annotation
pub fn error(message: &str) {
    annotate(AnnotationLevel::Error, message);
}

/// Outputs a notice annotation
pub fn notice(message: &str) {
    annotate(AnnotationLevel::Notice, message);
}

/// Outputs a check result summary
/// Used for final status reporting
pub fn result(check_name: &str, passed: bool) {
    let status = if passed { "✓ PASSED" } else { "✗ FAILED" };
    println!("[{}] {}", check_name, status);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_level_error_as_str() {
        assert_eq!(AnnotationLevel::Error.as_str(), "error");
    }

    #[test]
    fn test_annotation_level_notice_as_str() {
        assert_eq!(AnnotationLevel::Notice.as_str(), "notice");
    }

    #[test]
    fn test_error_function() {
        // This function calls annotate with Error level
        // We're testing it executes without panic
        error("test error message");
    }

    #[test]
    fn test_notice_function() {
        // This function calls annotate with Notice level
        notice("test notice message");
    }

    #[test]
    fn test_result_passed() {
        result("Test Check", true);
    }

    #[test]
    fn test_result_failed() {
        result("Test Check", false);
    }
}
