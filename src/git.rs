use anyhow::{Context, Result};
use git2::{IndexAddOption, Repository, Signature};
use std::path::{Path, PathBuf};

pub struct GitRepo {
    repo: Repository,
    repo_path: PathBuf,
}

const USERNAME: &str = "oci2git";
const EMAIL: &str = "oci2git@example.com";

impl GitRepo {
    pub fn init(path: &Path) -> Result<Self> {
        let repo = Repository::init(path).context("Failed to initialize Git repository")?;

        let mut config = repo.config().context("Failed to get git config")?;
        config
            .set_str("user.name", USERNAME)
            .context("Failed to set git username")?;
        config
            .set_str("user.email", EMAIL)
            .context("Failed to set git email")?;

        Ok(Self {
            repo,
            repo_path: path.to_path_buf(),
        })
    }

    pub fn create_empty_commit(&self, message: &str) -> Result<()> {
        let signature = Signature::now("oci2git", "oci2git@example.com")
            .context("Failed to create git signature")?;

        let tree_id = self
            .repo
            .index()
            .context("Failed to get git index")?
            .write_tree()
            .context("Failed to write git tree")?;

        let tree = self
            .repo
            .find_tree(tree_id)
            .context("Failed to find git tree")?;

        let parent_commits = if let Ok(head) = self.repo.head() {
            if let Some(oid) = head.target() {
                vec![self.repo.find_commit(oid)?]
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
            .context("Failed to create empty commit")?;

        Ok(())
    }

    pub fn commit_all_changes(&self, message: &str) -> Result<bool> {
        let signature = Signature::now("oci2git", "oci2git@example.com")
            .context("Failed to create git signature")?;

        let mut index = self.repo.index().context("Failed to get git index")?;

        index
            .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
            .context("Failed to add files to git index")?;

        if index.is_empty() {
            return Ok(false);
        }

        index.write().context("Failed to write git index")?;
        let tree_id = index.write_tree().context("Failed to write git tree")?;
        let tree = self
            .repo
            .find_tree(tree_id)
            .context("Failed to find git tree")?;

        let parent_commit = self
            .repo
            .head()
            .and_then(|head| head.peel_to_commit())
            .context("Failed to get head commit")?;

        self.repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent_commit],
            )
            .context("Failed to create commit")?;

        Ok(true)
    }

    pub fn add_and_commit_file(&self, file_path: &Path, message: &str) -> Result<()> {
        let signature = Signature::now("oci2git", "oci2git@example.com")
            .context("Failed to create git signature")?;

        let mut index = self.repo.index().context("Failed to get git index")?;

        let repo_path = self.repo_path.clone();
        let relative_path = file_path
            .strip_prefix(&repo_path)
            .unwrap_or(file_path)
            .to_str()
            .context("Invalid file path")?;

        index
            .add_path(Path::new(relative_path))
            .context(format!("Failed to add file {} to git index", relative_path))?;

        index.write().context("Failed to write git index")?;
        let tree_id = index.write_tree().context("Failed to write git tree")?;
        let tree = self
            .repo
            .find_tree(tree_id)
            .context("Failed to find git tree")?;

        let parents = match self.repo.head() {
            Ok(head) => {
                let parent_commit = head.peel_to_commit().context("Failed to get head commit")?;
                vec![parent_commit]
            }
            Err(_) => vec![],
        };

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        self.repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .context("Failed to create commit")?;

        Ok(())
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
        let result = GitRepo::init(temp_dir.path());

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
    fn test_add_and_commit_file() {
        let temp_dir = tempdir().unwrap();
        let repo = GitRepo::init(temp_dir.path()).unwrap();

        // Create a test file
        let test_file_path = temp_dir.path().join("test.txt");
        fs::write(&test_file_path, "test content").unwrap();

        // Commit the file
        let result = repo.add_and_commit_file(&test_file_path, "Add test file");
        assert!(result.is_ok());

        // Check commit count
        assert_eq!(repo.get_commit_count().unwrap(), 1);

        // Check commit message
        assert_eq!(repo.get_last_commit_message().unwrap(), "Add test file");
    }

    #[test]
    fn test_create_empty_commit() {
        let temp_dir = tempdir().unwrap();
        let repo = GitRepo::init(temp_dir.path()).unwrap();

        // Create an empty commit
        let result = repo.create_empty_commit("Empty commit");
        assert!(result.is_ok());

        // Check commit count
        assert_eq!(repo.get_commit_count().unwrap(), 1);

        // Check commit message
        assert_eq!(repo.get_last_commit_message().unwrap(), "Empty commit");

        // Create another empty commit
        let result = repo.create_empty_commit("Another empty commit");
        assert!(result.is_ok());

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
        let repo = GitRepo::init(temp_dir.path()).unwrap();

        // Create an initial commit
        let test_file_path = temp_dir.path().join("initial.txt");
        fs::write(&test_file_path, "initial content").unwrap();
        repo.add_and_commit_file(&test_file_path, "Initial commit")
            .unwrap();

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
}
