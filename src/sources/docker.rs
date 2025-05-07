use anyhow::{anyhow, Context, Result};
use log::info;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

use super::Source;

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
}
