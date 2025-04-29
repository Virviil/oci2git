use anyhow::Result;
use std::fs;
use tempfile::tempdir;

// Let's implement our own test helpers since we can't easily access the container tests module
use chrono::{TimeZone, Utc};
use oci2git::converter::ImageToGitConverter;
use oci2git::metadata::{ContainerConfig, ImageMetadata};
use oci2git::{container::Layer, ContainerEngine};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

// Mock container engine for testing
struct MockContainerEngine {
    layers: Vec<Layer>,
    metadata: ImageMetadata,
    test_files: Vec<(String, String)>, // (filename, content)
}

impl MockContainerEngine {
    fn new() -> Self {
        // Create sample layers
        let layers = vec![
            Layer {
                id: "layer1".to_string(),
                command: "FROM base".to_string(),
                created_at: Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap(),
                is_empty: false,
            },
            Layer {
                id: "layer2".to_string(),
                command: "RUN echo hello".to_string(),
                created_at: Utc.with_ymd_and_hms(2023, 1, 1, 1, 0, 0).unwrap(),
                is_empty: false,
            },
            Layer {
                id: "layer3".to_string(),
                command: "ENV FOO=bar".to_string(),
                created_at: Utc.with_ymd_and_hms(2023, 1, 1, 2, 0, 0).unwrap(),
                is_empty: true,
            },
        ];

        // Create sample metadata
        let mut env = Vec::new();
        env.push("PATH=/usr/local/bin:/usr/bin".to_string());
        env.push("FOO=bar".to_string());

        let mut labels = HashMap::new();
        labels.insert("maintainer".to_string(), "test@example.com".to_string());

        let metadata = ImageMetadata {
            id: "sha256:mockimage".to_string(),
            repo_tags: vec!["mockimage:latest".to_string()],
            created: "2023-01-01T00:00:00Z".to_string(),
            container_config: ContainerConfig {
                env,
                cmd: Some(vec!["bash".to_string()]),
                entrypoint: None,
                exposed_ports: None,
                working_dir: Some("/app".to_string()),
                volumes: None,
                labels: Some(labels),
            },
            history: Vec::new(),
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
        };

        // Sample test files
        let test_files = vec![
            ("file1.txt".to_string(), "content of file 1".to_string()),
            (
                "dir1/file2.txt".to_string(),
                "content of file 2".to_string(),
            ),
            (
                "dir1/dir2/file3.txt".to_string(),
                "content of file 3".to_string(),
            ),
        ];

        Self {
            layers,
            metadata,
            test_files,
        }
    }
}

impl ContainerEngine for MockContainerEngine {
    fn get_layers(&self, _extract_dir: &Path) -> Result<Vec<Layer>> {
        Ok(self.layers.clone())
    }

    fn extract_image(&self, _extract_dir: &Path, output_dir: &Path) -> Result<()> {
        // Create test files in the output directory
        for (file_path, content) in &self.test_files {
            let full_path = output_dir.join(file_path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::File::create(&full_path)?;
            file.write_all(content.as_bytes())?;
        }
        Ok(())
    }

    fn get_metadata(&self, _extract_dir: &Path) -> Result<ImageMetadata> {
        Ok(self.metadata.clone())
    }

    // Mock implementation of get_layer_tarballs
    // Since we can't easily create real tarballs in tests, return empty vector
    // This will cause the converter to fall back to the old behavior for tests
    fn get_layer_tarballs(&self, _extract_dir: &Path) -> Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }
}

#[test]
fn test_full_workflow() -> Result<()> {
    // Create temporary directory for output
    let output_dir = tempdir()?;

    // Create mock container engine
    let engine = MockContainerEngine::new();

    // Create the converter
    let converter = ImageToGitConverter::new(engine);

    // Convert the image (with beautiful_progress set to false for testing)
    converter.convert("test:latest", output_dir.path(), false)?;

    // Verify structure
    let image_md_path = output_dir.path().join("Image.md");
    let rootfs_dir = output_dir.path().join("rootfs");

    assert!(image_md_path.exists(), "Image.md file should exist");
    assert!(rootfs_dir.exists(), "rootfs directory should exist");

    // Verify file content
    let image_md_content = fs::read_to_string(image_md_path)?;
    assert!(
        image_md_content.contains("mockimage:latest"),
        "Image.md should contain image tag"
    );

    // Verify that Image.md is in the first commit
    let repo = git2::Repository::open(output_dir.path())?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::REVERSE)?;

    let commits: Vec<_> = revwalk.collect::<Result<Vec<_>, _>>()?;
    assert!(!commits.is_empty(), "Should have at least one commit");

    // First commit should contain Image.md file
    let first_commit = repo.find_commit(commits[0])?;
    let first_tree = first_commit.tree()?;

    assert!(
        first_tree.get_name("Image.md").is_some(),
        "First commit should contain Image.md"
    );

    // Verify rootfs files
    let file1_path = rootfs_dir.join("file1.txt");
    assert!(file1_path.exists(), "rootfs/file1.txt should exist");
    let content = fs::read_to_string(&file1_path)?;
    assert_eq!(content, "content of file 1", "File content should match");

    // Open the git repository and verify the history
    let repo = git2::Repository::open(output_dir.path())?;

    // Get commits in chronological order
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::REVERSE)?;

    let commits: Vec<_> = revwalk.collect::<Result<Vec<_>, _>>()?;

    // Should have 5 commits with fallback extraction: metadata + filesystem + 3 layer info commits
    assert_eq!(commits.len(), 5, "Should have 5 commits");

    // Verify that the repository has the expected commits
    // Check for layer commits

    // Check that the first commit has metadata
    let first_commit = repo.find_commit(commits[0])?;
    assert!(
        first_commit.message().unwrap().contains("Metadata"),
        "First commit should contain metadata"
    );

    // Check that all expected layers are represented
    let mut has_from_layer = false;
    let mut has_echo_layer = false;
    let mut has_env_layer = false;

    for commit_id in &commits {
        let cm = repo.find_commit(*commit_id)?;
        let msg = cm.message().unwrap_or("");

        if msg.contains("FROM base") {
            has_from_layer = true;
        } else if msg.contains("RUN echo hello") {
            has_echo_layer = true;
        } else if msg.contains("ENV FOO=bar") {
            has_env_layer = true;
        }
    }

    assert!(
        has_from_layer,
        "Repository should have a commit for 'FROM base' layer"
    );
    assert!(
        has_echo_layer,
        "Repository should have a commit for 'RUN echo hello' layer"
    );
    assert!(
        has_env_layer,
        "Repository should have a commit for 'ENV FOO=bar' layer"
    );

    Ok(())
}

#[test]
fn test_empty_layers_create_empty_commits() -> Result<()> {
    // Create temporary directory
    let output_dir = tempdir()?;

    // Create a mock engine with an empty layer
    let engine = MockContainerEngine::new();
    // The third layer in MockContainerEngine is marked as empty (ENV FOO=bar)

    // Convert the image
    let converter = ImageToGitConverter::new(engine);
    converter.convert("test:latest", output_dir.path(), false)?;

    // Verify git history
    let repo = git2::Repository::open(output_dir.path())?;

    // Get the last commit (should be the empty layer)
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;

    // Verify it's a commit for the ENV layer
    // We need a flexible check to verify the layer commits
    let mut has_env_layer = false;

    // Get all commits and check if any contains the ENV layer message
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    for commit_id in revwalk {
        let cm = repo.find_commit(commit_id?)?;
        let msg = cm.message().unwrap_or("");
        if msg.contains("ENV FOO=bar") {
            has_env_layer = true;
            break;
        }
    }

    assert!(has_env_layer, "Should have a commit for ENV FOO=bar layer");

    // Get the tree of the last commit
    let tree = commit.tree()?;

    // Get parent commit's tree
    let parent = commit.parent(0)?;
    let parent_tree = parent.tree()?;

    // Diff the trees to ensure no changes
    let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
    let stats = diff.stats()?;

    assert_eq!(
        stats.insertions(),
        0,
        "Empty commit should have no insertions"
    );
    assert_eq!(
        stats.deletions(),
        0,
        "Empty commit should have no deletions"
    );

    Ok(())
}
