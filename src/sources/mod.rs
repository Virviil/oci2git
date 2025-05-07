use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

/// Source trait for getting OCI images from different container sources
pub trait Source {
    /// Returns the name of the source for identification purposes
    fn name(&self) -> &str;
    
    /// Retrieves an OCI image tarball and returns the path to it along with temp directory if created
    /// The image_name parameter can be an image reference (for registry sources)
    /// or a filesystem path (for local sources)
    /// 
    /// Returns a tuple with the path to the tarball and an optional TempDir that needs to be kept alive
    /// for the duration of the tarball use
    fn get_image_tarball(&self, image_name: &str) -> Result<(PathBuf, Option<TempDir>)>;
}

// Re-export source implementations
pub mod docker;
pub mod tar;
pub mod nerdctl;

pub use docker::DockerSource;
pub use tar::TarSource;
pub use nerdctl::NerdctlSource;