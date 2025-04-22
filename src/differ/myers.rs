use crate::differ::{Change, DiffAlgorithm};
use crate::Differ;
use std::cmp::min;

use super::{create_patch, handle_empty_files, process_changes_to_chunks};

/// The Myers differ implementation that uses Myers algorithm for diffing
pub struct MyersDiffer<'a> {
    differ: &'a Differ,
}

impl<'a> MyersDiffer<'a> {
    /// Create a new MyersDiffer from a base Differ instance
    pub fn new(differ: &'a Differ) -> Self {
        Self { differ }
    }

    fn myers_diff(&self, old_lines: &[&str], new_lines: &[&str]) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut i = 0;
        let mut j = 0;

        // For each line, decide whether it's equal, insert, or delete
        while i < old_lines.len() || j < new_lines.len() {
            if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
                // Equal lines
                changes.push(Change::Equal(i, j));
                i += 1;
                j += 1;
            } else {
                // Find the next matching lines using a simple approach
                // This is a simpler approach than the full Myers diff
                // But helps produce patches more compatible with the naive approach
                let mut found_match = false;

                // First look for line in new that matches current old
                if i < old_lines.len() {
                    let old_line = old_lines[i];
                    for look_ahead in 0..min(5, new_lines.len() - j) {
                        if new_lines[j + look_ahead] == old_line {
                            // We found the line - mark everything before as inserted
                            if look_ahead > 0 {
                                changes.push(Change::Insert(j, look_ahead));
                                j += look_ahead;
                            }
                            changes.push(Change::Equal(i, j));
                            i += 1;
                            j += 1;
                            found_match = true;
                            break;
                        }
                    }
                }

                // If we didn't find a match, look for line in old that matches current new
                if !found_match && j < new_lines.len() {
                    let new_line = new_lines[j];
                    for look_ahead in 0..min(5, old_lines.len() - i) {
                        if old_lines[i + look_ahead] == new_line {
                            // We found the line - mark everything before as deleted
                            if look_ahead > 0 {
                                changes.push(Change::Delete(i, look_ahead));
                                i += look_ahead;
                            }
                            changes.push(Change::Equal(i, j));
                            i += 1;
                            j += 1;
                            found_match = true;
                            break;
                        }
                    }
                }

                // If still no match, delete/insert current line and continue
                if !found_match {
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

impl DiffAlgorithm for MyersDiffer<'_> {
    /// Generate a patch between the old and new content using the Myers diffing algorithm
    fn generate(&self) -> crate::Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();

        // Handle special cases for empty files
        if let Some(patch) = handle_empty_files(&old_lines, &new_lines) {
            return patch;
        }

        // Find the line-level changes using our simplified algorithm
        let changes = self.myers_diff(&old_lines, &new_lines);

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
    fn test_simple_myers_diff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3";

        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();

        assert_eq!(patch.chunks.len(), 0); // No changes, so no chunks
    }

    #[test]
    fn test_myers_add_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3\nline4";

        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_myers_remove_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline3";

        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_myers_empty_files() {
        // Empty to non-empty
        let old = "";
        let new = "line1\nline2";

        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch.clone()).apply(old, false).unwrap();
        assert_eq!(result, new);

        // Non-empty to empty
        let old = "line1\nline2";
        let new = "";

        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_myers_multiple_changes() {
        let old = "line1\nline2\nline3\nline4\nline5";
        let new = "line1\nmodified line\nline3\nline4\nnew line";

        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();

        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }
}
