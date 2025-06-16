use anyhow::{anyhow, Context, Result};
use log::info;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

use super::{naming, Source};

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
}

impl Source for DockerSource {
    fn name(&self) -> &str {
        "docker"
    }

    fn get_image_tarball(&self, image_name: &str) -> Result<(PathBuf, Option<TempDir>)> {
        // Create a temporary directory to save the image
        let temp_dir = TempDir::new().context("Failed to create temporary directory")?;
        let tarball_path = temp_dir.path().join("image.tar");

        // Use docker save to export the full image with all layers
        info!("Exporting Docker image '{}' to tarball...", image_name);
        self.run_command(&["save", "-o", tarball_path.to_str().unwrap(), image_name])?;

        // Return both the tarball path and the tempdir to ensure it stays alive
        Ok((tarball_path, Some(temp_dir)))
    }

    fn branch_name(&self, image_name: &str, image_digest: &str) -> String {
        let base_branch = naming::container_image_to_branch(image_name);
        naming::combine_branch_with_digest(&base_branch, image_digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_to_branch() {
        assert_eq!(docker_to_branch("hello-world:latest"), "hello-world#latest");
        assert_eq!(docker_to_branch("hello-world"), "hello-world#latest");
        assert_eq!(docker_to_branch("nginx"), "nginx#latest");
        assert_eq!(docker_to_branch("nginx/nginx:1.21"), "nginx-nginx#1.21");
        assert_eq!(
            docker_to_branch("registry.example.com/my-app:v1.0"),
            "registry.example.com-my-app#v1.0"
        );
        assert_eq!(
            docker_to_branch("alpine@sha256:abc123"),
            "alpine-sha256#abc123"
        );
        assert_eq!(
            docker_to_branch("library/ubuntu:20.04"),
            "library-ubuntu#20.04"
        );
        assert_eq!(docker_to_branch("library/ubuntu"), "library-ubuntu#latest");
    }

    #[test]
    fn test_docker_source_branch_name() {
        let source = DockerSource::new().unwrap();
        assert_eq!(
            source.branch_name("hello-world:latest", "sha256:1234567890abcdef"),
            "hello-world#latest#1234567890ab"
        );
        assert_eq!(
            source.branch_name("hello-world", "sha256:1234567890abcdef"),
            "hello-world#latest#1234567890ab"
        );
        assert_eq!(
            source.branch_name("nginx/nginx:1.21", "sha256:9876543210fedcba"),
            "nginx-nginx#1.21#9876543210fe"
        );
        assert_eq!(
            source.branch_name("nginx", "sha256:abcdef123456789"),
            "nginx#latest#abcdef123456"
        );
        // Test fallback for digest without sha256: prefix
        assert_eq!(
            source.branch_name("nginx", "abcdef123456789"),
            "nginx#latest#abcdef123456789"
        );
    }
}
