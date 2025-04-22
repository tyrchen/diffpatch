use crate::{Chunk, Differ, Operation, Patch};

impl Differ {
    /// Create a new Differ with the old and new content
    pub fn new(old: &str, new: &str) -> Self {
        Self {
            old: old.to_string(),
            new: new.to_string(),
            context_lines: 3, // Default context lines
        }
    }

    /// Set the number of context lines to include
    pub fn context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }

    /// Generate a patch between the old and new content
    pub fn generate(&self) -> Patch {
        let old_lines: Vec<&str> = self.old.lines().collect();
        let new_lines: Vec<&str> = self.new.lines().collect();

        // Find the longest common subsequence to identify changes
        let lcs = self.longest_common_subsequence(&old_lines, &new_lines);
        let mut chunks = Vec::new();

        let mut i = 0;
        let mut j = 0;

        let mut current_chunk: Option<Chunk> = None;

        while i < old_lines.len() || j < new_lines.len() {
            // Check if we're in an LCS (unchanged) section
            if i < old_lines.len()
                && j < new_lines.len()
                && old_lines[i] == new_lines[j]
                && lcs.contains(&(i, j))
            {
                // If we have an open chunk and we're past the context lines, close it
                if let Some(chunk) = current_chunk.take() {
                    chunks.push(chunk);
                }

                // Add context line if we're starting a new chunk
                if let Some(chunk) = &mut current_chunk {
                    chunk
                        .operations
                        .push(Operation::Context(old_lines[i].to_string()));
                    chunk.old_lines += 1;
                    chunk.new_lines += 1;
                }

                i += 1;
                j += 1;
            } else {
                // We're in a changed section
                if current_chunk.is_none() {
                    // Start a new chunk with context
                    let context_start = i.saturating_sub(self.context_lines);
                    let context_lines = i - context_start;

                    let mut operations = Vec::new();

                    // Add context lines
                    for line in old_lines.iter().skip(context_start).take(context_lines) {
                        operations.push(Operation::Context(line.to_string()));
                    }

                    current_chunk = Some(Chunk {
                        old_start: context_start,
                        old_lines: context_lines,
                        new_start: j.saturating_sub(context_lines),
                        new_lines: context_lines,
                        operations,
                    });
                }

                // Process removals (lines in old but not in new)
                if i < old_lines.len() && (j >= new_lines.len() || !lcs.contains(&(i, j))) {
                    if let Some(chunk) = &mut current_chunk {
                        chunk
                            .operations
                            .push(Operation::Remove(old_lines[i].to_string()));
                        chunk.old_lines += 1;
                    }
                    i += 1;
                }
                // Process additions (lines in new but not in old)
                else if j < new_lines.len() && (i >= old_lines.len() || !lcs.contains(&(i, j))) {
                    if let Some(chunk) = &mut current_chunk {
                        chunk
                            .operations
                            .push(Operation::Add(new_lines[j].to_string()));
                        chunk.new_lines += 1;
                    }
                    j += 1;
                }
            }
        }

        // Add the last chunk if there is one
        if let Some(chunk) = current_chunk {
            chunks.push(chunk);
        }

        Patch {
            preemble: None,
            old_file: "original".to_string(),
            new_file: "modified".to_string(),
            chunks,
        }
    }

    /// Find the longest common subsequence between two sequences
    fn longest_common_subsequence<T: PartialEq>(&self, a: &[T], b: &[T]) -> Vec<(usize, usize)> {
        if a.is_empty() || b.is_empty() {
            return Vec::new();
        }

        // Create a matrix of lengths of LCS
        let mut lengths = vec![vec![0; b.len() + 1]; a.len() + 1];

        // Fill the matrix
        for (i, a_item) in a.iter().enumerate() {
            for (j, b_item) in b.iter().enumerate() {
                if a_item == b_item {
                    lengths[i + 1][j + 1] = lengths[i][j] + 1;
                } else {
                    lengths[i + 1][j + 1] = std::cmp::max(lengths[i + 1][j], lengths[i][j + 1]);
                }
            }
        }

        // Backtrack to find the actual sequence
        let mut result = Vec::new();
        let mut i = a.len();
        let mut j = b.len();

        while i > 0 && j > 0 {
            if a[i - 1] == b[j - 1] {
                result.push((i - 1, j - 1));
                i -= 1;
                j -= 1;
            } else if lengths[i - 1][j] >= lengths[i][j - 1] {
                i -= 1;
            } else {
                j -= 1;
            }
        }

        result.reverse();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Patcher;

    #[test]
    fn test_simple_diff() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2 modified\nline3\nline4";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        assert_eq!(patch.chunks.len(), 1);
        assert_eq!(patch.chunks[0].old_start, 0);
        assert_eq!(patch.chunks[0].old_lines, 4);
        assert_eq!(patch.chunks[0].new_start, 0);
        assert_eq!(patch.chunks[0].new_lines, 4);

        // Try applying the patch
        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_add_line() {
        let old = "line1\nline2\nline4";
        let new = "line1\nline2\nline3\nline4";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_remove_line() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2\nline4";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_multiple_changes() {
        let old = "line1\nline2\nline3\nline4\nline5\nline6";
        let new = "line1\nmodified2\nline3\nnew line\nline5\nline6 changed";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_empty_files() {
        let old = "";
        let new = "new content";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_identical_files() {
        let content = "line1\nline2\nline3";

        let differ = Differ::new(content, content);
        let patch = differ.generate();

        assert_eq!(patch.chunks.len(), 0);
    }
}
