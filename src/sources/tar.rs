use anyhow::{anyhow, Result};
use log::info;
use std::path::PathBuf;
use tempfile::TempDir;

use super::Source;

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

    fn get_image_tarball(&self, image_path: &str) -> Result<(PathBuf, Option<TempDir>)> {
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
            info!("Warning: File does not have .tar extension. Proceeding anyway, but this might not be a valid image tarball.");
        }

        // Just return the existing path - no temp dir needed for tar source
        Ok((tarball_path, None))
    }
}
