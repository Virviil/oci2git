pub mod docker;
pub mod nerdctl;
pub mod tar;

// Naming utilities for branch name generation
pub mod naming;

// Source trait
mod source;
pub use source::Source;

pub use docker::DockerSource;
pub use nerdctl::NerdctlSource;
pub use tar::TarSource;

/// Sanitizes a string to be safe for Git branch naming
/// Removes/replaces characters that are problematic in Git branch names
pub fn sanitize_branch_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            // Replace whitespace with hyphens
            ' ' | '\t' | '\n' | '\r' => '-',
            // Replace problematic characters with hyphens
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            // Replace dots with hyphens (except when used in version numbers)
            '.' => '-',
            // Keep alphanumeric, hyphens, underscores, and hash
            c if c.is_alphanumeric() || c == '-' || c == '_' || c == '#' => c,
            // Replace everything else with hyphen
            _ => '-',
        })
        .collect::<String>()
        // Remove consecutive hyphens
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        // Ensure it doesn't start or end with hyphen
        .trim_matches('-')
        .to_string()
}

/// Extracts a short digest from an image ID
/// Takes the first 12 characters after "sha256:"
pub fn extract_short_digest(image_id: &str) -> Option<String> {
    image_id
        .strip_prefix("sha256:")
        .map(|digest| digest.chars().take(12).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_short_digest() {
        assert_eq!(
            extract_short_digest("sha256:1234567890abcdef1234567890abcdef12345678"),
            Some("1234567890ab".to_string())
        );
        assert_eq!(
            extract_short_digest("sha256:abcdef123456"),
            Some("abcdef123456".to_string())
        );
        assert_eq!(extract_short_digest("invalid-id"), None);
        assert_eq!(extract_short_digest(""), None);
    }

    #[test]
    fn test_sanitize_branch_name() {
        assert_eq!(sanitize_branch_name("hello-world"), "hello-world");
        assert_eq!(sanitize_branch_name("hello world"), "hello-world");
        assert_eq!(sanitize_branch_name("my:app/v1.0"), "my-app-v1-0");
        assert_eq!(
            sanitize_branch_name("file with spaces & symbols!"),
            "file-with-spaces-symbols"
        );
        assert_eq!(sanitize_branch_name("---test---"), "test");
        assert_eq!(sanitize_branch_name("a..b..c"), "a-b-c");
        assert_eq!(
            sanitize_branch_name("nginx_1.21-alpine"),
            "nginx_1-21-alpine"
        );
    }
}
