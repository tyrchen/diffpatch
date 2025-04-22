use crate::differ::{Change, DiffAlgorithm};
use crate::{Differ, Patch};

use super::{create_patch, handle_empty_files, process_changes_to_chunks};

/// XDiff differ implementation based on LibXDiff algorithm
pub struct XDiffDiffer<'a> {
    differ: &'a Differ,
}

impl<'a> XDiffDiffer<'a> {
    /// Create a new XDiffDiffer from a base Differ instance
    pub fn new(differ: &'a Differ) -> Self {
        Self { differ }
    }

    /// Implementation of the XDiff algorithm
    fn xdiff(&self, old_lines: &[&str], new_lines: &[&str]) -> Vec<Change> {
        let old_len = old_lines.len();
        let new_len = new_lines.len();

        // Create hash vectors for faster comparison
        let old_hash: Vec<u64> = old_lines.iter().map(|&line| self.hash_line(line)).collect();
        let new_hash: Vec<u64> = new_lines.iter().map(|&line| self.hash_line(line)).collect();

        // Initialize change markers
        let mut old_changes = vec![false; old_len];
        let mut new_changes = vec![false; new_len];

        // Compare each line in old file with each line in new file
        // This is a naive approach, but simpler to implement reliably in Rust
        self.compare_files(
            &old_hash,
            &mut old_changes,
            0,
            old_len,
            &new_hash,
            &mut new_changes,
            0,
            new_len,
        );

        // Build change script
        self.build_script(&old_changes, &new_changes, old_len, new_len)
    }

    /// Simple hash function for lines
    fn hash_line(&self, line: &str) -> u64 {
        // FNV-1a hash algorithm
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in line.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Compare files and mark changes
    fn compare_files(
        &self,
        old_hash: &[u64],
        old_changes: &mut [bool],
        old_start: usize,
        old_end: usize,
        new_hash: &[u64],
        new_changes: &mut [bool],
        new_start: usize,
        new_end: usize,
    ) {
        // Find common prefix
        let mut prefix_len = 0;
        let max_prefix = std::cmp::min(old_end - old_start, new_end - new_start);

        while prefix_len < max_prefix
            && old_hash[old_start + prefix_len] == new_hash[new_start + prefix_len]
        {
            prefix_len += 1;
        }

        // Find common suffix
        let mut suffix_len = 0;
        let max_suffix = max_prefix - prefix_len;

        while suffix_len < max_suffix
            && old_hash[old_end - 1 - suffix_len] == new_hash[new_end - 1 - suffix_len]
        {
            suffix_len += 1;
        }

        // Adjust the ranges
        let old_mid_start = old_start + prefix_len;
        let old_mid_end = old_end - suffix_len;
        let new_mid_start = new_start + prefix_len;
        let new_mid_end = new_end - suffix_len;

        // If one file segment is empty, all lines in the other must be changed
        if old_mid_start == old_mid_end {
            for i in new_mid_start..new_mid_end {
                new_changes[i] = true;
            }
            return;
        } else if new_mid_start == new_mid_end {
            for i in old_mid_start..old_mid_end {
                old_changes[i] = true;
            }
            return;
        }

        // Find the longest common subsequence in the middle
        let (old_idx, new_idx, len) = self.find_longest_common_subsequence(
            old_hash,
            old_mid_start,
            old_mid_end,
            new_hash,
            new_mid_start,
            new_mid_end,
        );

        if len == 0 {
            // No common subsequence, all lines are changed
            for i in old_mid_start..old_mid_end {
                old_changes[i] = true;
            }
            for i in new_mid_start..new_mid_end {
                new_changes[i] = true;
            }
        } else {
            // Recursively process the segments before and after LCS
            self.compare_files(
                old_hash,
                old_changes,
                old_mid_start,
                old_idx,
                new_hash,
                new_changes,
                new_mid_start,
                new_idx,
            );

            self.compare_files(
                old_hash,
                old_changes,
                old_idx + len,
                old_mid_end,
                new_hash,
                new_changes,
                new_idx + len,
                new_mid_end,
            );
        }
    }

    /// Find the longest common subsequence between two arrays
    fn find_longest_common_subsequence(
        &self,
        old_hash: &[u64],
        old_start: usize,
        old_end: usize,
        new_hash: &[u64],
        new_start: usize,
        new_end: usize,
    ) -> (usize, usize, usize) {
        let old_len = old_end - old_start;
        let new_len = new_end - new_start;

        if old_len == 0 || new_len == 0 {
            return (old_start, new_start, 0);
        }

        // Create a dynamic programming table
        let mut lcs = vec![vec![0; new_len + 1]; old_len + 1];

        // Fill the LCS table
        for i in 1..=old_len {
            for j in 1..=new_len {
                if old_hash[old_start + i - 1] == new_hash[new_start + j - 1] {
                    lcs[i][j] = lcs[i - 1][j - 1] + 1;
                } else {
                    lcs[i][j] = std::cmp::max(lcs[i - 1][j], lcs[i][j - 1]);
                }
            }
        }

        // Find the starting point of the LCS
        if lcs[old_len][new_len] == 0 {
            return (old_start, new_start, 0);
        }

        // Backtrack to find the LCS position
        let mut i = old_len;
        let mut j = new_len;
        let mut len = 0;

        while i > 0 && j > 0 {
            if old_hash[old_start + i - 1] == new_hash[new_start + j - 1] {
                len += 1;
                i -= 1;
                j -= 1;
            } else if lcs[i - 1][j] >= lcs[i][j - 1] {
                i -= 1;
            } else {
                j -= 1;
            }
        }

        let start_old = old_start + i;
        let start_new = new_start + j;

        (start_old, start_new, len)
    }

    /// Build a change script from the comparison results
    fn build_script(
        &self,
        old_changes: &[bool],
        new_changes: &[bool],
        old_len: usize,
        new_len: usize,
    ) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut old_idx = 0;
        let mut new_idx = 0;

        while old_idx < old_len || new_idx < new_len {
            // Find consecutive changed lines in old
            if old_idx < old_len && old_changes[old_idx] {
                let start_idx = old_idx;
                while old_idx < old_len && old_changes[old_idx] {
                    old_idx += 1;
                }
                changes.push(Change::Delete(start_idx, old_idx - start_idx));
                continue;
            }

            // Find consecutive changed lines in new
            if new_idx < new_len && new_changes[new_idx] {
                let start_idx = new_idx;
                while new_idx < new_len && new_changes[new_idx] {
                    new_idx += 1;
                }
                changes.push(Change::Insert(start_idx, new_idx - start_idx));
                continue;
            }

            // Equal lines
            if old_idx < old_len && new_idx < new_len {
                changes.push(Change::Equal(old_idx, new_idx));
                old_idx += 1;
                new_idx += 1;
            } else {
                // We've reached the end of one file but not the other
                if old_idx < old_len {
                    changes.push(Change::Delete(old_idx, old_len - old_idx));
                    old_idx = old_len;
                }
                if new_idx < new_len {
                    changes.push(Change::Insert(new_idx, new_len - new_idx));
                    new_idx = new_len;
                }
            }
        }

        // Merge adjacent changes of the same type
        self.post_process_changes(changes)
    }

    /// Merge adjacent changes of the same type
    fn post_process_changes(&self, changes: Vec<Change>) -> Vec<Change> {
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

                    result.push(Change::Equal(old_idx, new_idx));
                    i = j;
                }
            }
        }

        result
    }
}

impl DiffAlgorithm for XDiffDiffer<'_> {
    /// Generate a patch between the old and new content using a simplified XDiff algorithm
    fn generate(&self) -> Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();

        // Handle special cases for empty files
        if let Some(patch) = handle_empty_files(&old_lines, &new_lines) {
            return patch;
        }

        // Find the line-level changes using our implementation
        let changes = self.xdiff(&old_lines, &new_lines);

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
    use crate::{differ::DiffAlgorithmType, Patcher};

    #[test]
    fn test_simple_xdiff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3";

        let differ = Differ::new(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        assert_eq!(patch.chunks.len(), 0); // No changes, so no chunks
    }

    #[test]
    fn test_xdiff_add_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2\nline3\nline4";

        let differ = Differ::new(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_xdiff_remove_line() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline3";

        let differ = Differ::new(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_xdiff_complex_changes() {
        let old = "line1\nline2\nline3\nline4\nline5";
        let new = "line1\nmodified\nline3\nadded\nline5";

        let differ = Differ::new(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        assert!(patch.chunks.len() > 0);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }
}
