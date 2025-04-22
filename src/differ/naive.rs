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

        // No match found
        (0, 0)
    }

    /// Find line-level changes between old and new content
    fn find_line_changes(&self, old_lines: &[&str], new_lines: &[&str]) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut i = 0;
        let mut j = 0;

        // Find the line-level changes using a simple algorithm
        while i < old_lines.len() || j < new_lines.len() {
            if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
                // Equal lines
                changes.push(Change::Equal(i, j));
                i += 1;
                j += 1;
            } else {
                // Find the best match looking ahead
                let matching_lines = self.find_next_match(&old_lines[i..], &new_lines[j..], 10);

                if matching_lines.0 > 0 {
                    // There are deleted lines
                    changes.push(Change::Delete(i, matching_lines.0));
                    i += matching_lines.0;
                }

                if matching_lines.1 > 0 {
                    // There are inserted lines
                    changes.push(Change::Insert(j, matching_lines.1));
                    j += matching_lines.1;
                }

                if matching_lines.0 == 0 && matching_lines.1 == 0 {
                    // No match found, just advance both sequences
                    if i < old_lines.len() {
                        changes.push(Change::Delete(i, 1));
                        i += 1;
                    }
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
    use crate::Patcher;

    #[test]
    fn test_simple_diff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3";

        let differ = Differ::new(old, new);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();

        assert_eq!(patch.chunks.len(), 0); // No changes, so no chunks
    }

    #[test]
    fn test_add_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3\nline4";

        let differ = Differ::new(old, new);
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

        let differ = Differ::new(old, new);
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

        let differ = Differ::new(old, new);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch.clone()).apply(old, false).unwrap();
        assert_eq!(result, new);

        // Non-empty to empty
        let old = "line1\nline2";
        let new = "";

        let differ = Differ::new(old, new);
        let naive = NaiveDiffer::new(&differ);
        let patch = naive.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }
}
