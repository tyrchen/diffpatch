use crate::{Error, Operation, Patch, Patcher};

impl Patcher {
    /// Create a new Patcher with the given patch
    pub fn new(patch: Patch) -> Self {
        Self { patch }
    }

    /// Apply the patch to the content
    pub fn apply(&self, content: &str, reverse: bool) -> Result<String, Error> {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut current_line = 0;

        for chunk in &self.patch.chunks {
            let (start, operations) = if reverse {
                // In reverse mode, we use new_start and reverse the operations
                (chunk.new_start, self.reverse_operations(&chunk.operations))
            } else {
                (chunk.old_start, chunk.operations.clone())
            };

            // Find context lines to determine the actual starting point
            let context_lines = operations
                .iter()
                .take_while(|op| matches!(op, Operation::Context(_)))
                .collect::<Vec<_>>();

            // If we have context lines at the beginning, use them for better matching
            let actual_start = if !context_lines.is_empty() {
                self.find_best_context_match(&lines, current_line, &context_lines, start)
                    .unwrap_or(start)
            } else {
                // When no context lines at the beginning, try to find trailing context from previous chunk
                let trailing_context = operations
                    .iter()
                    .rev()
                    .take_while(|op| matches!(op, Operation::Context(_)))
                    .collect::<Vec<_>>();

                if !trailing_context.is_empty() {
                    // Look for trailing context to position this chunk if no leading context
                    self.find_fuzzy_match(&lines, current_line, start, &operations)
                        .unwrap_or(start)
                } else {
                    start
                }
            };

            // Add lines up to the actual starting point
            while current_line < actual_start {
                if current_line >= lines.len() {
                    return Err(Error::LineNotFound(format!(
                        "Line {} not found in content",
                        current_line + 1
                    )));
                }
                result.push(lines[current_line].to_string());
                current_line += 1;
            }

            // Apply operations
            for op in operations {
                match op {
                    Operation::Context(line) => {
                        // Context lines should match the content, but we'll be more lenient
                        // by allowing whitespace differences and trying to continue if possible
                        if current_line >= lines.len() {
                            return Err(Error::ApplyError(format!(
                                "Context mismatch at line {}. Expected '{}', got EOF",
                                current_line + 1,
                                line
                            )));
                        }

                        // Try various match strategies with increasing leniency
                        let exact_match = lines[current_line] == line;
                        let whitespace_normalized_match = if !exact_match {
                            // Normalize whitespace (trim and collapse multiple spaces)
                            let normalized_line = normalize_whitespace(lines[current_line]);
                            let normalized_expected = normalize_whitespace(&line);
                            normalized_line == normalized_expected
                        } else {
                            true
                        };

                        let content_fuzzy_match = if !whitespace_normalized_match {
                            // Allow for some fuzziness in matching by checking content similarity
                            similarity_score(lines[current_line], &line) >= 0.7
                        } else {
                            true
                        };

                        if !content_fuzzy_match {
                            // Context doesn't match - provide detailed error
                            let actual = if current_line < lines.len() {
                                format!("'{}'", lines[current_line])
                            } else {
                                "EOF".to_string()
                            };

                            return Err(Error::ApplyError(format!(
                                "Context mismatch at line {}. Expected '{}', got {}",
                                current_line + 1,
                                line,
                                actual
                            )));
                        }

                        // Preserve the original line rather than replacing it with the context line
                        // This maintains any extra content that might be in the original line
                        result.push(lines[current_line].to_string());
                        current_line += 1;
                    }
                    Operation::Add(line) => {
                        // Add the new line
                        result.push(line);
                    }
                    Operation::Remove(_) => {
                        // Skip the line to be removed
                        if current_line >= lines.len() {
                            return Err(Error::LineNotFound(format!(
                                "Line {} not found to remove",
                                current_line + 1
                            )));
                        }
                        current_line += 1;
                    }
                }
            }
        }

        // Add any remaining lines
        while current_line < lines.len() {
            result.push(lines[current_line].to_string());
            current_line += 1;
        }

        Ok(result.join("\n"))
    }

    /// Reverse the operations in a chunk
    fn reverse_operations(&self, operations: &[Operation]) -> Vec<Operation> {
        operations
            .iter()
            .map(|op| match op {
                Operation::Add(line) => Operation::Remove(line.clone()),
                Operation::Remove(line) => Operation::Add(line.clone()),
                Operation::Context(line) => Operation::Context(line.clone()),
            })
            .collect()
    }

    /// Find a fuzzy match for a chunk based on the overall operation patterns
    fn find_fuzzy_match(
        &self,
        lines: &[&str],
        start_from: usize,
        default_pos: usize,
        operations: &[Operation],
    ) -> Option<usize> {
        // Extract context lines from operations
        let context_pairs: Vec<(usize, &str)> = operations
            .iter()
            .enumerate()
            .filter_map(|(i, op)| {
                if let Operation::Context(line) = op {
                    Some((i, line.as_str()))
                } else {
                    None
                }
            })
            .collect();

        if context_pairs.is_empty() {
            return Some(default_pos);
        }

        // Search in a reasonable range around the expected position
        let search_range = 50; // Increased search range for fuzzy matching
        let start_search = start_from.saturating_sub(search_range);
        let end_search = (default_pos + search_range).min(lines.len());

        let mut best_position = None;
        let mut best_score = 0.0;

        // For each potential starting position
        for pos in start_search..end_search {
            let mut position_score = 0.0;
            let mut matches = 0;

            // Check each context line
            for (op_index, context_line) in &context_pairs {
                let target_line = pos + op_index - context_pairs[0].0;
                if target_line >= lines.len() {
                    continue;
                }

                let score = similarity_score(lines[target_line], context_line);
                if score > 0.7 {
                    // 70% similarity threshold
                    position_score += score;
                    matches += 1;
                }
            }

            // Calculate overall score for this position
            if matches > 0 {
                let avg_score = position_score / matches as f64;
                let match_ratio = matches as f64 / context_pairs.len() as f64;
                let combined_score = avg_score * match_ratio;

                if combined_score > best_score {
                    best_score = combined_score;
                    best_position = Some(pos);
                }
            }
        }

        if best_score > 0.5 {
            // Overall threshold for considering it a match
            best_position
        } else {
            None
        }
    }

    /// Find the best matching position for a set of context lines
    fn find_best_context_match(
        &self,
        lines: &[&str],
        start_from: usize,
        context_ops: &[&Operation],
        default_pos: usize,
    ) -> Option<usize> {
        // Extract expected context lines from operations
        let context_lines: Vec<&str> = context_ops
            .iter()
            .map(|op| match op {
                Operation::Context(line) => line.as_str(),
                _ => unreachable!(),
            })
            .collect();

        if context_lines.is_empty() {
            return Some(default_pos);
        }

        // Look for the context pattern in the file
        let search_range = 50; // Increased search range (was 20)
        let start_search = start_from.saturating_sub(search_range);
        let end_search = (default_pos + search_range).min(lines.len());

        // First try exact matches
        for i in start_search..end_search {
            if i + context_lines.len() > lines.len() {
                continue;
            }

            // Check if this position matches all context lines
            let mut matches = true;
            for (j, context) in context_lines.iter().enumerate() {
                // Try both exact match and trimmed match for flexibility
                let exact_match = lines[i + j] == *context;
                let trimmed_match = lines[i + j].trim() == context.trim();

                if !exact_match && !trimmed_match {
                    matches = false;
                    break;
                }
            }

            if matches {
                return Some(i);
            }
        }

        // If no exact match found, try fuzzy matching with word-level comparison
        let mut best_match = None;
        let mut best_score = 0.0;

        for i in start_search..end_search {
            if i + context_lines.len() > lines.len() {
                continue;
            }

            // Calculate similarity scores for each line
            let mut total_score = 0.0;
            for (j, context) in context_lines.iter().enumerate() {
                let score = similarity_score(lines[i + j], context);
                total_score += score;
            }

            let avg_score = total_score / context_lines.len() as f64;
            if avg_score > 0.7 && avg_score > best_score {
                // Require at least 70% similarity
                best_score = avg_score;
                best_match = Some(i);
            }
        }

        // If still no good match, try more lenient partial matching
        if best_match.is_none() {
            let min_match_ratio = 0.6; // Lower threshold (was 0.8)
            let mut best_partial_score = 0;

            for i in start_search..end_search {
                if i + context_lines.len() > lines.len() {
                    continue;
                }

                // Count how many lines match with increasing leniency
                let mut match_count = 0;
                for (j, context) in context_lines.iter().enumerate() {
                    // Try various matching strategies
                    let exact_match = lines[i + j] == *context;
                    let trimmed_match = lines[i + j].trim() == context.trim();
                    let whitespace_normalized =
                        normalize_whitespace(lines[i + j]) == normalize_whitespace(context);
                    let fuzzy_match = similarity_score(lines[i + j], context) > 0.6;

                    if exact_match || trimmed_match || whitespace_normalized || fuzzy_match {
                        match_count += 1;
                    }
                }

                let match_ratio = match_count as f64 / context_lines.len() as f64;
                if match_ratio >= min_match_ratio && match_count > best_partial_score {
                    best_partial_score = match_count;
                    best_match = Some(i);
                }
            }
        }

        best_match
    }
}

/// Normalize whitespace by trimming and collapsing multiple spaces
fn normalize_whitespace(text: &str) -> String {
    let trimmed = text.trim();
    let mut result = String::with_capacity(trimmed.len());
    let mut last_was_space = false;

    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(c);
            last_was_space = false;
        }
    }

    result
}

/// Calculate a similarity score between two strings
/// Returns a value between 0.0 (no similarity) and 1.0 (identical)
fn similarity_score(a: &str, b: &str) -> f64 {
    // If one string is a prefix of the other, count that as a high similarity
    if a.starts_with(b) || b.starts_with(a) {
        // Calculate similarity based on length difference
        let max_len = a.len().max(b.len()) as f64;
        let min_len = a.len().min(b.len()) as f64;
        return 0.8 + (0.2 * (min_len / max_len)); // At least 80% similarity for prefix matches
    }

    // Check if one string contains the other (not just as prefix)
    if a.contains(b) || b.contains(a) {
        // Calculate similarity based on length difference
        let max_len = a.len().max(b.len()) as f64;
        let min_len = a.len().min(b.len()) as f64;
        return 0.75 + (0.25 * (min_len / max_len)); // At least 75% similarity for substring matches
    }

    // Split into words and compare
    let words_a: Vec<&str> = a.split_whitespace().collect();
    let words_b: Vec<&str> = b.split_whitespace().collect();

    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }

    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    // Count matching words
    let mut matches = 0;
    for word_a in &words_a {
        if words_b.contains(word_a) {
            matches += 1;
        }
    }

    // Calculate Jaccard similarity coefficient
    let total_unique_words = words_a.len() + words_b.len() - matches;
    matches as f64 / total_unique_words as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{differ::DiffAlgorithmType, Differ};

    #[test]
    fn test_apply_patch() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2 modified\nline3\nline4";

        let differ = Differ::new(old, new, DiffAlgorithmType::Myers);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_reverse_patch() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3\nnew line";

        let differ = Differ::new(old, new, DiffAlgorithmType::Myers);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);

        // Apply forward
        let forward = patcher.apply(old, false).unwrap();
        assert_eq!(forward, new);

        // Apply backward
        let backward = patcher.apply(new, true).unwrap();
        assert_eq!(backward, old);
    }

    #[test]
    fn test_apply_patch_with_offset() {
        // Test applying a patch to content with an offset
        let old = "header1\nheader2\nline1\nline2\nline3\nline4";
        let new = "header1\nheader2\nline1\nline2 modified\nline3\nline4";

        // Create a patch that expects line2 at position 2
        let patch_content = "line1\nline2\nline3\nline4";
        let patch_target = "line1\nline2 modified\nline3\nline4";

        let differ = Differ::new(patch_content, patch_target, DiffAlgorithmType::Myers);
        let patch = differ.generate();

        // Try to apply to the content that has header lines
        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();

        // Should correctly identify the offset and apply the patch
        assert_eq!(result, new);
    }

    #[test]
    fn test_apply_patch_with_similar_context() {
        // Test with content that has similar but not identical context
        let old = "start\n  line1  \nline2\nline3\nend";
        let patch_content = "line1\nline2\nline3";
        let patch_target = "line1\nmodified line\nline3";

        let differ = Differ::new(patch_content, patch_target, DiffAlgorithmType::Myers);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();

        // The result should replace line2 with "modified line"
        assert_eq!(result, "start\n  line1  \nmodified line\nline3\nend");
    }

    #[test]
    fn test_apply_patch_with_different_context() {
        // In our improved implementation, when applying a patch to a file with
        // different context (where line3 has extra content),
        // we should preserve the extra content when applying context lines
        let old = "start\n  line1  \nline2\nline3 with extra stuff\nend";
        let patch_content = "line1\nline2\nline3";
        let patch_target = "line1\nmodified line\nline3";

        let differ = Differ::new(patch_content, patch_target, DiffAlgorithmType::Myers);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();

        // The result should preserve "with extra stuff" part since we're keeping
        // the original content for context lines
        let expected = "start\n  line1  \nmodified line\nline3 with extra stuff\nend";
        assert_eq!(result, expected);
    }
}
