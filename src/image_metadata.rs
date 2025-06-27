use crate::digest_tracker::{DigestTracker, LayerDigest};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Complete structured representation of Image.md content
#[derive(Debug, Clone, PartialEq)]
pub struct ImageMetadata {
    pub basic_info: Option<BasicInfo>,
    pub container_config: Option<ContainerConfig>,
    pub layer_digests: Vec<LayerDigest>,
}

/// Basic image information section
#[derive(Debug, Clone, PartialEq)]
pub struct BasicInfo {
    pub name: String,
    pub id: String,
    pub tags: Vec<String>,
    pub created: String,
    pub architecture: String,
    pub os: String,
}

/// Container configuration section
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerConfig {
    pub environment_variables: Vec<String>,
    pub command: Option<String>,
    pub entrypoint: Option<String>,
    pub working_directory: String,
    pub exposed_ports: Vec<String>,
    pub labels: HashMap<String, String>,
}

impl ImageMetadata {
    /// Create a new ImageMetadata instance
    pub fn new(basic_info: Option<BasicInfo>, container_config: Option<ContainerConfig>) -> Self {
        Self {
            basic_info,
            container_config,
            layer_digests: Vec::new(),
        }
    }

    /// Update layer digests from DigestTracker
    pub fn update_layer_digests(&mut self, digest_tracker: &DigestTracker) {
        self.layer_digests = digest_tracker.layer_digests.clone();
    }

    /// Convert from legacy metadata::ImageMetadata format
    pub fn from_legacy(
        legacy: &crate::metadata::ImageMetadata,
        digest_tracker: &DigestTracker,
        image_name: &str,
    ) -> Self {
        let basic_info = BasicInfo {
            name: image_name.to_string(),
            id: legacy.id.clone(),
            tags: legacy.repo_tags.clone(),
            created: legacy.created.clone(),
            architecture: legacy.architecture.clone(),
            os: legacy.os.clone(),
        };

        let container_config = ContainerConfig {
            environment_variables: legacy.container_config.env.clone(),
            command: legacy
                .container_config
                .cmd
                .as_ref()
                .map(|cmd| cmd.join(" ")),
            entrypoint: legacy
                .container_config
                .entrypoint
                .as_ref()
                .map(|ep| ep.join(" ")),
            working_directory: legacy
                .container_config
                .working_dir
                .clone()
                .unwrap_or_else(|| "/".to_string()),
            exposed_ports: legacy
                .container_config
                .exposed_ports
                .as_ref()
                .map(|ports| ports.keys().cloned().collect())
                .unwrap_or_default(),
            labels: legacy.container_config.labels.clone().unwrap_or_default(),
        };

        Self {
            basic_info: Some(basic_info),
            container_config: Some(container_config),
            layer_digests: digest_tracker.layer_digests.clone(),
        }
    }

    /// Render the metadata as markdown
    pub fn render_markdown(&self) -> Result<String> {
        let mut markdown = String::new();

        // Header
        if let Some(basic_info) = &self.basic_info {
            markdown.push_str(&format!("# Image: {}\n\n", basic_info.name));
        } else {
            markdown.push_str("# Image: Unknown\n\n");
        }

        // Basic Information
        if let Some(basic_info) = &self.basic_info {
            markdown.push_str("## Basic Information\n\n");
            markdown.push_str(&format!("- **Name**: {}\n", basic_info.name));
            markdown.push_str(&format!("- **ID**: `{}`\n", basic_info.id));
            if !basic_info.tags.is_empty() {
                markdown.push_str(&format!("- **Tags**: {}\n", basic_info.tags.join(", ")));
            }
            markdown.push_str(&format!("- **Created**: {}\n", basic_info.created));
            markdown.push_str(&format!(
                "- **Architecture**: {}\n",
                basic_info.architecture
            ));
            markdown.push_str(&format!("- **OS**: {}\n", basic_info.os));
            markdown.push('\n');
        }

        // Container Configuration
        if let Some(container_config) = &self.container_config {
            markdown.push_str("## Container Configuration\n\n");

            if !container_config.environment_variables.is_empty() {
                markdown.push_str("### Environment Variables\n\n");
                markdown.push_str("```\n");
                for env in &container_config.environment_variables {
                    markdown.push_str(&format!("{}\n", env));
                }
                markdown.push_str("```\n\n");
            }

            if let Some(cmd) = &container_config.command {
                markdown.push_str("### Command\n\n");
                markdown.push_str("```\n");
                markdown.push_str(&format!("{}\n", cmd));
                markdown.push_str("```\n\n");
            }

            if let Some(entrypoint) = &container_config.entrypoint {
                markdown.push_str("### Entrypoint\n\n");
                markdown.push_str("```\n");
                markdown.push_str(&format!("{}\n", entrypoint));
                markdown.push_str("```\n\n");
            }

            if !container_config.working_directory.is_empty() {
                markdown.push_str(&format!(
                    "### Working Directory\n\n`{}`\n\n",
                    container_config.working_directory
                ));
            }

            if !container_config.exposed_ports.is_empty() {
                markdown.push_str("### Exposed Ports\n\n");
                for port in &container_config.exposed_ports {
                    markdown.push_str(&format!("- `{}`\n", port));
                }
                markdown.push('\n');
            }

            if !container_config.labels.is_empty() {
                markdown.push_str("### Labels\n\n");
                markdown.push_str("| Key | Value |\n");
                markdown.push_str("|-----|-------|\n");
                for (key, value) in &container_config.labels {
                    markdown.push_str(&format!("| `{}` | `{}` |\n", key, value));
                }
                markdown.push('\n');
            }
        }

        // Layer History
        if !self.layer_digests.is_empty() {
            markdown.push_str("## Layer History\n\n");
            markdown.push_str("| Created | Command | Comment | Digest | Empty |\n");
            markdown.push_str("|---------|---------|---------|--------|-------|\n");

            for layer in &self.layer_digests {
                let comment = layer.comment.as_deref().unwrap_or("");
                // Escape pipes in the content for proper markdown display
                let escaped_command = layer.command.replace("|", "\\|");
                let escaped_comment = comment.replace("|", "\\|");
                
                markdown.push_str(&format!(
                    "| {} | `{}` | {} | `{}` | {} |\n",
                    layer.created, escaped_command, escaped_comment, layer.digest, layer.is_empty
                ));
            }
            markdown.push('\n');
        }

        Ok(markdown)
    }

    /// Parse markdown content back to ImageMetadata
    pub fn parse_markdown(content: &str) -> Result<Self> {
        let mut basic_info = BasicInfo {
            name: String::new(),
            id: String::new(),
            tags: Vec::new(),
            created: String::new(),
            architecture: String::new(),
            os: String::new(),
        };

        let mut container_config = ContainerConfig {
            environment_variables: Vec::new(),
            command: None,
            entrypoint: None,
            working_directory: "/".to_string(),
            exposed_ports: Vec::new(),
            labels: HashMap::new(),
        };

        let mut layer_digests = Vec::new();

        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Parse header
            if line.starts_with("# Image: ") {
                basic_info.name = line.replace("# Image: ", "");
            }
            // Parse basic information
            else if line.starts_with("- **Name**: ") {
                basic_info.name = line.replace("- **Name**: ", "");
            } else if line.starts_with("- **ID**: `") {
                basic_info.id = line.replace("- **ID**: `", "").replace("`", "");
            } else if line.starts_with("- **Tags**: ") {
                let tags_str = line.replace("- **Tags**: ", "");
                basic_info.tags = tags_str.split(", ").map(|s| s.to_string()).collect();
            } else if line.starts_with("- **Created**: ") {
                basic_info.created = line.replace("- **Created**: ", "");
            } else if line.starts_with("- **Architecture**: ") {
                basic_info.architecture = line.replace("- **Architecture**: ", "");
            } else if line.starts_with("- **OS**: ") {
                basic_info.os = line.replace("- **OS**: ", "");
            }
            // Parse environment variables
            else if line == "### Environment Variables" {
                i += 2; // Skip to content after ```
                while i < lines.len() && lines[i].trim() != "```" {
                    if !lines[i].trim().is_empty() {
                        container_config
                            .environment_variables
                            .push(lines[i].to_string());
                    }
                    i += 1;
                }
            }
            // Parse command
            else if line == "### Command" {
                i += 2; // Skip to content after ```
                if i < lines.len() && lines[i].trim() != "```" {
                    let cmd_str = lines[i].trim();
                    container_config.command = Some(cmd_str.to_string());
                }
            }
            // Parse entrypoint
            else if line == "### Entrypoint" {
                i += 2; // Skip to content after ```
                if i < lines.len() && lines[i].trim() != "```" {
                    let ep_str = lines[i].trim();
                    container_config.entrypoint = Some(ep_str.to_string());
                }
            }
            // Parse working directory
            else if line.starts_with("### Working Directory") {
                i += 2; // Skip to content
                if i < lines.len() {
                    let wd = lines[i].trim().replace("`", "");
                    if !wd.is_empty() {
                        container_config.working_directory = wd;
                    }
                }
            }
            // Parse exposed ports
            else if line == "### Exposed Ports" {
                i += 2; // Skip to content
                while i < lines.len() && lines[i].trim().starts_with("- `") {
                    let port = lines[i].trim().replace("- `", "").replace("`", "");
                    container_config.exposed_ports.push(port);
                    i += 1;
                }
                i -= 1; // Adjust for loop increment
            }
            // Parse labels table
            else if line == "### Labels" {
                i += 3; // Skip header and separator
                while i < lines.len()
                    && lines[i].trim().starts_with("|")
                    && !lines[i].trim().starts_with("| Key |")
                {
                    let parts: Vec<&str> = lines[i].split('|').collect();
                    if parts.len() >= 4 {
                        let key = parts[1].trim().replace("`", "");
                        let value = parts[2].trim().replace("`", "");
                        if !key.is_empty() && !value.is_empty() {
                            container_config.labels.insert(key, value);
                        }
                    }
                    i += 1;
                }
                i -= 1; // Adjust for loop increment
            }
            // Parse layer history table (now contains digest info)
            else if line == "## Layer History" {
                i += 2; // Skip to table header

                // Skip table header line
                if i < lines.len() && lines[i].trim().starts_with("| Created |") {
                    i += 1;
                }

                // Skip separator line
                if i < lines.len() && lines[i].trim().starts_with("|------") {
                    i += 1;
                }

                // Now process data rows
                while i < lines.len()
                    && lines[i].trim().starts_with("|")
                    && !lines[i].trim().is_empty()
                {
                    let line_content = lines[i];
                    
                    // Find unescaped pipe positions using regex approach
                    // Look for pipes that are either at start or not preceded by backslash
                    let mut unescaped_pipe_positions = Vec::new();
                    let chars: Vec<char> = line_content.chars().collect();
                    
                    for (pos, &ch) in chars.iter().enumerate() {
                        if ch == '|' {
                            // Pipe is unescaped if it's at the start or not preceded by backslash
                            if pos == 0 || chars[pos - 1] != '\\' {
                                unescaped_pipe_positions.push(pos);
                            }
                        }
                    }
                    
                    // Split by unescaped pipe positions
                    let mut parts = Vec::new();
                    let mut start = 0;
                    for &pos in &unescaped_pipe_positions {
                        parts.push(&line_content[start..pos]);
                        start = pos + 1;
                    }
                    if start < line_content.len() {
                        parts.push(&line_content[start..]);
                    }
                    
                    if parts.len() >= 6 {
                        let created = parts[1].trim().to_string();
                        let command = parts[2].trim().replace("`", "").replace("\\|", "|");
                        let comment = parts[3].trim().replace("\\|", "|");
                        let digest = parts[4].trim().replace("`", "");
                        let is_empty = parts[5].trim() == "true";

                        if !created.is_empty() && !digest.is_empty() {
                            layer_digests.push(LayerDigest {
                                digest,
                                command,
                                created,
                                is_empty,
                                comment: if comment.is_empty() {
                                    None
                                } else {
                                    Some(comment)
                                },
                            });
                        }
                    }
                    i += 1;
                }
                i -= 1; // Adjust for loop increment
            }

            i += 1;
        }

        // Only include basic_info if we found any basic information
        let basic_info_option = if basic_info.name.is_empty() && basic_info.id.is_empty() {
            None
        } else {
            Some(basic_info)
        };

        // Only include container_config if we found any container configuration
        let container_config_option = if container_config.environment_variables.is_empty()
            && container_config.command.is_none()
            && container_config.entrypoint.is_none()
            && container_config.working_directory == "/"
            && container_config.exposed_ports.is_empty()
            && container_config.labels.is_empty()
        {
            None
        } else {
            Some(container_config)
        };

        Ok(Self {
            basic_info: basic_info_option,
            container_config: container_config_option,
            layer_digests,
        })
    }

    /// Save as markdown file
    pub fn save_markdown(&self, path: &Path) -> Result<()> {
        let markdown = self.render_markdown()?;
        fs::write(path, markdown).context("Failed to write markdown file")?;
        Ok(())
    }

    /// Load from markdown file
    pub fn load_markdown(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).context("Failed to read markdown file")?;
        Self::parse_markdown(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_metadata() -> ImageMetadata {
        let basic_info = BasicInfo {
            name: "test:latest".to_string(),
            id: "sha256:1234567890abcdef".to_string(),
            tags: vec!["test:latest".to_string(), "test:1.0".to_string()],
            created: "2023-01-01T00:00:00Z".to_string(),
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
        };

        let mut labels = HashMap::new();
        labels.insert("maintainer".to_string(), "test@example.com".to_string());

        let container_config = ContainerConfig {
            environment_variables: vec![
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
            ],
            command: Some("bash".to_string()),
            entrypoint: Some("docker-entrypoint.sh".to_string()),
            working_directory: "/app".to_string(),
            exposed_ports: vec!["80/tcp".to_string()],
            labels,
        };

        let layer_digests = vec![
            LayerDigest {
                digest: "sha256:abc123".to_string(),
                command: "FROM alpine".to_string(),
                created: "2023-01-01T00:00:00Z".to_string(),
                is_empty: false,
                comment: None,
            },
            LayerDigest {
                digest: "sha256:def456".to_string(),
                command: "CMD [\"bash\"]".to_string(),
                created: "2023-01-01T00:00:00Z".to_string(),
                is_empty: true,
                comment: None,
            },
        ];

        ImageMetadata {
            basic_info: Some(basic_info),
            container_config: Some(container_config),
            layer_digests,
        }
    }

    #[test]
    fn test_render_markdown() {
        let metadata = create_test_metadata();
        let result = metadata.render_markdown().unwrap();

        // Verify basic sections
        assert!(result.contains("# Image: test:latest"));
        assert!(result.contains("## Basic Information"));
        assert!(result.contains("- **Name**: test:latest"));
        assert!(result.contains("- **ID**: `sha256:1234567890abcdef`"));
        assert!(result.contains("- **Tags**: test:latest, test:1.0"));
        assert!(result.contains("- **Architecture**: amd64"));

        // Verify container config
        assert!(result.contains("### Environment Variables"));
        assert!(result.contains("PATH=/usr/local/sbin"));
        assert!(result.contains("### Command"));
        assert!(result.contains("bash"));

        // Verify layer history (now includes digests)
        assert!(result.contains("## Layer History"));
        assert!(result.contains("CMD [\"bash\"]"));
        assert!(result.contains("sha256:abc123"));
        assert!(result.contains("sha256:def456"));
    }

    #[test]
    fn test_save_and_load_markdown() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("test.md");

        let original = create_test_metadata();
        original.save_markdown(&path).unwrap();

        assert!(path.exists());
        let loaded = ImageMetadata::load_markdown(&path).unwrap();

        // Basic comparisons (full parsing might not be perfect due to markdown complexity)
        assert_eq!(
            loaded.basic_info.as_ref().unwrap().id,
            original.basic_info.as_ref().unwrap().id
        );
        assert_eq!(
            loaded.basic_info.as_ref().unwrap().architecture,
            original.basic_info.as_ref().unwrap().architecture
        );
    }

    #[test]
    fn test_pipe_escaping() {
        let basic_info = BasicInfo {
            name: "test:latest".to_string(),
            id: "sha256:1234567890abcdef".to_string(),
            tags: vec!["test:latest".to_string()],
            created: "2023-01-01T00:00:00Z".to_string(),
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
        };

        let container_config = ContainerConfig {
            environment_variables: vec![],
            command: Some("bash".to_string()),
            entrypoint: Some("docker-entrypoint.sh".to_string()),
            working_directory: "/app".to_string(),
            exposed_ports: vec![],
            labels: HashMap::new(),
        };

        let layer_digests = vec![LayerDigest {
            digest: "sha256:abc123".to_string(),
            command: "RUN echo 'test | with | pipes'".to_string(),
            created: "2023-01-01T00:00:00Z".to_string(),
            is_empty: false,
            comment: Some("comment | with | pipes".to_string()),
        }];

        let metadata = ImageMetadata {
            basic_info: Some(basic_info),
            container_config: Some(container_config),
            layer_digests,
        };

        let result = metadata.render_markdown().unwrap();

        // Verify that pipes are escaped in the rendered output for proper markdown display
        assert!(result.contains("RUN echo 'test \\| with \\| pipes'"));
        assert!(result.contains("comment \\| with \\| pipes"));

        // Test round-trip parsing
        let parsed = ImageMetadata::parse_markdown(&result).unwrap();
        assert_eq!(
            parsed.layer_digests[0].command,
            "RUN echo 'test | with | pipes'"
        );
        assert_eq!(
            parsed.layer_digests[0].comment.as_ref().unwrap(),
            "comment | with | pipes"
        );
    }

    #[test]
    fn test_real_world_round_trip() {
        // Test with real data from alp/Image.md that contains complex commands with pipes
        let basic_info = BasicInfo {
            name: "postgres:16.9-alpine3.21".to_string(),
            id: "sha256:48ae07b5a3dfabc83a914aec99d42d083677f57853398ac14c5f25884da09f14".to_string(),
            tags: vec!["postgres:16.9-alpine3.21".to_string()],
            created: "2025-06-06T18:27:47Z".to_string(),
            architecture: "arm64".to_string(),
            os: "linux".to_string(),
        };

        let mut labels = HashMap::new();
        labels.insert("maintainer".to_string(), "postgres team".to_string());

        let container_config = ContainerConfig {
            environment_variables: vec![
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                "GOSU_VERSION=1.17".to_string(),
                "PGDATA=/var/lib/postgresql/data".to_string(),
            ],
            command: Some("postgres".to_string()),
            entrypoint: Some("docker-entrypoint.sh".to_string()),
            working_directory: "/".to_string(),
            exposed_ports: vec!["5432/tcp".to_string()],
            labels,
        };

        // Include real complex commands from alp/Image.md that contain pipes
        let layer_digests = vec![
            LayerDigest {
                digest: "sha256:6e771e15690e2fabf2332d3a3b744495411d6e0b00b2aea64419b58b0066cf81".to_string(),
                command: "ADD alpine-minirootfs-3.21.3-aarch64.tar.gz / # buildkit".to_string(),
                created: "2025-02-14T03:28:36+00:00".to_string(),
                is_empty: false,
                comment: Some("buildkit.dockerfile.v0".to_string()),
            },
            LayerDigest {
                digest: "sha256:7253dfc6422805ac3c15fda3414a5e3fb679f89df5a9ecfb3b80db788b4e8dcf".to_string(),
                command: "RUN set -eux; apk add --no-cache --virtual .gosu-deps ca-certificates dpkg gnupg; dpkgArch=\"$(dpkg --print-architecture | awk -F- '{ print $NF }')\"; wget -O /usr/local/bin/gosu \"https://github.com/tianon/gosu/releases/download/$GOSU_VERSION/gosu-$dpkgArch\"; wget -O /usr/local/bin/gosu.asc \"https://github.com/tianon/gosu/releases/download/$GOSU_VERSION/gosu-$dpkgArch.asc\"; export GNUPGHOME=\"$(mktemp -d)\"; gpg --batch --keyserver hkps://keys.openpgp.org --recv-keys B42F6819007F00F88E364FD4036A9C25BF357DD4; gpg --batch --verify /usr/local/bin/gosu.asc /usr/local/bin/gosu; gpgconf --kill all; rm -rf \"$GNUPGHOME\" /usr/local/bin/gosu.asc; apk del --no-network .gosu-deps; chmod +x /usr/local/bin/gosu; gosu --version; gosu nobody true # buildkit".to_string(),
                created: "2025-06-06T18:27:47+00:00".to_string(),
                is_empty: false,
                comment: Some("buildkit.dockerfile.v0".to_string()),
            },
            LayerDigest {
                digest: "sha256:c9d81a483d3df409a38c9a58f1a0aed7d439f67b1200e39485beee626b61b66e".to_string(),
                command: "RUN set -eux; wget -O postgresql.tar.bz2 \"https://ftp.postgresql.org/pub/source/v$PG_VERSION/postgresql-$PG_VERSION.tar.bz2\"; echo \"$PG_SHA256 *postgresql.tar.bz2\" | sha256sum -c -; mkdir -p /usr/src/postgresql; tar --extract --file postgresql.tar.bz2 --directory /usr/src/postgresql --strip-components 1; rm postgresql.tar.bz2; apk add --no-cache --virtual .build-deps $DOCKER_PG_LLVM_DEPS bison coreutils dpkg-dev dpkg flex g++ gcc krb5-dev libc-dev libedit-dev libxml2-dev libxslt-dev linux-headers make openldap-dev openssl-dev perl-dev perl-ipc-run perl-utils python3-dev tcl-dev util-linux-dev zlib-dev icu-dev lz4-dev zstd-dev; cd /usr/src/postgresql; awk '$1 == \"#define\" && $2 == \"DEFAULT_PGSOCKET_DIR\" && $3 == \"\\\"/tmp\\\"\" { $3 = \"\\\"/var/run/postgresql\\\"\"; print; next } { print }' src/include/pg_config_manual.h > src/include/pg_config_manual.h.new; grep '/var/run/postgresql' src/include/pg_config_manual.h.new; mv src/include/pg_config_manual.h.new src/include/pg_config_manual.h; gnuArch=\"$(dpkg-architecture --query DEB_BUILD_GNU_TYPE)\"; export LLVM_CONFIG=\"/usr/lib/llvm19/bin/llvm-config\"; export CLANG=clang-19; ./configure --enable-option-checking=fatal --build=\"$gnuArch\" --enable-integer-datetimes --enable-thread-safety --enable-tap-tests --disable-rpath --with-uuid=e2fs --with-pgport=5432 --with-system-tzdata=/usr/share/zoneinfo --prefix=/usr/local --with-includes=/usr/local/include --with-libraries=/usr/local/lib --with-gssapi --with-ldap --with-tcl --with-perl --with-python --with-openssl --with-libxml --with-libxslt --with-icu --with-llvm --with-lz4 --with-zstd; make -j \"$(nproc)\" world-bin; make install-world-bin; make -C contrib install; runDeps=\"$( scanelf --needed --nobanner --format '%n#p' --recursive /usr/local | tr ',' '\\n' | sort -u | awk 'system(\"[ -e /usr/local/lib/\" $1 \" ]\") == 0 { next } { print \"so:\" $1 }' | grep -v -e perl -e python -e tcl )\"; apk add --no-cache --virtual .postgresql-rundeps $runDeps bash tzdata zstd icu-data-full $([ \"$(apk --print-arch)\" != 'ppc64le' ] && echo 'nss_wrapper'); apk del --no-network .build-deps; cd /; rm -rf /usr/src/postgresql /usr/local/share/doc /usr/local/share/man; postgres --version # buildkit".to_string(),
                created: "2025-06-06T18:27:47+00:00".to_string(),
                is_empty: false,
                comment: Some("buildkit.dockerfile.v0".to_string()),
            },
            LayerDigest {
                digest: "sha256:d5b0bb61acee74b02675e9f87df8e6c1f747d93dc7e017908aae89187f4180e9".to_string(),
                command: "RUN set -eux; cp -v /usr/local/share/postgresql/postgresql.conf.sample /usr/local/share/postgresql/postgresql.conf.sample.orig; sed -ri \"s!^#?(listen_addresses)\\s*=\\s*\\S+.*!\\1 = '*'!\" /usr/local/share/postgresql/postgresql.conf.sample; grep -F \"listen_addresses = '*'\" /usr/local/share/postgresql/postgresql.conf.sample # buildkit".to_string(),
                created: "2025-06-06T18:27:47+00:00".to_string(),
                is_empty: false,
                comment: Some("buildkit.dockerfile.v0".to_string()),
            },
        ];

        let original_metadata = ImageMetadata {
            basic_info: Some(basic_info),
            container_config: Some(container_config),
            layer_digests,
        };

        // Test the round-trip: render to markdown, then parse back
        let rendered_markdown = original_metadata.render_markdown().unwrap();
        
        // Verify that pipes are properly escaped in the rendered markdown
        assert!(rendered_markdown.contains("dpkg --print-architecture \\| awk"));
        assert!(rendered_markdown.contains("sha256sum -c -"));
        assert!(rendered_markdown.contains("\\| tr ','"));
        assert!(rendered_markdown.contains("\\| sort -u"));
        assert!(rendered_markdown.contains("\\| awk 'system"));
        assert!(rendered_markdown.contains("\\| grep -v"));
        
        // Parse the markdown back to metadata
        let parsed_metadata = ImageMetadata::parse_markdown(&rendered_markdown).unwrap();
        
        // Verify basic info
        assert_eq!(parsed_metadata.basic_info.as_ref().unwrap().name, original_metadata.basic_info.as_ref().unwrap().name);
        assert_eq!(parsed_metadata.basic_info.as_ref().unwrap().id, original_metadata.basic_info.as_ref().unwrap().id);
        assert_eq!(parsed_metadata.basic_info.as_ref().unwrap().architecture, original_metadata.basic_info.as_ref().unwrap().architecture);
        
        // Verify layer digests count
        assert_eq!(parsed_metadata.layer_digests.len(), original_metadata.layer_digests.len());
        
        // Verify each layer digest matches exactly, especially the complex commands with pipes
        for (i, (original, parsed)) in original_metadata.layer_digests.iter().zip(parsed_metadata.layer_digests.iter()).enumerate() {
            assert_eq!(parsed.digest, original.digest, "Layer {} digest mismatch", i);
            assert_eq!(parsed.command, original.command, "Layer {} command mismatch", i);
            assert_eq!(parsed.created, original.created, "Layer {} created mismatch", i);
            assert_eq!(parsed.is_empty, original.is_empty, "Layer {} empty flag mismatch", i);
            assert_eq!(parsed.comment, original.comment, "Layer {} comment mismatch", i);
        }
        
        // Specifically test the complex command with multiple pipes
        let complex_layer = &parsed_metadata.layer_digests[2]; // The postgresql build command
        assert!(complex_layer.command.contains("| sha256sum -c -"));
        assert!(complex_layer.command.contains("| tr ',' '\\n'"));
        assert!(complex_layer.command.contains("| sort -u"));
        assert!(complex_layer.command.contains("| awk 'system"));
        assert!(complex_layer.command.contains("| grep -v"));
        
        // Ensure no escaped pipes remain in the parsed content
        assert!(!complex_layer.command.contains("\\|"));
        assert!(!complex_layer.comment.as_ref().unwrap_or(&String::new()).contains("\\|"));
    }
}
