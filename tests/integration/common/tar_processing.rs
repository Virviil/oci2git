//! Universal tar processing tests - used by all source types
//!
//! Since all sources (docker, nerdctl, tar) ultimately produce a tar file
//! that gets processed the same way, these tests verify the core tar processing
//! functionality that's shared across all sources.

use anyhow::Result;
use oci2git::notifier::AnyNotifier;
use oci2git::notifier::NotifierFlavor;
use oci2git::processor::ImageProcessor;
use oci2git::sources::Source;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Test helper: Process any tar file and verify basic structure
pub fn test_basic_tar_processing<S: Source>(source: S, tar_path: &str) -> Result<()> {
    let output_dir = TempDir::new()?;
    let notifier = AnyNotifier::new(NotifierFlavor::Simple, 0);
    let processor = ImageProcessor::new(source, notifier);

    // Process the tar file
    processor.convert(tar_path, output_dir.path())?;

    // Verify git repository was created
    let git_dir = output_dir.path().join(".git");
    assert!(git_dir.exists(), "Git directory should exist");

    // Verify rootfs directory was created
    let rootfs_dir = output_dir.path().join("rootfs");
    assert!(rootfs_dir.exists(), "Rootfs directory should exist");

    // Verify Image.md metadata was created
    let image_md = output_dir.path().join("Image.md");
    assert!(image_md.exists(), "Image.md should exist");

    Ok(())
}

/// Test helper: Verify specific files were extracted correctly
pub fn verify_extracted_files(output_dir: &Path, expected_files: &[(&str, &str)]) -> Result<()> {
    for (file_path, expected_content) in expected_files {
        let full_path = output_dir.join("rootfs").join(file_path);
        assert!(full_path.exists(), "File {file_path} should exist");

        let content = fs::read_to_string(&full_path)?;
        assert!(
            content.contains(expected_content),
            "File {file_path} should contain '{expected_content}'"
        );
    }
    Ok(())
}

/// Test helper: Verify environment variables in metadata
pub fn verify_environment_variables(output_dir: &Path, expected_vars: &[&str]) -> Result<()> {
    let image_md = output_dir.join("Image.md");
    let metadata_content = fs::read_to_string(&image_md)?;

    for env_var in expected_vars {
        assert!(
            metadata_content.contains(env_var),
            "Metadata should contain environment variable: {env_var}"
        );
    }
    Ok(())
}

/// Test helper: Verify entrypoint/cmd in metadata
pub fn verify_entrypoint(output_dir: &Path, expected_entrypoint: &str) -> Result<()> {
    let image_md = output_dir.join("Image.md");
    let metadata_content = fs::read_to_string(&image_md)?;

    assert!(
        metadata_content.contains(expected_entrypoint),
        "Metadata should contain entrypoint: {expected_entrypoint}"
    );
    Ok(())
}

/// Test helper: Verify git repository structure
pub fn verify_git_structure(output_dir: &Path) -> Result<()> {
    let git_dir = output_dir.join(".git");
    assert!(git_dir.exists(), "Git directory should exist");

    let git_config = git_dir.join("config");
    assert!(git_config.exists(), "Git config should exist");

    let git_head = git_dir.join("HEAD");
    assert!(git_head.exists(), "Git HEAD should exist");

    // Verify at least one commit was created
    let objects_dir = git_dir.join("objects");
    assert!(objects_dir.exists(), "Git objects directory should exist");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci2git::sources::TarSource;

    const FIXTURE_TAR_PATH: &str = "tests/integration/fixtures/oci2git-test.tar";

    #[test]
    fn test_universal_tar_processing() -> Result<()> {
        // Skip if fixture tar doesn't exist
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!("Skipping test: fixture tar file not found at {FIXTURE_TAR_PATH}");
            return Ok(());
        }

        let tar_source = TarSource::new()?;
        test_basic_tar_processing(tar_source, FIXTURE_TAR_PATH)?;
        Ok(())
    }

    #[test]
    fn test_universal_file_extraction() -> Result<()> {
        // Skip if fixture tar doesn't exist
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!("Skipping test: fixture tar file not found at {FIXTURE_TAR_PATH}");
            return Ok(());
        }

        let output_dir = TempDir::new()?;
        let tar_source = TarSource::new()?;
        let notifier = AnyNotifier::new(NotifierFlavor::Simple, 0);
        let processor = ImageProcessor::new(tar_source, notifier);

        // Process the image
        processor.convert(FIXTURE_TAR_PATH, output_dir.path())?;

        // Verify our test files
        let expected_files = [
            ("app/hello.txt", "Hello from oci2git test container!"),
            ("app/config.json", "test-app"),
            ("app/script.sh", "#!/bin/sh"),
        ];

        verify_extracted_files(output_dir.path(), &expected_files)?;
        Ok(())
    }

    #[test]
    fn test_universal_metadata_extraction() -> Result<()> {
        // Skip if fixture tar doesn't exist
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!("Skipping test: fixture tar file not found at {FIXTURE_TAR_PATH}");
            return Ok(());
        }

        let output_dir = TempDir::new()?;
        let tar_source = TarSource::new()?;
        let notifier = AnyNotifier::new(NotifierFlavor::Simple, 0);
        let processor = ImageProcessor::new(tar_source, notifier);

        // Process the image
        processor.convert(FIXTURE_TAR_PATH, output_dir.path())?;

        // Verify environment variables
        let expected_vars = ["APP_NAME", "APP_VERSION", "DEBUG"];
        verify_environment_variables(output_dir.path(), &expected_vars)?;

        // Verify entrypoint
        verify_entrypoint(output_dir.path(), "/app/script.sh")?;

        Ok(())
    }

    #[test]
    fn test_universal_git_structure() -> Result<()> {
        // Skip if fixture tar doesn't exist
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!("Skipping test: fixture tar file not found at {FIXTURE_TAR_PATH}");
            return Ok(());
        }

        let output_dir = TempDir::new()?;
        let tar_source = TarSource::new()?;
        let notifier = AnyNotifier::new(NotifierFlavor::Simple, 0);
        let processor = ImageProcessor::new(tar_source, notifier);

        // Process the image
        processor.convert(FIXTURE_TAR_PATH, output_dir.path())?;

        // Verify git structure
        verify_git_structure(output_dir.path())?;

        Ok(())
    }
}
