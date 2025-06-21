use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

use crate::notifier::Notifier;

/// Source trait for getting OCI images from different container sources
pub trait Source {
    /// Returns the name of the source for identification purposes
    fn name(&self) -> &str;

    /// Retrieves an OCI image tarball and returns the path to it along with temp directory if created
    /// The image_name parameter can be an image reference (for registry sources)
    /// or a filesystem path (for local sources)
    ///
    /// Returns a tuple with the path to the tarball and an optional TempDir that needs to be kept alive
    /// for the duration of the tarball use
    fn get_image_tarball(
        &self,
        image_name: &str,
        notifier: &Notifier,
    ) -> Result<(PathBuf, Option<TempDir>)>;

    /// Generates a Git branch name from the image name/path
    /// Each source type implements its own naming strategy
    /// The image_digest parameter is mandatory and provided by the processor after extracting metadata
    fn branch_name(&self, image_name: &str, image_digest: &str) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::{DockerSource, TarSource};

    #[test]
    fn test_polymorphic_branch_naming() {
        // Test Docker source - digest is always provided by processor
        let docker_source = DockerSource::new().unwrap();
        assert_eq!(
            docker_source.branch_name("hello-world:latest", "sha256:1234567890abcdef"),
            "hello-world#latest#1234567890ab"
        );
        assert_eq!(
            docker_source.branch_name("nginx/nginx:1.21", "sha256:9876543210fedcba"),
            "nginx-nginx#1.21#9876543210fe"
        );

        // Test Tar source - digest is always provided by processor
        let tar_source = TarSource::new().unwrap();
        assert_eq!(
            tar_source.branch_name("/path/to/my-image.tar", "sha256:1234567890abcdef"),
            "my-image#1234567890ab"
        );
        assert_eq!(
            tar_source.branch_name("ubuntu 20.04.tar", "sha256:abcdef123456789"),
            "ubuntu-20-04#abcdef123456"
        );
    }
}
