use anyhow::Result;
use std::path::PathBuf;

/// Source trait for getting OCI images from different container sources
pub trait Source {
    /// Returns the name of the source for identification purposes
    fn name(&self) -> &str;
    
    /// Retrieves an OCI image tarball and returns the path to it
    /// The image_name parameter can be an image reference (for registry sources)
    /// or a filesystem path (for local sources)
    fn get_image_tarball(&self, image_name: &str) -> Result<PathBuf>;
}

// Re-export source implementations
pub mod docker;
pub mod tar;
pub mod nerdctl;

pub use docker::DockerSource;
pub use tar::TarSource;
pub use nerdctl::NerdctlSource;