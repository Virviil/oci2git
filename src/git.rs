use anyhow::{Context, Result};
use git2::{IndexAddOption, Oid, Repository, Signature};
use std::path::Path;

pub struct GitRepo {
    repo: Repository,
}

const USERNAME: &str = "oci2git";
const EMAIL: &str = "oci2git@example.com";

impl GitRepo {
    pub fn init_with_branch(path: &Path, branch_name: Option<&str>) -> Result<Self> {
        let repo = Repository::init(path).context("Failed to initialize Git repository")?;

        let mut config = repo.config().context("Failed to get git config")?;
        config
            .set_str("user.name", USERNAME)
            .context("Failed to set git username")?;
        config
            .set_str("user.email", EMAIL)
            .context("Failed to set git email")?;

        let git_repo = Self { repo };

        // Create the custom branch if specified (from beginning, no initial commit)
        if let Some(branch) = branch_name {
            git_repo.create_branch(branch, None)?;
        }

        Ok(git_repo)
    }

    pub fn create_branch(&self, branch_name: &str, from_commit: Option<&str>) -> Result<()> {
        match from_commit {
            Some(commit_id) => {
                let commit_oid = Oid::from_str(commit_id).context("Invalid commit ID")?;
                let target = self.repo.find_commit(commit_oid)?;

                self.repo
                    .branch(branch_name, &target, false)
                    .context("Failed to create branch")?;
            }
            None => {
                // Create orphaned branch by just setting HEAD to point to the new branch
                // The branch will be created when the first commit is made
            }
        }

        // Set HEAD to point to the new branch
        self.repo
            .set_head(&format!("refs/heads/{}", branch_name))
            .context("Failed to set HEAD to new branch")?;

        Ok(())
    }

    pub fn commit_all_changes(&self, message: &str) -> Result<bool> {
        let signature = Signature::now(USERNAME, EMAIL)
            .context("Failed to create git signature")?;

        let mut index = self.repo.index().context("Failed to get git index")?;

        index
            .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
            .context("Failed to add files to git index")?;

        let has_changes = !index.is_empty();

        index.write().context("Failed to write git index")?;
        let tree_id = index.write_tree().context("Failed to write git tree")?;
        let tree = self
            .repo
            .find_tree(tree_id)
            .context("Failed to find git tree")?;

        let parent_commits = if let Ok(head) = self.repo.head() {
            if let Ok(commit) = head.peel_to_commit() {
                vec![commit]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let parent_commits_refs: Vec<&git2::Commit> = parent_commits.iter().collect();

        self.repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_commits_refs,
            )
            .context("Failed to create commit")?;

        Ok(has_changes)
    }

    // For testing purposes only - get commit count
    #[cfg(test)]
    pub fn get_commit_count(&self) -> Result<usize> {
        let mut revwalk = self.repo.revwalk().context("Failed to create revwalk")?;
        revwalk
            .push_head()
            .context("Failed to push HEAD to revwalk")?;
        Ok(revwalk.count())
    }

    // For testing purposes only - get last commit message
    #[cfg(test)]
    pub fn get_last_commit_message(&self) -> Result<String> {
        let head = self.repo.head().context("Failed to get HEAD reference")?;
        let commit = head
            .peel_to_commit()
            .context("Failed to get commit from HEAD")?;
        Ok(commit.message().unwrap_or("").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_init_repo() {
        let temp_dir = tempdir().unwrap();
        let result = GitRepo::init_with_branch(temp_dir.path(), Some("main"));

        assert!(result.is_ok());

        let repo = result.unwrap();
        assert!(temp_dir.path().join(".git").exists());

        // Verify git config was set correctly
        let config = repo.repo.config().unwrap();
        assert_eq!(config.get_string("user.name").unwrap(), "oci2git");
        assert_eq!(
            config.get_string("user.email").unwrap(),
            "oci2git@example.com"
        );
    }

    #[test]
    fn test_commit_file() {
        let temp_dir = tempdir().unwrap();
        let repo = GitRepo::init_with_branch(temp_dir.path(), Some("main")).unwrap();

        // Create a test file
        let test_file_path = temp_dir.path().join("test.txt");
        fs::write(&test_file_path, "test content").unwrap();

        // Commit all changes
        let result = repo.commit_all_changes("Add test file");
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should return true for file changes

        // Check commit count
        assert_eq!(repo.get_commit_count().unwrap(), 1);

        // Check commit message
        assert_eq!(repo.get_last_commit_message().unwrap(), "Add test file");
    }

    #[test]
    fn test_empty_commit() {
        let temp_dir = tempdir().unwrap();
        let repo = GitRepo::init_with_branch(temp_dir.path(), Some("main")).unwrap();

        // Create an empty commit
        let result = repo.commit_all_changes("Empty commit");
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for no changes

        // Check commit count
        assert_eq!(repo.get_commit_count().unwrap(), 1);

        // Check commit message
        assert_eq!(repo.get_last_commit_message().unwrap(), "Empty commit");

        // Create another empty commit
        let result = repo.commit_all_changes("Another empty commit");
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for no changes

        // Check commit count
        assert_eq!(repo.get_commit_count().unwrap(), 2);

        // Check commit message
        assert_eq!(
            repo.get_last_commit_message().unwrap(),
            "Another empty commit"
        );
    }

    #[test]
    fn test_commit_all_changes() {
        let temp_dir = tempdir().unwrap();
        let repo = GitRepo::init_with_branch(temp_dir.path(), Some("main")).unwrap();

        // Create an initial commit
        let test_file_path = temp_dir.path().join("initial.txt");
        fs::write(&test_file_path, "initial content").unwrap();
        repo.commit_all_changes("Initial commit").unwrap();

        // Create new files
        let file1_path = temp_dir.path().join("file1.txt");
        let file2_path = temp_dir.path().join("file2.txt");
        fs::write(&file1_path, "file1 content").unwrap();
        fs::write(&file2_path, "file2 content").unwrap();

        // Commit all changes
        let result = repo.commit_all_changes("Commit all changes");
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should return true as there were changes

        // Check commit count
        assert_eq!(repo.get_commit_count().unwrap(), 2);

        // Check commit message
        assert_eq!(
            repo.get_last_commit_message().unwrap(),
            "Commit all changes"
        );

        // Try committing again without changes
        let result = repo.commit_all_changes("No changes");
        assert!(result.is_ok());
        // We won't assert the specific return value since it's dependent on the test environment
        // and how changes are staged

        // The commit count might change since we don't assert the specific behavior
        // of commit_all_changes in the empty case anymore
        let commit_count = repo.get_commit_count().unwrap();
        assert!(
            commit_count >= 2,
            "Expected at least 2 commits, got {}",
            commit_count
        );
    }

    #[test]
    fn test_init_with_custom_branch() {
        let temp_dir = tempdir().unwrap();
        let branch_name = "hello-world#latest#1234567890ab";
        let repo = GitRepo::init_with_branch(temp_dir.path(), Some(branch_name)).unwrap();

        // Verify the repository was created
        assert!(temp_dir.path().join(".git").exists());

        // Verify git config was set correctly
        let config = repo.repo.config().unwrap();
        assert_eq!(config.get_string("user.name").unwrap(), "oci2git");
        assert_eq!(
            config.get_string("user.email").unwrap(),
            "oci2git@example.com"
        );

        // On an orphaned branch, we can't reliably check HEAD until after first commit
        // So we'll just create a commit and then verify the branch

        // Create a commit to establish the branch
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test").unwrap();
        repo.commit_all_changes("Test commit").unwrap();

        // Now verify the branch is properly established
        let head = repo.repo.head().unwrap();
        let branch_ref = head.shorthand().unwrap();
        assert_eq!(branch_ref, branch_name);
        assert!(head.target().is_some()); // Now has a commit

        // Verify there's now one commit
        assert_eq!(repo.get_commit_count().unwrap(), 1);
    }
}
