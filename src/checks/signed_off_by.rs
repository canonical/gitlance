// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use crate::git::Commit;
use regex::Regex;

/// Checks if commits have a valid Signed-off-by trailer.
///
/// Format: Signed-off-by: Name <email@domain>
///
/// Returns a Vec of (sha, reason) tuples for commits that FAILED the check.
/// An empty Vec means all commits passed.
pub fn check_commits(commits: &[Commit]) -> Vec<(String, String)> {
    let signoff_pattern =
        Regex::new(r"^Signed-off-by: .+ <.+@.+>$").expect("Failed to compile Signed-off-by regex");

    let mut failures = Vec::new();

    for commit in commits {
        let mut has_signoff = false;

        // Check if any line in the commit message matches the pattern
        for line in commit.message.lines() {
            if signoff_pattern.is_match(line) {
                has_signoff = true;
                break;
            }
        }

        if !has_signoff {
            failures.push((
                commit.sha.clone(),
                "Missing Signed-off-by trailer".to_string(),
            ));
        }
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_signed_off_by_pass() {
        let (_temp, repo, shas) = setup_repo_with_commits(&[
            "initial",
            "feat: add feature\n\nSigned-off-by: Test User <test@example.com>",
        ]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert!(
            failures.is_empty(),
            "Expected no failures, got: {:?}",
            failures
        );
    }

    #[test]
    fn test_signed_off_by_fail() {
        let (_temp, repo, shas) = setup_repo_with_commits(&["initial", "feat: add feature"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert_eq!(failures.len(), 1, "Expected 1 failure");
        assert!(failures[0].1.contains("Missing Signed-off-by"));
    }

    #[test]
    fn test_signed_off_by_fail_invalid_format() {
        let (_temp, repo, shas) =
            setup_repo_with_commits(&["initial", "feat: add feature\n\nSigned-off-by: Test User"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert_eq!(failures.len(), 1, "Expected 1 failure");
        assert!(failures[0].1.contains("Missing Signed-off-by"));
    }
}
