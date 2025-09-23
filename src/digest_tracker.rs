//! Tracks and validates container image layer lineage.
//!
//! This module provides [`DigestTracker`], which:
//! - Records layer metadata in build order (digest, command, created, empty, comment).
//! - Loads existing history from `Image.md` via `image_metadata::ImageMetadata`.
//! - Compares recorded entries with `extracted_image::Layer` to detect continuity.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerDigest {
    pub digest: String,
    pub command: String,
    pub created: String,
    pub is_empty: bool,
    /// Additional comment for empty layers
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestTracker {
    /// Layer digest info in sequential order (0-based indexing)
    pub layer_digests: Vec<LayerDigest>,
}

impl DigestTracker {
    pub fn new() -> Self {
        Self {
            layer_digests: Vec::new(),
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(path).context("Failed to read Image.md")?;
        let image_metadata = crate::image_metadata::ImageMetadata::parse_markdown(&content)
            .context("Failed to parse Image.md")?;
        let tracker = Self {
            layer_digests: image_metadata.layer_digests,
        };
        Ok(tracker)
    }

    pub fn add_layer(
        &mut self,
        position: usize,
        digest: String,
        command: String,
        created: String,
        is_empty: bool,
        comment: Option<String>,
    ) {
        let layer_digest = LayerDigest {
            digest,
            command,
            created,
            is_empty,
            comment,
        };

        // Layers should be added sequentially, so position should equal current length
        assert_eq!(
            position,
            self.layer_digests.len(),
            "Layers must be added sequentially. Expected position {}, got {}",
            self.layer_digests.len(),
            position
        );

        self.layer_digests.push(layer_digest);
    }

    pub fn get_layer(&self, position: usize) -> Option<&LayerDigest> {
        self.layer_digests.get(position)
    }

    /// Check if this tracker's layer at position matches the given layer
    /// Used by SuccessorNavigator for layer comparison across branches
    pub fn layer_matches(&self, position: usize, layer: &crate::extracted_image::Layer) -> bool {
        if let Some(existing_layer) = self.get_layer(position) {
            self.layers_match(existing_layer, layer)
        } else {
            false
        }
    }

    fn layers_match(&self, existing: &LayerDigest, new: &crate::extracted_image::Layer) -> bool {
        if existing.is_empty != new.is_empty {
            return false;
        }

        // First check if timestamps match - layers must have same creation time
        let new_timestamp = new.created_at.to_rfc3339();
        // Handle both Z and +00:00 timezone formats
        let existing_normalized = existing.created.replace("Z", "+00:00");
        let new_normalized = new_timestamp.replace("Z", "+00:00");
        if existing_normalized != new_normalized {
            return false;
        }

        if existing.is_empty {
            // For empty layers, compare command (timestamp already matched above)
            existing.command == new.command
        } else {
            // For non-empty layers, compare digest (timestamp already matched above)
            // Extract digest from layer ID (which is filename of tarball path)
            let new_digest = Self::extract_digest_from_layer_id(&new.id);
            existing.digest == new_digest
        }
    }

    fn extract_digest_from_layer_id(layer_id: &str) -> String {
        // Layer ID is typically the filename of the tarball path like "abc123def456"
        // The full digest would be "sha256:abc123def456"
        if layer_id.starts_with("sha256:") {
            layer_id.to_string()
        } else if layer_id.starts_with('<') && layer_id.ends_with('>') {
            // Empty layer ID like "<empty-layer-1>"
            layer_id.to_string()
        } else {
            // Assume it's just the hash part
            format!("sha256:{}", layer_id)
        }
    }

    /// Get the digest that should be used for a layer tarball path
    pub fn extract_digest_from_tarball_path<P: AsRef<Path>>(tarball_path: P) -> String {
        let path = tarball_path.as_ref();

        // Handle paths like "blobs/sha256/abc123def456" or just "abc123def456"
        if let Some(parent) = path.parent() {
            if parent.file_name().map(|s| s.to_str()) == Some(Some("sha256")) {
                // This is a blob path like "blobs/sha256/digest"
                if let Some(digest) = path.file_name().and_then(|s| s.to_str()) {
                    return format!("sha256:{}", digest);
                }
            }
        }

        // Fallback: use filename as digest
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            if filename.starts_with("sha256:") {
                filename.to_string()
            } else {
                format!("sha256:{}", filename)
            }
        } else {
            "unknown".to_string()
        }
    }
}

impl Default for DigestTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn test_digest_tracker_creation() {
        let tracker = DigestTracker::new();
        assert_eq!(tracker.layer_digests.len(), 0);
        assert!(tracker.layer_digests.is_empty());
    }

    #[test]
    fn test_add_and_get_layer() {
        let mut tracker = DigestTracker::new();

        tracker.add_layer(
            0,
            "sha256:abc123".to_string(),
            "FROM alpine".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            false,
            None,
        );

        let layer = tracker.get_layer(0).unwrap();
        assert_eq!(layer.digest, "sha256:abc123");
        assert_eq!(layer.command, "FROM alpine");
        assert!(!layer.is_empty);
    }

    #[test]
    fn test_load_from_image_md() {
        let temp_dir = tempdir().unwrap();
        let image_md_path = temp_dir.path().join("Image.md");

        // Create a sample Image.md with layer history
        let image_md_content = r#"# Image: test:latest

## Basic Information

- **Name**: test:latest
- **ID**: `sha256:test123`

## Layer History

| Created | Command | Comment | Digest | Empty |
|---------|---------|---------|--------|-------|
| 2023-01-01T00:00:00Z | `FROM alpine` | buildkit.dockerfile.v0 | `sha256:abc123` | false |
"#;
        std::fs::write(&image_md_path, image_md_content).unwrap();

        // Load
        let loaded_tracker = DigestTracker::load_from_file(&image_md_path).unwrap();
        assert_eq!(loaded_tracker.layer_digests.len(), 1);

        let layer = loaded_tracker.get_layer(0).unwrap();
        assert_eq!(layer.digest, "sha256:abc123");
        assert_eq!(layer.command, "FROM alpine");
        assert!(!layer.is_empty);
    }

    #[test]
    fn test_extract_digest_from_tarball_path() {
        // Test blob path
        let digest1 = DigestTracker::extract_digest_from_tarball_path("blobs/sha256/abc123def456");
        assert_eq!(digest1, "sha256:abc123def456");

        // Test simple filename
        let digest2 = DigestTracker::extract_digest_from_tarball_path("abc123def456");
        assert_eq!(digest2, "sha256:abc123def456");

        // Test already prefixed
        let digest3 = DigestTracker::extract_digest_from_tarball_path("sha256:abc123def456");
        assert_eq!(digest3, "sha256:abc123def456");
    }

    #[test]
    fn test_layer_matches() {
        let mut tracker = DigestTracker::new();

        // Add some existing layers
        tracker.add_layer(
            0,
            "sha256:layer1".to_string(),
            "FROM alpine".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            false,
            None,
        );
        tracker.add_layer(
            1,
            "sha256:layer2".to_string(),
            "RUN apk add curl".to_string(),
            "2023-01-01T01:00:00Z".to_string(),
            false,
            None,
        );
        tracker.add_layer(
            2,
            "empty".to_string(),
            "ENV PATH=/bin".to_string(),
            "2023-01-01T02:00:00Z".to_string(),
            true,
            None,
        );

        // Test matching layers - use same timestamps as in tracker
        let matching_layer1 = crate::extracted_image::Layer {
            id: "layer1".to_string(),
            command: "FROM alpine".to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_empty: false,
            tarball_path: Some(std::path::PathBuf::from("layer1.tar")),
            digest: "sha256:layer1".to_string(),
            comment: Some("FROM alpine".to_string()),
        };
        assert!(tracker.layer_matches(0, &matching_layer1));

        let matching_layer2 = crate::extracted_image::Layer {
            id: "layer2".to_string(),
            command: "RUN apk add curl".to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339("2023-01-01T01:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_empty: false,
            tarball_path: Some(std::path::PathBuf::from("layer2.tar")),
            digest: "sha256:layer2".to_string(),
            comment: Some("RUN apk add curl".to_string()),
        };
        assert!(tracker.layer_matches(1, &matching_layer2));

        // Test non-matching layer (different command, same timestamp)
        let non_matching_layer = crate::extracted_image::Layer {
            id: "<empty-layer-2>".to_string(),
            command: "ENV NEWVAR=value".to_string(), // Different command
            created_at: chrono::DateTime::parse_from_rfc3339("2023-01-01T02:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_empty: true,
            tarball_path: None,
            digest: "empty".to_string(),
            comment: Some("ENV NEWVAR=value".to_string()),
        };
        assert!(!tracker.layer_matches(2, &non_matching_layer));

        // Test timestamp mismatch (same command, different timestamp)
        let timestamp_mismatch_layer = crate::extracted_image::Layer {
            id: "<empty-layer-2>".to_string(),
            command: "ENV PATH=/bin".to_string(), // Same command
            created_at: chrono::DateTime::parse_from_rfc3339("2023-01-01T03:00:00Z")
                .unwrap()
                .with_timezone(&Utc), // Different timestamp
            is_empty: true,
            tarball_path: None,
            digest: "empty".to_string(),
            comment: Some("ENV PATH=/bin".to_string()),
        };
        assert!(!tracker.layer_matches(2, &timestamp_mismatch_layer));

        // Test out of bounds
        assert!(!tracker.layer_matches(10, &matching_layer1));
    }
}
