use crate::Differ;
use crate::differ::{Change, DiffAlgorithm};

use super::{create_patch, handle_empty_files, process_changes_to_chunks};

/// The Myers differ implementation that uses Myers algorithm for diffing
///
/// This implementation uses the Longest Common Subsequence (LCS) approach,
/// which is a dynamic programming solution that provides the foundation of Myers' algorithm.
/// While the full Myers O(ND) optimization uses a greedy approach with diagonal paths,
/// this implementation prioritizes correctness and readability based on LCS.
pub struct MyersDiffer<'a> {
    differ: &'a Differ,
}

impl<'a> MyersDiffer<'a> {
    /// Create a new MyersDiffer from a base Differ instance
    pub fn new(differ: &'a Differ) -> Self {
        Self { differ }
    }

    /// Implements a diffing algorithm based on Myers' principles (using LCS)
    /// Finds the shortest edit script (SES) between old_lines and new_lines
    fn myers_diff(&self, old_lines: &[&str], new_lines: &[&str]) -> Vec<Change> {
        // Special cases for empty inputs
        if old_lines.is_empty() && new_lines.is_empty() {
            return Vec::new();
        }
        if old_lines.is_empty() {
            // All new lines are insertions
            return vec![Change::Insert(0, new_lines.len())];
        }
        if new_lines.is_empty() {
            // All old lines are deletions
            return vec![Change::Delete(0, old_lines.len())];
        }
        // If files are identical, return no changes
        if old_lines == new_lines {
            return Vec::new();
        }

        // Use Longest Common Subsequence (LCS) table to find the differences.
        // This is equivalent to finding the shortest edit path in Myers' algorithm,
        // although the O(ND) version avoids constructing the full table explicitly.
        let n = old_lines.len();
        let m = new_lines.len();
        // `lcs[i][j]` stores the length of the LCS between old_lines[0..i] and new_lines[0..j]
        let mut lcs = vec![vec![0; m + 1]; n + 1];
        for i in 1..=n {
            for j in 1..=m {
                if old_lines[i - 1] == new_lines[j - 1] {
                    lcs[i][j] = lcs[i - 1][j - 1] + 1; // Match: extend LCS diagonally
                } else {
                    // No match: take max LCS from deletion (up) or insertion (left)
                    lcs[i][j] = std::cmp::max(lcs[i - 1][j], lcs[i][j - 1]);
                }
            }
        }

        // Backtrack through the LCS table to reconstruct the edit script (Changes)
        let mut changes = Vec::new();
        let mut i = n;
        let mut j = m;
        while i > 0 || j > 0 {
            if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
                // Match found: move diagonally up-left
                changes.push(Change::Equal(i - 1, j - 1));
                i -= 1;
                j -= 1;
            } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
                // Insertion preferred (or only choice): move left
                changes.push(Change::Insert(j - 1, 1));
                j -= 1;
            } else if i > 0 {
                // Deletion preferred (or only choice): move up
                changes.push(Change::Delete(i - 1, 1));
                i -= 1;
            } else {
                // Should be unreachable if LCS table and backtracking are correct
                break;
            }
        }

        // Changes were collected in reverse order during backtrack
        changes.reverse();

        // Merging adjacent operations is not needed here, as process_changes_to_chunks
        // expects individual changes (including single Change::Equal). The old logic
        // for merging Delete/Insert/Equal has been removed.
        changes
    }
}

impl DiffAlgorithm for MyersDiffer<'_> {
    /// Generate a patch between the old and new content using the Myers diffing algorithm (LCS based)
    fn generate(&self) -> crate::Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();
        // Handle special cases for empty files
        if let Some(patch) = handle_empty_files(&old_lines, &new_lines) {
            return patch;
        }
        // Find the line-level changes using Myers/LCS
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
    use crate::{PatchAlgorithm, Patcher, test_utils::load_fixture};

    #[test]
    fn test_simple_myers_diff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3";
        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();
        assert!(
            patch.chunks.is_empty(),
            "Patch should be empty for identical files"
        );
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

    #[test]
    fn test_myers_complex_diff() {
        let old = "A\nB\nC\nA\nB\nB\nA";
        let new = "C\nB\nA\nB\nA\nC";
        let differ = Differ::new(old, new);
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    // New tests using fixtures
    #[test]
    fn test_myers_fixture_simple() {
        let old = load_fixture("simple_before.rs");
        let new = load_fixture("simple_after.rs");
        let differ = Differ::new(&old, &new); // Myers is default
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();
        let result = Patcher::new(patch).apply(&old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_myers_fixture_complex() {
        let old = load_fixture("complex_before.rs");
        let new = load_fixture("complex_after.rs");
        let differ = Differ::new(&old, &new); // Myers is default
        let myers = MyersDiffer::new(&differ);
        let patch = myers.generate();
        let result = Patcher::new(patch).apply(&old, false).unwrap();
        assert_eq!(result, new);
    }
}
