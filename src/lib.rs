pub mod git;
pub mod metadata;
pub mod processor;
pub mod sources;

// Re-exports for easy access
pub use git::GitRepo;
pub use processor::ImageProcessor;
pub use processor::Layer;
pub use sources::DockerSource;
pub use sources::NerdctlSource;
pub use sources::Source;
pub use sources::TarSource;
