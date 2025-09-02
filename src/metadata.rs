use anyhow::Result;
use oci_spec::image::ImageConfiguration;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// Keeping the original types for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(default, rename = "RepoTags")]
    pub repo_tags: Vec<String>,
    #[serde(rename = "Created")]
    pub created: String,
    #[serde(rename = "Config")]
    pub container_config: ContainerConfig,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(rename = "Architecture")]
    pub architecture: String,
    #[serde(rename = "Os")]
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    #[serde(default, rename = "Env")]
    pub env: Vec<String>,
    #[serde(rename = "Cmd")]
    pub cmd: Option<Vec<String>>,
    #[serde(rename = "Entrypoint")]
    pub entrypoint: Option<Vec<String>>,
    #[serde(default, rename = "ExposedPorts")]
    pub exposed_ports: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, rename = "WorkingDir")]
    pub working_dir: Option<String>,
    #[serde(default, rename = "Volumes")]
    pub volumes: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, rename = "Labels")]
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub created: String,
    pub created_by: String,
    pub comment: Option<String>,
    pub empty_layer: Option<bool>,
}

// Conversion function from oci-spec types to our internal types
pub fn from_oci_config(config: &ImageConfiguration) -> ImageMetadata {
    // Extract the main config
    let config_obj = config.config().as_ref();

    // Create exposed ports HashMap if available
    let exposed_ports = config_obj.and_then(|c| {
        c.exposed_ports().as_ref().map(|ports| {
            let mut map = HashMap::new();
            for port in ports {
                map.insert(port.clone(), serde_json::Value::Null);
            }
            map
        })
    });

    // Create volumes HashMap if available
    let volumes = config_obj.and_then(|c| {
        c.volumes().as_ref().map(|vols| {
            let mut map = HashMap::new();
            for vol in vols {
                map.insert(vol.clone(), serde_json::Value::Null);
            }
            map
        })
    });

    // Create container config from OCI config
    let container_config = ContainerConfig {
        env: config_obj
            .map(|c| c.env().clone().unwrap_or_default())
            .unwrap_or_default(),
        cmd: config_obj.and_then(|c| c.cmd().clone()),
        entrypoint: config_obj.and_then(|c| c.entrypoint().clone()),
        exposed_ports,
        working_dir: config_obj.and_then(|c| c.working_dir().clone()),
        volumes,
        labels: config_obj.and_then(|c| c.labels().clone()),
    };

    // Convert history entries
    let history: Vec<HistoryEntry> = config
        .history()
        .as_ref()
        .map(|hist_vec| {
            hist_vec
                .iter()
                .map(|h| HistoryEntry {
                    created: h.created().clone().unwrap_or_default(),
                    created_by: h.created_by().clone().unwrap_or_default(),
                    comment: h.comment().clone(),
                    empty_layer: h.empty_layer(),
                })
                .collect()
        })
        .unwrap_or_default();

    // Create ImageMetadata with a placeholder ID that will be replaced with the actual ID from the manifest
    ImageMetadata {
        id: "sha256:".to_string(), // We'll get the real ID from the manifest.json
        repo_tags: vec![],         // This usually comes from the manifest, not the config
        created: config.created().clone().unwrap_or_default(),
        container_config,
        history,
        architecture: config.architecture().to_string(),
        os: config.os().to_string(),
    }
}

pub fn generate_markdown_metadata(metadata: &ImageMetadata, output_path: &Path) -> Result<()> {
    let markdown = format_image_metadata_markdown(metadata)?;
    fs::write(output_path, markdown)?;
    Ok(())
}

fn format_image_metadata_markdown(metadata: &ImageMetadata) -> Result<String> {
    let mut markdown = String::new();

    markdown.push_str(&format!("# Image: {}\n\n", metadata.id));

    markdown.push_str("## Basic Information\n\n");
    markdown.push_str(&format!("- **ID**: `{}`\n", metadata.id));
    if !metadata.repo_tags.is_empty() {
        markdown.push_str(&format!("- **Tags**: {}\n", metadata.repo_tags.join(", ")));
    }
    markdown.push_str(&format!("- **Created**: {}\n", metadata.created));
    markdown.push_str(&format!("- **Architecture**: {}\n", metadata.architecture));
    markdown.push_str(&format!("- **OS**: {}\n", metadata.os));
    markdown.push('\n');

    markdown.push_str("## Container Configuration\n\n");

    if !metadata.container_config.env.is_empty() {
        markdown.push_str("### Environment Variables\n\n");
        markdown.push_str("```\n");
        for env in &metadata.container_config.env {
            markdown.push_str(&format!("{}\n", env));
        }
        markdown.push_str("```\n\n");
    }

    if let Some(cmd) = &metadata.container_config.cmd {
        markdown.push_str("### Command\n\n");
        markdown.push_str("```\n");
        markdown.push_str(&format!("{}\n", cmd.join(" ")));
        markdown.push_str("```\n\n");
    }

    if let Some(entrypoint) = &metadata.container_config.entrypoint {
        markdown.push_str("### Entrypoint\n\n");
        markdown.push_str("```\n");
        markdown.push_str(&format!("{}\n", entrypoint.join(" ")));
        markdown.push_str("```\n\n");
    }

    if let Some(working_dir) = &metadata.container_config.working_dir {
        if !working_dir.is_empty() {
            markdown.push_str(&format!("### Working Directory\n\n`{}`\n\n", working_dir));
        }
    }

    if let Some(ports) = &metadata.container_config.exposed_ports {
        if !ports.is_empty() {
            markdown.push_str("### Exposed Ports\n\n");
            for port in ports.keys() {
                markdown.push_str(&format!("- `{}`\n", port));
            }
            markdown.push('\n');
        }
    }

    if let Some(volumes) = &metadata.container_config.volumes {
        if !volumes.is_empty() {
            markdown.push_str("### Volumes\n\n");
            for volume in volumes.keys() {
                markdown.push_str(&format!("- `{}`\n", volume));
            }
            markdown.push('\n');
        }
    }

    if let Some(labels) = &metadata.container_config.labels {
        if !labels.is_empty() {
            markdown.push_str("### Labels\n\n");
            markdown.push_str("| Key | Value |\n");
            markdown.push_str("|-----|-------|\n");
            for (key, value) in labels {
                markdown.push_str(&format!("| `{}` | `{}` |\n", key, value));
            }
            markdown.push('\n');
        }
    }

    markdown.push_str("## Layer History\n\n");
    markdown.push_str("| Created | Command | Comment |\n");
    markdown.push_str("|---------|---------|--------|\n");

    for entry in &metadata.history {
        let empty_string = String::new();
        let comment = entry.comment.as_ref().unwrap_or(&empty_string);
        let created_by = &entry.created_by;

        // Clean up the command by removing the shell prefix and any trailing whitespace
        // This preserves special syntax like |9 in commands while removing /bin/sh -c #(nop) prefix
        let mut formatted_command = if created_by.contains("/bin/sh -c #(nop) ") {
            // For non-execution instructions, remove the shell prefix and trim any leading whitespace
            created_by
                .replace("/bin/sh -c #(nop) ", "")
                .trim_start()
                .to_string()
        } else if created_by.contains("/bin/sh -c ") {
            // For execution instructions, remove the shell prefix and trim any leading whitespace
            created_by
                .replace("/bin/sh -c ", "")
                .trim_start()
                .to_string()
        } else {
            // For other instructions, keep the entire command
            created_by.clone()
        };

        // Escape pipe characters to prevent breaking markdown tables
        formatted_command = formatted_command.replace("|", "\\|");

        markdown.push_str(&format!(
            "| {} | `{}` | {} |\n",
            entry.created, formatted_command, comment
        ));
    }

    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_metadata() -> ImageMetadata {
        let mut exposed_ports = HashMap::new();
        exposed_ports.insert("80/tcp".to_string(), serde_json::Value::Null);

        let mut volumes = HashMap::new();
        volumes.insert("/data".to_string(), serde_json::Value::Null);

        let mut labels = HashMap::new();
        labels.insert("maintainer".to_string(), "test@example.com".to_string());

        ImageMetadata {
            id: "sha256:1234567890abcdef".to_string(),
            repo_tags: vec!["test:latest".to_string(), "test:1.0".to_string()],
            created: "2023-01-01T00:00:00Z".to_string(),
            container_config: ContainerConfig {
                env: vec![
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                ],
                cmd: Some(vec!["bash".to_string()]),
                entrypoint: Some(vec!["docker-entrypoint.sh".to_string()]),
                exposed_ports: Some(exposed_ports),
                working_dir: Some("/app".to_string()),
                volumes: Some(volumes),
                labels: Some(labels),
            },
            history: vec![
                HistoryEntry {
                    created: "2023-01-01T00:00:00Z".to_string(),
                    created_by: "/bin/sh -c #(nop)   CMD [\"bash\"]".to_string(),
                    comment: None,
                    empty_layer: Some(true),
                },
                HistoryEntry {
                    created: "2023-01-01T00:00:00Z".to_string(),
                    created_by: "/bin/sh -c #(nop) WORKDIR /app".to_string(),
                    comment: None,
                    empty_layer: Some(true),
                },
            ],
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
        }
    }

    #[test]
    fn test_format_image_metadata_markdown() {
        let metadata = create_test_metadata();
        let result = format_image_metadata_markdown(&metadata).unwrap();

        // Verify the Markdown output contains expected sections
        assert!(result.contains("# Image: sha256:1234567890abcdef"));
        assert!(result.contains("## Basic Information"));
        assert!(result.contains("- **Tags**: test:latest, test:1.0"));
        assert!(result.contains("- **Architecture**: amd64"));
        assert!(result.contains("- **OS**: linux"));

        // Check container configuration
        assert!(result.contains("### Environment Variables"));
        assert!(
            result.contains("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
        );
        assert!(result.contains("### Command"));
        assert!(result.contains("bash"));
        assert!(result.contains("### Entrypoint"));
        assert!(result.contains("docker-entrypoint.sh"));
        assert!(result.contains("### Working Directory"));
        assert!(result.contains("/app"));

        // Check exposed ports
        assert!(result.contains("### Exposed Ports"));
        assert!(result.contains("- `80/tcp`"));

        // Check volumes
        assert!(result.contains("### Volumes"));
        assert!(result.contains("- `/data`"));

        // Check labels
        assert!(result.contains("### Labels"));
        assert!(result.contains("| `maintainer` | `test@example.com` |"));

        // Check history
        assert!(result.contains("## Layer History"));
        assert!(result.contains("| 2023-01-01T00:00:00Z | `CMD [\"bash\"]` |"));
        assert!(result.contains("| 2023-01-01T00:00:00Z | `WORKDIR /app` |"));
    }

    #[test]
    fn test_generate_markdown_metadata() {
        let temp_dir = tempdir().unwrap();
        let output_path = temp_dir.path().join("Image.md");

        let metadata = create_test_metadata();
        let result = generate_markdown_metadata(&metadata, &output_path);

        assert!(result.is_ok());
        assert!(output_path.exists());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(!content.is_empty());
        assert!(content.contains("# Image: sha256:1234567890abcdef"));
    }

    #[test]
    fn test_long_command_formatting() {
        let mut metadata = create_test_metadata();

        // Add a history entry with a very long command
        let long_command = "This is a very long command that should not be truncated in the output table even though it exceeds 50 characters";
        metadata.history.push(HistoryEntry {
            created: "2023-01-01T00:00:00Z".to_string(),
            created_by: long_command.to_string(),
            comment: None,
            empty_layer: Some(false),
        });

        let result = format_image_metadata_markdown(&metadata).unwrap();

        // Check that the long command is preserved completely (not truncated)
        assert!(result.contains(long_command));
    }

    #[test]
    fn test_pipe_character_escaping() {
        let mut metadata = create_test_metadata();

        // Add a history entry with a pipe character
        let command_with_pipe = "RUN |9 MAINTAINER=Apache NiFi <dev@nifi.apache.org>";
        metadata.history.push(HistoryEntry {
            created: "2023-01-01T00:00:00Z".to_string(),
            created_by: command_with_pipe.to_string(),
            comment: None,
            empty_layer: Some(false),
        });

        let result = format_image_metadata_markdown(&metadata).unwrap();

        // Check that pipe characters are escaped
        assert!(result.contains("RUN \\|9 MAINTAINER=Apache NiFi <dev@nifi.apache.org>"));
    }
}
