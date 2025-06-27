use crate::metadata::{self, ImageMetadata};
use crate::notifier::Notifier;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tar::Archive;

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

    pub fn metadata(&self, image_name: &str) -> Result<ImageMetadata> {
        // Override the image name with the provided one
        let mut metadata = self.metadata.clone();
        metadata.id = image_name.to_string();
        Ok(metadata)
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
        // Try to detect if the file is gzip compressed by checking the magic bytes
        let mut file_for_detection = File::open(tar_path)?;
        let mut magic_bytes = [0u8; 2];
        file_for_detection.read_exact(&mut magic_bytes)?;

        if magic_bytes == [0x1f, 0x8b] {
            // This is a gzip file
            let file = File::open(tar_path)?;
            let reader = BufReader::new(file);
            let decoder = GzDecoder::new(reader);
            let mut archive = Archive::new(decoder);
            archive
                .unpack(extract_dir)
                .context(format!("Failed to extract gzip tar file: {:?}", tar_path))?;
        } else {
            // This is a plain tar file
            let file = File::open(tar_path)?;
            let reader = BufReader::new(file);
            let mut archive = Archive::new(reader);
            archive
                .unpack(extract_dir)
                .context(format!("Failed to extract tar file: {:?}", tar_path))?;
        }

        Ok(())
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

        // Extract digest from config file path (format: blobs/sha256/HASH)
        if let Some(digest_hash) = config_file.strip_prefix("blobs/sha256/") {
            metadata.id = format!("sha256:{}", digest_hash);
        } else if let Some(digest_hash) = config_file.strip_suffix(".json") {
            metadata.id = format!("sha256:{}", digest_hash);
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
