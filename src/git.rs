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

/// Returns the staged changes (index vs. HEAD) as a unified diff string.
///
/// Returns an empty string if nothing is staged. If the repository has no
/// commits yet (unborn HEAD), diffs the index against an empty tree so
/// initial staged additions are still picked up.
pub fn get_staged_diff(repo: &Repository) -> Result<String, CheckError> {
    let head_tree = match repo.head() {
        Ok(head_ref) => Some(
            head_ref
                .peel_to_tree()
                .map_err(|e| CheckError::Git(format!("Failed to peel HEAD to tree: {}", e)))?,
        ),
        Err(e)
            if e.code() == git2::ErrorCode::UnbornBranch
                || e.code() == git2::ErrorCode::NotFound =>
        {
            None
        }
        Err(e) => return Err(CheckError::Git(format!("Failed to read HEAD: {}", e))),
    };

    let diff = repo
        .diff_tree_to_index(head_tree.as_ref(), None, None)
        .map_err(|e| CheckError::Git(format!("Failed to diff index against HEAD: {}", e)))?;

    let mut buf = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let content = String::from_utf8_lossy(line.content());
        if matches!(line.origin(), '+' | '-' | ' ') {
            buf.push(line.origin());
        }
        buf.push_str(&content);
        true
    })
    .map_err(|e| CheckError::Git(format!("Failed to render diff: {}", e)))?;

    Ok(buf)
}

/// Reads the configured `user.name`/`user.email` identity, used for the
/// auto-appended `Signed-off-by` trailer.
pub fn get_git_identity(repo: &Repository) -> Result<(String, String), CheckError> {
    let signature = repo
        .signature()
        .map_err(|e| CheckError::Git(format!("Failed to read git identity: {}", e)))?;

    let name = signature
        .name()
        .map_err(|e| CheckError::Git(format!("user.name is not valid UTF-8: {}", e)))?
        .to_string();
    let email = signature
        .email()
        .map_err(|e| CheckError::Git(format!("user.email is not valid UTF-8: {}", e)))?
        .to_string();

    Ok((name, email))
}

/// Resolves a git reference (SHA, branch, tag, HEAD~n, etc.) to a commit OID.
///
/// Accepts any valid git revision specification:
/// - Full SHAs: `abc123...`
/// - Short SHAs: `abc123`
/// - Symbolic refs: `HEAD`, `HEAD~4`, `HEAD^`, `main`, `origin/main`
/// - Tags: `v1.0.0`
pub fn resolve_ref(repo: &Repository, refspec: &str) -> Result<Oid, CheckError> {
    let object = repo
        .revparse_single(refspec)
        .map_err(|e| CheckError::InvalidRef(format!("Cannot resolve '{}': {}", refspec, e)))?;

    object.peel_to_commit().map(|c| c.id()).map_err(|e| {
        CheckError::InvalidRef(format!("'{}' does not point to a commit: {}", refspec, e))
    })
}

/// Gets all commits in the range [base, head]
/// Returns commits from base (exclusive) to head (inclusive)
///
/// Accepts any valid git revision specification for base and head:
/// - Full/short SHAs, branches, tags, HEAD~n, etc.
///
/// If `skip_merge_commits` is true, merge commits (commits with more than one parent)
/// are excluded from the results.
pub fn get_commits_in_range(
    repo: &Repository,
    base: &str,
    head: &str,
    skip_merge_commits: bool,
) -> Result<Vec<Commit>, CheckError> {
    let base_oid = resolve_ref(repo, base)?;
    let head_oid = resolve_ref(repo, head)?;

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
    fn test_get_staged_diff_empty_when_nothing_staged() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        create_commit(&repo_path, "initial");
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let diff = get_staged_diff(&repo).expect("get_staged_diff failed");
        assert!(diff.is_empty());
    }

    #[test]
    fn test_get_staged_diff_contains_staged_content() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        create_commit(&repo_path, "initial");

        std::fs::write(
            std::path::Path::new(&repo_path).join("new_file.txt"),
            "hello world\n",
        )
        .expect("Failed to write file");
        run_cmd(&repo_path, "git", &["add", "new_file.txt"]);

        let repo = open_repo(&repo_path).expect("Failed to open repo");
        let diff = get_staged_diff(&repo).expect("get_staged_diff failed");

        assert!(diff.contains("new_file.txt"));
        assert!(diff.contains("hello world"));
    }

    #[test]
    fn test_get_staged_diff_no_commits_yet() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );

        std::fs::write(
            std::path::Path::new(&repo_path).join("new_file.txt"),
            "hello world\n",
        )
        .expect("Failed to write file");
        run_cmd(&repo_path, "git", &["add", "new_file.txt"]);

        let repo = open_repo(&repo_path).expect("Failed to open repo");
        let diff = get_staged_diff(&repo).expect("get_staged_diff failed");

        assert!(diff.contains("new_file.txt"));
    }

    #[test]
    fn test_get_staged_diff_includes_non_utf8_content_lossily() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        create_commit(&repo_path, "initial");

        // Invalid UTF-8 bytes (no NUL byte, so git still treats it as text
        // rather than binary) surrounded by valid ASCII.
        let mut content = b"hello ".to_vec();
        content.extend_from_slice(&[0xFF, 0xFE]);
        content.extend_from_slice(b" world\n");
        std::fs::write(
            std::path::Path::new(&repo_path).join("non_utf8.txt"),
            &content,
        )
        .expect("Failed to write file");
        run_cmd(&repo_path, "git", &["add", "non_utf8.txt"]);

        let repo = open_repo(&repo_path).expect("Failed to open repo");
        let diff = get_staged_diff(&repo).expect("get_staged_diff failed");

        assert!(diff.contains("non_utf8.txt"));
        assert!(diff.contains("hello"));
        assert!(diff.contains("world"));
    }

    #[test]
    fn test_get_git_identity() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let (name, email) = get_git_identity(&repo).expect("get_git_identity failed");
        assert_eq!(name, "Test User");
        assert_eq!(email, "test@example.com");
    }

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
    fn test_resolve_ref_full_sha() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let sha = create_commit(&repo_path, "test commit");
        let oid = resolve_ref(&repo, &sha);
        assert!(oid.is_ok());
        assert_eq!(oid.unwrap().to_string(), sha);
    }

    #[test]
    fn test_resolve_ref_short_sha() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let full_sha = create_commit(&repo_path, "test commit");
        let short_sha = &full_sha[..7];
        let oid = resolve_ref(&repo, short_sha);
        assert!(oid.is_ok());
        assert_eq!(oid.unwrap().to_string(), full_sha);
    }

    #[test]
    fn test_resolve_ref_head() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let sha = create_commit(&repo_path, "test commit");
        let oid = resolve_ref(&repo, "HEAD");
        assert!(oid.is_ok());
        assert_eq!(oid.unwrap().to_string(), sha);
    }

    #[test]
    fn test_resolve_ref_head_tilde() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let sha1 = create_commit(&repo_path, "first commit");
        let _sha2 = create_commit(&repo_path, "second commit");
        let oid = resolve_ref(&repo, "HEAD~1");
        assert!(oid.is_ok());
        assert_eq!(oid.unwrap().to_string(), sha1);
    }

    #[test]
    fn test_resolve_ref_invalid() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let oid = resolve_ref(&repo, "invalid_ref");
        assert!(oid.is_err());
        if let Err(err) = oid {
            assert!(err.to_string().contains("Invalid reference"));
        }
    }

    #[test]
    fn test_get_commits_in_range_with_head_refs() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let _base_sha = create_commit(&repo_path, "initial");
        let _sha1 = create_commit(&repo_path, "second commit");
        let _sha2 = create_commit(&repo_path, "third commit");

        let commits = get_commits_in_range(&repo, "HEAD~2", "HEAD", false);
        assert!(commits.is_ok());

        let commits = commits.expect("Failed to get commits in range");
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message.trim(), "third commit");
        assert_eq!(commits[1].message.trim(), "second commit");
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
    fn test_get_commits_in_range_invalid_base_ref() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let _ = create_commit(&repo_path, "initial");

        let commits = get_commits_in_range(&repo, "invalid_ref", "abc123", false);
        assert!(commits.is_err());
        if let Err(err) = commits {
            assert!(err.to_string().contains("Invalid reference"));
        }
    }

    #[test]
    fn test_get_commits_in_range_invalid_head_ref() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let base_sha = create_commit(&repo_path, "initial");

        let commits = get_commits_in_range(&repo, &base_sha, "invalid_ref", false);
        assert!(commits.is_err());
        if let Err(err) = commits {
            assert!(err.to_string().contains("Invalid reference"));
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
