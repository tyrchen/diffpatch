use crate::differ::{Change, DiffAlgorithm};
use crate::{Differ, Patch};
use std::cmp::min;

use super::{create_patch, handle_empty_files, process_changes_to_chunks};

/// The Naive differ implementation
pub struct NaiveDiffer<'a> {
    differ: &'a Differ,
}

impl<'a> NaiveDiffer<'a> {
    /// Create a new NaiveDiffer from a base Differ instance
    pub fn new(differ: &'a Differ) -> Self {
        Self { differ }
    }

    /// Find the next match looking ahead a certain number of lines
    fn find_next_match(
        &self,
        old_lines: &[&str],
        new_lines: &[&str],
        max_look_ahead: usize,
    ) -> (usize, usize) {
        let max_old_look_ahead = min(old_lines.len(), max_look_ahead);
        let max_new_look_ahead = min(new_lines.len(), max_look_ahead);
        // Simple implementation: just look for the first line that matches
        for (i, old_line) in old_lines.iter().enumerate().take(max_old_look_ahead) {
            for (j, new_line) in new_lines.iter().enumerate().take(max_new_look_ahead) {
                if old_line == new_line {
                    return (i, j);
                }
            }
        }
        // No match found within the lookahead window
        (0, 0)
    }

    /// Find line-level changes between old and new content using a simple heuristic
    fn find_line_changes(&self, old_lines: &[&str], new_lines: &[&str]) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut i = 0; // current index for old_lines
        let mut j = 0; // current index for new_lines

        while i < old_lines.len() || j < new_lines.len() {
            if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
                // Equal lines
                changes.push(Change::Equal(i, j));
                i += 1;
                j += 1;
            } else {
                // Lines differ, look ahead for a match
                // Note: A small lookahead (e.g., 10) keeps it somewhat naive
                let (skip_old, skip_new) =
                    self.find_next_match(&old_lines[i..], &new_lines[j..], 10);

                if skip_old > 0 {
                    // If a match was found skipping some old lines, mark them as deleted
                    changes.push(Change::Delete(i, skip_old));
                    i += skip_old;
                }
                if skip_new > 0 {
                    // If a match was found skipping some new lines, mark them as inserted
                    changes.push(Change::Insert(j, skip_new));
                    j += skip_new;
                }

                // If no match was found in the lookahead (skip_old == 0 && skip_new == 0)
                // or if only one side skipped lines to find a match (which shouldn't happen
                // with the current find_next_match returning (0,0) or (i,j) for the first match),
                // we handle the non-matching lines.
                if skip_old == 0 && skip_new == 0 {
                    // No match found nearby, assume the current lines are different
                    // Mark the current old line as delete (if exists)
                    if i < old_lines.len() {
                        changes.push(Change::Delete(i, 1));
                        i += 1;
                    }
                    // Mark the current new line as insert (if exists)
                    if j < new_lines.len() {
                        changes.push(Change::Insert(j, 1));
                        j += 1;
                    }
                }
            }
        }
        changes
    }
}

impl DiffAlgorithm for NaiveDiffer<'_> {
    /// Generate a patch between the old and new content using the naive diffing algorithm
    fn generate(&self) -> Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();
        // Handle special cases for empty files
        if let Some(patch) = handle_empty_files(&old_lines, &new_lines) {
            return patch;
        }
        // Find the line-level changes
        let changes = self.find_line_changes(&old_lines, &new_lines);
        // Process the changes into chunks with context
        let chunks =
            process_changes_to_chunks(&changes, &old_lines, &new_lines, self.differ.context_lines);
        // Create the final patch
        create_patch(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PatchAlgorithm, Patcher, differ::DiffAlgorithmType, test_utils::load_fixture};

    #[test]
    fn test_simple_diff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        assert!(
            patch.chunks.is_empty(),
            "Patch should be empty for identical files"
        );
    }

    #[test]
    fn test_add_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3\nline4";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_remove_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline3";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_empty_files() {
        // Empty to non-empty
        let old = "";
        let new = "line1\nline2";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch.clone()).apply(old, false).unwrap();
        assert_eq!(result, new);

        // Non-empty to empty
        let old = "line1\nline2";
        let new = "";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    // New tests using fixtures
    #[test]
    fn test_naive_fixture_simple() {
        let old = load_fixture("simple_before.rs");
        let new = load_fixture("simple_after.rs");
        let differ = Differ::new_with_algorithm(&old, &new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        let result = Patcher::new(patch).apply(&old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_naive_fixture_python() {
        let old = load_fixture("old.py");
        let new = load_fixture("new.py");
        let differ = Differ::new_with_algorithm(&old, &new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        let result = Patcher::new(patch).apply(&old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_naive_fixture_complex() {
        let old = load_fixture("complex_before.rs");
        let new = load_fixture("complex_after.rs");
        let differ = Differ::new_with_algorithm(&old, &new, DiffAlgorithmType::Naive);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();
        let result = Patcher::new(patch).apply(&old, false).unwrap();
        assert_eq!(result, new);
    }
}
