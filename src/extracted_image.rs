//! Extract an OCI/Docker image tarball into a typed, queryable structure.
//!
//! [`ExtractedImage`] unwraps a `docker save`/OCI image tarball into:
//! - High-level [`ImageMetadata`] (id, repo tags, os, architecture).
//! - Ordered [`Layer`] records (oldest → newest) with:
//!   - `id` (derived from blob filename or `<empty-layer-N>`),
//!   - normalized `command` (shell prefix stripped),
//!   - `created_at` (`chrono::DateTime<Utc>`),
//!   - `is_empty` (from history `empty_layer`),
//!   - `tarball_path` (`Some` for non-empty),
//!   - `digest` (`sha256:<hash>` for blobs, `"empty"` for empty).
//!
//! Key behavior:
//! - Supports plain `.tar` and gzip (`.tar.gz`) by checking magic bytes, then invoking `tar`.
//! - Validates expected layout (`manifest.json` required).
//! - Loads metadata from `manifest.json`, `index.json`, and the config JSON
//!   (prefers manifest digest; falls back to config path).
//! - Maps history entries to blob layers by walking history in reverse and pairing
//!   them with manifest `Layers`, then re-reverses to chronological order.
//! - Canonicalizes layer digests via `digest_tracker::DigestTracker::extract_digest_from_tarball_path`.
//!
//! Public API highlights:
//! - [`ExtractedImage::from_tarball`] — extract + parse into memory (with progress via [`Notifier`]).
//! - [`ExtractedImage::metadata`] / [ExtractedImage::os] / [ExtractedImage::architecture] — access image facts.
//! - [`ExtractedImage::layers`] — get the ordered layer list.
//! - [`ExtractedImage::extract_layer_to`] — unpack a single layer tarball into a directory.
//! - [`ExtractedImage::extract_dir`] — path to the temporary extraction root.
//!
//! Errors include malformed manifests/configs, missing files, or `tar` failures.
//! Temporary extraction is scoped to the instance lifetime via `tempfile::TempDir`.

use crate::metadata::{self, ImageMetadata};
use crate::notifier::Notifier;
use crate::tar_extractor;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: String,
    pub command: String,
    pub created_at: DateTime<Utc>,
    pub is_empty: bool,
    pub tarball_path: Option<std::path::PathBuf>, // Some for non-empty layers, None for empty layers
    pub digest: String, // Always present - either tarball digest or "empty" for empty layers
    pub comment: Option<String>, // Comment from image layer history
}

pub struct ExtractedImage {
    extract_dir: PathBuf,
    _temp_dir: tempfile::TempDir,
    metadata: ImageMetadata,
    layers: Vec<Layer>,
}

impl ExtractedImage {
    pub fn from_tarball<P: AsRef<Path>>(tarball_path: P, notifier: &Notifier) -> Result<Self> {
        let tarball_path = tarball_path.as_ref();

        notifier.debug(&format!("Extracting image tarball: {:?}", tarball_path));

        // Create a temporary directory for extraction
        let temp_dir = tempfile::tempdir().context("Failed to create temporary directory")?;
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir_all(&extract_dir)?;

        // Extract the tarball
        Self::extract_tar_file(tarball_path, &extract_dir)?;

        // Verify the extracted content has the expected OCI structure
        let manifest_path = extract_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err(anyhow!(
        "Invalid image tarball: manifest.json not found. This does not appear to be a valid OCI/Docker image tarball."
      ));
        }

        // Load metadata and layers using static helper methods
        notifier.debug("Loading image metadata...");
        let metadata = Self::load_metadata_from_dir(&extract_dir, "temp")?;

        notifier.debug("Loading image layers...");
        let layers = Self::load_layers_from_dir(&extract_dir)?;

        notifier.info(&format!("Successfully loaded {} layers", layers.len()));

        Ok(ExtractedImage {
            extract_dir,
            _temp_dir: temp_dir,
            metadata,
            layers,
        })
    }

    pub fn metadata(&self, _image_name: &str) -> Result<ImageMetadata> {
        // Return the metadata as-is, keeping the proper SHA digest as ID
        Ok(self.metadata.clone())
    }

    pub fn os(&self, image_name: &str) -> Result<String> {
        Ok(self.metadata(image_name)?.os)
    }

    pub fn architecture(&self, image_name: &str) -> Result<String> {
        Ok(self.metadata(image_name)?.architecture)
    }

    pub fn layers(&self) -> Result<Vec<Layer>> {
        Ok(self.layers.clone())
    }

    pub fn extract_layer_to<P: AsRef<Path>>(
        &self,
        layer_tarball: &Path,
        output_dir: P,
    ) -> Result<()> {
        let output_dir = output_dir.as_ref();
        fs::create_dir_all(output_dir)?;
        Self::extract_tar_file(layer_tarball, output_dir)
    }

    pub fn extract_dir(&self) -> &Path {
        &self.extract_dir
    }

    fn extract_tar_file(tar_path: &Path, extract_dir: &Path) -> Result<()> {
        tar_extractor::extract_tar(tar_path, extract_dir)
            .context(format!("Failed to extract tar file: {:?}", tar_path))
    }

    fn load_metadata_from_dir(extract_dir: &Path, image_name: &str) -> Result<ImageMetadata> {
        // Parse the manifest to get the config file path
        let manifest_path = extract_dir.join("manifest.json");
        let manifest_content =
            fs::read_to_string(&manifest_path).context("Failed to read manifest.json")?;

        let manifest: Vec<serde_json::Value> =
            serde_json::from_str(&manifest_content).context("Failed to parse manifest.json")?;

        if manifest.is_empty() {
            return Err(anyhow!("Empty manifest.json"));
        }

        // Get the config file name from the manifest
        let config_file = manifest[0]["Config"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid manifest format - missing Config"))?;

        // Read the config file as JSON
        let config_path = extract_dir.join(config_file);
        let config_content = fs::read_to_string(&config_path)
            .context(format!("Failed to read config file: {}", config_file))?;

        // Parse as OCI ImageConfiguration
        let config: oci_spec::image::ImageConfiguration =
            serde_json::from_str(&config_content).context("Failed to parse image configuration")?;

        // Convert to our metadata format
        let mut metadata = metadata::from_oci_config(&config);

        // Extract the image manifest digest from index.json (this matches what docker image inspect shows as ID)
        let index_path = extract_dir.join("index.json");
        if index_path.exists() {
            let index_content =
                fs::read_to_string(&index_path).context("Failed to read index.json")?;
            let index: serde_json::Value =
                serde_json::from_str(&index_content).context("Failed to parse index.json")?;

            if let Some(manifests) = index["manifests"].as_array() {
                if let Some(first_manifest) = manifests.first() {
                    if let Some(digest) = first_manifest["digest"].as_str() {
                        metadata.id = digest.to_string();
                    }
                }
            }
        }

        // Fallback: Extract digest from config file path (format: blobs/sha256/HASH)
        if metadata.id.is_empty() {
            if let Some(digest_hash) = config_file.strip_prefix("blobs/sha256/") {
                metadata.id = format!("sha256:{}", digest_hash);
            } else if let Some(digest_hash) = config_file.strip_suffix(".json") {
                metadata.id = format!("sha256:{}", digest_hash);
            }
        }

        // Add repo tags from the manifest (these are not in the config)
        if let Some(tags) = manifest[0]["RepoTags"].as_array() {
            metadata.repo_tags = tags
                .iter()
                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                .collect();
        }

        // If no repo tags were found, add a placeholder tag from the image_name
        if metadata.repo_tags.is_empty() {
            let path = PathBuf::from(image_name);
            if let Some(filename) = path.file_stem() {
                if let Some(name) = filename.to_str() {
                    metadata.repo_tags.push(format!("{}:latest", name));
                }
            }
        }

        Ok(metadata)
    }

    fn load_layers_from_dir(extract_dir: &Path) -> Result<Vec<Layer>> {
        // Parse the manifest to get the config file path
        let manifest_path = extract_dir.join("manifest.json");
        let manifest_content =
            fs::read_to_string(&manifest_path).context("Failed to read manifest.json")?;

        let manifest: Vec<serde_json::Value> =
            serde_json::from_str(&manifest_content).context("Failed to parse manifest.json")?;

        if manifest.is_empty() {
            return Err(anyhow!("Empty manifest.json"));
        }

        // Get the config file name from the manifest
        let config_file = manifest[0]["Config"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid manifest format - missing Config"))?;

        // Read the config file as JSON
        let config_path = extract_dir.join(config_file);
        let config_content = fs::read_to_string(&config_path)
            .context(format!("Failed to read config file: {}", config_file))?;

        let config: serde_json::Value =
            serde_json::from_str(&config_content).context("Failed to parse image configuration")?;

        // Get history from the config - this contains info about empty layers
        let history = config["history"]
            .as_array()
            .ok_or_else(|| anyhow!("No history found in image configuration"))?;

        // Get the actual layer paths (tarballs) from manifest
        let layers_list = manifest[0]["Layers"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid manifest format - missing Layers array"))?;

        let mut layer_tarballs = Vec::new();
        for layer_ref in layers_list {
            let layer_path = layer_ref
                .as_str()
                .ok_or_else(|| anyhow!("Invalid layer reference"))?;
            let full_path = extract_dir.join(layer_path);
            layer_tarballs.push(full_path);
        }

        // We need to track which history entries have associated layer blobs
        // Since we process history in reverse (newest to oldest), we need to also
        // process tarballs in reverse to maintain correct mapping
        let mut current_tarball_idx = layer_tarballs.len();
        let mut layers = Vec::new();

        // History is usually stored newest to oldest, so process in reverse
        for (i, hist_entry) in history.iter().enumerate().rev() {
            // Get created info
            let created_at_str = hist_entry["created"]
                .as_str()
                .unwrap_or("1970-01-01T00:00:00Z");

            let created_at = DateTime::parse_from_rfc3339(created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            // Get history command
            let created_by = hist_entry["created_by"].as_str().unwrap_or("");

            // Clean up the command by removing the shell prefix
            let command = if created_by.contains("/bin/sh -c #(nop) ") {
                created_by
                    .replace("/bin/sh -c #(nop) ", "")
                    .trim_start()
                    .to_string()
            } else if created_by.contains("/bin/sh -c ") {
                created_by
                    .replace("/bin/sh -c ", "")
                    .trim_start()
                    .to_string()
            } else {
                created_by.to_string()
            };

            // Read the empty_layer field directly from history entry
            let is_empty = hist_entry["empty_layer"].as_bool().unwrap_or(false);

            // Extract comment from history entry
            let comment = hist_entry["comment"].as_str().map(|s| s.to_string());

            // For non-empty layers, assign a tarball path and digest
            let (id, tarball_path, digest) = if !is_empty && current_tarball_idx > 0 {
                current_tarball_idx -= 1;
                let tarball = &layer_tarballs[current_tarball_idx];

                // Use the filename part of the tarball path as the ID
                let id = tarball
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("layer-{}", i));

                // Extract digest from tarball path
                let digest =
                    crate::digest_tracker::DigestTracker::extract_digest_from_tarball_path(tarball);

                (id, Some(tarball.clone()), digest)
            } else {
                // Empty layer or no tarball available
                let id = format!("<empty-layer-{}>", i);
                let digest = if is_empty {
                    "empty".to_string()
                } else {
                    "no-tarball".to_string()
                };
                (id, None, digest)
            };

            layers.push(Layer {
                id,
                command,
                created_at,
                is_empty,
                tarball_path,
                digest,
                comment,
            });
        }

        // Since we processed history in reverse order, reverse the layers to get oldest first
        layers.reverse();

        Ok(layers)
    }
}
