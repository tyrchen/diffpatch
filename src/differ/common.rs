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

/// Finds the start and end indices of the next block of relevant changes.
/// Skips leading `Equal` changes and merges adjacent non-equal changes
/// separated by fewer than `context_lines * 2` equal changes.
/// Returns `None` if no more non-equal blocks are found.
fn find_next_block(
    changes: &[Change],
    start_index: usize,
    context_lines: usize,
) -> Option<(usize, usize)> {
    // 1. Skip leading Equal changes
    let mut block_start_idx = start_index;
    while block_start_idx < changes.len() {
        if let Change::Equal(_, _) = changes[block_start_idx] {
            block_start_idx += 1;
        } else {
            break;
        }
    }

    if block_start_idx >= changes.len() {
        return None; // No more non-equal changes found
    }

    // 2. Find the end of the block, merging across small gaps of Equal changes
    let mut block_end_idx = block_start_idx;
    let mut consecutive_equals = 0;
    let merge_threshold = context_lines * 2; // Threshold for merging blocks

    while block_end_idx < changes.len() {
        match changes[block_end_idx] {
            Change::Equal(_, _) => {
                consecutive_equals += 1;
            }
            _ => {
                // Delete or Insert encountered
                // If the preceding gap of Equal changes was large enough, end the block before it.
                if consecutive_equals > merge_threshold {
                    // Use > not >= to keep context for both sides
                    block_end_idx -= consecutive_equals; // Adjust end index to before the gap
                    break;
                }
                consecutive_equals = 0; // Reset gap counter as we found a non-equal change
            }
        }
        block_end_idx += 1;

        // Special case: If we reached the end and the last changes were Equal, check the gap count.
        if block_end_idx == changes.len() && consecutive_equals > merge_threshold {
            block_end_idx -= consecutive_equals; // Adjust end index if trailing equals exceed threshold
        }
    }

    Some((block_start_idx, block_end_idx))
}

/// Builds the list of operations for a chunk, including context,
/// and calculates the old and new line counts for the chunk.
/// Returns the list of operations, the old line count, the new line count,
/// and the index in `changes` after adding trailing context.
fn build_chunk_operations<'a>(
    changes: &[Change],
    old_lines: &'a [&'a str],
    new_lines: &'a [&'a str],
    context_lines: usize,
    context_start_change_idx: usize,
    block_start_idx: usize,
    block_end_idx: usize,
) -> (Vec<Operation>, usize, usize, usize) {
    let mut operations = Vec::new();
    let mut chunk_old_lines_count = 0;
    let mut chunk_new_lines_count = 0;

    // Add context before the block
    for idx in context_start_change_idx..block_start_idx {
        if let Change::Equal(o, _) = changes[idx] {
            // Use get for safety, though indices should be valid based on how changes are generated
            if let Some(line) = old_lines.get(o) {
                operations.push(Operation::Context(line.to_string()));
                chunk_old_lines_count += 1;
                chunk_new_lines_count += 1;
            }
        }
    }

    // Add operations from the core block
    for idx in block_start_idx..block_end_idx {
        match changes[idx] {
            Change::Equal(o, _) => {
                if let Some(line) = old_lines.get(o) {
                    operations.push(Operation::Context(line.to_string()));
                    chunk_old_lines_count += 1;
                    chunk_new_lines_count += 1;
                }
            }
            Change::Delete(o, count) => {
                for j in 0..count {
                    if let Some(line) = old_lines.get(o + j) {
                        operations.push(Operation::Remove(line.to_string()));
                        chunk_old_lines_count += 1;
                    }
                }
            }
            Change::Insert(n, count) => {
                for j in 0..count {
                    if let Some(line) = new_lines.get(n + j) {
                        operations.push(Operation::Add(line.to_string()));
                        chunk_new_lines_count += 1;
                    }
                }
            }
        }
    }

    // Add context after the block
    let mut context_scan_idx = block_end_idx;
    let mut context_added_after = 0;
    while context_added_after < context_lines && context_scan_idx < changes.len() {
        if let Change::Equal(o, _) = changes[context_scan_idx] {
            if let Some(line) = old_lines.get(o) {
                operations.push(Operation::Context(line.to_string()));
                chunk_old_lines_count += 1;
                chunk_new_lines_count += 1;
                context_added_after += 1;
            } else {
                break; // Index out of bounds, stop adding context
            }
        } else {
            break; // Stop adding context if a non-Equal change is encountered
        }
        context_scan_idx += 1;
    }

    (
        operations,
        chunk_old_lines_count,
        chunk_new_lines_count,
        context_scan_idx, // Return the index after scanning for trailing context
    )
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
        // Find the next block of changes to process
        let Some((block_start_idx, block_end_idx)) =
            find_next_block(changes, current_change_idx, context_lines)
        else {
            break; // No more blocks found
        };

        // Calculate context boundaries needed before the block
        let context_start_change_idx = block_start_idx.saturating_sub(context_lines);

        // Determine the actual start line numbers for the chunk based on the first Equal change
        // in the context preceding the block. Fallback to the first change in the block if no context.
        let (chunk_old_start, chunk_new_start) =
            determine_chunk_start_indices(changes, context_start_change_idx, block_start_idx);

        // Build the operations and calculate line counts for the chunk
        let (operations, chunk_old_lines_count, chunk_new_lines_count, next_change_idx) =
            build_chunk_operations(
                changes,
                old_lines,
                new_lines,
                context_lines,
                context_start_change_idx,
                block_start_idx,
                block_end_idx,
            );

        // Create the chunk if it contains operations
        if !operations.is_empty() {
            let chunk = Chunk {
                old_start: chunk_old_start,
                old_lines: chunk_old_lines_count,
                new_start: chunk_new_start,
                new_lines: chunk_new_lines_count,
                operations,
            };
            chunks.push(chunk);
        }

        // Continue scanning from where the context scan stopped
        current_change_idx = next_change_idx;
    }

    chunks
}

/// Determines the starting line indices (old, new) for a chunk.
/// It looks for the first `Equal` change within the preceding context window.
/// If no `Equal` change is found in the context, it infers the start based on the first change in the block.
fn determine_chunk_start_indices(
    changes: &[Change],
    context_start_idx: usize,
    block_start_idx: usize,
) -> (usize, usize) {
    // Find the first Equal change in the context window before the block
    let context_start = changes[context_start_idx..block_start_idx]
        .iter()
        .find_map(|c| match c {
            Change::Equal(o, n) => Some((*o, *n)),
            _ => None,
        });

    if let Some((old_start, new_start)) = context_start {
        (old_start, new_start)
    } else {
        // If no Equal context before block, base start on the block's first change
        // This logic might need adjustment if the first block change isn't Equal
        match changes.get(block_start_idx) {
            Some(Change::Equal(o, n)) => (*o, *n),
            Some(Change::Delete(o, _)) => (*o, infer_previous_new_index(changes, block_start_idx)), // Need helper to infer previous state
            Some(Change::Insert(_, n)) => (infer_previous_old_index(changes, block_start_idx), *n), // Need helper to infer previous state
            None => (0, 0), // Should not happen if block_start_idx is valid
        }
    }
}

// Helper function to infer the new index before a Delete, if no preceding Equal context exists.
// This is a simplified inference; might not cover all edge cases perfectly without full state tracking.
fn infer_previous_new_index(changes: &[Change], current_idx: usize) -> usize {
    if current_idx == 0 {
        return 0;
    }
    // Look backwards for the state just before current_idx
    match changes[current_idx - 1] {
        Change::Equal(_, n_prev) => n_prev + 1,
        Change::Insert(n_prev, count) => n_prev + count,
        Change::Delete(_, _) => infer_previous_new_index(changes, current_idx - 1), // Recurse if previous was also delete
    }
}

// Helper function to infer the old index before an Insert, if no preceding Equal context exists.
fn infer_previous_old_index(changes: &[Change], current_idx: usize) -> usize {
    if current_idx == 0 {
        return 0;
    }
    // Look backwards for the state just before current_idx
    match changes[current_idx - 1] {
        Change::Equal(o_prev, _) => o_prev + 1,
        Change::Delete(o_prev, count) => o_prev + count,
        Change::Insert(_, _) => infer_previous_old_index(changes, current_idx - 1), // Recurse if previous was also insert
    }
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
