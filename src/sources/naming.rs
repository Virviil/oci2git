use std::path::PathBuf;

/// Converts a Docker/Nerdctl image name to a Git branch name
/// Replaces problematic characters according to Git branch naming rules
/// If no tag is specified, adds "latest" as the default tag
pub fn container_image_to_branch(image_name: &str) -> String {
    let normalized = if !image_name.contains(':') && !image_name.contains('@') {
        format!("{}:latest", image_name)
    } else {
        image_name.to_string()
    };

    normalized
        .replace(":", "#")
        .replace("/", "-")
        .replace("@", "-")
}

/// Extracts filename from a tar path and sanitizes it for Git branch naming
/// Removes file extension and sanitizes problematic characters
pub fn tar_path_to_branch(tar_path: &str) -> String {
    let path = PathBuf::from(tar_path);
    let filename = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("tar-image");

    super::sanitize_branch_name(filename)
}

/// Combines a base branch name with a digest to create the final branch name
/// Extracts short digest (first 12 characters after "sha256:")
pub fn combine_branch_with_digest(base_branch: &str, image_digest: &str) -> String {
    if let Some(short_digest) = super::extract_short_digest(image_digest) {
        format!("{}#{}", base_branch, short_digest)
    } else {
        // Fallback: use image_digest as-is if it doesn't have sha256: prefix
        format!("{}#{}", base_branch, image_digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_image_to_branch() {
        // Test Docker/Nerdctl naming with explicit tags
        assert_eq!(
            container_image_to_branch("hello-world:latest"),
            "hello-world#latest"
        );
        assert_eq!(
            container_image_to_branch("nginx/nginx:1.21"),
            "nginx-nginx#1.21"
        );
        assert_eq!(
            container_image_to_branch("registry.example.com/my-app:v1.0"),
            "registry.example.com-my-app#v1.0"
        );
        assert_eq!(
            container_image_to_branch("alpine@sha256:abc123"),
            "alpine-sha256#abc123"
        );
        assert_eq!(
            container_image_to_branch("library/ubuntu:20.04"),
            "library-ubuntu#20.04"
        );

        // Test automatic "latest" tag addition
        assert_eq!(
            container_image_to_branch("hello-world"),
            "hello-world#latest"
        );
        assert_eq!(container_image_to_branch("nginx"), "nginx#latest");
        assert_eq!(
            container_image_to_branch("library/ubuntu"),
            "library-ubuntu#latest"
        );
    }

    #[test]
    fn test_tar_path_to_branch() {
        assert_eq!(tar_path_to_branch("/path/to/my-image.tar"), "my-image");
        assert_eq!(
            tar_path_to_branch("./nginx-latest.tar.gz"),
            "nginx-latest-tar"
        );
        assert_eq!(tar_path_to_branch("ubuntu 20.04.tar"), "ubuntu-20-04");
        assert_eq!(tar_path_to_branch("my:app@v1.0.tar"), "my-app-v1-0");
        assert_eq!(tar_path_to_branch("hello world.tar"), "hello-world");
        assert_eq!(
            tar_path_to_branch("file with spaces & symbols!.tar"),
            "file-with-spaces-symbols"
        );
    }

    #[test]
    fn test_combine_branch_with_digest() {
        // Test with proper SHA256 prefix
        assert_eq!(
            combine_branch_with_digest("hello-world#latest", "sha256:1234567890abcdef"),
            "hello-world#latest#1234567890ab"
        );
        assert_eq!(
            combine_branch_with_digest("nginx#1.21", "sha256:9876543210fedcba"),
            "nginx#1.21#9876543210fe"
        );

        // Test fallback without SHA256 prefix
        assert_eq!(
            combine_branch_with_digest("my-image", "abcdef123456789"),
            "my-image#abcdef123456789"
        );
    }
}
