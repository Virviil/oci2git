use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};
use tar_rs as tar;

/// Normalizes a path from a tar archive to be safe for extraction
/// Removes any attempts to escape the root directory
fn normalize_tar_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();

    for comp in p.components() {
        match comp {
            Component::CurDir => { /* skip "." */ }
            Component::ParentDir => {
                // Don't allow escaping the rootfs
                out.pop();
            }
            Component::Normal(c) => out.push(c),
            // Ignore absolute paths and Windows prefixes
            Component::RootDir | Component::Prefix(_) => { /* ignore */ }
        }
    }

    out
}

/// Attempts to create a hardlink, falling back to copy if hardlinks aren't supported
/// Returns Ok(()) if successful, Err if the target doesn't exist (caller should skip)
fn try_link_or_copy(target: &Path, dest: &Path) -> Result<()> {
    if !target.exists() {
        // Target doesn't exist - this can happen if:
        // 1. The target was removed by a whiteout in this layer
        // 2. The target was replaced/removed in a previous layer
        // 3. The tar archive has broken hardlinks
        // We should skip this hardlink rather than failing
        return Err(anyhow::anyhow!("hardlink target missing: {}", target.display()));
    }

    // Remove destination if it already exists
    if dest.exists() {
        fs::remove_file(dest)
            .with_context(|| format!("Failed to remove existing file: {}", dest.display()))?;
    }

    // Try to create a hardlink first
    if let Err(e) = fs::hard_link(target, dest) {
        // Filesystem might not support hardlinks - fall back to copy
        log::debug!(
            "hardlink failed ({}), falling back to copy: {} -> {}",
            e,
            target.display(),
            dest.display()
        );
        fs::copy(target, dest)
            .with_context(|| format!("Failed to copy {} to {}", target.display(), dest.display()))?;
    }

    Ok(())
}

struct PendingHardlink {
    dest: PathBuf,
    target: PathBuf,
}

struct PendingSymlink {
    dest: PathBuf,
    target: PathBuf,
}

/// Extracts a tar archive (plain or gzipped) to the specified directory
/// Handles hardlinks, permissions, and whiteouts in a single pass
pub fn extract_tar(tar_path: &Path, extract_dir: &Path) -> Result<()> {
    // Detect if the file is gzip compressed
    let file = File::open(tar_path)
        .with_context(|| format!("Failed to open tar file: {}", tar_path.display()))?;

    let mut buf_reader = BufReader::new(file);
    let mut magic_bytes = [0u8; 2];
    buf_reader.read_exact(&mut magic_bytes)
        .context("Failed to read magic bytes from tar file")?;

    // Reopen the file since we consumed some bytes
    let file = File::open(tar_path)?;

    let mut archive: tar::Archive<Box<dyn Read>> = if magic_bytes == [0x1f, 0x8b] {
        // Gzip compressed
        tar::Archive::new(Box::new(GzDecoder::new(file)))
    } else {
        // Plain tar
        tar::Archive::new(Box::new(file))
    };

    // First pass: extract all regular files, directories, and symlinks
    // Store hardlinks and failed symlinks for second pass
    let mut pending_hardlinks = Vec::new();
    let mut pending_symlinks = Vec::new();

    for entry_result in archive.entries()? {
        let mut entry = entry_result.context("Failed to read tar entry")?;
        let header = entry.header();
        let entry_type = header.entry_type();

        let tar_path = entry.path()
            .context("Failed to get entry path")?;
        let rel_path = normalize_tar_path(&tar_path);

        // Check for whiteout files (overlay filesystem markers)
        if let Some(file_name) = rel_path.file_name().and_then(|n| n.to_str()) {
            if file_name == ".wh..wh..opq" {
                // Opaque directory marker - remove all contents of parent directory
                if let Some(parent) = rel_path.parent() {
                    let opaque_dir = extract_dir.join(parent);
                    if opaque_dir.exists() && opaque_dir.is_dir() {
                        log::debug!("Found opaque directory marker, clearing: {}", opaque_dir.display());
                        for entry in fs::read_dir(&opaque_dir)? {
                            let entry = entry?;
                            let path = entry.path();
                            if path.is_dir() {
                                fs::remove_dir_all(&path).ok();
                            } else {
                                fs::remove_file(&path).ok();
                            }
                        }
                    }
                }
                continue; // Skip the marker file itself
            } else if file_name.starts_with(".wh.") {
                // Whiteout marker - delete the target file/directory
                let deleted_name = &file_name[4..]; // Remove ".wh." prefix
                if let Some(parent) = rel_path.parent() {
                    let deleted_path = extract_dir.join(parent).join(deleted_name);
                    if deleted_path.exists() {
                        log::debug!("Found whiteout marker, deleting: {}", deleted_path.display());
                        if deleted_path.is_dir() {
                            fs::remove_dir_all(&deleted_path).ok();
                        } else {
                            fs::remove_file(&deleted_path).ok();
                        }
                    }
                }
                continue; // Skip the whiteout marker itself
            }
        }

        let dest = extract_dir.join(&rel_path);

        // Create parent directories and ensure they're writable
        if let Some(parent) = dest.parent() {
            log::debug!("Creating parent directory: {}", parent.display());
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;

            // Verify parent exists after creation
            if !parent.exists() {
                anyhow::bail!("Parent directory doesn't exist after create_dir_all: {}", parent.display());
            }

            // Always set writable permissions on parent (simple and safe)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms_result = fs::set_permissions(parent, fs::Permissions::from_mode(0o755));
                if let Err(e) = perms_result {
                    log::warn!("Failed to set permissions on {}: {}", parent.display(), e);
                }

                // Verify parent is actually a directory
                if !parent.is_dir() {
                    anyhow::bail!("Parent path exists but is not a directory: {}", parent.display());
                }
            }
        }

        match entry_type {
            tar::EntryType::Directory => {
                fs::create_dir_all(&dest)
                    .with_context(|| format!("Failed to create directory: {}", dest.display()))?;

                // Always set writable permissions on directories (0755 minimum)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = header.mode().unwrap_or(0o755);
                    // Ensure directory is at least writable/executable by owner
                    let safe_mode = mode | 0o700;
                    let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(safe_mode));
                }
            }
            tar::EntryType::Regular => {
                // Get mode before consuming entry
                #[cfg(unix)]
                let mode = header.mode().ok();

                // Delete existing file if it exists (overlay behavior)
                // Use symlink_metadata to detect symlinks even if they're broken
                if let Ok(metadata) = fs::symlink_metadata(&dest) {
                    log::debug!("Removing existing entry at: {} (is_symlink: {})", dest.display(), metadata.is_symlink());
                    if metadata.is_dir() && !metadata.is_symlink() {
                        fs::remove_dir_all(&dest).ok();
                    } else {
                        // Remove file or symlink
                        fs::remove_file(&dest).ok();
                    }
                }

                log::debug!("Creating file: {}", dest.display());
                let mut out_file = File::create(&dest)
                    .with_context(|| {
                        let parent_info = if let Some(parent) = dest.parent() {
                            format!(" (parent: {}, exists: {}, is_dir: {})",
                                parent.display(),
                                parent.exists(),
                                parent.is_dir())
                        } else {
                            String::from(" (no parent)")
                        };
                        format!("Failed to create file: {}{}", dest.display(), parent_info)
                    })?;

                std::io::copy(&mut entry, &mut out_file)
                    .with_context(|| format!("Failed to write file: {}", dest.display()))?;

                // Set permissions - ensure file is at least readable by owner for git
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mut mode) = mode {
                        // Ensure owner can read (add 0o400 if not present)
                        if mode & 0o400 == 0 {
                            mode |= 0o400;
                            log::debug!("Fixed unreadable file during extraction: {}", dest.display());
                        }
                        let perms = fs::Permissions::from_mode(mode);
                        let _ = fs::set_permissions(&dest, perms);
                    }
                }
            }
            tar::EntryType::Symlink => {
                let link_name = header.link_name()
                    .context("Failed to get symlink target")?
                    .ok_or_else(|| anyhow::anyhow!("Symlink without target"))?;

                // ALWAYS resolve target path relative to extract_dir (rootfs) as ABSOLUTE path
                let target_path = if link_name.is_absolute() {
                    // Absolute symlink like /usr/share/foo -> extract_dir/usr/share/foo
                    let normalized = normalize_tar_path(&link_name);
                    extract_dir.join(normalized)
                } else {
                    // Relative symlink - resolve from the symlink's parent directory
                    if let Some(parent) = dest.parent() {
                        parent.join(&link_name)
                    } else {
                        extract_dir.join(&link_name)
                    }
                };

                // Canonicalize extract_dir to ensure we have an absolute path
                let absolute_target = if target_path.is_absolute() {
                    target_path
                } else {
                    // Make relative paths absolute by joining with extract_dir
                    std::env::current_dir()
                        .ok()
                        .and_then(|cwd| cwd.join(&target_path).canonicalize().ok())
                        .unwrap_or(target_path)
                };

                #[cfg(unix)]
                {
                    // Try to create symlink with the absolute target path
                    if let Err(e) = std::os::unix::fs::symlink(&absolute_target, &dest) {
                        log::debug!(
                            "Failed to create symlink {} -> {}: {}. Will try to copy target...",
                            dest.display(),
                            absolute_target.display(),
                            e
                        );

                        // Save for retry after all files are extracted
                        pending_symlinks.push(PendingSymlink {
                            dest,
                            target: absolute_target,
                        });
                    }
                }

                #[cfg(not(unix))]
                {
                    log::warn!("Symlink support not implemented on this platform: {}", dest.display());
                }
            }
            tar::EntryType::Link => {
                // Hardlink - save for second pass
                let link_name = header.link_name()
                    .context("Failed to get hardlink target")?
                    .ok_or_else(|| anyhow::anyhow!("Hardlink without target"))?;

                let target_rel = normalize_tar_path(&link_name);
                let target = extract_dir.join(&target_rel);

                pending_hardlinks.push(PendingHardlink {
                    dest,
                    target,
                });
            }
            _ => {
                // Other entry types (char device, block device, fifo, etc.)
                log::debug!("Skipping unsupported entry type: {:?}", entry_type);
            }
        }
    }

    // Second pass: create hardlinks (with retry queue for missing targets)
    let mut failed_hardlinks = Vec::new();
    for hardlink in pending_hardlinks {
        if let Err(e) = try_link_or_copy(&hardlink.target, &hardlink.dest) {
            log::debug!(
                "Hardlink target not found yet, will retry: {} -> {}: {}",
                hardlink.dest.display(),
                hardlink.target.display(),
                e
            );
            // Add to dead letter queue - target might be extracted later
            failed_hardlinks.push(hardlink);
        }
    }

    // Third pass: retry failed hardlinks (targets might now exist)
    for hardlink in failed_hardlinks {
        if let Err(e) = try_link_or_copy(&hardlink.target, &hardlink.dest) {
            log::warn!(
                "Skipping broken hardlink (target still missing): {} -> {}: {}",
                hardlink.dest.display(),
                hardlink.target.display(),
                e
            );
            // Skip this hardlink - the target truly doesn't exist
        }
    }

    // Fourth pass: retry failed symlinks (copy target files)
    for symlink in pending_symlinks {
        if symlink.target.exists() {
            log::debug!(
                "Retrying symlink by copying: {} -> {}",
                symlink.target.display(),
                symlink.dest.display()
            );
            if let Err(e) = fs::copy(&symlink.target, &symlink.dest) {
                log::warn!(
                    "Failed to copy symlink target {} -> {}: {}. Skipping.",
                    symlink.target.display(),
                    symlink.dest.display(),
                    e
                );
            }
        } else {
            log::debug!(
                "Symlink target still does not exist: {} -> {}. Skipping.",
                symlink.dest.display(),
                symlink.target.display()
            );
        }
    }

    Ok(())
}
