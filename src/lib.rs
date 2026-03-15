//! Converts container OCI/Docker images to Git repositories and generates filesystem
//! bills of materials (fsbom). Each container layer is represented as a Git commit,
//! preserving the history and structure of the original image.
//!
//! This lets you use the power of Git to:
//! - Analyze image layer structures to find redundant operations or large files that could be consolidated, helping to reduce image size.
//! - Track when dependencies were added, upgraded, or removed across the image history.
//! - Inspect layer composition to optimize Dockerfile instructions for better caching and smaller image size.
//! - Easily compare related images by converting multiple images and using Git’s diff tools
//!   to see similarities and differences.
//!
//! # Commands
//!
//! ## `convert` — OCI image → Git repository
//!
//! ```text
//! oci2git convert [OPTIONS] <IMAGE>
//! oci2git <IMAGE>
//! ```
//!
//! Arguments:
//! - `<IMAGE>` Image name to convert (e.g., `ubuntu:latest`) or path to tarball when using the tar engine
//!
//! Options:
//! - `-o` `--output` `<OUTPUT>`  Output directory for Git repository `[default: ./container_repo]`
//! - `-e` `--engine` `<ENGINE>`  Container engine to use (docker, nerdctl, tar) `[default: docker]`
//! - `-v` `--verbose`            Verbose mode
//!
//! ## `fsbom` — Filesystem bill of materials
//!
//! ```text
//! oci2git fsbom [OPTIONS] <IMAGE>
//! ```
//!
//! Arguments:
//! - `<IMAGE>` Image name or path to tarball when using the tar engine
//!
//! Options:
//! - `-o` `--output` `<OUTPUT>`  Output path for the YAML BOM file `[default: ./fsbom.yml]`
//! - `-e` `--engine` `<ENGINE>`  Container engine to use (docker, nerdctl, tar) `[default: docker]`
//! - `-v` `--verbose`            Verbose mode
//!
//! ## Environment Variables
//!
//! - `TMPDIR`  Override the directory used for intermediate data processing
//!   (platform-dependent: `TMPDIR` on Unix/macOS, `TEMP` or `TMP` on Windows).
//!
//! # Examples
//!
//! Convert an image to a Git repository:
//! ```text
//! oci2git ubuntu:latest
//! ```
//!
//! Generate a filesystem bill of materials:
//! ```text
//! oci2git fsbom ubuntu:latest -o ubuntu.yml
//! ```
//!
//! The `convert` command produces a Git repository in `./container_repo` containing:
//! - `Image.md` — Complete metadata about the image
//! - `rootfs/` — The filesystem content from the container
//!
//! The `fsbom` command produces a YAML file with per-layer entries:
//! - `type: file | hardlink | symlink | directory`
//! - `stat: "n:uid:gid"` for new entries, `"m:uid:gid"` for modified
//! - Deleted files (OCI whiteouts) are excluded
//!
//! ```text
//! container_repo/
//! ├── .git/
//! ├── Image.md     # Complete image metadata
//! └── rootfs/      # Filesystem content from the container
//! ```

pub mod digest_tracker;
pub mod extracted_image;
pub mod fsbom;
pub mod git;
pub mod image_metadata;
pub mod metadata;
pub mod notifier;
pub mod processor;
pub mod sources;
pub mod successor_navigator;
pub mod tar_extractor;

// Re-exports for easy access
pub use extracted_image::{ExtractedImage, Layer};
pub use fsbom::{FsBom, LayerBom};
pub use git::GitRepo;
pub use notifier::Notifier;
pub use processor::ImageProcessor;
pub use sources::DockerSource;
pub use sources::NerdctlSource;
pub use sources::Source;
pub use sources::TarSource;
