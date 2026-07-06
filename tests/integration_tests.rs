// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use gitlance::test_utils::*;
    use tempfile::TempDir;

    /// Runs the binary with specific check and arguments (for integration testing).
    /// Arguments can be omitted (None) to test error cases.
    fn run_check(
        check: Option<&str>,
        repo_path: &str,
        base: Option<&str>,
        head: Option<&str>,
    ) -> bool {
        use assert_cmd::Command;

        let mut cmd = Command::cargo_bin("gitlance").expect("Failed to find binary");

        if let Some(c) = check {
            cmd.arg(c);
        }

        cmd.args(["--repo", repo_path]);

        if let Some(r) = base {
            cmd.args(["--base", r]);
        }
        if let Some(r) = head {
            cmd.args(["--head", r]);
        }

        cmd.ok().is_ok()
    }

    // ===== All Checks Tests =====

    #[test]
    fn test_all_checks_pass() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base_sha = create_commit(&repo_path, "initial");
        let message = "feat: add feature\n\nSigned-off-by: Test User <test@example.com>";
        let sha1 = create_commit(&repo_path, message);

        assert!(
            run_check(Some("all"), &repo_path, Some(&base_sha), Some(&sha1)),
            "Expected all checks to pass"
        );
    }

    #[test]
    fn test_all_checks_fail_one() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base_sha = create_commit(&repo_path, "initial");
        // Missing Signed-off-by but has valid conventional format
        let sha1 = create_commit(&repo_path, "feat: add feature");

        assert!(
            !run_check(Some("all"), &repo_path, Some(&base_sha), Some(&sha1)),
            "Expected all checks to fail when one check fails"
        );
    }

    // ===== Error Handling Tests =====

    #[test]
    fn test_missing_base_ref() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let _ = create_commit(&repo_path, "initial");

        assert!(
            !run_check(None, &repo_path, None, Some("abc123")),
            "Expected check to fail without base ref"
        );
    }

    #[test]
    fn test_missing_head_ref() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let _ = create_commit(&repo_path, "initial");

        assert!(
            !run_check(None, &repo_path, Some("abc123"), None),
            "Expected check to fail without head ref"
        );
    }

    #[test]
    fn test_invalid_ref() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let base = create_commit(&repo_path, "initial");

        assert!(
            !run_check(None, &repo_path, Some(&base), Some("invalid")),
            "Expected check to fail with invalid ref"
        );
    }

    #[test]
    fn test_default_command_is_all() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base_sha = create_commit(&repo_path, "initial");
        let message = "feat: add feature\n\nSigned-off-by: Test User <test@example.com>";
        let sha1 = create_commit(&repo_path, message);

        // Run without explicit subcommand - should still succeed with default "all"
        assert!(
            run_check(None, &repo_path, Some(&base_sha), Some(&sha1)),
            "Expected default to 'all' checks"
        );
    }

    // ===== Individual Check Command Tests =====

    #[test]
    fn test_wip_fixup_command() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base_sha = create_commit(&repo_path, "initial");
        let sha1 = create_commit(&repo_path, "feat: normal commit");

        assert!(
            run_check(Some("wip-fixup"), &repo_path, Some(&base_sha), Some(&sha1)),
            "Expected wip-fixup check to pass for normal commit"
        );
    }

    #[test]
    fn test_signed_off_by_command() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base_sha = create_commit(&repo_path, "initial");
        let message = "commit message\n\nSigned-off-by: Test User <test@example.com>";
        let sha1 = create_commit(&repo_path, message);

        assert!(
            run_check(
                Some("signed-off-by"),
                &repo_path,
                Some(&base_sha),
                Some(&sha1)
            ),
            "Expected signed-off-by check to pass"
        );
    }

    #[test]
    fn test_conventional_commits_command() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base_sha = create_commit(&repo_path, "initial");
        let sha1 = create_commit(&repo_path, "feat: add feature");

        assert!(
            run_check(
                Some("conventional-commits"),
                &repo_path,
                Some(&base_sha),
                Some(&sha1)
            ),
            "Expected conventional-commits check to pass"
        );
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_empty_commit_range() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base_sha = create_commit(&repo_path, "initial");

        // Using the same SHA for base and head should result in no commits
        assert!(
            run_check(None, &repo_path, Some(&base_sha), Some(&base_sha)),
            "Expected success with empty commit range"
        );
    }

    #[test]
    fn test_invalid_repository_path() {
        assert!(
            !run_check(
                None,
                "/nonexistent/repo/path",
                Some("abc123"),
                Some("def456")
            ),
            "Expected check to fail with invalid repository path"
        );
    }

    #[test]
    fn test_head_reference_as_head() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let base = create_commit(&repo_path, "initial");
        let message = "feat: add feature\n\nSigned-off-by: Test User <test@example.com>";
        let _sha1 = create_commit(&repo_path, message);

        assert!(
            run_check(Some("all"), &repo_path, Some(&base), Some("HEAD")),
            "Expected check to pass with HEAD reference"
        );
    }

    #[test]
    fn test_head_tilde_reference() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let _base = create_commit(&repo_path, "initial");
        let message1 = "feat: feature 1\n\nSigned-off-by: Test User <test@example.com>";
        let _sha1 = create_commit(&repo_path, message1);
        let message2 = "feat: feature 2\n\nSigned-off-by: Test User <test@example.com>";
        let _sha2 = create_commit(&repo_path, message2);

        assert!(
            run_check(Some("all"), &repo_path, Some("HEAD~2"), Some("HEAD")),
            "Expected check to pass with HEAD~2 and HEAD references"
        );
    }
}
