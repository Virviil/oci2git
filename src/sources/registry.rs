use anyhow::{anyhow, Context, Result};
use oci_client::{Client, Reference};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use super::{naming, Source};
use crate::notifier::Notifier;

/// OCI Registry implementation of the Source trait
/// Pulls images directly from OCI-compliant registries
pub struct RegistrySource {
    client: Client,
}

impl RegistrySource {
    pub fn new() -> Result<Self> {
        let client = Client::default();
        Ok(Self { client })
    }

    async fn pull_image_async(
        &self,
        image_ref: &Reference,
        notifier: &Notifier,
    ) -> Result<(PathBuf, TempDir)> {
        notifier.info(&format!("Pulling OCI image '{}'...", image_ref));

        // Create a temporary directory to save the image
        let temp_dir = TempDir::new().context("Failed to create temporary directory")?;
        let tarball_path = temp_dir.path().join("image.tar");

        // Pull the image manifest
        let (manifest, _) = self
            .client
            .pull_manifest(image_ref)
            .await
            .context("Failed to pull manifest from registry")?;

        notifier.info("Manifest pulled successfully, downloading layers...");

        // Create tarball file
        let mut tarball_file = File::create(&tarball_path)
            .await
            .context("Failed to create tarball file")?;

        // Pull image layers and create a tarball
        // This is a simplified approach - in a full implementation, we would need to
        // properly construct an OCI image tarball with the correct directory structure
        // For now, we'll pull all the blobs and create a simple tar structure
        
        let layers = match &manifest {
            oci_client::manifest::OciImageManifest::Image(image_manifest) => {
                &image_manifest.layers
            }
            _ => return Err(anyhow!("Unsupported manifest type")),
        };

        notifier.info(&format!("Downloading {} layers...", layers.len()));

        // For each layer, pull the blob and add it to our tarball
        for (i, layer_descriptor) in layers.iter().enumerate() {
            notifier.info(&format!("Downloading layer {}/{}", i + 1, layers.len()));

            let layer_data = self
                .client
                .pull_blob(image_ref, &layer_descriptor.digest)
                .await
                .context(format!("Failed to pull layer {}", layer_descriptor.digest))?;

            // Write layer data to our tarball
            // Note: This is a simplified approach. A proper implementation would
            // structure the tarball according to OCI image layout specification
            tarball_file
                .write_all(&layer_data)
                .await
                .context("Failed to write layer data to tarball")?;
        }

        notifier.info(&format!(
            "Successfully pulled OCI image '{}' to tarball",
            image_ref
        ));

        Ok((tarball_path, temp_dir))
    }
}

impl Source for RegistrySource {
    fn name(&self) -> &str {
        "registry"
    }

    fn get_image_tarball(
        &self,
        image_name: &str,
        notifier: &Notifier,
    ) -> Result<(PathBuf, Option<TempDir>)> {
        // Parse the image reference
        let image_ref = Reference::try_from(image_name)
            .context(format!("Failed to parse image reference: {}", image_name))?;

        // Create a new async runtime for this operation
        let rt = tokio::runtime::Runtime::new()
            .context("Failed to create async runtime")?;

        // Run the async pull operation
        let (tarball_path, temp_dir) = rt
            .block_on(self.pull_image_async(&image_ref, notifier))
            .context("Failed to pull image from registry")?;

        Ok((tarball_path, Some(temp_dir)))
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
    fn test_registry_source_branch_name() {
        let source = RegistrySource::new().unwrap();
        assert_eq!(
            source.branch_name(
                "hello-world:latest",
                "linux-amd64",
                "sha256:1234567890abcdef"
            ),
            "hello-world#latest#linux-amd64#1234567890ab"
        );
        assert_eq!(
            source.branch_name(
                "localhost:5000/my-app:v1.0",
                "linux-arm64",
                "sha256:9876543210fedcba"
            ),
            "localhost#5000-my-app#v1.0#linux-arm64#9876543210fe"
        );
    }
}