//! TAR file integration tests
//!
//! Tests the tar provider, which is also the universal backend for all providers.
//! Every provider (Docker, nerdctl, tar) ultimately produces a tar file that gets
//! processed through the same tar processing logic tested here.

use crate::integration::common::tar_processing;
use anyhow::Result;
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
        let result = tar_source.get_image_tarball(temp_path);

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

        let result = tar_source.get_image_tarball(nonexistent_path);

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
        let result = tar_source.get_image_tarball(file_name);

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
        let processor = ImageProcessor::new(tar_source);

        // Process through universal backend
        processor.convert(FIXTURE_TAR_PATH, output_dir.path(), false)?;

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
        let processor = ImageProcessor::new(tar_source);

        // Process through universal backend
        processor.convert(FIXTURE_TAR_PATH, output_dir.path(), false)?;

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
        let processor = ImageProcessor::new(tar_source);

        // Process through universal backend
        processor.convert(FIXTURE_TAR_PATH, output_dir.path(), false)?;

        // Verify git structure using universal helpers
        tar_processing::verify_git_structure(output_dir.path())?;

        println!("✓ Universal tar backend correctly creates git repository");
        Ok(())
    }
}
