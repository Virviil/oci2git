//! This crate is using for converts container OCI/Docker images to Git repositories.
//! The whole image unpacked into Git repo and each container layer is represented as a Git commit,
//! preserving the history and structure of the original image.
//!
//! This lets you use the power of Git to:
//! - Analyze image layer structures to find redundant operations or large files that could be consolidated, helping to reduce image size.
//! - Track when dependencies were added, upgraded, or removed across the image history.
//! - Inspect layer composition to optimize Dockerfile instructions for better caching and smaller image size.
//! - Easily compare related images by converting multiple images and using Git’s diff tools
//!   to see similarities and differences.
//!
//! # Usage
//!
//! `oci2git [OPTIONS] <IMAGE>`
//!
//! Arguments:
//! - `<IMAGE>` Image name to convert (e.g., 'ubuntu:latest') or path to tarball when using the tar engine
//! - Options:
//!     - `-o` `--output` `<o>`  Output directory for Git repository `[default: ./container_repo]`
//!     - `-e` `--engine` `<ENGINE>`  Container engine to use (docker, nerdctl, tar) `[default: docker]`
//!     - `-h` `--help`  Print help information
//!     - `-V` `--version` Print version information
//!
//! - Environment Variables:
//!     - `TMPDIR`  Set this environment variable to change the default location used for intermediate data processing. This is platform-dependent (e.g., TMPDIR on Unix/macOS, TEMP or TMP on Windows).
//!
//! # Example
//!
//! ```oci2git ubuntu:latest```
//!
//! This will create a Git repository in `./container_repo` folder containing:
//!
//! - Image.md - Complete metadata about the image in Markdown format
//! - rootfs/ - The filesystem content from the container
//! - The Git history reflects the container's layer history:
//!
//! The first commit contains only the Image.md file with full metadata
//! Each subsequent commit represents a layer from the original image
//! Commits include the Dockerfile command as the commit message
//!
//! Repository Structure:
//! ```text
//! container_repo/
//! ├── .git/
//! ├── Image.md     # Complete image metadata
//! └── rootfs/      # Filesystem content from the container
//! ```

pub mod digest_tracker;
pub mod extracted_image;
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
pub use git::GitRepo;
pub use notifier::Notifier;
pub use processor::ImageProcessor;
pub use sources::DockerSource;
pub use sources::NerdctlSource;
pub use sources::Source;
pub use sources::TarSource;
