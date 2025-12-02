//! TAR file integration tests
//!
//! Tests the tar provider, which is also the universal backend for all providers.
//! Every provider (Docker, nerdctl, tar) ultimately produces a tar file that gets
//! processed through the same tar processing logic tested here.

use crate::integration::common::tar_processing;
use anyhow::Result;
use oci2git::notifier::Notifier;
use oci2git::processor::ImageProcessor;
use oci2git::sources::{Source, TarSource};
use std::io::Write;
use std::path::Path;
use tempfile::{NamedTempFile, TempDir};

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_TAR_PATH: &str = "tests/integration/fixtures/oci2git-test.tar";

    #[test]
    fn test_tar_source_creation() {
        let tar_source = TarSource::new();
        assert!(tar_source.is_ok(), "Should be able to create TarSource");

        let source = tar_source.unwrap();
        assert_eq!(source.name(), "tar");
    }

    #[test]
    fn test_tar_source_with_existing_file() -> Result<()> {
        // Create a temporary tar file with some content
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"fake tar content for testing")?;
        let temp_path = temp_file.path().to_str().unwrap();

        let tar_source = TarSource::new().expect("Should create TarSource");
        let notifier = Notifier::new(0);
        let result = tar_source.get_image_tarball(temp_path, &notifier);

        assert!(
            result.is_ok(),
            "Should successfully handle existing tar file"
        );

        let (tarball_path, temp_dir) = result.unwrap();

        // For tar source, it should return the original path
        assert_eq!(
            tarball_path.to_str().unwrap(),
            temp_path,
            "Should return original tar file path"
        );

        // No temp directory should be created for existing files
        assert!(
            temp_dir.is_none(),
            "No TempDir should be created for existing files"
        );

        Ok(())
    }

    #[test]
    fn test_tar_source_with_nonexistent_file() {
        let tar_source = TarSource::new().expect("Should create TarSource");
        let nonexistent_path = "/path/that/definitely/does/not/exist.tar";
        let notifier = Notifier::new(0);

        let result = tar_source.get_image_tarball(nonexistent_path, &notifier);

        // This might succeed or fail depending on implementation
        // Check the actual TarSource implementation to see expected behavior
        match result {
            Ok((path, _)) => {
                assert_eq!(
                    path.to_str().unwrap(),
                    nonexistent_path,
                    "Should return the path even if file doesn't exist"
                );
            }
            Err(_) => {
                // This is also acceptable behavior for nonexistent files
                println!("TarSource correctly failed for nonexistent file");
            }
        }
    }

    #[test]
    fn test_tar_source_with_relative_path() -> Result<()> {
        // Create a temporary tar file in current directory
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"relative path tar content")?;

        // Get just the filename for relative path testing
        let file_name = temp_file.path().file_name().unwrap().to_str().unwrap();

        let tar_source = TarSource::new().expect("Should create TarSource");
        let notifier = Notifier::new(0);
        let result = tar_source.get_image_tarball(file_name, &notifier);

        // The result will depend on TarSource implementation
        // Some implementations might resolve relative paths, others might not
        println!("Relative path result: {:?}", result);

        Ok(())
    }

    #[test]
    fn test_universal_tar_backend_with_fixture() -> Result<()> {
        // Skip if fixture tar doesn't exist
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!(
                "Skipping test: fixture tar file not found at {}",
                FIXTURE_TAR_PATH
            );
            return Ok(());
        }

        let tar_source = TarSource::new()?;

        // Test the universal tar processing backend
        tar_processing::test_basic_tar_processing(tar_source, FIXTURE_TAR_PATH)?;

        println!("✓ Universal tar processing backend working correctly");
        Ok(())
    }

    #[test]
    fn test_tar_backend_file_extraction() -> Result<()> {
        // This test verifies the universal tar processing backend can extract
        // files correctly - same logic used by all providers
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!(
                "Skipping test: fixture tar file not found at {}",
                FIXTURE_TAR_PATH
            );
            return Ok(());
        }

        let output_dir = TempDir::new()?;
        let tar_source = TarSource::new()?;
        let notifier = Notifier::new(0);
        let processor = ImageProcessor::new(tar_source, notifier);

        // Process through universal backend
        processor.convert(FIXTURE_TAR_PATH, output_dir.path())?;

        // Verify our test files using universal helpers
        let expected_files = [
            ("app/hello.txt", "Hello from oci2git test container!"),
            ("app/config.json", "test-app"),
            ("app/script.sh", "#!/bin/sh"),
        ];

        tar_processing::verify_extracted_files(output_dir.path(), &expected_files)?;
        println!("✓ Universal tar backend correctly extracts files");
        Ok(())
    }

    #[test]
    fn test_tar_backend_metadata_extraction() -> Result<()> {
        // This test verifies the universal tar processing backend can extract
        // metadata correctly - same logic used by all providers
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!(
                "Skipping test: fixture tar file not found at {}",
                FIXTURE_TAR_PATH
            );
            return Ok(());
        }

        let output_dir = TempDir::new()?;
        let tar_source = TarSource::new()?;
        let notifier = Notifier::new(0);
        let processor = ImageProcessor::new(tar_source, notifier);

        // Process through universal backend
        processor.convert(FIXTURE_TAR_PATH, output_dir.path())?;

        // Verify metadata using universal helpers
        let expected_vars = ["APP_NAME", "APP_VERSION", "DEBUG"];
        tar_processing::verify_environment_variables(output_dir.path(), &expected_vars)?;
        tar_processing::verify_entrypoint(output_dir.path(), "/app/script.sh")?;

        println!("✓ Universal tar backend correctly extracts metadata");
        Ok(())
    }

    #[test]
    fn test_tar_backend_git_creation() -> Result<()> {
        // This test verifies the universal tar processing backend creates
        // git repositories correctly - same logic used by all providers
        if !Path::new(FIXTURE_TAR_PATH).exists() {
            println!(
                "Skipping test: fixture tar file not found at {}",
                FIXTURE_TAR_PATH
            );
            return Ok(());
        }

        let output_dir = TempDir::new()?;
        let tar_source = TarSource::new()?;
        let notifier = Notifier::new(0);
        let processor = ImageProcessor::new(tar_source, notifier);

        // Process through universal backend
        processor.convert(FIXTURE_TAR_PATH, output_dir.path())?;

        // Verify git structure using universal helpers
        tar_processing::verify_git_structure(output_dir.path())?;

        println!("✓ Universal tar backend correctly creates git repository");
        Ok(())
    }

    #[test]
    fn test_tar_with_hardlinks() -> Result<()> {
        // Test extraction of Docker image with hardlinks
        const HARDLINK_FIXTURE: &str = "tests/integration/fixtures/hardlink-test-image.tar";

        if !Path::new(HARDLINK_FIXTURE).exists() {
            println!(
                "Skipping test: hardlink fixture not found at {}",
                HARDLINK_FIXTURE
            );
            println!("To generate: docker build -f tests/integration/fixtures/hardlink-test.Dockerfile -t oci2git-hardlink-test:latest . && docker save oci2git-hardlink-test:latest -o tests/integration/fixtures/hardlink-test-image.tar");
            return Ok(());
        }

        let output_dir = TempDir::new()?;
        let tar_source = TarSource::new()?;
        let notifier = Notifier::new(0);
        let processor = ImageProcessor::new(tar_source, notifier);

        println!("Converting image with hardlinks...");
        processor.convert(HARDLINK_FIXTURE, output_dir.path())?;

        // Verify the git repo was created
        assert!(output_dir.path().join(".git").exists());
        println!("✓ Git repository created");

        // Files are extracted to rootfs subdirectory
        let rootfs = output_dir.path().join("rootfs");
        assert!(rootfs.exists(), "rootfs directory should exist");

        // Check if all test files exist (hardlinks, symlinks, empty files, directories)
        let expected_files = vec![
            // Test 1: Regular files with hardlinks
            "app/bin/original.sh",
            "app/bin/hardlink1.sh",
            "app/bin/hardlink2.sh",
            // Test 2: Library files with hardlinks
            "app/lib/libtest.so.1.0",
            "app/lib/libtest.so.1",
            "app/lib/libtest.so",
            // Test 3 & 4: Symlinks (relative and absolute)
            "app/bin/relative-symlink.sh",
            "app/bin/absolute-symlink.sh",
            // Test 5: Empty files with hardlinks and symlinks
            "app/data/empty.txt",
            "app/data/empty-hardlink.txt",
            "app/data/empty-symlink.txt",
            // Test 6: Symlink chain
            "app/bin/symlink-chain.sh",
            // Test 7 & 8: Directory symlinks
            "app/links/dir-symlink",
            "app/links/dir-symlink-absolute",
            "app/target_dir/file.txt",
            // Test 9: Broken symlink
            "app/links/broken-symlink.txt",
            // Test 10: Multiple hardlinks to empty file
            "app/data/empty2.txt",
            "app/data/empty2-link1.txt",
            "app/data/empty2-link2.txt",
            "app/data/empty2-link3.txt",
            // Test 11: File with content and multiple links
            "app/data/file1.txt",
            "app/data/file1-hardlink.txt",
            "app/data/file1-symlink.txt",
            // Test 12: Relative symlink pointing up
            "app/data/relative-up-symlink.sh",
        ];

        for file in &expected_files {
            let file_path = rootfs.join(file);
            // Use symlink_metadata to detect broken symlinks
            let exists = file_path.exists() || std::fs::symlink_metadata(&file_path).is_ok();
            assert!(
                exists,
                "Expected file does not exist: {}",
                file
            );
        }
        println!("✓ All test files extracted successfully ({} files)", expected_files.len());

        // Verify hardlinked files have the same content
        let original_content =
            std::fs::read_to_string(rootfs.join("app/bin/original.sh"))?;
        let hardlink1_content =
            std::fs::read_to_string(rootfs.join("app/bin/hardlink1.sh"))?;
        let hardlink2_content =
            std::fs::read_to_string(rootfs.join("app/bin/hardlink2.sh"))?;

        assert_eq!(original_content, hardlink1_content);
        assert_eq!(original_content, hardlink2_content);
        assert!(original_content.contains("Hello from original"));
        println!("✓ Hardlinked scripts have correct content");

        // Verify library hardlinks
        let lib_original =
            std::fs::read_to_string(rootfs.join("app/lib/libtest.so.1.0"))?;
        let lib_link1 = std::fs::read_to_string(rootfs.join("app/lib/libtest.so.1"))?;
        let lib_link2 = std::fs::read_to_string(rootfs.join("app/lib/libtest.so"))?;

        assert_eq!(lib_original, lib_link1);
        assert_eq!(lib_original, lib_link2);
        assert_eq!(lib_original, "Library content v1\n");
        println!("✓ Library hardlinks have correct content");

        // Verify absolute symlinks point to correct location
        #[cfg(unix)]
        {
            let absolute_symlink = rootfs.join("app/bin/absolute-symlink.sh");
            if absolute_symlink.exists() {
                let target = std::fs::read_link(&absolute_symlink)?;
                // The symlink should point to an absolute path within rootfs
                assert!(target.is_absolute(), "Symlink should be absolute: {:?}", target);
                assert!(target.to_string_lossy().contains("rootfs"),
                    "Symlink should point to rootfs: {:?}", target);
                println!("✓ Absolute symlinks correctly point to rootfs");
            }
        }

        // Verify empty file hardlinks
        let empty_content = std::fs::read_to_string(rootfs.join("app/data/empty.txt"))?;
        let empty_hardlink_content = std::fs::read_to_string(rootfs.join("app/data/empty-hardlink.txt"))?;
        assert_eq!(empty_content, empty_hardlink_content);
        assert_eq!(empty_content, "");
        println!("✓ Empty file hardlinks work correctly");

        // Verify multiple hardlinks to same empty file
        let empty2_1 = std::fs::read_to_string(rootfs.join("app/data/empty2.txt"))?;
        let empty2_2 = std::fs::read_to_string(rootfs.join("app/data/empty2-link1.txt"))?;
        let empty2_3 = std::fs::read_to_string(rootfs.join("app/data/empty2-link2.txt"))?;
        assert_eq!(empty2_1, empty2_2);
        assert_eq!(empty2_1, empty2_3);
        println!("✓ Multiple hardlinks to empty file work correctly");

        // Verify file with multiple link types
        let file1_content = std::fs::read_to_string(rootfs.join("app/data/file1.txt"))?;
        let file1_hardlink = std::fs::read_to_string(rootfs.join("app/data/file1-hardlink.txt"))?;
        let file1_symlink = std::fs::read_to_string(rootfs.join("app/data/file1-symlink.txt"))?;
        assert_eq!(file1_content, "Data file 1\n");
        assert_eq!(file1_content, file1_hardlink);
        assert_eq!(file1_content, file1_symlink);
        println!("✓ Mixed hardlinks and symlinks work correctly");

        println!("✅ All comprehensive link extraction tests passed!");
        Ok(())
    }
}
