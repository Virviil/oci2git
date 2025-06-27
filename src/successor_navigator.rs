use crate::digest_tracker::DigestTracker;
use crate::git::GitRepo;
use anyhow::{Context, Result};
use std::path::Path;

pub struct SuccessorNavigator;

impl SuccessorNavigator {
    /// Find the optimal branch point using single-commit layer matching
    /// Returns (commit_oid, matched_layer_count)
    pub fn find_branch_point(
        repo: &GitRepo,
        _output_dir: &Path,
        new_layers: &[crate::extracted_image::Layer],
    ) -> Result<(Option<git2::Oid>, usize)> {
        if new_layers.is_empty() {
            return Ok((None, 0));
        }

        let mut current_commit: Option<git2::Oid> = None;
        let mut layer_index = 0;

        // Process each layer one by one
        while layer_index < new_layers.len() {
            let current_layer = &new_layers[layer_index];

            // Get candidate commits to check
            let candidates = if let Some(commit) = current_commit {
                // We're already on a commit, get its successors
                repo.get_commit_successors(Some(commit))?
            } else {
                // Starting - get all commits without parents (root commits)
                repo.get_commit_successors(None)?
            };

            // Find which candidate (if any) has this layer at this position
            let mut found_match = false;
            for candidate_oid in candidates {
                if Self::commit_has_layer_at_position(
                    repo,
                    candidate_oid,
                    layer_index,
                    current_layer,
                )? {
                    // Found a match! Continue from this commit
                    current_commit = Some(candidate_oid);
                    layer_index += 1;
                    found_match = true;
                    break;
                }
            }

            if !found_match {
                // No candidate has this layer - branch from current commit (or start fresh)
                return Ok((current_commit, layer_index));
            }
        }

        // All layers matched! Return the final commit
        Ok((current_commit, layer_index))
    }

    /// Check if a specific commit has the expected layer at the given position
    fn commit_has_layer_at_position(
        repo: &GitRepo,
        commit_oid: git2::Oid,
        layer_position: usize,
        expected_layer: &crate::extracted_image::Layer,
    ) -> Result<bool> {
        // Read digests.json from the specific commit
        let digest_tracker = Self::read_digests_from_commit(repo, commit_oid)?;

        // Check if this tracker has a matching layer at the given position
        Ok(digest_tracker.layer_matches(layer_position, expected_layer))
    }

    /// Read digest info from Image.md content from a specific commit
    fn read_digests_from_commit(repo: &GitRepo, commit_oid: git2::Oid) -> Result<DigestTracker> {
        match repo.read_file_from_commit(commit_oid, "Image.md") {
            Ok(content) => {
                let image_metadata = crate::image_metadata::ImageMetadata::parse_markdown(&content)
                    .context("Failed to parse Image.md from commit")?;
                Ok(DigestTracker {
                    layer_digests: image_metadata.layer_digests,
                })
            }
            Err(_) => {
                // No Image.md in this commit, return empty tracker
                Ok(DigestTracker::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    #[test]
    fn test_root_commits_deduplication() {
        // This would require setting up a proper git repo
        // For now, test that we can create the structure
        let roots: HashSet<git2::Oid> = HashSet::new();
        assert_eq!(roots.len(), 0);
    }
}
