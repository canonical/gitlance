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

impl Commit {
    /// Creates a Commit from a raw commit message (no SHA available).
    ///
    /// Comment lines are stripped, matching how git treats a commit message.
    pub fn from_message(message: String) -> Self {
        Self {
            sha: "-".to_string(),
            message: strip_comments(&message),
        }
    }
}

/// The git "scissors" line. Everything from this line onward is discarded by
/// git when interpreting a commit message (e.g. with `commit --verbose`).
const SCISSORS: &str = "# ------------------------ >8 ------------------------";

/// Strips content the way git interprets a commit message: everything from the
/// scissors line onward is dropped, along with any comment lines (starting with
/// `#`, ignoring leading whitespace).
fn strip_comments(message: &str) -> String {
    message
        .lines()
        .take_while(|line| line.trim_start() != SCISSORS)
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Opens a git repository at the specified path
pub fn open_repo(repo_path: &str) -> Result<Repository, CheckError> {
    Repository::open(repo_path)
        .map_err(|e| CheckError::Repository(format!("Failed to open repository: {}", e)))
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

/// Lists all remote-tracking refs (`refs/remotes/*`) in the repository.
///
/// These represent everything already pushed to or fetched from any remote,
/// and are used to determine which commits are genuinely new.
pub fn remote_tracking_refs(repo: &Repository) -> Result<Vec<String>, CheckError> {
    let refs = repo
        .references_glob("refs/remotes/*")
        .map_err(|e| CheckError::Git(format!("Failed to list remote-tracking refs: {}", e)))?;

    let mut names = Vec::new();
    for reference in refs {
        let reference = reference
            .map_err(|e| CheckError::Git(format!("Failed to read remote-tracking ref: {}", e)))?;
        let name = reference
            .name()
            .map_err(|e| CheckError::Git(format!("Remote-tracking ref has invalid name: {}", e)))?;
        names.push(name.to_string());
    }

    Ok(names)
}

/// Gets all commits reachable from `head` but not from any of `excludes`.
///
/// This is the core commit-selection routine. Each exclude ref is hidden from
/// the revwalk, so the result contains only commits unique to `head`. Passing a
/// single base gives a `base..head` range; passing all remote-tracking refs
/// gives the commits that are not yet on any remote.
///
/// Accepts any valid git revision specification for `head` and each exclude:
/// - Full/short SHAs, branches, tags, HEAD~n, etc.
///
/// If `skip_merge_commits` is true, merge commits (commits with more than one parent)
/// are excluded from the results.
pub fn get_commits_excluding(
    repo: &Repository,
    head: &str,
    excludes: &[impl AsRef<str>],
    skip_merge_commits: bool,
) -> Result<Vec<Commit>, CheckError> {
    let head_oid = resolve_ref(repo, head)?;

    let mut revwalk = repo
        .revwalk()
        .map_err(|e| CheckError::Git(format!("Failed to create revwalk: {}", e)))?;

    // Start from head and walk back
    revwalk
        .push(head_oid)
        .map_err(|e| CheckError::Git(format!("Failed to push head to revwalk: {}", e)))?;

    // Hide each exclude ref so its commits (and their ancestors) are omitted
    for exclude in excludes {
        let exclude = exclude.as_ref();
        let exclude_oid = resolve_ref(repo, exclude)?;
        revwalk.hide(exclude_oid).map_err(|e| {
            CheckError::Git(format!("Failed to hide '{}' in revwalk: {}", exclude, e))
        })?;
    }

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
    get_commits_excluding(repo, head, &[base], skip_merge_commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use tempfile::TempDir;

    #[test]
    fn test_strip_comments_hash_not_at_line_start_is_kept() {
        assert_eq!(strip_comments("fix: issue #42"), "fix: issue #42");
    }

    #[test]
    fn test_strip_comments_indented_hash_is_comment() {
        // Git treats leading-whitespace `#` lines as comments too.
        assert_eq!(strip_comments("feat: x\n   # indented"), "feat: x");
    }

    #[test]
    fn test_strip_comments_all_comments_yields_empty() {
        assert_eq!(strip_comments("# only\n# comments"), "");
    }

    #[test]
    fn test_strip_comments_drops_content_after_scissors() {
        let raw = "feat: subject\n\nBody\n# ------------------------ >8 ------------------------\ndiff --git a/x b/x\nWIP leftover";
        assert_eq!(strip_comments(raw), "feat: subject\n\nBody");
    }

    #[test]
    fn test_strip_comments_indented_scissors_is_honored() {
        let raw =
            "feat: subject\n   # ------------------------ >8 ------------------------\nignored";
        assert_eq!(strip_comments(raw), "feat: subject");
    }

    #[test]
    fn test_from_message_strips_comment_lines() {
        let raw = "# Please enter the commit message\nfeat: subject\n\n# a comment\nBody\n";
        let commit = Commit::from_message(raw.to_string());
        assert_eq!(commit.sha, "-");
        assert_eq!(commit.message, "feat: subject\n\nBody");
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

    #[test]
    fn test_remote_tracking_refs_lists_remote_refs() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let base_sha = create_commit(&repo_path, "initial");

        // Simulate a fetched remote-tracking ref by writing it directly.
        run_cmd(
            &repo_path,
            "git",
            &["update-ref", "refs/remotes/origin/main", &base_sha],
        );

        let refs = remote_tracking_refs(&repo).expect("Failed to list remote-tracking refs");
        assert_eq!(refs, vec!["refs/remotes/origin/main".to_string()]);
    }

    #[test]
    fn test_get_commits_excluding_empty_excludes_includes_root() {
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

        // With no excludes, every commit reachable from head is returned,
        // including the root commit that has no parent.
        let commits = get_commits_excluding(&repo, "HEAD", &[] as &[&str], false)
            .expect("Failed to get commits");

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message.trim(), "second commit");
        assert_eq!(commits[1].message.trim(), "initial");
    }

    #[test]
    fn test_get_commits_excluding_head_fully_excluded_is_empty() {
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

        // Excluding head itself leaves nothing new.
        let commits =
            get_commits_excluding(&repo, "HEAD", &["HEAD"], false).expect("Failed to get commits");

        assert!(commits.is_empty());
    }

    #[test]
    fn test_get_commits_excluding_via_remote_tracking_refs() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = create_test_repo(
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert temp dir path to string"),
        );
        let repo = open_repo(&repo_path).expect("Failed to open repo");

        let _base_sha = create_commit(&repo_path, "initial");
        let sha1 = create_commit(&repo_path, "second commit");
        let _sha2 = create_commit(&repo_path, "third commit");

        // Pretend the first two commits are already published on a remote.
        run_cmd(
            &repo_path,
            "git",
            &["update-ref", "refs/remotes/origin/main", &sha1],
        );

        let excludes = remote_tracking_refs(&repo).expect("Failed to list remote-tracking refs");
        let commits =
            get_commits_excluding(&repo, "HEAD", &excludes, false).expect("Failed to get commits");

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message.trim(), "third commit");
    }
}
