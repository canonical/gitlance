// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use crate::git::Commit;
use regex::Regex;

/// Checks if commits indicate WIP, fixup, squash, or amend status.
///
/// Pattern matches:
/// - "fixup! ..." (git fixup prefix)
/// - "squash! ..." (git squash prefix)
/// - "amend! ..." (git amend prefix)
/// - "WIP ..." or "WIP:" (work in progress prefix)
/// - "wip ..." or "wip:" (lowercase variant)
///
/// Returns a Vec of (sha, reason) tuples for commits that FAILED the check.
/// An empty Vec means all commits passed.
pub fn check_commits(commits: &[Commit]) -> Vec<(String, String)> {
    // The last WIP$ catches exactly "WIP" alone
    let wip_pattern = Regex::new(r"^(fixup!|squash!|amend!|[Ww][Ii][Pp][ :]|WIP$)")
        .expect("Failed to compile WIP regex");

    let mut failures = Vec::new();

    for commit in commits {
        let first_line = commit.message.lines().next().unwrap_or("");

        if wip_pattern.is_match(first_line) {
            failures.push((
                commit.sha.clone(),
                format!("WIP/fixup/squash/amend commit: {}", first_line),
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
    fn test_wip_fixup_pass() {
        let (_temp, repo, shas) =
            setup_repo_with_commits(&["initial", "feat: add new feature", "fix: resolve bug"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[2], check_commits);
        assert!(
            failures.is_empty(),
            "Expected no failures, got: {:?}",
            failures
        );
    }

    #[test]
    fn test_wip_fixup_fail_fixup() {
        let (_temp, repo, shas) = setup_repo_with_commits(&[
            "initial",
            "feat: initial feature",
            "fixup! feat: initial feature",
        ]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[2], check_commits);
        assert_eq!(failures.len(), 1, "Expected 1 failure");
        assert!(failures[0].1.contains("fixup"));
    }

    #[test]
    fn test_wip_fixup_fail_squash() {
        let (_temp, repo, shas) = setup_repo_with_commits(&["initial", "squash! previous commit"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert_eq!(failures.len(), 1, "Expected 1 failure");
        assert!(failures[0].1.contains("squash"));
    }

    #[test]
    fn test_wip_fixup_fail_wip() {
        let (_temp, repo, shas) = setup_repo_with_commits(&["initial", "WIP: work in progress"]);

        let failures = check_commits_in_range(&repo, &shas[0], &shas[1], check_commits);
        assert_eq!(failures.len(), 1, "Expected 1 failure");
        assert!(failures[0].1.contains("WIP"));
    }
}
