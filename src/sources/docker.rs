use super::{naming, Source};
use crate::notifier::{AnyNotifier, EnhancedNotifier, SimpleNotifier};
use anyhow::{anyhow, Context, Result};
use console::{style, Emoji};
use futures_util::TryStreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use serde_json::Value;
use shiplift::{builder::PullOptions, Docker};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;
use std::{collections::HashMap, time::Duration};
use tempfile::TempDir;

static LOOK: Emoji<'_, '_> = Emoji("🔍 ", "");

/// Docker implementation of the Source trait
pub struct DockerSource;

impl DockerSource {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    fn run_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("docker")
            .args(args)
            .output()
            .context(format!("Failed to execute docker command: {:?}", args))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Docker command failed: {}", error));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    fn pull_image(&self, image_name: &str, notifier: &SimpleNotifier) -> Result<()> {
        notifier.info(&format!("Pulling Docker image '{}'...", image_name));

        let output = Command::new("docker")
            .args(["pull", image_name])
            .output()
            .context("Failed to execute docker pull command")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Docker pull failed: {}", error));
        }

        notifier.info(&format!(
            "Successfully pulled Docker image '{}'",
            image_name
        ));
        Ok(())
    }
    #[tokio::main]
    pub async fn pull_image_enhanced(
        &self,
        image_name: &str,
        notifier: &EnhancedNotifier,
    ) -> Result<()> {
        notifier.println_above(format!("Pull Docker image '{}'...", image_name));
        let docker = Docker::new();

        let tag = image_name
            .rsplit_once(':')
            .map(|(_, t)| t) // take the part after the last ':'
            .unwrap_or("latest");

        let opts = PullOptions::builder().image(image_name).tag(tag).build();
        let mut stream = docker.images().pull(&opts);

        let m = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(15));

        let overall = m.add(ProgressBar::new(0));
        overall.set_style(ProgressStyle::with_template(
            "{prefix:.bold} [{bar:60.cyan/blue}] {bytes}/{total_bytes} [ETA:{eta}]",
        )?);

        let host = "docker.io";
        let pull_from_message = format!("Pull from {}", host);

        overall.set_prefix(pull_from_message);
        overall.enable_steady_tick(Duration::from_millis(80));

        let layer_style = ProgressStyle::with_template(
            "{prefix:.dim} {spinner} [{bar:40.cyan/blue}] {bytes}/{total_bytes} {wide_msg}",
        )?
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

        struct Layer {
            pb: ProgressBar,
            total: u64,
            current: u64,
            finished: bool,
        }
        let mut layers: HashMap<String, Layer> = HashMap::new();
        let mut overall_total: u64 = 0;

        let update_overall =
            |layers: &HashMap<String, Layer>, overall: &ProgressBar, total_ref: &mut u64| {
                let mut sum_total = 0u64;
                let mut sum_curr = 0u64;
                for l in layers.values() {
                    if l.total > 0 {
                        sum_total = sum_total.saturating_add(l.total);
                    }
                    sum_curr = sum_curr.saturating_add(l.current);
                }
                if sum_total > *total_ref {
                    *total_ref = sum_total;
                    overall.set_length(sum_total);
                }
                if *total_ref > 0 {
                    overall.set_position(sum_curr.min(*total_ref));
                    overall.set_message(format!(
                        "({} layers downloaded)",
                        layers.values().filter(|l| l.finished).count()
                    ));
                }
            };

        let important_msg = |status: &str| -> bool {
            matches!(
                status,
                s if s.starts_with("Pulling from")
                  || s.starts_with("Digest:")
                  || s.starts_with("Status:")
                  || s.starts_with("Downloaded newer image for")
            )
        };

        while let Some(chunk) = stream.try_next().await? {
            let v: &Value = &chunk;
            let status = v.get("status").and_then(Value::as_str).unwrap_or("");
            let id_full = v.get("id").and_then(Value::as_str).unwrap_or_default();
            let id_short = if id_full.len() > 12 {
                &id_full[..12]
            } else {
                id_full
            };
            let pd = v.get("progressDetail").and_then(Value::as_object);
            let total = pd
                .and_then(|o| o.get("total"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let current = pd
                .and_then(|o| o.get("current"))
                .and_then(Value::as_u64)
                .unwrap_or(0);

            // global messages
            if id_full.is_empty() {
                if important_msg(status) {
                    m.println(status)?;
                }
                continue;
            }

            // We want bars only while Downloading.
            match status {
                "Downloading" => {
                    let entry = layers.entry(id_full.to_string()).or_insert_with(|| {
                        let pb = m.insert_after(&overall, ProgressBar::new(0));
                        pb.set_style(layer_style.clone());
                        pb.set_prefix(id_short.to_string());
                        pb.enable_steady_tick(Duration::from_millis(80));
                        Layer {
                            pb,
                            total: 0,
                            current: 0,
                            finished: false,
                        }
                    });

                    if total > 0 && entry.total == 0 {
                        entry.total = total;
                        entry.pb.set_length(total);
                    }
                    if total > 0 {
                        entry.current = current;
                        entry.pb.set_position(current);
                    }
                    entry.pb.set_message("Downloading");

                    update_overall(&layers, &overall, &mut overall_total);
                }

                // As soon as a layer is no longer Downloading, remove its bar
                "Extracting" | "Verifying Checksum" | "Download complete" | "Pull complete"
                | "Already exists" | "Mounted from" => {
                    if let Some(entry) = layers.get_mut(id_full) {
                        // Snap to full if we know total, then clear bar
                        if entry.total > 0 {
                            entry.current = entry.total.max(entry.current);
                            entry.pb.set_position(entry.total);
                        }
                        if !entry.finished {
                            entry.finished = true;
                            entry.pb.finish_and_clear();
                        }
                    }
                    update_overall(&layers, &overall, &mut overall_total);
                }

                _ => {}
            }
        }
        // overall.finish_with_message("waiting....");

        // Finish whatever remains
        for (_, l) in layers {
            if !l.finished {
                l.pb.finish_and_clear();
            }
        }
        if !overall.is_finished() {
            overall.finish_and_clear();
        }

        Ok(())
    }
}

impl Source for DockerSource {
    fn name(&self) -> &str {
        "docker"
    }

    /// Best-effort estimate using `docker images inspect {{.Size}}`.
    fn estimate_image_size(image: &str) -> Option<u64> {
        let _ = Command::new("docker")
            .args(["images", image, "--format", "{{.Size}}"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        let raw_out = Command::new("docker")
            .args(["image", "inspect", image, "--format", "{{.Size}}"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if !raw_out.status.success() {
            return None;
        }

        let bytes_size: u64 = String::from_utf8_lossy(&raw_out.stdout)
            .trim()
            .parse()
            .ok()?;

        Some(bytes_size)
    }

    /// Stream `docker save` -> file with live progress.
    /// - No deadlocks, no giant buffering
    /// - Safe (writes to .partial then renames)
    ///
    fn image_save_with_progress(image: &str, tar_path: &Path) -> Result<()> {
        let tag = image
            .rsplit_once(':')
            .map(|(_, t)| t) // take the part after the last ':'
            .unwrap_or("latest");

        let image = format!("{image}:{tag}");

        // --- progress bar
        let pb = ProgressBar::new(Self::estimate_image_size(image.as_str()).unwrap_or(0));
        pb.set_style(ProgressStyle::with_template(
            "{prefix:.dim} [{bar:40.cyan/blue}] {bytes}/{total_bytes} [ETA:{eta}]",
        )?);

        pb.set_prefix(format!("Exporting Docker image '{}' to tarball...", image));

        // --- create temp file (atomic rename later)
        let tmp_path = tar_path.with_extension("partial");
        let tmp_file = fs::File::create(&tmp_path)
            .with_context(|| format!("create temp file at {}", tmp_path.display()))?;
        let mut writer = BufWriter::new(tmp_file);

        // --- spawn docker save (to stdout)
        let mut child = Command::new("docker")
            .args(["save", image.as_str()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()) // capture errors
            .spawn()
            .context("failed to spawn `docker save`")?;

        // --- continuously drain stderr to avoid pipe blocking + keep logs
        let mut err_reader = BufReader::new(child.stderr.take().expect("no stderr"));
        let err_collector = std::thread::spawn(move || {
            let mut buf = String::new();
            let mut acc = String::new();
            while let Ok(n) = err_reader.read_line(&mut buf) {
                if n == 0 {
                    break;
                }
                acc.push_str(&buf);
                buf.clear();
            }
            acc
        });

        // --- stream stdout to file and update progress
        let mut out = child.stdout.take().context("child has no stdout")?;
        let mut buf = vec![0u8; 1024 * 1024]; // 1 MiB chunks
        let mut written: u64 = 0;

        loop {
            let n = out.read(&mut buf)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n])?;
            written += n as u64;
            pb.set_position(written);
        }
        writer.flush()?;

        // --- wait and check status
        let status = child.wait()?;
        let stderr_text = err_collector.join().unwrap_or_default();

        if !status.success() {
            pb.finish_and_clear();
            // leave the .partial file for forensics or remove it:
            let _ = fs::remove_file(&tmp_path);
            return Err(anyhow!(
                "`docker save` failed (exit {:?}). stderr:\n{}",
                status.code(),
                stderr_text
            ));
        }

        // --- rename atomically
        fs::rename(&tmp_path, tar_path)
            .with_context(|| format!("rename {} -> {}", tmp_path.display(), tar_path.display()))?;

        // pb.finish_with_message("✅");
        pb.finish_and_clear();

        Ok(())
    }

    fn get_image_tarball(
        &self,
        image_name: &str,
        notifier: &AnyNotifier,
    ) -> Result<(PathBuf, Option<TempDir>)> {
        // Create a temporary directory to save the image
        let temp_dir = TempDir::new().context("Failed to create temporary directory")?;
        let tarball_path = temp_dir.path().join("image.tar");

        notifier.println_above(format!("{} {}", LOOK, style("Resolving image").bold()));

        notifier.finish_spinner();

        let save_result = Self::image_save_with_progress(image_name, &tarball_path);

        match save_result {
            Ok(_) => {
                // Success - return the tarball path
                Ok((tarball_path, Some(temp_dir)))
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Check if the error is about missing image
                if error_msg.contains("No such image")
                    || error_msg.contains("pull access denied")
                    || error_msg.contains("reference does not exist")
                {
                    notifier.info(&format!(
                        "Image '{}' not found locally, attempting to pull...",
                        image_name
                    ));

                    // Try to pull the image
                    match notifier {
                        AnyNotifier::Simple(notifier) => self
                            .pull_image(image_name, notifier)
                            .context(format!("Failed to pull image '{}'", image_name))?,
                        AnyNotifier::Enhanced(notifier) => self
                            .pull_image_enhanced(image_name, notifier)
                            .context(format!("Failed to pull image '{}'", image_name))?,
                    }

                    // Retry the save command after successful pull
                    notifier.info(&format!(
                        "Retrying export of Docker image '{}' to tarball...",
                        image_name
                    ));
                    self.run_command(&["save", "-o", tarball_path.to_str().unwrap(), image_name])
                        .context(format!("Failed to save image '{}' after pull", image_name))?;

                    Ok((tarball_path, Some(temp_dir)))
                } else {
                    // Different error - propagate it
                    Err(e)
                }
            }
        }
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
    fn test_docker_source_branch_name() {
        let source = DockerSource::new().unwrap();
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
