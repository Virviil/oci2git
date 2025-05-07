use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

use super::Source;

pub struct NerdctlSource;

impl NerdctlSource {
    pub fn new() -> Result<Self> {
        let output = Command::new("nerdctl")
            .arg("--version")
            .output()
            .context("Failed to execute nerdctl command. Is nerdctl installed?")?;

        if !output.status.success() {
            return Err(anyhow!("nerdctl is not available"));
        }

        Ok(Self)
    }
}

impl Source for NerdctlSource {
    fn name(&self) -> &str {
        "nerdctl"
    }

    fn get_image_tarball(&self, _image_name: &str) -> Result<(PathBuf, Option<TempDir>)> {
        // This will be implemented in the future
        unimplemented!("nerdctl support is not yet implemented")
    }
}
