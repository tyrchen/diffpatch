use crate::differ::DiffAlgorithm;
use crate::{Chunk, Differ, Operation, Patch};
use std::cmp::{max, min};

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
}

/// Change type used internally for the diffing algorithm
enum Change {
    Equal(usize, usize),  // (old_index, new_index)
    Delete(usize, usize), // (old_index, count)
    Insert(usize, usize), // (new_index, count)
}

impl DiffAlgorithm for NaiveDiffer<'_> {
    /// Generate a patch between the old and new content using the naive diffing algorithm
    fn generate(&self) -> Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();

        // Special case for empty files
        if old_lines.is_empty() && !new_lines.is_empty() {
            // Adding content to an empty file
            let mut operations = Vec::new();
            for line in &new_lines {
                operations.push(Operation::Add(line.to_string()));
            }

            return Patch {
                preemble: None,
                old_file: "original".to_string(),
                new_file: "modified".to_string(),
                chunks: vec![Chunk {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 0,
                    new_lines: new_lines.len(),
                    operations,
                }],
            };
        } else if !old_lines.is_empty() && new_lines.is_empty() {
            // Removing all content
            let mut operations = Vec::new();
            for line in &old_lines {
                operations.push(Operation::Remove(line.to_string()));
            }

            return Patch {
                preemble: None,
                old_file: "original".to_string(),
                new_file: "modified".to_string(),
                chunks: vec![Chunk {
                    old_start: 0,
                    old_lines: old_lines.len(),
                    new_start: 0,
                    new_lines: 0,
                    operations,
                }],
            };
        } else if old_lines.is_empty() && new_lines.is_empty() {
            // Both files are empty, no diff needed
            return Patch {
                preemble: None,
                old_file: "original".to_string(),
                new_file: "modified".to_string(),
                chunks: Vec::new(),
            };
        }

        // First, find all line-level diffs
        let mut chunks = Vec::new();
        let mut i = 0;
        let mut j = 0;

        let mut changes = Vec::new();

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

        // Now convert the changes to chunks with proper context
        if !changes.is_empty() {
            let mut change_start = 0;
            while change_start < changes.len() {
                // Skip equal changes at the beginning
                while change_start < changes.len() {
                    if let Change::Equal(_, _) = changes[change_start] {
                        change_start += 1;
                    } else {
                        break;
                    }
                }

                if change_start >= changes.len() {
                    break;
                }

                // Find the end of consecutive changes (including Equal changes)
                let mut change_end = change_start + 1;
                while change_end < changes.len() {
                    if let Change::Equal(_, _) = changes[change_end] {
                        // Include equal lines within this chunk
                        change_end += 1;
                    } else {
                        change_end += 1;
                        // Look for a run of Equal changes
                        let mut consecutive_equals = 0;
                        while change_end < changes.len() {
                            if let Change::Equal(_, _) = changes[change_end] {
                                consecutive_equals += 1;
                                if consecutive_equals >= self.differ.context_lines {
                                    break;
                                }
                                change_end += 1;
                            } else {
                                consecutive_equals = 0;
                                change_end += 1;
                            }
                        }
                    }
                }

                // Get the line indices for the chunk boundaries
                let mut old_start = usize::MAX;
                let mut old_end = 0;
                let mut new_start = usize::MAX;
                let mut new_end = 0;

                for (i, change) in changes
                    .iter()
                    .enumerate()
                    .take(min(change_end, changes.len()))
                    .skip(change_start)
                {
                    match change {
                        Change::Equal(o, n) => {
                            old_start = min(old_start, *o);
                            old_end = max(old_end, *o + 1);
                            new_start = min(new_start, *n);
                            new_end = max(new_end, *n + 1);
                        }
                        Change::Delete(o, count) => {
                            old_start = min(old_start, *o);
                            old_end = max(old_end, *o + *count);
                            new_start = min(new_start, j);
                            new_end = max(new_end, j);
                        }
                        Change::Insert(n, count) => {
                            old_start = min(old_start, i);
                            old_end = max(old_end, i);
                            new_start = min(new_start, *n);
                            new_end = max(new_end, *n + *count);
                        }
                    }
                }

                // Extend backward for context
                let context_before = self.differ.context_lines;
                let old_adjusted_start = old_start.saturating_sub(context_before);
                let new_adjusted_start = new_start.saturating_sub(context_before);

                // Add context lines before
                let mut operations = Vec::new();
                for i in 0..context_before {
                    if old_adjusted_start + i < old_start && new_adjusted_start + i < new_start {
                        let old_idx = old_adjusted_start + i;
                        if old_idx < old_lines.len() {
                            operations.push(Operation::Context(old_lines[old_idx].to_string()));
                        }
                    }
                }

                // Process the changes
                for change in changes
                    .iter()
                    .take(min(change_end, changes.len()))
                    .skip(change_start)
                {
                    match change {
                        Change::Equal(o, _) => {
                            if *o < old_lines.len() {
                                operations.push(Operation::Context(old_lines[*o].to_string()));
                            }
                        }
                        Change::Delete(o, count) => {
                            for j in 0..*count {
                                if *o + j < old_lines.len() {
                                    operations
                                        .push(Operation::Remove(old_lines[*o + j].to_string()));
                                }
                            }
                        }
                        Change::Insert(n, count) => {
                            for j in 0..*count {
                                if *n + j < new_lines.len() {
                                    operations.push(Operation::Add(new_lines[*n + j].to_string()));
                                }
                            }
                        }
                    }
                }

                // Add context lines after
                let context_after = self.differ.context_lines;
                let mut remaining_context = context_after;
                let mut ctx_idx = min(change_end, changes.len());

                while remaining_context > 0 && ctx_idx < changes.len() {
                    if let Change::Equal(o, _) = changes[ctx_idx] {
                        if o < old_lines.len() {
                            operations.push(Operation::Context(old_lines[o].to_string()));
                        }
                        remaining_context -= 1;
                    }
                    ctx_idx += 1;
                }

                // Create the chunk
                let chunk = Chunk {
                    old_start: old_adjusted_start,
                    old_lines: old_end - old_adjusted_start,
                    new_start: new_adjusted_start,
                    new_lines: new_end - new_adjusted_start,
                    operations,
                };

                chunks.push(chunk);
                change_start = change_end;
            }
        }

        Patch {
            preemble: None,
            old_file: "original".to_string(),
            new_file: "modified".to_string(),
            chunks,
        }
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
