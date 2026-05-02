use std::path::Path;

use git2::{IndexAddOption, Repository, Signature};

/// Information about a single commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub timestamp: i64,
}

/// Initialize a git repository at the given path.
/// If a `.git/` directory already exists, opens the existing repo.
/// Otherwise, creates a new repository and makes an initial commit.
pub fn init_repo(path: &Path) -> color_eyre::Result<Repository> {
    let git_dir = path.join(".git");
    if git_dir.exists() {
        let repo = Repository::open(path)?;
        Ok(repo)
    } else {
        let repo = Repository::init(path)?;
        // Stage all existing files and create initial commit
        commit(&repo, "Initial state")?;
        Ok(repo)
    }
}

/// Stage all changes and create a commit with the given message.
pub fn commit(repo: &Repository, message: &str) -> color_eyre::Result<()> {
    let sig = default_signature(repo)?;
    let mut index = repo.index()?;

    // Add all files (new, modified, deleted)
    index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // Check if HEAD exists (first commit has no parent)
    let parent_commit = repo
        .head()
        .ok()
        .and_then(|head| head.peel_to_commit().ok());

    let parents: Vec<&git2::Commit<'_>> = match &parent_commit {
        Some(c) => vec![c],
        None => vec![],
    };

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
    Ok(())
}

/// Revert the last commit by resetting to HEAD~1.
/// Returns the hash of the reverted commit.
pub fn revert_last(repo: &Repository) -> color_eyre::Result<String> {
    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;
    let reverted_hash = head_commit.id().to_string();

    let parent = head_commit.parent(0)?;
    repo.reset(parent.as_object(), git2::ResetType::Hard, None)?;

    Ok(reverted_hash)
}

/// Re-apply a previously reverted commit by cherry-picking it.
pub fn reapply(repo: &Repository, commit_hash: &str) -> color_eyre::Result<()> {
    let oid = git2::Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(oid)?;
    repo.cherrypick(&commit, None)?;

    // Complete the cherry-pick with a commit
    let sig = default_signature(repo)?;
    let mut index = repo.index()?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    let head_commit = repo.head()?.peel_to_commit()?;

    let msg = commit.message().unwrap_or("Redo");
    repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&head_commit])?;
    repo.cleanup_state()?;

    Ok(())
}

/// Get the last `n` commits from the log.
pub fn log(repo: &Repository, n: usize) -> color_eyre::Result<Vec<CommitInfo>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    // Use topological sort — reliable even when commits share the same timestamp
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

    let mut entries = Vec::new();
    for oid in revwalk.take(n) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        entries.push(CommitInfo {
            hash: oid.to_string()[..7].to_string(),
            message: commit.message().unwrap_or("").trim().to_string(),
            timestamp: commit.time().seconds(),
        });
    }

    Ok(entries)
}

/// Check if the repository has uncommitted changes (staged or unstaged).
pub fn has_uncommitted_changes(repo: &Repository) -> color_eyre::Result<bool> {
    let statuses = repo.statuses(None)?;
    Ok(!statuses.is_empty())
}

/// Detect uncommitted changes and commit them as "Manual edit detected".
pub fn commit_manual_edits(repo: &Repository) -> color_eyre::Result<bool> {
    if has_uncommitted_changes(repo)? {
        commit(repo, "Manual edit detected")?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Create a default signature for commits.
fn default_signature(repo: &Repository) -> color_eyre::Result<Signature<'static>> {
    // Try to get from git config, fall back to defaults
    match repo.signature() {
        Ok(sig) => Ok(Signature::now(
            sig.name().unwrap_or("Rustory"),
            sig.email().unwrap_or("rustory@local"),
        )?),
        Err(_) => Ok(Signature::now("Rustory", "rustory@local")?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        // Create a file so the initial commit has content
        std::fs::write(dir.path().join("readme.txt"), "hello").unwrap();
        let repo = init_repo(dir.path()).unwrap();
        (dir, repo)
    }

    #[test]
    fn test_init_repo_creates_git_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();
        let _repo = init_repo(dir.path()).unwrap();
        assert!(dir.path().join(".git").exists());
    }

    #[test]
    fn test_init_repo_creates_initial_commit() {
        let (_dir, repo) = setup_test_repo();
        let entries = log(&repo, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Initial state");
    }

    #[test]
    fn test_init_repo_opens_existing() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();
        let _repo1 = init_repo(dir.path()).unwrap();
        // Second call should just open the existing repo
        let repo2 = init_repo(dir.path()).unwrap();
        let entries = log(&repo2, 10).unwrap();
        assert_eq!(entries.len(), 1); // Still just the initial commit
    }

    #[test]
    fn test_commit_creates_new_entry() {
        let (dir, repo) = setup_test_repo();
        std::fs::write(dir.path().join("new_file.txt"), "new content").unwrap();
        commit(&repo, "Added new file").unwrap();

        let entries = log(&repo, 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "Added new file");
        assert_eq!(entries[1].message, "Initial state");
    }

    #[test]
    fn test_commit_modified_file() {
        let (dir, repo) = setup_test_repo();
        std::fs::write(dir.path().join("readme.txt"), "modified").unwrap();
        commit(&repo, "Modified readme").unwrap();

        let entries = log(&repo, 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "Modified readme");
    }

    #[test]
    fn test_revert_last() {
        let (dir, repo) = setup_test_repo();
        std::fs::write(dir.path().join("readme.txt"), "modified").unwrap();
        commit(&repo, "Bad change").unwrap();

        let reverted = revert_last(&repo).unwrap();
        assert!(!reverted.is_empty());

        // File should be restored
        let content = std::fs::read_to_string(dir.path().join("readme.txt")).unwrap();
        assert_eq!(content, "hello");

        // Log should only have initial commit
        let entries = log(&repo, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Initial state");
    }

    #[test]
    fn test_reapply_after_revert() {
        let (dir, repo) = setup_test_repo();
        std::fs::write(dir.path().join("readme.txt"), "modified").unwrap();
        commit(&repo, "A change").unwrap();

        let reverted_hash = revert_last(&repo).unwrap();

        // File should be restored to original
        let content = std::fs::read_to_string(dir.path().join("readme.txt")).unwrap();
        assert_eq!(content, "hello");

        // Re-apply the reverted commit
        reapply(&repo, &reverted_hash).unwrap();

        // File should be modified again
        let content = std::fs::read_to_string(dir.path().join("readme.txt")).unwrap();
        assert_eq!(content, "modified");

        // Log should have entries: reapply + initial
        let entries = log(&repo, 10).unwrap();
        assert!(entries.len() >= 2);
        assert_eq!(entries[0].message, "A change"); // cherry-picked commit message preserved
    }

    #[test]
    fn test_log_returns_entries_in_topological_order() {
        let (dir, repo) = setup_test_repo();

        for i in 0..3 {
            std::fs::write(
                dir.path().join(format!("file{i}.txt")),
                format!("content{i}"),
            )
            .unwrap();
            commit(&repo, &format!("Commit {i}")).unwrap();
        }

        let entries = log(&repo, 10).unwrap();
        assert_eq!(entries.len(), 4); // initial + 3 commits
        assert_eq!(entries[0].message, "Commit 2"); // newest first
        assert_eq!(entries[3].message, "Initial state"); // oldest last
    }

    #[test]
    fn test_log_limits_to_n() {
        let (dir, repo) = setup_test_repo();

        for i in 0..5 {
            std::fs::write(
                dir.path().join(format!("file{i}.txt")),
                format!("content{i}"),
            )
            .unwrap();
            commit(&repo, &format!("Commit {i}")).unwrap();
        }

        let entries = log(&repo, 2).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "Commit 4"); // newest
        assert_eq!(entries[1].message, "Commit 3");
    }

    #[test]
    fn test_has_uncommitted_changes_clean() {
        let (_dir, repo) = setup_test_repo();
        assert!(!has_uncommitted_changes(&repo).unwrap());
    }

    #[test]
    fn test_has_uncommitted_changes_modified() {
        let (dir, repo) = setup_test_repo();
        std::fs::write(dir.path().join("readme.txt"), "changed").unwrap();
        assert!(has_uncommitted_changes(&repo).unwrap());
    }

    #[test]
    fn test_has_uncommitted_changes_new_file() {
        let (dir, repo) = setup_test_repo();
        std::fs::write(dir.path().join("new.txt"), "new").unwrap();
        assert!(has_uncommitted_changes(&repo).unwrap());
    }

    #[test]
    fn test_commit_manual_edits_when_clean() {
        let (_dir, repo) = setup_test_repo();
        let committed = commit_manual_edits(&repo).unwrap();
        assert!(!committed);
    }

    #[test]
    fn test_commit_manual_edits_when_dirty() {
        let (dir, repo) = setup_test_repo();
        std::fs::write(dir.path().join("readme.txt"), "manual edit").unwrap();

        let committed = commit_manual_edits(&repo).unwrap();
        assert!(committed);

        // Should now be clean
        assert!(!has_uncommitted_changes(&repo).unwrap());

        // Verify commit message
        let entries = log(&repo, 1).unwrap();
        assert_eq!(entries[0].message, "Manual edit detected");
    }

    #[test]
    fn test_commit_info_hash_is_abbreviated() {
        let (_dir, repo) = setup_test_repo();
        let entries = log(&repo, 1).unwrap();
        assert_eq!(entries[0].hash.len(), 7);
    }

    #[test]
    fn test_commit_info_has_timestamp() {
        let (_dir, repo) = setup_test_repo();
        let entries = log(&repo, 1).unwrap();
        assert!(entries[0].timestamp > 0);
    }
}
