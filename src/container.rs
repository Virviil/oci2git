use crate::metadata::{self, ImageMetadata};
use crate::sources::Source;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use log::{debug, trace};
use oci_spec::image::ImageConfiguration;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub trait ContainerEngine: std::any::Any {
    fn get_layers(&self, extract_dir: &Path) -> Result<Vec<Layer>>;
    fn extract_image(&self, extract_dir: &Path, output_dir: &Path) -> Result<()>;
    fn get_metadata(&self, extract_dir: &Path) -> Result<ImageMetadata>;
    fn get_layer_tarballs(&self, _extract_dir: &Path) -> Result<Vec<PathBuf>> {
        // Default implementation - not all engines will implement this
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: String,
    pub command: String,
    pub created_at: DateTime<Utc>,
    pub is_empty: bool,
}

pub struct DockerEngine {
    source: crate::sources::DockerSource,
}

impl DockerEngine {
    pub fn new() -> Result<Self> {
        let source = crate::sources::DockerSource::new()?;
        Ok(Self { source })
    }

    /// Downloads and extracts an image once, returning the directory with the extracted image
    /// This can be used by other methods to avoid downloading and extracting the same image multiple times
    pub fn download_image(&self, image_name: &str) -> Result<PathBuf> {
        log::info!("Downloading and extracting image: {}", image_name);
        self.source.get_oci_image(image_name)
    }
}

impl ContainerEngine for DockerEngine {
    fn get_layers(&self, extract_dir: &Path) -> Result<Vec<Layer>> {
        // Parse the Docker manifest to get the config file path
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

        // Get the actual layer paths (tarballs)
        let layer_tarballs = self.get_layer_tarballs(extract_dir)?;

        // We need to track which history entries have associated layer blobs
        // Some history entries (empty layers) don't have associated blobs
        let mut current_tarball_idx = 0;
        let mut layers = Vec::new();

        // Track each history entry
        debug!("Docker image history entries (oldest to newest):");

        // History is usually stored newest to oldest, so process in reverse
        for (i, hist_entry) in history.iter().enumerate().rev() {
            // Get created info
            let created_at_str = hist_entry["created"]
                .as_str()
                .unwrap_or("1970-01-01T00:00:00Z");

            let created_at = DateTime::parse_from_rfc3339(created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            // Get history command - this is the full, untruncated command from the image metadata
            let created_by = hist_entry["created_by"].as_str().unwrap_or("");

            // Clean up the command by removing the shell prefix and any trailing whitespace
            // This preserves special syntax like |9 in commands
            let command = if created_by.contains("/bin/sh -c #(nop) ") {
                // For non-execution instructions, remove the shell prefix and trim any leading whitespace
                created_by.replace("/bin/sh -c #(nop) ", "").trim_start().to_string()
            } else if created_by.contains("/bin/sh -c ") {
                // For execution instructions, remove the shell prefix and trim any leading whitespace
                created_by.replace("/bin/sh -c ", "").trim_start().to_string()
            } else {
                // For other instructions, keep the entire command
                created_by.to_string()
            };

            // IMPORTANT: Read the empty_layer field directly from history entry
            // This is the proper way to detect empty layers in the OCI spec
            let is_empty = hist_entry["empty_layer"].as_bool().unwrap_or(false);

            // For non-empty layers, assign a tarball path
            let id = if !is_empty && current_tarball_idx < layer_tarballs.len() {
                let tarball_path = &layer_tarballs[current_tarball_idx];
                current_tarball_idx += 1;

                // Use the filename part of the tarball path as the ID
                tarball_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("layer-{}", i))
            } else {
                format!("<empty-layer-{}>", i)
            };

            trace!(
                "Layer {}: {} | Empty: {} | Command: {}",
                i, // Zero-based indexing
                id,
                is_empty,
                command
            );

            layers.push(Layer {
                id,
                command,
                created_at,
                is_empty,
            });
        }

        // Since we processed history in reverse order, reverse the layers to get oldest first
        layers.reverse();

        debug!("Found {} layers in image history", layers.len());

        Ok(layers)
    }

    fn extract_image(&self, extract_dir: &Path, output_dir: &Path) -> Result<()> {
        // Use get_layer_tarballs to extract tarball paths in correct order
        let layer_tarballs = self.get_layer_tarballs(extract_dir)?;

        // Create the output directory
        fs::create_dir_all(output_dir)?;

        // For compatibility with tests and other code, we extract at least one layer
        // In practice, this will be overridden by the layer-by-layer extraction in the converter
        if !layer_tarballs.is_empty() {
            let layer_path = &layer_tarballs[0];

            // Extract this layer tarball to the output directory
            let extract_status = Command::new("tar")
                .args([
                    "-xf",
                    layer_path.to_str().unwrap(),
                    "-C",
                    output_dir.to_str().unwrap(),
                ])
                .status()
                .context(format!("Failed to extract layer tarball: {:?}", layer_path))?;

            if !extract_status.success() {
                return Err(anyhow!(
                    "Failed to extract layer content from {:?}",
                    layer_path
                ));
            }
        }

        Ok(())
    }

    fn get_layer_tarballs(&self, extract_dir: &Path) -> Result<Vec<PathBuf>> {
        // Read the manifest.json to understand the layer order
        let manifest_path = extract_dir.join("manifest.json");
        let manifest_content =
            fs::read_to_string(&manifest_path).context("Failed to read manifest.json")?;

        let manifest: Vec<serde_json::Value> =
            serde_json::from_str(&manifest_content).context("Failed to parse manifest.json")?;

        if manifest.is_empty() {
            return Err(anyhow!("Empty manifest.json"));
        }

        // Get the ordered list of layer tarballs - these are typically in oldest to newest order
        let layers = manifest[0]["Layers"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid manifest format - missing Layers array"))?;

        // Store the layer tarball paths in order
        let mut layer_paths = Vec::new();

        debug!("Layer tarballs in manifest:");
        for (i, layer) in layers.iter().enumerate() {
            let layer_path = layer
                .as_str()
                .ok_or_else(|| anyhow!("Invalid layer path in manifest"))?;

            let full_path = extract_dir.join(layer_path);
            trace!("Layer {}: {}", i, layer_path);

            layer_paths.push(full_path);
        }

        Ok(layer_paths)
    }

    fn get_metadata(&self, extract_dir: &Path) -> Result<ImageMetadata> {
        // Parse the Docker manifest to get the config file path
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
        let config: ImageConfiguration =
            serde_json::from_str(&config_content).context("Failed to parse image configuration")?;

        // Convert to our metadata format
        let mut metadata = metadata::from_oci_config(&config);

        // Extract ID from the image name if present or from config file
        if let Some(id) = config_file.strip_suffix(".json") {
            metadata.id = format!("sha256:{}", id);
        }

        // Add repo tags from the manifest (these are not in the config)
        if let Some(tags) = manifest[0]["RepoTags"].as_array() {
            metadata.repo_tags = tags
                .iter()
                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                .collect();
        }

        Ok(metadata)
    }
}

// We'll define a NerdctlSource when implementing NerdctlEngine
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
    fn get_oci_image(&self, _image_name: &str) -> Result<PathBuf> {
        // This will be implemented in the future
        unimplemented!("nerdctl source is not yet implemented")
    }
}

pub struct NerdctlEngine {
    source: NerdctlSource,
}

impl NerdctlEngine {
    pub fn new() -> Result<Self> {
        let source = NerdctlSource::new()?;
        Ok(Self { source })
    }
}

impl ContainerEngine for NerdctlEngine {
    fn get_layers(&self, _extract_dir: &Path) -> Result<Vec<Layer>> {
        // Will use self.source.get_oci_image() when implemented
        let _unused = &self.source; // Prevents unused field warning
        unimplemented!("nerdctl support is not yet implemented")
    }

    fn extract_image(&self, _extract_dir: &Path, _output_dir: &Path) -> Result<()> {
        // Will use self.source.get_oci_image() when implemented
        unimplemented!("nerdctl support is not yet implemented")
    }

    fn get_metadata(&self, _extract_dir: &Path) -> Result<ImageMetadata> {
        // Will use self.source.get_oci_image() when implemented
        unimplemented!("nerdctl support is not yet implemented")
    }
}

#[cfg(any(test, feature = "test-utils"))]
pub mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::HashMap;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // Mock implementation of ContainerEngine for testing
    pub struct MockContainerEngine {
        pub layers: Vec<Layer>,
        pub metadata: ImageMetadata,
        pub test_files: Vec<(String, String)>, // (filename, content)
        // We'll split the test files into separate layer files for testing the layer-by-layer approach
        pub layer_files: HashMap<usize, Vec<(String, String)>>,
    }

    impl MockContainerEngine {
        pub fn new() -> Self {
            // Create sample layers
            let layers = vec![
                Layer {
                    id: "layer1".to_string(),
                    command: "FROM base".to_string(),
                    created_at: Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap(),
                    is_empty: false,
                },
                Layer {
                    id: "layer2".to_string(),
                    command: "RUN echo hello".to_string(),
                    created_at: Utc.with_ymd_and_hms(2023, 1, 1, 1, 0, 0).unwrap(),
                    is_empty: false,
                },
                Layer {
                    id: "layer3".to_string(),
                    command: "ENV FOO=bar".to_string(),
                    created_at: Utc.with_ymd_and_hms(2023, 1, 1, 2, 0, 0).unwrap(),
                    is_empty: true,
                },
            ];

            // Create sample metadata
            let mut env = Vec::new();
            env.push("PATH=/usr/local/bin:/usr/bin".to_string());
            env.push("FOO=bar".to_string());

            let mut labels = HashMap::new();
            labels.insert("maintainer".to_string(), "test@example.com".to_string());

            let metadata = ImageMetadata {
                id: "sha256:mockimage".to_string(),
                repo_tags: vec!["mockimage:latest".to_string()],
                created: "2023-01-01T00:00:00Z".to_string(),
                container_config: crate::metadata::ContainerConfig {
                    env,
                    cmd: Some(vec!["bash".to_string()]),
                    entrypoint: None,
                    exposed_ports: None,
                    working_dir: Some("/app".to_string()),
                    volumes: None,
                    labels: Some(labels),
                },
                history: Vec::new(),
                architecture: "amd64".to_string(),
                os: "linux".to_string(),
            };

            // Sample test files (for backward compatibility)
            let test_files = vec![
                ("file1.txt".to_string(), "content of file 1".to_string()),
                (
                    "dir1/file2.txt".to_string(),
                    "content of file 2".to_string(),
                ),
                (
                    "dir1/dir2/file3.txt".to_string(),
                    "content of file 3".to_string(),
                ),
            ];

            // Split files by layer
            let mut layer_files = HashMap::new();

            // Layer 1: base filesystem
            layer_files.insert(
                0,
                vec![("file1.txt".to_string(), "initial content".to_string())],
            );

            // Layer 2: add some files
            layer_files.insert(
                1,
                vec![
                    (
                        "dir1/file2.txt".to_string(),
                        "content of file 2".to_string(),
                    ),
                    ("file1.txt".to_string(), "updated content".to_string()), // Modified file
                ],
            );

            // Layer 3: no files (empty layer)

            Self {
                layers,
                metadata,
                test_files,
                layer_files,
            }
        }
    }

    impl ContainerEngine for MockContainerEngine {
        fn get_layers(&self, _extract_dir: &Path) -> Result<Vec<Layer>> {
            Ok(self.layers.clone())
        }

        fn extract_image(&self, _extract_dir: &Path, output_dir: &Path) -> Result<()> {
            // Create test files in the output directory
            for (file_path, content) in &self.test_files {
                let full_path = output_dir.join(file_path);
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent).context("Failed to create parent directory")?;
                }
                let mut file = fs::File::create(&full_path).context("Failed to create file")?;
                file.write_all(content.as_bytes())
                    .context("Failed to write content")?;
            }
            Ok(())
        }

        fn get_metadata(&self, _extract_dir: &Path) -> Result<ImageMetadata> {
            Ok(self.metadata.clone())
        }

        fn get_layer_tarballs(&self, _extract_dir: &Path) -> Result<Vec<PathBuf>> {
            // Create temporary layer tarballs for testing
            let temp_dir = tempfile::tempdir()?;
            let mut layer_paths = Vec::new();

            // Create a tarball for each layer
            for (layer_idx, files) in &self.layer_files {
                if files.is_empty() {
                    continue;
                }

                // Create temporary directory for this layer's files
                let layer_files_dir = temp_dir.path().join(format!("layer{}", layer_idx));
                fs::create_dir_all(&layer_files_dir)?;

                // Create the files
                for (file_path, content) in files {
                    let full_path = layer_files_dir.join(file_path);
                    if let Some(parent) = full_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut file = fs::File::create(&full_path)?;
                    file.write_all(content.as_bytes())?;
                }

                // Create a tarball for this layer
                let tarball_path = temp_dir.path().join(format!("layer{}.tar", layer_idx));

                // Create the tarball using the tar command
                let status = std::process::Command::new("tar")
                    .current_dir(&layer_files_dir)
                    .args(["-cf", tarball_path.to_str().unwrap(), "."])
                    .status()
                    .context("Failed to create layer tarball")?;

                if !status.success() {
                    return Err(anyhow!("Failed to create layer tarball"));
                }

                layer_paths.push(tarball_path);
            }

            // We need to return owned paths that won't be deleted when temp_dir is dropped
            let owned_paths: Vec<PathBuf> = layer_paths.iter().map(|p| p.to_owned()).collect();

            // Store the temp_dir to prevent it from being dropped and deleting our tarballs
            static TEMP_DIRS: once_cell::sync::Lazy<std::sync::Mutex<Vec<tempfile::TempDir>>> =
                once_cell::sync::Lazy::new(|| std::sync::Mutex::new(Vec::new()));

            TEMP_DIRS.lock().unwrap().push(temp_dir);

            Ok(owned_paths)
        }
    }

    #[test]
    fn test_mock_container_engine() {
        let engine = MockContainerEngine::new();
        let dummy_path = Path::new("/tmp");

        // Test get_layers
        let layers = engine.get_layers(dummy_path).unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].command, "FROM base");
        assert_eq!(layers[1].command, "RUN echo hello");
        assert_eq!(layers[2].command, "ENV FOO=bar");
        assert!(layers[2].is_empty);

        // Test get_metadata
        let metadata = engine.get_metadata(dummy_path).unwrap();
        assert_eq!(metadata.id, "sha256:mockimage");
        assert_eq!(metadata.repo_tags, vec!["mockimage:latest"]);
        assert_eq!(metadata.container_config.env.len(), 2);
        assert_eq!(metadata.container_config.env[1], "FOO=bar");
        assert_eq!(
            metadata.container_config.cmd,
            Some(vec!["bash".to_string()])
        );
        assert_eq!(
            metadata.container_config.working_dir,
            Some("/app".to_string())
        );

        // Test extract_image
        let temp_dir = tempdir().unwrap();
        engine.extract_image(dummy_path, temp_dir.path()).unwrap();

        let file1_path = temp_dir.path().join("file1.txt");
        let file2_path = temp_dir.path().join("dir1/file2.txt");
        let file3_path = temp_dir.path().join("dir1/dir2/file3.txt");

        assert!(file1_path.exists());
        assert!(file2_path.exists());
        assert!(file3_path.exists());

        let content1 = fs::read_to_string(file1_path).unwrap();
        let content2 = fs::read_to_string(file2_path).unwrap();
        let content3 = fs::read_to_string(file3_path).unwrap();

        assert_eq!(content1, "content of file 1");
        assert_eq!(content2, "content of file 2");
        assert_eq!(content3, "content of file 3");
    }
}
