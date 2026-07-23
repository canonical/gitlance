// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

pub mod checks;
pub mod config;
pub mod credential_store;
pub mod error;
pub mod git;
pub mod output;

#[doc(hidden)]
pub mod test_utils;

pub use error::CheckError;
pub use git::{get_commits_in_range, open_repo, resolve_ref};

/// Length to abbreviate SHAs in output messages
const SHA_ABBREV_LEN: usize = 8;

/// Abbreviates a SHA to the first 8 characters (or less if shorter).
fn abbreviate_sha(sha: &str) -> &str {
    &sha[..SHA_ABBREV_LEN.min(sha.len())]
}

/// Runs a check and reports results.
///
/// Takes a check name and a list of failures (sha, reason) tuples.
/// Outputs results to stdout using GitHub Actions annotation format.
///
/// Returns true if all commits passed (failures is empty), false otherwise.
pub fn run_check(check_name: &str, failures: &[(String, String)]) -> bool {
    let passed = failures.is_empty();
    output::result(check_name, passed);

    for (sha, reason) in failures {
        output::error(&format!(
            "[{}] {}: {}",
            check_name,
            abbreviate_sha(sha),
            reason
        ));
    }

    passed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_check_passed() {
        let failures: Vec<(String, String)> = vec![];
        assert!(run_check("Test Check", &failures));
    }

    #[test]
    fn test_run_check_failed() {
        let failures = vec![("abc123".to_string(), "Test failure".to_string())];
        assert!(!run_check("Test Check", &failures));
    }
}
