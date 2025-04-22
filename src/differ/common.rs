use crate::{Chunk, Operation, Patch};
use std::cmp::{max, min};

/// Change type used internally for the diffing algorithms
pub enum Change {
    Equal(usize, usize),  // (old_index, new_index)
    Delete(usize, usize), // (old_index, count)
    Insert(usize, usize), // (new_index, count)
}

/// Handle special cases for empty files
pub fn handle_empty_files(old_lines: &[&str], new_lines: &[&str]) -> Option<Patch> {
    // Special case for empty files
    if old_lines.is_empty() && !new_lines.is_empty() {
        // Adding content to an empty file
        let mut operations = Vec::new();
        for line in new_lines {
            operations.push(Operation::Add(line.to_string()));
        }

        return Some(Patch {
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
        });
    } else if !old_lines.is_empty() && new_lines.is_empty() {
        // Removing all content
        let mut operations = Vec::new();
        for line in old_lines {
            operations.push(Operation::Remove(line.to_string()));
        }

        return Some(Patch {
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
        });
    } else if old_lines.is_empty() && new_lines.is_empty() {
        // Both files are empty, no diff needed
        return Some(Patch {
            preemble: None,
            old_file: "original".to_string(),
            new_file: "modified".to_string(),
            chunks: Vec::new(),
        });
    }

    None
}

/// Process changes to generate chunks with proper context
pub fn process_changes_to_chunks(
    changes: &[Change],
    old_lines: &[&str],
    new_lines: &[&str],
    context_lines: usize,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();

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
                            if consecutive_equals >= context_lines {
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

            for i in change_start..min(change_end, changes.len()) {
                match changes[i] {
                    Change::Equal(o, n) => {
                        old_start = min(old_start, o);
                        old_end = max(old_end, o + 1);
                        new_start = min(new_start, n);
                        new_end = max(new_end, n + 1);
                    }
                    Change::Delete(o, count) => {
                        old_start = min(old_start, o);
                        old_end = max(old_end, o + count);
                        // We need to use the appropriate new index, but we don't have
                        // a direct mapping from old to new for deletes
                        // So we'll use 0 as a conservative estimate
                        if new_start == usize::MAX {
                            new_start = if i > 0 {
                                match changes[i - 1] {
                                    Change::Equal(_, n) => n + 1,
                                    Change::Insert(n, _) => n,
                                    _ => 0,
                                }
                            } else {
                                0
                            };
                        }
                        new_end = max(new_end, new_start);
                    }
                    Change::Insert(n, count) => {
                        new_start = min(new_start, n);
                        new_end = max(new_end, n + count);
                        // Similar logic for inserts - use 0 as a conservative estimate for old index
                        if old_start == usize::MAX {
                            old_start = if i > 0 {
                                match changes[i - 1] {
                                    Change::Equal(o, _) => o + 1,
                                    Change::Delete(o, _) => o,
                                    _ => 0,
                                }
                            } else {
                                0
                            };
                        }
                        old_end = max(old_end, old_start);
                    }
                }
            }

            // Extend backward for context
            let context_before = context_lines;
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
                                operations.push(Operation::Remove(old_lines[*o + j].to_string()));
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
            let context_after = context_lines;
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

    chunks
}

/// Create a patch with the specified chunks
pub fn create_patch(chunks: Vec<Chunk>) -> Patch {
    Patch {
        preamble: None,
        old_file: "original".to_string(),
        new_file: "modified".to_string(),
        chunks,
    }
}
