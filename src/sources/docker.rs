use anyhow::{anyhow, Context, Result};
use log::{info, warn};
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
        
        // Try to save the image first
        let save_result = self.run_command(&["save", "-o", tarball_path.to_str().unwrap(), image_name]);
        
        match save_result {
            Ok(_) => {
                // Success - return the tarball path
                Ok((tarball_path, Some(temp_dir)))
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Check if the error is about missing image
                if error_msg.contains("No such image") || error_msg.contains("pull access denied") {
                    warn!("Image '{}' not found locally, attempting to pull...", image_name);
                    
                    // Try to pull the image
                    info!("Pulling Docker image '{}'...", image_name);
                    self.run_command(&["pull", image_name])
                        .context(format!("Failed to pull image '{}'", image_name))?;
                    
                    // Retry the save command after successful pull
                    info!("Retrying export of Docker image '{}' to tarball...", image_name);
                    self.run_command(&["save", "-o", tarball_path.to_str().unwrap(), image_name])
                        .context(format!("Failed to save image '{}' after pull", image_name))?;
                    
                    Ok((tarball_path, Some(temp_dir)))
                } else {
                    // Different error - propagate it
                    Err(e)
                }
            }
        }
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
