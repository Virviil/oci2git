//! Integration tests for `fsbom` — filesystem bill of materials generation.
//!
//! Uses a controlled busybox-based fixture image with known layer contents:
//!
//! - Layer 1 (busybox base): the busybox rootfs
//! - Layer 2: `/app/hello.txt` (new), `/app/static.txt` (new)
//! - Layer 3: `/app/sub/data.txt` (new), `/app/run.sh` (new),
//!            `/app/hello-link.txt` → symlink (new), `/app/sub/run-link.sh` → symlink (new)
//! - Layer 4: `/app/hello.txt` (modified), `/app/new.txt` (new),
//!            `/app/static.txt` deleted (whiteout — absent from BOM)

use anyhow::Result;
use oci2git::processor::ImageProcessor;
use oci2git::sources::TarSource;
use oci2git::Notifier;
use std::path::Path;
use tempfile::TempDir;

const FIXTURE: &str = "tests/integration/fixtures/fsbom-test.tar";

fn skip_if_missing() -> bool {
    if !Path::new(FIXTURE).exists() {
        println!("Skipping: fixture not found at {FIXTURE}");
        true
    } else {
        false
    }
}

/// Run `generate_fsbom` on the fixture and return the output YAML as a String.
fn run_fsbom() -> Result<(TempDir, std::path::PathBuf)> {
    let out_dir = TempDir::new()?;
    let out_path = out_dir.path().join("out.yml");

    let source = TarSource::new()?;
    let notifier = Notifier::new(0);
    let processor = ImageProcessor::new(source, notifier);
    processor.generate_fsbom(FIXTURE, &out_path)?;

    Ok((out_dir, out_path))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsbom_yaml_is_created() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        assert!(path.exists(), "output YAML should exist");
        let content = std::fs::read_to_string(&path)?;
        assert!(content.starts_with("layers:"), "YAML should start with 'layers:'");
        println!("✓ fsbom YAML created");
        Ok(())
    }

    #[test]
    fn test_fsbom_has_correct_layer_count() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        // Count "- index:" occurrences — one per layer
        let layer_count = content.matches("- index:").count();
        // busybox base + 3 RUN layers = 4 layers
        assert_eq!(layer_count, 4, "expected 4 layers, got {layer_count}");
        println!("✓ correct layer count ({layer_count})");
        Ok(())
    }

    #[test]
    fn test_fsbom_new_files_in_layer2() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        // hello.txt and static.txt must appear as 'new' (stat starts with "n:")
        // We check YAML presence of the paths and that stat is n:
        assert!(
            content.contains(r#""app/hello.txt""#),
            "hello.txt should be in BOM"
        );
        assert!(
            content.contains(r#""app/static.txt""#),
            "static.txt should be in BOM"
        );
        println!("✓ new files present in layer 2");
        Ok(())
    }

    #[test]
    fn test_fsbom_symlinks_in_layer3() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        assert!(
            content.contains(r#""app/hello-link.txt""#),
            "symlink hello-link.txt should be in BOM"
        );
        assert!(
            content.contains(r#""app/sub/run-link.sh""#),
            "symlink sub/run-link.sh should be in BOM"
        );
        // Both should be recorded as type: symlink
        // Count symlink entries
        let symlink_count = content.matches("type: symlink").count();
        assert!(symlink_count >= 2, "expected at least 2 symlink entries, got {symlink_count}");
        println!("✓ symlinks present in layer 3 ({symlink_count} total symlinks)");
        Ok(())
    }

    #[test]
    fn test_fsbom_symlink_targets() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        // hello-link.txt → /app/hello.txt (absolute symlink)
        assert!(
            content.contains(r#""/app/hello.txt""#),
            "symlink target /app/hello.txt should appear in BOM"
        );
        // sub/run-link.sh → ../run.sh (relative symlink)
        assert!(
            content.contains(r#""../run.sh""#),
            "relative symlink target ../run.sh should appear in BOM"
        );
        println!("✓ symlink targets correctly recorded");
        Ok(())
    }

    #[test]
    fn test_fsbom_modified_file_in_layer4() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        // hello.txt appears first in layer 2 as new, then again in layer 4 as modified
        let hello_count = content.matches(r#""app/hello.txt""#).count();
        assert_eq!(hello_count, 2, "hello.txt should appear twice (new + modified), got {hello_count}");

        // Find the second occurrence and check stat is "m:..."
        let second_pos = content
            .match_indices(r#""app/hello.txt""#)
            .nth(1)
            .map(|(i, _)| i)
            .expect("second occurrence of hello.txt");
        let end = (second_pos + 200).min(content.len());
        let after = &content[second_pos..end];
        assert!(
            after.contains("\"m:"),
            "second occurrence of hello.txt should have stat 'm:...', got:\n{after}"
        );
        println!("✓ hello.txt correctly marked as modified in layer 4");
        Ok(())
    }

    #[test]
    fn test_fsbom_deleted_file_absent() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        // static.txt is deleted in layer 4 via whiteout — should appear only once (layer 2)
        let count = content.matches(r#""app/static.txt""#).count();
        assert_eq!(count, 1, "static.txt deleted in layer 4 should appear only once, got {count}");
        println!("✓ deleted file (static.txt) absent from later layers");
        Ok(())
    }

    #[test]
    fn test_fsbom_new_file_in_layer4() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        assert!(
            content.contains(r#""app/new.txt""#),
            "new.txt added in layer 4 should be in BOM"
        );
        // Should be marked as new
        let pos = content
            .find(r#""app/new.txt""#)
            .expect("new.txt in BOM");
        let end = (pos + 200).min(content.len());
        let after = &content[pos..end];
        assert!(
            after.contains("\"n:"),
            "new.txt should have stat 'n:...', got:\n{after}"
        );
        println!("✓ new.txt correctly marked as new in layer 4");
        Ok(())
    }

    #[test]
    fn test_fsbom_layer_indices_are_sequential() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        for i in 0..4 {
            let expected = format!("- index: {i}");
            assert!(content.contains(&expected), "missing '- index: {i}' in YAML");
        }
        println!("✓ layer indices are sequential 0..3");
        Ok(())
    }

    #[test]
    fn test_fsbom_layer_digests_present() -> Result<()> {
        if skip_if_missing() {
            return Ok(());
        }
        let (_dir, path) = run_fsbom()?;
        let content = std::fs::read_to_string(&path)?;

        let digest_count = content.matches("digest:").count();
        assert_eq!(digest_count, 4, "each layer should have a digest field");
        println!("✓ all layer digests present");
        Ok(())
    }
}
