// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use crate::error::CheckError;
use git2::{Oid, Repository};

/// Represents a single commit with its SHA and message
#[derive(Debug, Clone)]
pub struct Commit {
    pub sha: String,
    pub message: String,
}

/// Opens a git repository at the specified path
pub fn open_repo(repo_path: &str) -> Result<Repository, CheckError> {
    Repository::open(repo_path)
        .map_err(|e| CheckError::Repository(format!("Failed to open repository: {}", e)))
}

/// Parses a SHA string into a git2 Oid
fn parse_oid(sha: &str) -> Result<Oid, CheckError> {
    Oid::from_str(sha).map_err(|e| CheckError::InvalidSha(format!("Invalid SHA '{}': {}", sha, e)))
}

/// Gets all commits in the range [base_sha, head_sha]
/// Returns commits from base (exclusive) to head (inclusive)
///
/// If `skip_merge_commits` is true, merge commits (commits with more than one parent)
/// are excluded from the results.
pub fn get_commits_in_range(
    repo: &Repository,
    base_sha: &str,
    head_sha: &str,
    skip_merge_commits: bool,
) -> Result<Vec<Commit>, CheckError> {
    let base_oid = parse_oid(base_sha)?;
    let head_oid = parse_oid(head_sha)?;

    let mut revwalk = repo
        .revwalk()
        .map_err(|e| CheckError::Git(format!("Failed to create revwalk: {}", e)))?;

    // Start from head and walk back
    revwalk
        .push(head_oid)
        .map_err(|e| CheckError::Git(format!("Failed to push head to revwalk: {}", e)))?;

    // Don't include the base commit itself
    revwalk
        .hide(base_oid)
        .map_err(|e| CheckError::Git(format!("Failed to hide base in revwalk: {}", e)))?;

    let mut commits = Vec::new();

    for oid_result in revwalk {
        let oid = oid_result.map_err(|e| CheckError::Git(format!("Revwalk error: {}", e)))?;

        let commit = repo
            .find_commit(oid)
            .map_err(|e| CheckError::Git(format!("Failed to find commit {}: {}", oid, e)))?;

        if skip_merge_commits && commit.parent_count() > 1 {
            continue;
        }

        let message = commit
            .message()
            .map_err(|e| CheckError::Git(format!("Commit has invalid message encoding: {}", e)))?
            .to_string();

        commits.push(Commit {
            sha: oid.to_string(),
            message,
        });
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use tempfile::TempDir;

    #[test]
    fn test_open_repo_success() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path);
        assert!(repo.is_ok());
    }

    #[test]
    fn test_open_repo_not_found() {
        let repo = open_repo("/nonexistent/path/to/repo");
        assert!(repo.is_err());
        if let Err(err) = repo {
            assert!(err.to_string().contains("Failed to open repository"));
        }
    }

    #[test]
    fn test_parse_oid_valid() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let oid = parse_oid(sha);
        assert!(oid.is_ok());
    }

    #[test]
    fn test_parse_oid_invalid() {
        let sha = "invalid_sha";
        let oid = parse_oid(sha);
        assert!(oid.is_err());
        if let Err(err) = oid {
            assert!(err.to_string().contains("Invalid SHA"));
        }
    }

    #[test]
    fn test_get_commits_in_range_success() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let base_sha = create_commit(&repo_path, "initial");
        let _sha1 = create_commit(&repo_path, "second commit");
        let sha2 = create_commit(&repo_path, "third commit");

        let commits = get_commits_in_range(&repo, &base_sha, &sha2, false);
        assert!(commits.is_ok());

        let commits = commits.expect("Failed to get commits in range");
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message.trim(), "third commit");
        assert_eq!(commits[1].message.trim(), "second commit");
    }

    #[test]
    fn test_get_commits_in_range_skip_merge_commits() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let initial_sha = create_commit(&repo_path, "initial");
        let commit1_sha = create_commit(&repo_path, "commit 1");
        let commit2_sha = create_commit(&repo_path, "commit 2");

        let commit1_oid = Oid::from_str(&commit1_sha).expect("Failed to parse commit1 SHA");
        let commit2_oid = Oid::from_str(&commit2_sha).expect("Failed to parse commit2 SHA");

        let commit1 = repo
            .find_commit(commit1_oid)
            .expect("Failed to find commit1");
        let commit2 = repo
            .find_commit(commit2_oid)
            .expect("Failed to find commit2");

        let tree = commit2.tree().expect("Failed to get tree");
        let sig = repo.signature().expect("Failed to get signature");

        let merge_oid = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Merge commit",
                &tree,
                &[&commit2, &commit1],
            )
            .expect("Failed to create merge commit");

        let merge_sha = merge_oid.to_string();

        let commits_with_merge = get_commits_in_range(&repo, &initial_sha, &merge_sha, false)
            .expect("Failed to get commits with merge");

        assert_eq!(commits_with_merge.len(), 3, "Should include merge commit");
        assert!(
            commits_with_merge[0].message.contains("Merge commit"),
            "First commit should be merge commit"
        );

        let commits_without_merge = get_commits_in_range(&repo, &initial_sha, &merge_sha, true)
            .expect("Failed to get commits without merge");

        assert_eq!(
            commits_without_merge.len(),
            2,
            "Should exclude merge commit"
        );
        assert!(
            !commits_without_merge
                .iter()
                .any(|c| c.message.contains("Merge commit")),
            "Should not contain merge commit"
        );
    }

    #[test]
    fn test_get_commits_in_range_invalid_base_sha() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let _ = create_commit(&repo_path, "initial");

        let commits = get_commits_in_range(&repo, "invalid_sha", "abc123", false);
        assert!(commits.is_err());
        if let Err(err) = commits {
            assert!(err.to_string().contains("Invalid SHA"));
        }
    }

    #[test]
    fn test_get_commits_in_range_invalid_head_sha() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let base_sha = create_commit(&repo_path, "initial");

        let commits = get_commits_in_range(&repo, &base_sha, "invalid_sha", false);
        assert!(commits.is_err());
        if let Err(err) = commits {
            assert!(err.to_string().contains("Invalid SHA"));
        }
    }

    #[test]
    fn test_commit_struct() {
        let commit = Commit {
            sha: "abc123".to_string(),
            message: "test message".to_string(),
        };

        assert_eq!(commit.sha, "abc123");
        assert_eq!(commit.message, "test message");

        // Test Clone
        let commit_clone = commit.clone();
        assert_eq!(commit_clone.sha, commit.sha);
    }
}
