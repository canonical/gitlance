// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use crate::git::Commit;
use regex::Regex;

/// Checks if commits follow the conventional commit format.
///
/// Format: type(scope)?: description
/// Valid types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
/// The ! before : indicates a breaking change
///
/// Returns a Vec of (sha, reason) tuples for commits that FAILED the check.
/// An empty Vec means all commits passed.
pub fn check_commits(commits: &[Commit]) -> Vec<(String, String)> {
    let conventional_pattern = Regex::new(
        r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+\))?!?: .+",
    )
    .expect("Failed to compile conventional commit regex");

    let mut failures = Vec::new();

    for commit in commits {
        let first_line = commit.message.lines().next().unwrap_or("");

        if !conventional_pattern.is_match(first_line) {
            failures.push((
                commit.sha.clone(),
                format!("Invalid conventional commit format: {}", first_line),
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
    fn test_conventional_commits_pass() {
        let (_temp, repo, shas) = setup_repo_with_commits(&[
            "initial",
            "feat: add new feature",
            "fix(api): resolve bug",
            "docs: update README",
        ]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[3], check_commits);
        assert!(
            failures.is_empty(),
            "Expected no failures, got: {:?}",
            failures
        );
    }

    #[test]
    fn test_conventional_commits_fail_no_type() {
        let (_temp, repo, shas) = setup_repo_with_commits(&["initial", "this is not conventional"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert_eq!(failures.len(), 1, "Expected 1 failure");
        assert!(failures[0].1.contains("Invalid conventional commit format"));
    }

    #[test]
    fn test_conventional_commits_fail_invalid_type() {
        let (_temp, repo, shas) =
            setup_repo_with_commits(&["initial", "invalid: this type is not allowed"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert_eq!(failures.len(), 1, "Expected 1 failure");
        assert!(failures[0].1.contains("Invalid conventional commit format"));
    }

    #[test]
    fn test_conventional_commits_pass_with_scope() {
        let (_temp, repo, shas) =
            setup_repo_with_commits(&["initial", "feat(database): add caching layer"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert!(
            failures.is_empty(),
            "Expected no failures, got: {:?}",
            failures
        );
    }

    #[test]
    fn test_conventional_commits_pass_with_breaking_change() {
        let (_temp, repo, shas) =
            setup_repo_with_commits(&["initial", "feat!: breaking change in API"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert!(
            failures.is_empty(),
            "Expected no failures, got: {:?}",
            failures
        );
    }
}
