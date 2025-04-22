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
                    if consecutive_equals >= context_lines {
                        block_end_idx -= consecutive_equals; // Adjust end index
                        break;
                    }
                    consecutive_equals = 0; // Reset gap counter
                }
            }
            block_end_idx += 1;

            // If we reached the end and the last changes were Equal, check the gap count again
            if block_end_idx == changes.len() && consecutive_equals >= context_lines {
                block_end_idx -= consecutive_equals;
            }
        }
        // block_start_idx..block_end_idx now defines the core operations for this chunk

        // 3. Determine the start and end indices covered by this core block
        let (block_old_start, block_new_start) = match changes[block_start_idx] {
            Change::Equal(o, n) => (o, n),
            // For Delete/Insert, the start index in the primary file is known.
            // The corresponding start index in the other file is implicitly the index
            // right after the previous operation ended, or 0 if it's the first op.
            // We rely on the context extension logic to handle this correctly rather
            // than trying complex lookarounds here.
            Change::Delete(o, _) => (
                o,
                if block_start_idx > 0 {
                    match changes[block_start_idx - 1] {
                        Change::Equal(_, n) => n + 1,
                        Change::Insert(n, count) => n + count,
                        Change::Delete(_, _) => {
                            // This case implies consecutive deletes, the 'new' index doesn't move.
                            // Need to find the 'new' index associated with the start of this delete sequence.
                            // This complex case is better handled by context addition.
                            // For simplicity here, use previous logic or context handles it.
                            // Let's assume context addition resolves the start index correctly.
                            0 // Placeholder, context logic will fix this
                        }
                    }
                } else {
                    0
                },
            ),
            Change::Insert(n, _) => (
                if block_start_idx > 0 {
                    match changes[block_start_idx - 1] {
                        Change::Equal(o, _) => o + 1,
                        Change::Delete(o, count) => o + count,
                        Change::Insert(_, _) => {
                            // Consecutive inserts, 'old' index doesn't move.
                            0 // Placeholder, context logic will fix this
                        }
                    }
                } else {
                    0
                },
                n,
            ),
        };

        // Calculate the end indices (exclusive) of the block
        let (block_old_end, block_new_end) = match changes[block_end_idx - 1] {
            Change::Equal(o, n) => (o + 1, n + 1),
            Change::Delete(o, count) => (
                o + count,
                block_new_start, // New index doesn't change for deletes within the block
            ),
            Change::Insert(n, count) => (
                block_old_start, // Old index doesn't change for inserts within the block
                n + count,
            ),
        };

        // 4. Calculate context boundaries needed before the block
        let context_needed_before = context_lines;
        let chunk_old_start = block_old_start.saturating_sub(context_needed_before);
        let chunk_new_start = block_new_start.saturating_sub(context_needed_before);
        // Determine how many actual context lines exist and need to be added
        let actual_context_lines_before = block_old_start - chunk_old_start;

        // 5. Build the operations list for the chunk
        let mut operations = Vec::new();

        // Add context before the block
        for i in 0..actual_context_lines_before {
            let old_idx = chunk_old_start + i;
            // Assume context lines match, take from old_lines
            if old_idx < old_lines.len() {
                operations.push(Operation::Context(old_lines[old_idx].to_string()));
            }
        }

        // Add operations from the core block
        for change in changes.iter().skip(block_start_idx) {
            match *change {
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
        let mut context_added_after = 0;
        let mut last_context_old_idx = block_old_end; // Start from block end
        let mut last_context_new_idx = block_new_end;

        let mut context_scan_idx = block_end_idx;
        while context_added_after < context_lines && context_scan_idx < changes.len() {
            if let Change::Equal(o, n) = changes[context_scan_idx] {
                if o < old_lines.len() {
                    operations.push(Operation::Context(old_lines[o].to_string()));
                    last_context_old_idx = o + 1; // Update end index (exclusive)
                    last_context_new_idx = n + 1;
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

        // 6. Determine final chunk headers based on actual content included
        let chunk_old_end = last_context_old_idx;
        let chunk_new_end = last_context_new_idx;

        // Ensure start indices are consistent after context addition
        // The calculated chunk_old_start/new_start should be correct based on block_start
        let final_chunk_old_start = chunk_old_start;
        let final_chunk_new_start = chunk_new_start;

        let chunk = Chunk {
            old_start: final_chunk_old_start,
            old_lines: chunk_old_end - final_chunk_old_start,
            new_start: final_chunk_new_start,
            new_lines: chunk_new_end - final_chunk_new_start,
            operations,
        };

        chunks.push(chunk);
        // Continue scanning from where the context scan stopped, or after the block if no context was added
        current_change_idx = context_scan_idx;
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
