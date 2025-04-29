use anyhow::{anyhow, Context, Result};
use log::info;
use std::path::PathBuf;
use std::process::Command;
use tempfile;

/// Source trait for getting OCI images from different container sources
pub trait Source {
    /// Retrieves an OCI image and extracts it to a temporary directory
    /// Returns the path to the directory containing the extracted image
    fn get_oci_image(&self, image_name: &str) -> Result<PathBuf>;
}

/// Docker implementation of the Source trait
pub struct DockerSource;

impl DockerSource {
    pub fn new() -> Result<Self> {
        let output = Command::new("docker")
            .arg("--version")
            .output()
            .context("Failed to execute docker command. Is Docker installed and running?")?;

        if !output.status.success() {
            return Err(anyhow!("Docker is not available"));
        }

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
    fn get_oci_image(&self, image_name: &str) -> Result<PathBuf> {
        // Create a temporary directory to save the image
        let temp_dir = tempfile::tempdir().context("Failed to create temporary directory")?;
        let archive_path = temp_dir.path().join("image.tar");

        // Use docker save to export the full image with all layers
        info!("Exporting Docker image to tarball...");
        self.run_command(&["save", "-o", archive_path.to_str().unwrap(), image_name])?;

        // Extract the image archive to get access to layers
        let extract_dir = temp_dir.path().join("extracted");
        std::fs::create_dir_all(&extract_dir)?;

        info!("Extracting Docker image archive...");
        let status = Command::new("tar")
            .args([
                "-xf",
                archive_path.to_str().unwrap(),
                "-C",
                extract_dir.to_str().unwrap(),
            ])
            .status()
            .context("Failed to extract image archive")?;

        if !status.success() {
            return Err(anyhow!("Failed to extract Docker image archive"));
        }

        // Store the temp_dir in a static variable to prevent it from being dropped
        // This ensures the temporary directory stays around until the program exits
        static EXTRACT_DIRS: once_cell::sync::Lazy<std::sync::Mutex<Vec<tempfile::TempDir>>> =
            once_cell::sync::Lazy::new(|| std::sync::Mutex::new(Vec::new()));

        // Add our tempdir to the list of preserved directories
        EXTRACT_DIRS.lock().unwrap().push(temp_dir);

        Ok(extract_dir)
    }
}
