// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

/// Custom error type for pr-commit-checks operations
#[derive(Debug)]
pub enum CheckError {
    /// Repository-related errors
    Repository(String),
    /// Git operation errors
    Git(String),
    /// Missing or invalid git reference
    InvalidRef(String),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::Repository(msg) => write!(f, "Repository error: {}", msg),
            CheckError::Git(msg) => write!(f, "Git error: {}", msg),
            CheckError::InvalidRef(msg) => write!(f, "Invalid reference: {}", msg),
        }
    }
}

impl std::error::Error for CheckError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_error_repository_display() {
        let err = CheckError::Repository("not found".to_string());
        assert_eq!(err.to_string(), "Repository error: not found");
    }

    #[test]
    fn test_check_error_git_display() {
        let err = CheckError::Git("merge conflict".to_string());
        assert_eq!(err.to_string(), "Git error: merge conflict");
    }

    #[test]
    fn test_check_error_invalid_ref_display() {
        let err = CheckError::InvalidRef("bad ref".to_string());
        assert_eq!(err.to_string(), "Invalid reference: bad ref");
    }

    #[test]
    fn test_check_error_debug() {
        let err = CheckError::Repository("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Repository"));
    }
}
