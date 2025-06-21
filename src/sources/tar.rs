use anyhow::{anyhow, Result};
use std::path::PathBuf;
use tempfile::TempDir;

use super::Source;
use crate::notifier::Notifier;

/// Extracts filename from a tar path and sanitizes it for Git branch naming
/// Removes file extension and sanitizes problematic characters
fn tar_to_branch(tar_path: &str) -> String {
    let path = PathBuf::from(tar_path);
    let filename = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("tar-image");

    super::sanitize_branch_name(filename)
}

/// Tar implementation of the Source trait for pre-downloaded tarballs
pub struct TarSource;

impl TarSource {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Source for TarSource {
    fn name(&self) -> &str {
        "tar"
    }

    fn get_image_tarball(
        &self,
        image_path: &str,
        notifier: &Notifier,
    ) -> Result<(PathBuf, Option<TempDir>)> {
        // For tar source, image_path is the path to the existing tarball
        let tarball_path = PathBuf::from(image_path);

        // Verify the tarball exists
        if !tarball_path.exists() {
            return Err(anyhow!(
                "Tarball file does not exist: {}",
                tarball_path.display()
            ));
        }

        // Check if it's a file
        if !tarball_path.is_file() {
            return Err(anyhow!("Path is not a file: {}", tarball_path.display()));
        }

        // Verify it's a tar file (just basic name check, could be improved)
        let extension = tarball_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        if extension != "tar" {
            notifier.info("Warning: File does not have .tar extension. Proceeding anyway, but this might not be a valid image tarball.");
        }

        // Just return the existing path - no temp dir needed for tar source
        Ok((tarball_path, None))
    }

    fn branch_name(&self, image_path: &str, image_digest: &str) -> String {
        let base_branch = tar_to_branch(image_path);
        if let Some(short_digest) = super::extract_short_digest(image_digest) {
            format!("{}#{}", base_branch, short_digest)
        } else {
            // Fallback: use image_digest as-is if it doesn't have sha256: prefix
            format!("{}#{}", base_branch, image_digest)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tar_to_branch() {
        assert_eq!(tar_to_branch("/path/to/my-image.tar"), "my-image");
        assert_eq!(tar_to_branch("./nginx-latest.tar.gz"), "nginx-latest-tar");
        assert_eq!(tar_to_branch("ubuntu 20.04.tar"), "ubuntu-20-04");
        assert_eq!(tar_to_branch("my:app@v1.0.tar"), "my-app-v1-0");
        assert_eq!(tar_to_branch("hello world.tar"), "hello-world");
        assert_eq!(
            tar_to_branch("file with spaces & symbols!.tar"),
            "file-with-spaces-symbols"
        );
    }

    #[test]
    fn test_tar_source_branch_name() {
        let source = TarSource::new().unwrap();
        assert_eq!(
            source.branch_name("/path/to/my-image.tar", "sha256:1234567890abcdef"),
            "my-image#1234567890ab"
        );
        assert_eq!(
            source.branch_name("nginx-latest.tar", "sha256:9876543210fedcba"),
            "nginx-latest#9876543210fe"
        );
        assert_eq!(
            source.branch_name("ubuntu 20.04.tar", "sha256:abcdef123456789"),
            "ubuntu-20-04#abcdef123456"
        );
        // Test fallback for digest without sha256: prefix
        assert_eq!(
            source.branch_name("ubuntu 20.04.tar", "abcdef123456789"),
            "ubuntu-20-04#abcdef123456789"
        );
    }
}
