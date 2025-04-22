use crate::{Chunk, Operation, Patch};

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
        let operations = new_lines
            .iter()
            .map(|&line| Operation::Add(line.to_string()))
            .collect();

        return Some(Patch {
            preamble: None,
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
        let operations = old_lines
            .iter()
            .map(|&line| Operation::Remove(line.to_string()))
            .collect();

        return Some(Patch {
            preamble: None,
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
            preamble: None,
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
    if changes.is_empty() {
        return chunks;
    }

    let mut current_change_idx = 0;
    while current_change_idx < changes.len() {
        // 1. Skip leading Equal changes to find the start of the next relevant block
        while current_change_idx < changes.len() {
            if let Change::Equal(_, _) = changes[current_change_idx] {
                current_change_idx += 1;
            } else {
                break;
            }
        }

        if current_change_idx >= changes.len() {
            break; // No more non-equal changes found
        }

        // 2. Find the end of the block, merging across context_lines gaps of Equal changes
        let block_start_idx = current_change_idx;
        let mut block_end_idx = block_start_idx;
        let mut consecutive_equals = 0;

        while block_end_idx < changes.len() {
            match changes[block_end_idx] {
                Change::Equal(_, _) => {
                    consecutive_equals += 1;
                }
                _ => {
                    // Delete or Insert encountered
                    // If the preceding gap of Equal changes was large enough, end the block before it.
                    if consecutive_equals > context_lines * 2 {
                        // Use > not >= to keep context for both sides
                        block_end_idx -= consecutive_equals; // Adjust end index
                        break;
                    }
                    consecutive_equals = 0; // Reset gap counter
                }
            }
            block_end_idx += 1;

            // If we reached the end and the last changes were Equal, check the gap count again
            if block_end_idx == changes.len() && consecutive_equals > context_lines * 2 {
                block_end_idx -= consecutive_equals;
            }
        }
        // block_start_idx..block_end_idx now defines the core operations for this chunk

        // 3. Determine the start and end indices covered by this core block
        // Use the indices from the *first* operation in the block
        let (block_old_start, block_new_start) = match changes[block_start_idx] {
            Change::Equal(o, n) => (o, n),
            Change::Delete(o, _) => (
                o,
                if block_start_idx > 0 {
                    // Find the new_index corresponding to the start of this delete
                    match changes[block_start_idx - 1] {
                        Change::Equal(_, n) => n + 1,
                        Change::Insert(n, count) => n + count,
                        // If previous was also delete, new index doesn't change. Need to find last non-delete.
                        // This simple lookbehind is insufficient. We rely on context logic.
                        // Assume the `new_index` where the deletion *starts*.
                        Change::Delete(_, _) => {
                            find_preceding_new_index(changes, block_start_idx).unwrap_or(0)
                        }
                    }
                } else {
                    0
                },
            ),
            Change::Insert(_, n) => (
                if block_start_idx > 0 {
                    // Find the old_index corresponding to the start of this insert
                    match changes[block_start_idx - 1] {
                        Change::Equal(o, _) => o + 1,
                        Change::Delete(o, count) => o + count,
                        Change::Insert(_, _) => {
                            find_preceding_old_index(changes, block_start_idx).unwrap_or(0)
                        }
                    }
                } else {
                    0
                },
                n,
            ),
        };

        // 4. Calculate context boundaries needed before the block
        let context_start_change_idx = block_start_idx.saturating_sub(context_lines);
        let chunk_old_start = changes[context_start_change_idx..block_start_idx]
            .iter()
            .find_map(|c| match c {
                Change::Equal(o, _) => Some(*o), // First equal gives the start
                _ => None,                       // Should only be equals here
            })
            .unwrap_or(block_old_start); // Fallback to block start if no context found
        let chunk_new_start = changes[context_start_change_idx..block_start_idx]
            .iter()
            .find_map(|c| match c {
                Change::Equal(_, n) => Some(*n),
                _ => None,
            })
            .unwrap_or(block_new_start);

        // 5. Build the operations list for the chunk
        let mut operations = Vec::new();

        // Add context before the block (from context_start_change_idx to block_start_idx)
        for idx in context_start_change_idx..block_start_idx {
            if let Change::Equal(o, _) = changes[idx] {
                if o < old_lines.len() {
                    // Boundary check
                    operations.push(Operation::Context(old_lines[o].to_string()));
                }
            }
        }

        // Add operations from the core block (block_start_idx to block_end_idx)
        for idx in block_start_idx..block_end_idx {
            match changes[idx] {
                Change::Equal(o, _) => {
                    if o < old_lines.len() {
                        operations.push(Operation::Context(old_lines[o].to_string()));
                    }
                }
                Change::Delete(o, count) => {
                    for j in 0..count {
                        if o + j < old_lines.len() {
                            operations.push(Operation::Remove(old_lines[o + j].to_string()));
                        }
                    }
                }
                Change::Insert(n, count) => {
                    for j in 0..count {
                        if n + j < new_lines.len() {
                            operations.push(Operation::Add(new_lines[n + j].to_string()));
                        }
                    }
                }
            }
        }

        // Add context after the block
        let mut context_scan_idx = block_end_idx;
        let mut context_added_after = 0;
        let mut final_old_lines_count = 0; // Calculate counts based on actual ops added
        let mut final_new_lines_count = 0;

        // First pass: count lines from context before + core block
        for op in &operations {
            match op {
                Operation::Context(_) | Operation::Remove(_) => final_old_lines_count += 1,
                _ => {}
            }
            match op {
                Operation::Context(_) | Operation::Add(_) => final_new_lines_count += 1,
                _ => {}
            }
        }

        // Now add trailing context and update counts
        while context_added_after < context_lines && context_scan_idx < changes.len() {
            if let Change::Equal(o, _) = changes[context_scan_idx] {
                if o < old_lines.len() {
                    operations.push(Operation::Context(old_lines[o].to_string()));
                    final_old_lines_count += 1;
                    final_new_lines_count += 1;
                    context_added_after += 1;
                } else {
                    break; // Should not happen with valid input
                }
            } else {
                // Stop adding context if a non-Equal change is encountered
                break;
            }
            context_scan_idx += 1;
        }

        // 6. Create the chunk
        let chunk = Chunk {
            old_start: chunk_old_start, // Use calculated start including context
            old_lines: final_old_lines_count,
            new_start: chunk_new_start, // Use calculated start including context
            new_lines: final_new_lines_count,
            operations,
        };

        chunks.push(chunk);
        // Continue scanning from where the context scan stopped
        current_change_idx = context_scan_idx;
    }

    chunks
}

/// Helper to find the effective new_index preceding a Change::Delete sequence.
fn find_preceding_new_index(changes: &[Change], current_idx: usize) -> Option<usize> {
    for i in (0..current_idx).rev() {
        match changes[i] {
            Change::Equal(_, n) => return Some(n + 1),
            Change::Insert(n, count) => return Some(n + count),
            Change::Delete(_, _) => continue, // Keep looking back
        }
    }
    Some(0) // Default to 0 if no preceding non-delete found
}

/// Helper to find the effective old_index preceding a Change::Insert sequence.
fn find_preceding_old_index(changes: &[Change], current_idx: usize) -> Option<usize> {
    for i in (0..current_idx).rev() {
        match changes[i] {
            Change::Equal(o, _) => return Some(o + 1),
            Change::Delete(o, count) => return Some(o + count),
            Change::Insert(_, _) => continue, // Keep looking back
        }
    }
    Some(0) // Default to 0 if no preceding non-insert found
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
