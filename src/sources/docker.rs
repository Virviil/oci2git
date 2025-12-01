use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

use super::{naming, Source};
use crate::notifier::Notifier;

/// Docker implementation of the Source trait
pub struct DockerSource;

impl DockerSource {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    fn run_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("docker")
            .args(args)
            .output()
            .context(format!("Failed to execute docker command: {:?}", args))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Docker command failed: {}", error));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    fn image_exists(&self, image_name: &str) -> bool {
        Command::new("docker")
            .args(["image", "inspect", image_name])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn pull_image(&self, image_name: &str, notifier: &Notifier) -> Result<()> {
        notifier.info(&format!("Pulling Docker image '{}'...", image_name));

        let output = Command::new("docker")
            .args(["pull", image_name])
            .output()
            .context("Failed to execute docker pull command")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Docker pull failed: {}", error));
        }

        notifier.info(&format!(
            "Successfully pulled Docker image '{}'",
            image_name
        ));
        Ok(())
    }
}

impl Source for DockerSource {
    fn name(&self) -> &str {
        "docker"
    }

    fn get_image_tarball(
        &self,
        image_name: &str,
        notifier: &Notifier,
    ) -> Result<(PathBuf, Option<TempDir>)> {
        // Create a temporary directory to save the image
        let temp_dir = TempDir::new().context("Failed to create temporary directory")?;
        let tarball_path = temp_dir.path().join("image.tar");

        // Use docker save to export the full image with all layers
        notifier.info(&format!(
            "Exporting Docker image '{}' to tarball...",
            image_name
        ));

        // Try to save the image first
        let save_result =
            self.run_command(&["save", "-o", tarball_path.to_str().unwrap(), image_name]);

        match save_result {
            Ok(_) => {
                // Success - return the tarball path
                Ok((tarball_path, Some(temp_dir)))
            }
            Err(e) => {
                // Save failed - check if it's because the image doesn't exist
                if !self.image_exists(image_name) {
                    notifier.info(&format!(
                        "Image '{}' not found locally, attempting to pull...",
                        image_name
                    ));

                    // Try to pull the image
                    self.pull_image(image_name, notifier)
                        .context(format!("Failed to pull image '{}'", image_name))?;

                    // Retry the save command after successful pull
                    notifier.info(&format!(
                        "Retrying export of Docker image '{}' to tarball...",
                        image_name
                    ));
                    self.run_command(&["save", "-o", tarball_path.to_str().unwrap(), image_name])
                        .context(format!("Failed to save image '{}' after pull", image_name))?;

                    Ok((tarball_path, Some(temp_dir)))
                } else {
                    // Image exists but save failed for another reason - propagate original error
                    Err(e)
                }
            }
        }
    }

    fn branch_name(&self, image_name: &str, os_arch: &str, image_digest: &str) -> String {
        let base_branch = naming::container_image_to_branch(image_name);
        naming::combine_branch_with_digest(&base_branch, os_arch, image_digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_source_branch_name() {
        let source = DockerSource::new().unwrap();
        assert_eq!(
            source.branch_name(
                "hello-world:latest",
                "linux-amd64",
                "sha256:1234567890abcdef"
            ),
            "hello-world#latest#linux-amd64#1234567890ab"
        );
        assert_eq!(
            source.branch_name("hello-world", "linux-arm64", "sha256:1234567890abcdef"),
            "hello-world#latest#linux-arm64#1234567890ab"
        );
        assert_eq!(
            source.branch_name("nginx/nginx:1.21", "linux-amd64", "sha256:9876543210fedcba"),
            "nginx-nginx#1.21#linux-amd64#9876543210fe"
        );
        assert_eq!(
            source.branch_name("nginx", "windows-amd64", "sha256:abcdef123456789"),
            "nginx#latest#windows-amd64#abcdef123456"
        );
        // Test fallback for digest without sha256: prefix
        assert_eq!(
            source.branch_name("nginx", "linux-amd64", "abcdef123456789"),
            "nginx#latest#linux-amd64#abcdef123456789"
        );
    }
}
