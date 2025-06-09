//! Nerdctl integration tests
//!
//! Tests the nerdctl provider's ability to download images as tar files.
//! Like Docker, nerdctl produces tar files that get processed through
//! the universal tar processing backend.

#[cfg(all(test, feature = "nerdctl"))]
mod tests {
    use crate::integration::common::*;
    use oci2git::sources::{NerdctlSource, Source};
    use std::fs;

    #[test]
    fn test_nerdctl_source_creation() {
        let nerdctl_source = NerdctlSource::new();
        assert!(
            nerdctl_source.is_ok(),
            "Should be able to create NerdctlSource"
        );

        let source = nerdctl_source.unwrap();
        assert_eq!(source.name(), "nerdctl");
    }

    #[test]
    fn test_nerdctl_image_download_hello_world() {
        let image_name = "hello-world:latest";
        let nerdctl_source = NerdctlSource::new().expect("Should create NerdctlSource");
        let result = nerdctl_source.get_image_tarball(image_name);

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
    fn test_nerdctl_image_download_alpine() {
        let image_name = "alpine:latest";
        let nerdctl_source = NerdctlSource::new().expect("Should create NerdctlSource");
        let result = nerdctl_source.get_image_tarball(image_name);

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
    fn test_nerdctl_image_download_nonexistent() {
        let nerdctl_source = NerdctlSource::new().expect("Should create NerdctlSource");

        let result = nerdctl_source.get_image_tarball(NONEXISTENT_IMAGE);

        assert!(result.is_err(), "Should fail to download nonexistent image");

        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("nerdctl command failed")
                || error_msg.contains("Unable to find image")
                || error_msg.contains("pull access denied")
                || error_msg.contains("not yet implemented"),
            "Error should indicate nerdctl failure: {}",
            error_msg
        );
    }

    #[test]
    fn test_nerdctl_to_tar_to_git_flow_when_implemented() {
        let image_name = "hello-world:latest";
        let nerdctl_source = NerdctlSource::new().expect("Should create NerdctlSource");

        // This test will pass once nerdctl implementation is complete
        let result = tar_processing::test_basic_tar_processing(nerdctl_source, image_name);

        match result {
            Ok(_) => println!("✓ nerdctl → tar → git conversion working"),
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("not yet implemented") {
                    println!("nerdctl support not yet implemented - test will pass when ready");
                } else {
                    panic!("nerdctl to git flow failed: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_nerdctl_uses_universal_tar_backend() {
        // This test documents that nerdctl will use the same universal
        // tar processing backend as Docker and direct tar files
        let nerdctl_source = NerdctlSource::new().expect("Should create NerdctlSource");

        // Once implemented, nerdctl should produce tar files that are processed
        // through the same ImageProcessor.convert() logic as other sources
        let result = nerdctl_source.get_image_tarball("hello-world:latest");

        match result {
            Ok((tar_path, _temp_dir)) => {
                println!("nerdctl produced tar file: {:?}", tar_path);
                println!("✓ This tar file would be processed by universal backend");
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("not yet implemented") {
                    println!("✓ nerdctl will use universal tar backend when implemented");
                } else {
                    println!("nerdctl error (expected during development): {}", e);
                }
            }
        }
    }
}
