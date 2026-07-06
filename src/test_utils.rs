// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

//! Shared test helpers for building throwaway git repositories.
//!
//! This module is available to both unit tests (in `src/`) and integration
//! tests (in `tests/`) without needing any feature flags or Cargo changes.

use std::process::Command;

/// Creates and initializes a test git repository, returning its path.
/// Takes a directory path (typically from `TempDir::new().path()`).
pub fn create_test_repo(repo_path: &str) -> String {
    let repo_path = repo_path.to_string();

    run_cmd(&repo_path, "git", &["init"]);
    run_cmd(
        &repo_path,
        "git",
        &["config", "user.email", "test@example.com"],
    );
    run_cmd(&repo_path, "git", &["config", "user.name", "Test User"]);
    run_cmd(&repo_path, "git", &["config", "commit.gpgSign", "false"]);

    repo_path
}

/// Runs a command in the given repository and returns its stdout.
/// Panics if the command fails to execute or exits with non-zero status.
pub fn run_cmd(repo_path: &str, cmd: &str, args: &[&str]) -> String {
    let output = Command::new(cmd)
        .current_dir(repo_path)
        .args(args)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "Failed to execute {} {:?} in {}: {}",
                cmd, args, repo_path, e
            )
        });

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "Command {} {:?} exited with status {:?}\nstdout: {}\nstderr: {}",
            cmd,
            args,
            output.status.code(),
            stdout,
            stderr
        );
    }

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Creates an empty commit with the given message and returns its SHA.
pub fn create_commit(repo_path: &str, message: &str) -> String {
    run_cmd(
        repo_path,
        "git",
        &["commit", "--allow-empty", "-m", message],
    );
    let sha = run_cmd(repo_path, "git", &["rev-parse", "HEAD"]);
    sha.trim().to_string()
}

/// Creates a test repository with multiple commits.
/// Returns (TempDir, repo_path, Vec<commit_shas>)
/// The TempDir must be kept alive for the duration of the test.
#[cfg(test)]
pub fn setup_repo_with_commits(messages: &[&str]) -> (tempfile::TempDir, String, Vec<String>) {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_test_repo(
        temp_dir
            .path()
            .to_str()
            .expect("Failed to convert temp dir path to string"),
    );

    let shas: Vec<String> = messages
        .iter()
        .map(|msg| create_commit(&repo_path, msg))
        .collect();

    (temp_dir, repo_path, shas)
}

/// Helper to check commits in a range using a provided check function.
/// Opens the repo, fetches commits, and runs the check.
#[cfg(test)]
pub fn check_commits_in_range<F>(
    repo_path: &str,
    base_sha: &str,
    head_sha: &str,
    check_fn: F,
) -> Vec<(String, String)>
where
    F: FnOnce(&[crate::git::Commit]) -> Vec<(String, String)>,
{
    let repo = crate::git::open_repo(repo_path).expect("Failed to open repo");
    let commits = crate::git::get_commits_in_range(&repo, base_sha, head_sha, false)
        .expect("Failed to get commits");
    check_fn(&commits)
}
