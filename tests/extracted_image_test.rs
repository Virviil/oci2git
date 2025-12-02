use oci2git::extracted_image::ExtractedImage;
use oci2git::Notifier;
use std::path::Path;

#[test]
fn test_extracted_image_eager_loading() {
    // Use the test fixture tarball
    let fixture_path = Path::new("tests/integration/fixtures/oci2git-test.tar");

    // Skip test if fixture doesn't exist
    if !fixture_path.exists() {
        eprintln!("Skipping test: fixture file not found at {fixture_path:?}");
        return;
    }

    let notifier = Notifier::new(0); // Verbosity level 0 for quiet

    // Test that from_tarball does all the work upfront
    let extracted_image = ExtractedImage::from_tarball(fixture_path, &notifier)
        .expect("Failed to extract image from tarball");

    // Test that metadata is immediately available (no lazy loading)
    let metadata = extracted_image
        .metadata("test-image")
        .expect("Failed to get metadata");

    // Verify metadata contains expected fields
    assert!(
        !metadata.architecture.is_empty(),
        "Architecture should be set"
    );
    assert!(!metadata.os.is_empty(), "OS should be set");
    assert_eq!(
        metadata.id, "sha256:6281ae58699c996183feb2c9732e340bff56a4951f1f85953c1901163931a5e7",
        "Image ID should match provided sha256 hash"
    );

    // Test that layers are immediately available (no lazy loading)
    let layers = extracted_image.layers().expect("Failed to get layers");

    // Verify we have some layers
    assert!(!layers.is_empty(), "Should have at least one layer");

    // Test that each layer has proper structure
    for (i, layer) in layers.iter().enumerate() {
        assert!(!layer.id.is_empty(), "Layer {i} should have an ID");
        assert!(!layer.command.is_empty(), "Layer {i} should have a command");
        assert!(!layer.digest.is_empty(), "Layer {i} should have a digest");

        // Verify digest and tarball path consistency
        if layer.is_empty {
            assert_eq!(
                layer.digest, "empty",
                "Empty layer should have 'empty' digest"
            );
            assert!(
                layer.tarball_path.is_none(),
                "Empty layer should not have tarball path"
            );
        } else {
            assert_ne!(
                layer.digest, "empty",
                "Non-empty layer should not have 'empty' digest"
            );
            // Non-empty layers should have either a tarball path or 'no-tarball' digest
            if layer.tarball_path.is_some() {
                assert!(
                    layer.digest.starts_with("sha256:"),
                    "Non-empty layer with tarball should have sha256 digest"
                );
            } else {
                assert_eq!(
                    layer.digest, "no-tarball",
                    "Non-empty layer without tarball should have 'no-tarball' digest"
                );
            }
        }
    }

    println!(
        "✅ Successfully validated ExtractedImage with {} layers",
        layers.len()
    );
    println!("   Architecture: {}", metadata.architecture);
    println!("   OS: {}", metadata.os);

    // Print layer summary
    let empty_layers = layers.iter().filter(|l| l.is_empty).count();
    let non_empty_layers = layers.len() - empty_layers;
    let layers_with_tarballs = layers.iter().filter(|l| l.tarball_path.is_some()).count();

    println!(
        "   Layers: {} total ({} empty, {} non-empty, {} with tarballs)",
        layers.len(),
        empty_layers,
        non_empty_layers,
        layers_with_tarballs
    );

    // Print detailed layer information to debug mapping
    println!("   Layer details:");
    for (i, layer) in layers.iter().enumerate() {
        let tarball_info = if let Some(ref path) = layer.tarball_path {
            format!(
                "tarball: {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        } else {
            "no tarball".to_string()
        };
        println!(
            "     {}: {} | {} | digest: {} | {}",
            i,
            if layer.is_empty { "EMPTY" } else { "FILES" },
            layer.command.chars().take(50).collect::<String>(),
            layer.digest.chars().take(20).collect::<String>(),
            tarball_info
        );
    }
}

#[test]
fn test_extracted_image_validation() {
    let notifier = Notifier::new(0);

    // Test with non-existent file
    let result = ExtractedImage::from_tarball("non-existent-file.tar", &notifier);
    assert!(result.is_err(), "Should fail with non-existent file");

    // Test with invalid tarball (use this source file as invalid tarball)
    let result = ExtractedImage::from_tarball(file!(), &notifier);
    assert!(result.is_err(), "Should fail with invalid tarball format");
}

#[test]
fn test_extracted_image_multiple_calls() {
    let fixture_path = Path::new("tests/integration/fixtures/oci2git-test.tar");

    if !fixture_path.exists() {
        eprintln!("Skipping test: fixture file not found at {fixture_path:?}");
        return;
    }

    let notifier = Notifier::new(0);
    let extracted_image = ExtractedImage::from_tarball(fixture_path, &notifier)
        .expect("Failed to extract image from tarball");

    // Test that multiple calls return consistent data (no lazy loading side effects)
    let metadata1 = extracted_image.metadata("test1").unwrap();
    let metadata2 = extracted_image.metadata("test2").unwrap();
    let layers1 = extracted_image.layers().unwrap();
    let layers2 = extracted_image.layers().unwrap();

    // Metadata should be consistent (except for overridden image name)
    assert_eq!(metadata1.architecture, metadata2.architecture);
    assert_eq!(metadata1.os, metadata2.os);
    assert_eq!(
        metadata1.id,
        "sha256:6281ae58699c996183feb2c9732e340bff56a4951f1f85953c1901163931a5e7"
    );
    assert_eq!(
        metadata2.id,
        "sha256:6281ae58699c996183feb2c9732e340bff56a4951f1f85953c1901163931a5e7"
    );

    // Layers should be identical
    assert_eq!(layers1.len(), layers2.len());
    for (l1, l2) in layers1.iter().zip(layers2.iter()) {
        assert_eq!(l1.id, l2.id);
        assert_eq!(l1.command, l2.command);
        assert_eq!(l1.digest, l2.digest);
        assert_eq!(l1.is_empty, l2.is_empty);
        assert_eq!(l1.tarball_path, l2.tarball_path);
    }
}
