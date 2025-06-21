use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

use super::Source;
use crate::notifier::Notifier;

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

    fn get_image_tarball(
        &self,
        _image_name: &str,
        _notifier: &Notifier,
    ) -> Result<(PathBuf, Option<TempDir>)> {
        // This will be implemented in the future
        unimplemented!("nerdctl support is not yet implemented")
    }

    fn branch_name(&self, image_name: &str, os_arch: &str, image_digest: &str) -> String {
        // nerdctl uses docker-like naming
        // If no tag is specified, add "latest" as the default tag
        let normalized = if !image_name.contains(':') && !image_name.contains('@') {
            format!("{}:latest", image_name)
        } else {
            image_name.to_string()
        };

        let base_branch = normalized
            .replace(":", "#")
            .replace("/", "-")
            .replace("@", "-");

        if let Some(short_digest) = super::extract_short_digest(image_digest) {
            format!("{}#{}#{}", base_branch, os_arch, short_digest)
        } else {
            // Fallback: use image_digest as-is if it doesn't have sha256: prefix
            format!("{}#{}#{}", base_branch, os_arch, image_digest)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nerdctl_source_branch_name() {
        // Create a source directly without checking if nerdctl is available
        let source = NerdctlSource;
        // The processor always provides the os_arch and digest extracted from image metadata
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
