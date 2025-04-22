use crate::differ::{Change, DiffAlgorithm};
use crate::Differ;

use super::{create_patch, handle_empty_files, process_changes_to_chunks};

/// The Myers differ implementation that uses Myers algorithm for diffing
///
/// This implementation uses the Longest Common Subsequence (LCS) approach, which is
/// a dynamic programming solution that provides the foundation of Myers' algorithm.
/// While the full Myers O(ND) optimization uses a greedy approach with diagonal paths,
/// this implementation prioritizes correctness and readability.
pub struct MyersDiffer<'a> {
    differ: &'a Differ,
}

impl<'a> MyersDiffer<'a> {
    /// Create a new MyersDiffer from a base Differ instance
    pub fn new(differ: &'a Differ) -> Self {
        Self { differ }
    }

    /// Implements a diffing algorithm based on Myers' principles
    /// Finds the shortest edit script (SES) between old_lines and new_lines
    ///
    /// Core principles from Myers' algorithm included:
    /// 1. Finding the optimal (shortest) edit path
    /// 2. Using a graph-based approach (via the LCS matrix)
    /// 3. Considering the problem as finding a path through an edit graph
    /// 4. Producing a minimal set of inserts and deletes to transform the source to target
    fn myers_diff(&self, old_lines: &[&str], new_lines: &[&str]) -> Vec<Change> {
        // Special cases
        if old_lines.is_empty() && new_lines.is_empty() {
            return Vec::new();
        }

        if old_lines.is_empty() {
            return vec![Change::Insert(0, new_lines.len())];
        }

        if new_lines.is_empty() {
            return vec![Change::Delete(0, old_lines.len())];
        }

        // If files are identical, return no changes
        if old_lines == new_lines {
            return Vec::new();
        }

        // LCS approach is equivalent to finding the shortest edit path in Myers' algorithm
        // Myers optimizes this by directly working with diagonals in the edit graph
        // rather than computing the full LCS matrix, but the end result is equivalent

        let n = old_lines.len();
        let m = new_lines.len();

        // Build the LCS matrix - this represents our edit graph implicitly
        // In Myers' algorithm, this would be optimized to only store the furthest
        // reaching D-paths for each diagonal k
        let mut lcs = vec![vec![0; m + 1]; n + 1];

        for i in 1..=n {
            for j in 1..=m {
                if old_lines[i - 1] == new_lines[j - 1] {
                    lcs[i][j] = lcs[i - 1][j - 1] + 1; // Diagonal move (match)
                } else {
                    // Choose best of vertical (insertion) or horizontal (deletion) move
                    lcs[i][j] = std::cmp::max(lcs[i - 1][j], lcs[i][j - 1]);
                }
            }
        }

        // Backtrack to find the changes - equivalent to reconstructing the path
        // in Myers' algorithm from the furthest reaching endpoints
        let mut changes = Vec::new();
        let mut i = n;
        let mut j = m;

        while i > 0 || j > 0 {
            if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
                // Diagonal move - matching elements (snake in Myers' terminology)
                changes.push(Change::Equal(i - 1, j - 1));
                i -= 1;
                j -= 1;
            } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
                // Vertical move - insertion
                changes.push(Change::Insert(j - 1, 1));
                j -= 1;
            } else if i > 0 {
                // Horizontal move - deletion
                changes.push(Change::Delete(i - 1, 1));
                i -= 1;
            }
        }

        // Reverse changes (we constructed them from end to start)
        changes.reverse();

        // Post-process to merge adjacent operations of the same type
        // This is an optimization for better patch representation
        let mut result = Vec::new();
        let mut i = 0;

        while i < changes.len() {
            match changes[i] {
                Change::Delete(idx, count) => {
                    let mut total = count;
                    let mut j = i + 1;

                    while j < changes.len() {
                        if let Change::Delete(next_idx, next_count) = changes[j] {
                            if next_idx == idx + total {
                                total += next_count;
                                j += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    result.push(Change::Delete(idx, total));
                    i = j;
                }
                Change::Insert(idx, count) => {
                    let mut total = count;
                    let mut j = i + 1;

                    while j < changes.len() {
                        if let Change::Insert(next_idx, next_count) = changes[j] {
                            if next_idx == idx + total {
                                total += next_count;
                                j += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    result.push(Change::Insert(idx, total));
                    i = j;
                }
                Change::Equal(old_idx, new_idx) => {
                    let mut count = 1;
                    let mut j = i + 1;

                    while j < changes.len() {
                        if let Change::Equal(next_old, next_new) = changes[j] {
                            if next_old == old_idx + count && next_new == new_idx + count {
                                count += 1;
                                j += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    for k in 0..count {
                        result.push(Change::Equal(old_idx + k, new_idx + k));
                    }

                    i = j;
                }
            }
        }

        result
    }
}

impl DiffAlgorithm for MyersDiffer<'_> {
    /// Generate a patch between the old and new content using the Myers diffing algorithm
    ///
    /// The algorithm finds the shortest edit script (minimum number of insertions and deletions)
    /// to transform the old content into the new content.
    fn generate(&self) -> crate::Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();

        // Handle special cases for empty files
        if let Some(patch) = handle_empty_files(&old_lines, &new_lines) {
            return patch;
        }

        // Find the line-level changes using our implementation of Myers algorithm
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
}
