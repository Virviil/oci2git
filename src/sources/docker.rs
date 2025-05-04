use anyhow::{anyhow, Context, Result};
use log::info;
use std::path::PathBuf;
use std::process::Command;
use tempfile;

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

    fn get_image_tarball(&self, image_name: &str) -> Result<PathBuf> {
        // Create a temporary directory to save the image
        let temp_dir = tempfile::tempdir().context("Failed to create temporary directory")?;
        let tarball_path = temp_dir.path().join("image.tar");

        // Use docker save to export the full image with all layers
        info!("Exporting Docker image '{}' to tarball...", image_name);
        self.run_command(&["save", "-o", tarball_path.to_str().unwrap(), image_name])?;

        // Store the temp_dir in a static variable to prevent it from being dropped
        // This ensures the temporary directory stays around until the program exits
        static TEMP_DIRS: once_cell::sync::Lazy<std::sync::Mutex<Vec<tempfile::TempDir>>> =
            once_cell::sync::Lazy::new(|| std::sync::Mutex::new(Vec::new()));

        // Add our tempdir to the list of preserved directories
        TEMP_DIRS.lock().unwrap().push(temp_dir);

        Ok(tarball_path)
    }
}
