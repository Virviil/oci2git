//! End-to-end “OCI image → Git repo” pipeline orchestrator.
//!
//! This module provides [`ImageProcessor`], a high-level orchestrator that:
//! - fetches an image tarball from a concrete [`crate::sources::Source`],
//! - unpacks and replays the ordered filesystem layers into a working `rootfs/`,
//! - commits each step into a Git branch (one commit per layer, preserving history),
//! - and finishes with a metadata commit (`Image.md`) that captures image basics,
//!   container config, and the full layer digest chain.
//!
//! Duplicate safety: if a matching branch exists and all layers match, conversion is skipped.
//!
//! Construction helpers:
//! - [`ImageProcessor::new`] — inject a concrete [`Source`] and a [`Notifier`].

use crate::digest_tracker::DigestTracker;
use crate::extracted_image::ExtractedImage;
use crate::git::GitRepo;
use crate::image_metadata::ImageMetadata;
use crate::notifier::AnyNotifier;
use crate::sources::Source;
use crate::successor_navigator::SuccessorNavigator;
use anyhow::{Context, Result};
use console::{style, Emoji};
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use walkdir;

/// Orchestrates the OCI image to Git repo conversion pipeline for a concrete [`Source`].
///
/// The processor downloads (or otherwise obtains) an image tarball via `S`,
/// replays its layers into `rootfs/` with overlay/whiteout semantics, and
/// records each step as a Git commit on a branch derived from source + tag + digest.
/// The last commit writes `Image.md` with comprehensive metadata.
///
/// ### Type parameters
/// - `S`: a concrete image source (see [`crate::sources`]) that knows how to
///   retrieve an image tarball and suggest a branch name.
///
/// ### Concurrency
/// `ImageProcessor<S>` does not spawn threads by itself. It is `Send`/`Sync` only
/// if `S` and [`Notifier`] are.
pub struct ImageProcessor<S: Source> {
    /// The concrete image source (registry/daemon/nerdctl/tar, etc.).
    source: S,
    notifier: AnyNotifier,
}

static SPARK: Emoji<'_, '_> = Emoji("✨ ", ":-)");
static EXTRACT: Emoji<'_, '_> = Emoji("🧱 ", ":-)");

impl<S: Source> ImageProcessor<S> {
    /// Constructs a new processor that will use the given [`Source`] and [`Notifier`].
    ///
    /// The processor has no internal state beyond these handles; reuse it to process
    /// multiple images with the same source/notification strategy.
    ///
    /// Check [`crate::notifier::VerbosityLevel`] for more verbosity levels params
    ///
    pub fn new(source: S, notifier: AnyNotifier) -> Self {
        Self { source, notifier }
    }
    /// Convert an image into a Git repository at `output_dir`.
    ///
    /// This will:
    /// 1. **Fetch** the image tarball via `S` and build an [`ExtractedImage`].
    /// 2. **Analyze** layers (oldest → newest) and read base metadata (OS, arch, id, tags).
    /// 3. **Initialize/Open** a [`GitRepo`] in `output_dir`, derive the branch name with
    ///    [`Source::branch_name`], and find an optimal branch point using
    ///    [`SuccessorNavigator`] (skipping already-materialized layers when possible).
    /// 4. **Replay** each layer into `rootfs/`, interpreting overlayfs whiteouts
    ///    (`.wh.*`) and opaque directories (`.wh..wh..opq`) as per the OCI layer spec.
    /// 5. **Commit** one layer per commit (empty layers become metadata-only commits),
    ///    maintain a running [`DigestTracker`], and keep `Image.md` in sync via
    ///    [`ImageMetadata`].
    /// 6. **Finish** with a final metadata commit including basic info, container config,
    ///    and the complete digest history.
    ///
    /// If a branch with matching content already exists, the conversion is skipped.
    ///
    /// # Parameters
    /// - `image_name`: something your [`Source`] can resolve (e.g. `"alpine:3.20"` or
    ///   a tar/OCI reference depending on `S`).
    /// - `output_dir`: an existing or new directory that will contain a Git repo,
    ///   a working `rootfs/`, and `Image.md`.
    ///
    /// # Returns
    /// `Ok(())` on success. On failure, returns an [`anyhow::Result`] describing
    /// the error with context. You can bubble these up or downcast as needed.
    ///
    /// # Errors
    /// - Image fetch/extraction failures from the underlying [`Source`] or tar processing
    ///   (I/O, format, missing layers).
    /// - Git repository initialization/commit errors.
    /// - Filesystem operations while applying layers (permissions, symlinks, deletions).
    /// - Metadata serialization/parsing of `Image.md`.
    ///
    /// # Panics
    /// This method is not intended to panic. If you observe a panic, please file a bug
    /// with the offending image and stack trace.
    ///
    /// ### Examples
    /// ```no_run
    /// use std::path::Path;
    /// use oci2git::notifier::{AnyNotifier, NotifierFlavor};
    /// use oci2git::{DockerSource, ImageProcessor };
    ///
    /// // Choose your source (e.g., Docker daemon/registry, nerdctl, tar file, etc.)
    /// // let src = DockerSource;    // or TarSource::new("image.tar")?
    /// let src = DockerSource;
    /// let notifier = AnyNotifier::new(NotifierFlavor::Simple, 0);
    ///
    /// let p = ImageProcessor::new(src, notifier);
    /// p.convert("ubuntu:latest", Path::new("./ubuntu-image-repo"))?;
    /// # anyhow::Ok(())
    /// ```
    pub fn convert(&self, image_name: &str, output_dir: &Path) -> Result<()> {
        let started = Instant::now();
        self.notifier.info(&format!(
            "Starting conversion of image with {} source: {}",
            self.source.name(),
            image_name
        ));
        self.notifier
            .debug(&format!("Output directory: {}", output_dir.display()));

        // Store all temporary directories we need to keep alive during processing
        let mut temp_dirs: Vec<tempfile::TempDir> = Vec::new();

        // Get the image tarball from the source
        self.notifier.info(&format!(
            "Getting image tarball using {} source...",
            self.source.name()
        ));

        let (tarball_path, tarball_temp_dir) =
            self.source.get_image_tarball(image_name, &self.notifier)?;

        // Store the tarball temp dir if it exists
        if let Some(temp_dir) = tarball_temp_dir {
            temp_dirs.push(temp_dir);
        }

        // Extract the tarball and create ExtractedImage
        self.notifier.info("Extracting image tarball...");

        self.notifier.println_above(format!(
            "{} {}",
            EXTRACT,
            style("Extracting & committing layers").bold()
        ));

        let extracted_image = ExtractedImage::from_tarball(&tarball_path, &self.notifier)?;

        // Get the layers in chronological order (oldest to newest)
        self.notifier.info("Analyzing image layers...");

        let layers = extracted_image.layers()?;
        self.notifier
            .debug(&format!("Found {} layers in the image", layers.len()));

        self.notifier.info("Extracting image metadata...");

        let metadata = extracted_image.metadata(image_name)?;
        self.notifier.debug(&format!("Image ID: {}", metadata.id));

        self.notifier.info("Initializing Git repository...");

        // Create branch name using polymorphic method from source
        let os_arch = format!("{}-{}", metadata.os, metadata.architecture);
        self.notifier.debug(&format!(
            "Creating branch name for image '{}' with os-arch '{}' and digest: '{}'",
            image_name, os_arch, metadata.id
        ));
        let branch_name = self.source.branch_name(image_name, &os_arch, &metadata.id);
        self.notifier
            .debug(&format!("Generated branch name: '{}'", branch_name));

        // Initialize or open repository
        let repo = GitRepo::init_with_branch(output_dir, None)?;

        // Determine start commit and skip count using successor navigation
        let (start_from_commit, skip_layers) = if repo.exists_and_has_commits() {
            self.notifier
                .info("Existing repository detected, finding optimal branch point...");

            let (branch_commit, matched_layers) =
                SuccessorNavigator::find_branch_point(&repo, output_dir, &layers)?;
            match branch_commit {
                Some(commit) => {
                    self.notifier.info(&format!(
                        "Found optimal branch point at commit {}, skipping {} matched layers",
                        commit, matched_layers
                    ));
                    (Some(commit), matched_layers)
                }
                None => {
                    self.notifier
                        .info("No matching path found, creating orphaned branch");
                    (None, 0)
                }
            }
        } else {
            self.notifier
                .info("New repository, creating initial branch");
            (None, 0)
        };

        // Check if this is a duplicate image - if branch exists, and we're skipping all layers,
        // it means we're processing the exact same image again
        if repo.branch_exists(&branch_name) && skip_layers == layers.len() {
            self.notifier.println_above(format!(
                "Image '{}' already exists as branch '{}' with identical content. Skipping duplicate processing.",
                image_name, branch_name
            ));
            return Ok(());
        }

        // Create the branch from the optimal point
        repo.create_branch(&branch_name, start_from_commit)?;

        // Create the rootfs directory
        let rootfs_dir = output_dir.join("rootfs");
        fs::create_dir_all(&rootfs_dir)?;

        // If there are no layers, exit early
        if layers.is_empty() {
            self.notifier.warn("No layers found in the image");
            self.notifier.info("Warning: No layers found in the image");
            return Ok(());
        }

        // Count layers with tarballs for debugging info
        let layers_with_tarballs = layers.iter().filter(|l| l.tarball_path.is_some()).count();
        self.notifier.debug(&format!(
            "Found {} layers with tarballs out of {} total layers",
            layers_with_tarballs,
            layers.len()
        ));

        // Process each layer in order (oldest to newest)
        // We'll process all layers from the history, but only extract the real layer tarballs

        // Create a temporary directory for layer extraction
        self.notifier.info("Preparing layer extraction...");

        // Create a temporary directory for layer extraction and keep a reference to its path
        let temp_layer_dir = tempfile::tempdir()?;
        let temp_layer_path = temp_layer_dir.path().to_path_buf();
        // Store the temp_dir to keep it alive until the end of the function
        temp_dirs.push(temp_layer_dir);

        // Each layer now contains its own tarball path and digest information
        self.notifier.debug(&format!(
            "Processing {} layers, {} with tarballs",
            layers.len(),
            layers_with_tarballs
        ));

        // Initialize digest tracker for new commits
        let mut new_digest_tracker = if let Some(start_commit) = start_from_commit {
            // Load existing digest tracker from the start commit Image.md
            match repo.read_file_from_commit(start_commit, "Image.md") {
                Ok(content) => {
                    let image_metadata = ImageMetadata::parse_markdown(&content)
                        .context("Failed to parse existing Image.md")?;
                    DigestTracker {
                        layer_digests: image_metadata.layer_digests,
                    }
                }
                Err(_) => {
                    // No Image.md in the start commit, create new tracker
                    DigestTracker::new()
                }
            }
        } else {
            // Starting fresh, create new tracker
            DigestTracker::new()
        };

        // Initialize structured image metadata with only layer data (no basic_info or container_config until final commit)
        let mut structured_metadata = ImageMetadata::new(None, None);
        structured_metadata.update_layer_digests(&new_digest_tracker);

        // Now process layers starting from the first unmatched layer
        let layers_to_process = layers.len() - skip_layers;
        self.notifier.info(&format!(
            "Processing {} layers (skipping {} matched layers)...",
            layers_to_process, skip_layers
        ));

        let use_multi = matches!(&self.notifier, AnyNotifier::Enhanced(_));

        let (m, pb1, sty2) = if use_multi && layers_to_process > 0 {
            let m = Arc::new(indicatif::MultiProgress::new());

            let sty = ProgressStyle::default_bar().template(
                "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} \
             {pos:>10}/{len:6} {msg} [ETA:{eta}]",
            )?;

            let sty2 = ProgressStyle::with_template(
                "{spinner:.green} {msg} {prefix:.bold.dim} {bar:40.cyan/blue} \
         {pos:>10}/{len:7} [{eta}]",
            )?
            .progress_chars("=> ");

            let pb1 = m.add(ProgressBar::new(layers_to_process as u64));
            pb1.set_style(sty);
            pb1.enable_steady_tick(Duration::from_millis(80));

            (Some(m), Some(pb1), Some(sty2))
        } else {
            (None, None, None)
        };

        if let Some(ref pb1) = pb1 {
            pb1.inc(1);
        }

        for (i, layer) in layers.iter().enumerate().skip(skip_layers) {
            self.notifier.info(&format!(
                "Layer {}/{}: {}",
                i + 1,
                layers.len(),
                layer.command
            ));
            self.notifier.debug(&format!(
                "Layer has tarball: {}",
                layer.tarball_path.is_some()
            ));

            // Check if this layer already exists at the same position in current branch
            if new_digest_tracker.layer_matches(i, layer) {
                self.notifier.debug(&format!(
                    "Layer {} already exists with same digest, skipping unpacking",
                    i + 1
                ));
                continue;
            }

            if layer.tarball_path.is_none() {
                // Create an empty commit for layers without file changes
                let commit_message = if layer.is_empty {
                    format!("⚪️ - {}", layer.command)
                } else {
                    format!("⚫ - {}", layer.command)
                };

                // Track empty layer in digest tracker
                // Use the current length of the digest tracker as the new position
                new_digest_tracker.add_layer(
                    new_digest_tracker.layer_digests.len(),
                    layer.digest.clone(),
                    layer.command.clone(),
                    layer.created_at.to_rfc3339(),
                    layer.is_empty,
                    layer.comment.clone(),
                );

                // Update structured metadata with current layer digests and save Image.md
                structured_metadata.update_layer_digests(&new_digest_tracker);
                let metadata_path = output_dir.join("Image.md");
                structured_metadata.save_markdown(&metadata_path)?;

                self.notifier.debug(&format!(
                    "Creating empty commit for layer: {}",
                    layer.command
                ));
                repo.commit_all_changes(&commit_message)?;
                continue;
            }

            let layer_tarball = layer.tarball_path.as_ref().unwrap();

            // Extract this layer to the temporary directory
            self.notifier.info(&format!("{}/{}", i + 1, layers.len()));

            self.notifier
                .debug(&format!("Extracting tarball: {:?}", layer_tarball));
            fs::create_dir_all(&temp_layer_path)?;

            // Extract the layer tarball to the temp directory
            extracted_image.extract_layer_to(layer_tarball, &temp_layer_path)?;

            // Recursively copy all files from the temp layer directory to rootfs
            // ---- count only what we'll actually process (files & symlinks, no whiteouts) ----
            let entry_count = walkdir::WalkDir::new(&temp_layer_path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let ft = e.file_type();
                    if !(ft.is_file() || ft.is_symlink()) {
                        return false;
                    }
                    let name = e.file_name().to_string_lossy();
                    // exclude overlayfs whiteouts from the progress denominator
                    name != ".wh..wh..opq" && !name.starts_with(".wh.")
                })
                .count() as u64;

            let pb2 = if let (Some(m), Some(sty2)) = (m.as_ref(), sty2.as_ref()) {
                let pb = m.add(ProgressBar::new(entry_count));
                pb.set_style(sty2.clone());
                pb.set_message("|________");
                Some(pb)
            } else {
                None
            };

            for entry in walkdir::WalkDir::new(&temp_layer_path).follow_links(false) {
                let entry = entry.context("Failed to read directory entry")?;
                let source_path = entry.path();

                if source_path == temp_layer_path {
                    continue;
                }

                let relative_path = source_path
                    .strip_prefix(&temp_layer_path)
                    .context("Failed to get relative path")?;

                // Skip whiteouts/opaque; do NOT tick for these
                if let Some(fname) = relative_path.file_name().and_then(|s| s.to_str()) {
                    if fname == ".wh..wh..opq" {
                        let parent_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
                        let opaque_dir = rootfs_dir.join(parent_dir);
                        if opaque_dir.exists() && opaque_dir.is_dir() {
                            for path in fs::read_dir(&opaque_dir)
                                .unwrap_or_else(|_| fs::read_dir(".").unwrap())
                                .flatten()
                                .map(|e| e.path())
                            {
                                if path.is_dir() {
                                    fs::remove_dir_all(&path).ok();
                                } else {
                                    fs::remove_file(&path).ok();
                                }
                            }
                        }
                        continue;
                    } else if let Some(deleted_name) = fname.strip_prefix(".wh.") {
                        let parent_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
                        let deleted_path = rootfs_dir.join(parent_dir).join(deleted_name);
                        if deleted_path.exists() {
                            if deleted_path.is_dir() {
                                fs::remove_dir_all(&deleted_path).ok();
                            } else {
                                fs::remove_file(&deleted_path).ok();
                            }
                        }
                        continue;
                    }
                }

                let target_path = rootfs_dir.join(relative_path);

                // Directories: ensure exist; do NOT tick
                if entry.file_type().is_dir() {
                    if let Err(err) = fs::create_dir_all(&target_path) {
                        if err.kind() == std::io::ErrorKind::PermissionDenied {
                            self.notifier.warn(&format!(
                                "Permission denied creating directory {:?} - skipping",
                                target_path
                            ));
                        }
                    }
                    continue;
                }

                // Symlinks (counted)
                if entry.file_type().is_symlink() {
                    let link_target = fs::read_link(source_path)?;
                    if target_path.exists() {
                        if target_path.is_dir() && !target_path.is_symlink() {
                            fs::remove_dir_all(&target_path).ok();
                        } else {
                            fs::remove_file(&target_path).ok();
                        }
                    }
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent).ok();
                    }
                    if let Err(err) = std::os::unix::fs::symlink(&link_target, &target_path) {
                        if err.kind() == std::io::ErrorKind::PermissionDenied {
                            self.notifier.warn(&format!(
                                "Permission denied creating symlink {:?} -> {:?} - skipping",
                                target_path, link_target
                            ));
                        }
                    }
                    if let Some(ref pb2) = pb2 {
                        pb2.inc(1);
                    }
                    continue;
                }

                // Regular files (counted)
                if entry.file_type().is_file() {
                    if target_path.exists() {
                        if target_path.is_dir() && !target_path.is_symlink() {
                            fs::remove_dir_all(&target_path).ok();
                        } else {
                            fs::remove_file(&target_path).ok();
                        }
                    }
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent).ok();
                    }
                    // Copy attempt (we tick regardless to avoid a stuck bar if copy is skipped/denied)
                    let copy_res = fs::copy(source_path, &target_path);
                    if let Err(err) = copy_res {
                        if err.kind() == std::io::ErrorKind::PermissionDenied {
                            self.notifier.warn(&format!(
                                "Permission denied copying {:?} - skipping",
                                source_path
                            ));
                        }
                    }
                    if let Some(ref pb2) = pb2 {
                        pb2.inc(1);
                    }
                    continue;
                }
            }

            if let Some(pb2) = pb2 {
                pb2.finish_and_clear();
            }
            if let Some(ref pb1) = pb1 {
                pb1.inc(1);
            }
            // Clear the temp directory for the next layer
            fs::remove_dir_all(&temp_layer_path).ok();
            fs::create_dir_all(&temp_layer_path)?;

            // Track non-empty layer with digest
            // Use the current length of the digest tracker as the new position
            new_digest_tracker.add_layer(
                new_digest_tracker.layer_digests.len(),
                layer.digest.clone(),
                layer.command.clone(),
                layer.created_at.to_rfc3339(),
                false,
                layer.comment.clone(),
            );

            // Update structured metadata with current layer digests and save Image.md
            structured_metadata.update_layer_digests(&new_digest_tracker);
            let metadata_path = output_dir.join("Image.md");
            structured_metadata.save_markdown(&metadata_path)?;

            // Commit the changes for this layer
            self.notifier
                .info(&format!("Committing layer {}/{}", i + 1, layers.len()));

            repo.commit_all_changes(&format!("🟢 - {}", layer.command))?;
        }

        // Ownership fixup removed - files will maintain their permissions from extraction

        // Final commit: Add Image.md with complete metadata (basic_info + container_config + layer digests)
        self.notifier.info("Creating metadata commit...");

        // Create complete structured metadata with all information for final commit
        let complete_metadata =
            ImageMetadata::from_legacy(&metadata, &new_digest_tracker, image_name);
        let metadata_path = output_dir.join("Image.md");
        complete_metadata.save_markdown(&metadata_path)?;
        repo.commit_all_changes("🛠️ - Metadata")?;

        let msg = format!(
            "Successfully converted image '{}' to Git repository at '{}'",
            image_name,
            output_dir.display()
        );
        self.notifier.info(&msg);

        if let Some(pb1) = pb1 {
            pb1.finish_and_clear();
        }

        match &self.notifier {
            AnyNotifier::Enhanced(_) => {
                self.notifier.println_above(format!(
                    "{} {} {}",
                    SPARK,
                    style("Done in").bold(),
                    style(HumanDuration(started.elapsed())).bold()
                ));
            }
            AnyNotifier::Simple(_) => {
                // Plain text for simple mode
                println!("Done!");
            }
        }

        Ok(())
    }
}
