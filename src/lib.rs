pub mod digest_tracker;
pub mod extracted_image;
pub mod git;
pub mod image_metadata;
pub mod metadata;
pub mod notifier;
pub mod processor;
pub mod sources;
pub mod successor_navigator;

// Re-exports for easy access
pub use extracted_image::{ExtractedImage, Layer};
pub use git::GitRepo;
pub use notifier::Notifier;
pub use processor::ImageProcessor;
pub use sources::DockerSource;
pub use sources::NerdctlSource;
pub use sources::Source;
pub use sources::TarSource;
