//! Common utilities for integration tests

pub mod tar_processing;

/// Common test images used across integration tests
#[allow(dead_code)]
pub const TEST_IMAGES: &[&str] = &["hello-world:latest", "alpine:latest"];

#[allow(dead_code)]
/// Test image that definitely doesn't exist
pub const NONEXISTENT_IMAGE: &str = "this-image-definitely-does-not-exist:never";
