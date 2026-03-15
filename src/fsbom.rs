//! Filesystem Bill of Materials (fsbom) generation.
//!
//! This module provides a pure read-only scan of OCI image layer tarballs to produce
//! a per-layer listing of all files introduced or modified in each layer.
//!
//! Key features:
//! - YAML format (hand-written, no extra dependency)
//! - `status: new | modified` per entry
//! - Deleted files (whiteouts) are excluded from output
//! - Single flat `entries` list per layer with type-tagged entries

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::path::Path;
use tar::EntryType;
use tar_rs as tar;

/// Status of a filesystem entry relative to previous layers.
pub enum EntryStatus {
    New,
    Modified,
}

/// A single filesystem entry in a layer's bill of materials.
pub enum FsEntry {
    /// Regular file.
    File {
        path: String,
        size: u64,
        mode: u32,
        stat: String,
    },
    /// Hard link — another directory entry pointing to the same inode as `target`.
    Hardlink {
        path: String,
        target: String,
        stat: String,
    },
    Directory {
        path: String,
        mode: u32,
        stat: String,
    },
    /// Soft (symbolic) link.
    Symlink {
        path: String,
        target: String,
        stat: String,
    },
}

fn stat_str(status: EntryStatus, uid: u32, gid: u32) -> String {
    let s = match status {
        EntryStatus::New => 'n',
        EntryStatus::Modified => 'm',
    };
    format!("{s}:{uid}:{gid}")
}

/// Bill of materials for a single image layer.
pub struct LayerBom {
    pub index: usize,
    pub command: String,
    pub digest: String,
    pub entries: Vec<FsEntry>,
}

/// Complete filesystem bill of materials for an image.
pub struct FsBom {
    pub layers: Vec<LayerBom>,
}

/// Scan a layer tarball and produce a [`LayerBom`].
///
/// Reads tar headers only — no extraction to disk.
/// `seen_paths` tracks all paths materialized so far across layers for new/modified detection.
pub fn scan_layer(
    tar_path: &Path,
    seen_paths: &mut HashSet<String>,
    index: usize,
    command: String,
    digest: String,
) -> Result<LayerBom> {
    let file = std::fs::File::open(tar_path)
        .with_context(|| format!("Failed to open layer tarball: {tar_path:?}"))?;

    // Detect gzip by magic bytes
    let mut magic = [0u8; 2];
    {
        use std::io::Read;
        let mut peek = std::io::BufReader::new(&file);
        peek.read_exact(&mut magic).unwrap_or(());
    }

    // Re-open for actual reading
    let file = std::fs::File::open(tar_path)
        .with_context(|| format!("Failed to re-open layer tarball: {tar_path:?}"))?;

    let mut entries: Vec<FsEntry> = Vec::new();

    if magic == [0x1f, 0x8b] {
        // gzip compressed
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        scan_archive(&mut archive, seen_paths, &mut entries)?;
    } else {
        let mut archive = tar::Archive::new(file);
        scan_archive(&mut archive, seen_paths, &mut entries)?;
    }

    Ok(LayerBom {
        index,
        command,
        digest,
        entries,
    })
}

fn scan_archive<R: std::io::Read>(
    archive: &mut tar::Archive<R>,
    seen_paths: &mut HashSet<String>,
    entries: &mut Vec<FsEntry>,
) -> Result<()> {
    for entry in archive.entries().context("Failed to read tar entries")? {
        let entry: tar::Entry<'_, R> = entry.context("Failed to read tar entry")?;
        let header = entry.header();

        let path = entry
            .path()
            .context("Failed to read entry path")?
            .to_string_lossy()
            .trim_start_matches("./")
            .to_string();

        // Handle whiteout files
        let file_name = Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if file_name == ".wh..wh..opq" {
            // Opaque whiteout: remove all seen_paths entries under parent dir
            let parent = Path::new(&path)
                .parent()
                .map(|p| {
                    let s = p.to_string_lossy().to_string();
                    if s.is_empty() {
                        String::new()
                    } else {
                        format!("{s}/")
                    }
                })
                .unwrap_or_default();
            seen_paths.retain(|p| !p.starts_with(&parent));
            continue;
        }

        if let Some(orig_name) = file_name.strip_prefix(".wh.") {
            // Regular whiteout: remove the specific path
            let parent = Path::new(&path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let orig_path = if parent.is_empty() {
                orig_name.to_string()
            } else {
                format!("{parent}/{orig_name}")
            };
            seen_paths.remove(&orig_path);
            continue;
        }

        let uid = header.uid().unwrap_or(0) as u32;
        let gid = header.gid().unwrap_or(0) as u32;
        let mode = header.mode().unwrap_or(0);

        let status = if seen_paths.contains(&path) {
            EntryStatus::Modified
        } else {
            EntryStatus::New
        };
        seen_paths.insert(path.clone());

        let stat = stat_str(status, uid, gid);

        let link_target = || {
            header
                .link_name()
                .ok()
                .flatten()
                .map(|p: std::borrow::Cow<'_, std::path::Path>| p.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        match header.entry_type() {
            EntryType::Regular => {
                let size = header.size().unwrap_or(0);
                entries.push(FsEntry::File {
                    path,
                    size,
                    mode,
                    stat,
                });
            }
            EntryType::Link => {
                entries.push(FsEntry::Hardlink {
                    path,
                    target: link_target(),
                    stat,
                });
            }
            EntryType::Directory => {
                entries.push(FsEntry::Directory { path, mode, stat });
            }
            EntryType::Symlink => {
                entries.push(FsEntry::Symlink {
                    path,
                    target: link_target(),
                    stat,
                });
            }
            _ => {
                // Skip other entry types (block/char devices, fifos, etc.)
            }
        }
    }
    Ok(())
}

impl FsBom {
    /// Serialize this BOM to YAML and write to `path`.
    ///
    /// Hand-written YAML — no external dependency.
    pub fn save_yaml(&self, path: &Path) -> Result<()> {
        let mut out = String::new();
        writeln!(out, "layers:").unwrap();

        for layer in &self.layers {
            writeln!(out, "  - index: {}", layer.index).unwrap();
            writeln!(out, "    command: {:?}", layer.command).unwrap();
            writeln!(out, "    digest: {:?}", layer.digest).unwrap();
            writeln!(out, "    entries:").unwrap();

            if layer.entries.is_empty() {
                writeln!(out, "      []").unwrap();
            } else {
                for entry in &layer.entries {
                    match entry {
                        FsEntry::File {
                            path,
                            size,
                            mode,
                            stat,
                        } => {
                            writeln!(out, "      - type: file").unwrap();
                            writeln!(out, "        path: {:?}", path).unwrap();
                            writeln!(out, "        size: {size}").unwrap();
                            writeln!(out, "        mode: {mode}").unwrap();
                            writeln!(out, "        stat: {:?}", stat).unwrap();
                        }
                        FsEntry::Hardlink { path, target, stat } => {
                            writeln!(out, "      - type: hardlink").unwrap();
                            writeln!(out, "        path: {:?}", path).unwrap();
                            writeln!(out, "        target: {:?}", target).unwrap();
                            writeln!(out, "        stat: {:?}", stat).unwrap();
                        }
                        FsEntry::Directory { path, mode, stat } => {
                            writeln!(out, "      - type: directory").unwrap();
                            writeln!(out, "        path: {:?}", path).unwrap();
                            writeln!(out, "        mode: {mode}").unwrap();
                            writeln!(out, "        stat: {:?}", stat).unwrap();
                        }
                        FsEntry::Symlink { path, target, stat } => {
                            writeln!(out, "      - type: symlink").unwrap();
                            writeln!(out, "        path: {:?}", path).unwrap();
                            writeln!(out, "        target: {:?}", target).unwrap();
                            writeln!(out, "        stat: {:?}", stat).unwrap();
                        }
                    }
                }
            }
        }

        std::fs::write(path, &out)
            .with_context(|| format!("Failed to write fsbom YAML to {path:?}"))?;
        Ok(())
    }
}
