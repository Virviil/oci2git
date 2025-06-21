//! Docker integration tests
//!
//! Tests the Docker provider's ability to download images as tar files.
//! All providers ultimately produce tar files that get processed the same way.

#[cfg(all(test, feature = "docker"))]
mod tests {
    use crate::integration::common::*;
    use oci2git::notifier::Notifier;
    use oci2git::processor::ImageProcessor;
    use oci2git::sources::{DockerSource, Source};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_docker_source_creation() {
        let docker_source = DockerSource::new();
        assert!(
            docker_source.is_ok(),
            "Should be able to create DockerSource"
        );

        let source = docker_source.unwrap();
        assert_eq!(source.name(), "docker");
    }

    #[test]
    fn test_docker_image_download_hello_world() {
        let image_name = "hello-world:latest";
        let docker_source = DockerSource::new().expect("Should create DockerSource");
        let notifier = Notifier::new(0);
        let result = docker_source.get_image_tarball(image_name, &notifier);

        assert!(result.is_ok(), "Should successfully download image tarball");

        let (tarball_path, temp_dir) = result.unwrap();

        // Verify the tarball file exists and has content
        assert!(tarball_path.exists(), "Tarball file should exist");

        let metadata = fs::metadata(&tarball_path).expect("Should get file metadata");
        assert!(metadata.len() > 0, "Tarball should not be empty");

        // Temp directory should be provided to keep the file alive
        assert!(temp_dir.is_some(), "TempDir should be provided");

        println!("Successfully downloaded image tarball: {:?}", tarball_path);
        println!("Tarball size: {} bytes", metadata.len());
    }

    #[test]
    fn test_docker_image_download_alpine() {
        let image_name = "alpine:latest";
        let docker_source = DockerSource::new().expect("Should create DockerSource");
        let notifier = Notifier::new(0);
        let result = docker_source.get_image_tarball(image_name, &notifier);

        assert!(
            result.is_ok(),
            "Should successfully download Alpine image tarball"
        );

        let (tarball_path, temp_dir) = result.unwrap();

        // Verify the tarball file exists and has content
        assert!(tarball_path.exists(), "Alpine tarball file should exist");

        let metadata = fs::metadata(&tarball_path).expect("Should get Alpine file metadata");
        assert!(metadata.len() > 0, "Alpine tarball should not be empty");

        // Alpine should be larger than hello-world
        assert!(
            metadata.len() > 1000,
            "Alpine tarball should be reasonably sized"
        );

        // Temp directory should be provided
        assert!(temp_dir.is_some(), "TempDir should be provided for Alpine");

        println!(
            "Successfully downloaded Alpine image tarball: {:?}",
            tarball_path
        );
        println!("Alpine tarball size: {} bytes", metadata.len());
    }

    #[test]
    fn test_docker_image_download_nonexistent() {
        let docker_source = DockerSource::new().expect("Should create DockerSource");
        let notifier = Notifier::new(0);

        let result = docker_source.get_image_tarball(NONEXISTENT_IMAGE, &notifier);

        assert!(result.is_err(), "Should fail to download nonexistent image");

        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("Docker command failed")
                || error_msg.contains("Unable to find image")
                || error_msg.contains("pull access denied"),
            "Error should indicate Docker failure: {}",
            error_msg
        );
    }

    #[test]
    fn test_docker_image_download_with_tag() {
        let image_name = "hello-world:linux";
        let docker_source = DockerSource::new().expect("Should create DockerSource");
        let result = docker_source.get_image_tarball(image_name);

        assert!(
            result.is_ok(),
            "Should successfully download tagged image tarball"
        );

        let (tarball_path, temp_dir) = result.unwrap();
        assert!(tarball_path.exists(), "Tagged image tarball should exist");
        assert!(
            temp_dir.is_some(),
            "TempDir should be provided for tagged image"
        );

        let metadata = fs::metadata(&tarball_path).expect("Should get tagged image metadata");
        println!("Tagged image tarball size: {} bytes", metadata.len());
    }

    #[test]
    fn test_docker_multiple_downloads() {
        let image_name = "hello-world:latest";
        let docker_source = DockerSource::new().expect("Should create DockerSource");

        // Download the same image multiple times
        let result1 = docker_source.get_image_tarball(image_name);
        let result2 = docker_source.get_image_tarball(image_name);

        assert!(result1.is_ok(), "First download should succeed");
        assert!(result2.is_ok(), "Second download should succeed");

        let (path1, _temp1) = result1.unwrap();
        let (path2, _temp2) = result2.unwrap();

        // Both should exist and have content
        assert!(path1.exists());
        assert!(path2.exists());

        let size1 = fs::metadata(&path1).unwrap().len();
        let size2 = fs::metadata(&path2).unwrap().len();

        // Both tarballs should be the same size (same image)
        assert_eq!(
            size1, size2,
            "Multiple downloads of same image should produce same size tarballs"
        );
    }

    #[test]
    fn test_docker_to_tar_to_git_flow() {
        let image_name = "hello-world:latest";
        let docker_source = DockerSource::new().expect("Should create DockerSource");

        // Test the complete flow: Docker → tar → git repository
        let result = tar_processing::test_basic_tar_processing(docker_source, image_name);

        match result {
            Ok(_) => println!("Successfully completed Docker → tar → git conversion"),
            Err(e) => panic!("Docker to git flow failed: {}", e),
        }
    }

    #[test]
    fn test_docker_tar_processing_produces_git_repo() {
        let image_name = "alpine:latest";
        let output_dir = TempDir::new().expect("Should create temp output dir");
        let docker_source = DockerSource::new().expect("Should create DockerSource");
        let processor = ImageProcessor::new(docker_source);

        // Process Docker image through unified tar processing
        let result = processor.convert(image_name, output_dir.path(), false);

        assert!(
            result.is_ok(),
            "Should successfully process Docker image: {:?}",
            result
        );

        // Verify the same git structure as other sources
        tar_processing::verify_git_structure(output_dir.path())
            .expect("Should have proper git structure");

        // Check that we have Alpine-specific content
        let rootfs_dir = output_dir.path().join("rootfs");
        assert!(rootfs_dir.exists(), "Rootfs should exist");

        // Alpine should have /etc/alpine-release or similar
        let etc_dir = rootfs_dir.join("etc");
        if etc_dir.exists() {
            println!("Alpine /etc directory found - Docker → tar → git flow working correctly");
        }
    }
}
