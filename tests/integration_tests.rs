// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use gitlance::test_utils::*;
    use tempfile::TempDir;

    /// Builds and runs the binary with the given arguments, returning the raw
    /// process output. Any argument can be omitted (None) to test error cases.
    fn run(
        check: Option<&str>,
        repo_path: Option<&str>,
        base: Option<&str>,
        head: Option<&str>,
        message_file: Option<&str>,
        not_on_remotes: bool,
    ) -> std::process::Output {
        use assert_cmd::Command;

        let mut cmd = Command::cargo_bin("gitlance").expect("Failed to find binary");

        if let Some(c) = check {
            cmd.arg(c);
        }

        if let Some(path) = repo_path {
            cmd.args(["--repo", path]);
        }

        if let Some(r) = base {
            cmd.args(["--base", r]);
        }
        if let Some(r) = head {
            cmd.args(["--head", r]);
        }
        if let Some(f) = message_file {
            cmd.args(["--message-file", f]);
        }
        if not_on_remotes {
            cmd.arg("--not-on-remotes");
        }

        cmd.output().expect("Failed to run binary")
    }

    /// Runs the binary and reports whether it exited successfully.
    /// Arguments can be omitted (None) to test error cases.
    fn run_check(
        check: Option<&str>,
        repo_path: Option<&str>,
        base: Option<&str>,
        head: Option<&str>,
        message_file: Option<&str>,
    ) -> bool {
        run(check, repo_path, base, head, message_file, false)
            .status
            .success()
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
            run_check(
                Some("all"),
                Some(&repo_path),
                Some(&base_sha),
                Some(&sha1),
                None
            ),
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
            !run_check(
                Some("all"),
                Some(&repo_path),
                Some(&base_sha),
                Some(&sha1),
                None
            ),
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
            !run_check(None, Some(&repo_path), None, Some("abc123"), None),
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
            !run_check(None, Some(&repo_path), Some("abc123"), None, None),
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
            !run_check(None, Some(&repo_path), Some(&base), Some("invalid"), None),
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
            run_check(None, Some(&repo_path), Some(&base_sha), Some(&sha1), None),
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
            run_check(
                Some("wip-fixup"),
                Some(&repo_path),
                Some(&base_sha),
                Some(&sha1),
                None
            ),
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
                Some(&repo_path),
                Some(&base_sha),
                Some(&sha1),
                None,
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
                Some(&repo_path),
                Some(&base_sha),
                Some(&sha1),
                None,
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
            run_check(
                None,
                Some(&repo_path),
                Some(&base_sha),
                Some(&base_sha),
                None
            ),
            "Expected success with empty commit range"
        );
    }

    #[test]
    fn test_invalid_repository_path() {
        assert!(
            !run_check(
                None,
                Some("/nonexistent/repo/path"),
                Some("abc123"),
                Some("def456"),
                None,
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
            run_check(
                Some("all"),
                Some(&repo_path),
                Some(&base),
                Some("HEAD"),
                None
            ),
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
            run_check(
                Some("all"),
                Some(&repo_path),
                Some("HEAD~2"),
                Some("HEAD"),
                None
            ),
            "Expected check to pass with HEAD~2 and HEAD references"
        );
    }

    // ===== Not-on-remotes Tests =====

    #[test]
    fn test_not_on_remotes_checks_only_unpublished_commits() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let published = create_commit(&repo_path, "chore: initial");
        let message = "feat: add feature\n\nSigned-off-by: Test User <test@example.com>";
        let _new = create_commit(&repo_path, message);

        // Mark the first commit as already present on a remote.
        run_cmd(
            &repo_path,
            "git",
            &["update-ref", "refs/remotes/origin/main", &published],
        );

        let output = run(None, Some(&repo_path), None, Some("HEAD"), None, true);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(output.status.success(), "Expected checks to pass");
        assert!(
            stdout.contains("Testing 1 commit"),
            "Expected only the unpublished commit to be checked, got: {}",
            stdout
        );
    }

    #[test]
    fn test_not_on_remotes_passes_when_nothing_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        let head = create_commit(&repo_path, "chore: initial");

        // Every commit is already on a remote.
        run_cmd(
            &repo_path,
            "git",
            &["update-ref", "refs/remotes/origin/main", &head],
        );

        let output = run(None, Some(&repo_path), None, Some("HEAD"), None, true);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(output.status.success(), "Expected a clean pass");
        assert!(
            stdout.contains("No new commits to check"),
            "Expected clean-pass message, got: {}",
            stdout
        );
    }

    // ===== Message File Tests =====

    #[test]
    fn test_message_file_all_checks_pass() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("COMMIT_EDITMSG");
        let message = "feat: add feature\n\nSigned-off-by: Test User <test@example.com>";
        std::fs::write(&file_path, message).expect("Failed to write message file");

        assert!(
            run_check(
                Some("all"),
                None,
                None,
                None,
                Some(file_path.to_str().unwrap())
            ),
            "Expected all checks to pass with message file"
        );
    }

    #[test]
    fn test_message_file_wip_check_fails() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("COMMIT_EDITMSG");
        std::fs::write(&file_path, "WIP add feature").expect("Failed to write message file");

        assert!(
            !run_check(
                Some("wip-fixup"),
                None,
                None,
                None,
                Some(file_path.to_str().unwrap())
            ),
            "Expected WIP check to fail"
        );
    }

    #[test]
    fn test_message_file_nonexistent() {
        assert!(
            !run_check(
                Some("all"),
                None,
                None,
                None,
                Some("/nonexistent/path/COMMIT_EDITMSG")
            ),
            "Expected failure with nonexistent message file"
        );
    }

    #[test]
    fn test_message_file_empty_is_rejected() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("COMMIT_EDITMSG");
        std::fs::write(&file_path, "# only a comment\n\n   \n")
            .expect("Failed to write message file");

        assert!(
            !run_check(
                Some("all"),
                None,
                None,
                None,
                Some(file_path.to_str().unwrap())
            ),
            "Expected failure with empty commit message"
        );
    }
}
