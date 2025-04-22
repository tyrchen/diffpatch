use crate::{Error, Operation, Patch, Patcher};

// Constants for search and matching
const SEARCH_RANGE: usize = 50;
const FUZZY_MATCH_THRESHOLD: f64 = 0.7;
const LENIENT_MATCH_THRESHOLD: f64 = 0.6;
const PREFIX_MATCH_SCORE: f64 = 0.8;
const SUBSTRING_MATCH_SCORE: f64 = 0.75;
const COMBINED_SCORE_THRESHOLD: f64 = 0.5;

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
            let (start, operations) = self.prepare_chunk_operations(chunk, reverse);
            let actual_start =
                self.find_chunk_start_position(&lines, current_line, start, &operations);

            // Add lines up to the actual starting point
            current_line =
                self.copy_lines_until(&lines, current_line, actual_start, &mut result)?;

            // Apply operations
            current_line = self.apply_operations(&lines, current_line, &operations, &mut result)?;
        }

        // Add any remaining lines
        self.copy_remaining_lines(&lines, current_line, &mut result);

        Ok(result.join("\n"))
    }

    /// Prepare chunk operations based on whether we're applying in reverse
    fn prepare_chunk_operations(
        &self,
        chunk: &crate::Chunk,
        reverse: bool,
    ) -> (usize, Vec<Operation>) {
        if reverse {
            // In reverse mode, we use new_start and reverse the operations
            (chunk.new_start, self.reverse_operations(&chunk.operations))
        } else {
            (chunk.old_start, chunk.operations.clone())
        }
    }

    /// Copy lines from source to result until reaching target line
    fn copy_lines_until(
        &self,
        lines: &[&str],
        mut current_line: usize,
        target_line: usize,
        result: &mut Vec<String>,
    ) -> Result<usize, Error> {
        while current_line < target_line {
            if current_line >= lines.len() {
                return Err(Error::LineNotFound(format!(
                    "Line {} not found in content",
                    current_line + 1
                )));
            }
            result.push(lines[current_line].to_string());
            current_line += 1;
        }
        Ok(current_line)
    }

    /// Copy remaining lines from source to result
    fn copy_remaining_lines(
        &self,
        lines: &[&str],
        mut current_line: usize,
        result: &mut Vec<String>,
    ) {
        while current_line < lines.len() {
            result.push(lines[current_line].to_string());
            current_line += 1;
        }
    }

    /// Find where to start applying a chunk by examining context
    fn find_chunk_start_position(
        &self,
        lines: &[&str],
        current_line: usize,
        default_pos: usize,
        operations: &[Operation],
    ) -> usize {
        // Find context lines to determine the actual starting point
        let leading_context = self.extract_leading_context(operations);

        if !leading_context.is_empty() {
            // If we have context lines at the beginning, use them for better matching
            self.find_best_context_match(lines, current_line, &leading_context, default_pos)
                .unwrap_or(default_pos)
        } else {
            // When no context lines at the beginning, try to find trailing context from previous chunk
            let trailing_context = self.extract_trailing_context(operations);

            if !trailing_context.is_empty() {
                // Look for trailing context to position this chunk if no leading context
                self.find_fuzzy_match(lines, current_line, default_pos, operations)
                    .unwrap_or(default_pos)
            } else {
                default_pos
            }
        }
    }

    /// Extract leading context lines from operations
    fn extract_leading_context<'a>(&self, operations: &'a [Operation]) -> Vec<&'a Operation> {
        operations
            .iter()
            .take_while(|op| matches!(op, Operation::Context(_)))
            .collect()
    }

    /// Extract trailing context lines from operations
    fn extract_trailing_context<'a>(&self, operations: &'a [Operation]) -> Vec<&'a Operation> {
        operations
            .iter()
            .rev()
            .take_while(|op| matches!(op, Operation::Context(_)))
            .collect()
    }

    /// Apply operations to current content
    fn apply_operations(
        &self,
        lines: &[&str],
        mut current_line: usize,
        operations: &[Operation],
        result: &mut Vec<String>,
    ) -> Result<usize, Error> {
        for op in operations {
            match op {
                Operation::Context(line) => {
                    current_line =
                        self.apply_context_operation(lines, current_line, line, result)?;
                }
                Operation::Add(line) => {
                    // Add the new line
                    result.push(line.clone());
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
        Ok(current_line)
    }

    /// Apply a context operation, preserving original line if it matches
    fn apply_context_operation(
        &self,
        lines: &[&str],
        current_line: usize,
        expected_line: &str,
        result: &mut Vec<String>,
    ) -> Result<usize, Error> {
        // Context lines should match the content, but we'll be more lenient
        if current_line >= lines.len() {
            return Err(Error::ApplyError(format!(
                "Context mismatch at line {}. Expected '{}', got EOF",
                current_line + 1,
                expected_line
            )));
        }

        if !self.is_context_match(lines[current_line], expected_line) {
            // Context doesn't match - provide detailed error
            let actual = if current_line < lines.len() {
                format!("'{}'", lines[current_line])
            } else {
                "EOF".to_string()
            };

            return Err(Error::ApplyError(format!(
                "Context mismatch at line {}. Expected '{}', got {}",
                current_line + 1,
                expected_line,
                actual
            )));
        }

        // Preserve the original line rather than replacing it with the context line
        // This maintains any extra content that might be in the original line
        result.push(lines[current_line].to_string());
        Ok(current_line + 1)
    }

    /// Check if a line matches expected context using various strategies
    fn is_context_match(&self, actual: &str, expected: &str) -> bool {
        // Try various match strategies with increasing leniency
        let exact_match = actual == expected;
        if exact_match {
            return true;
        }

        // Normalize whitespace (trim and collapse multiple spaces)
        let normalized_line = normalize_whitespace(actual);
        let normalized_expected = normalize_whitespace(expected);
        let whitespace_normalized_match = normalized_line == normalized_expected;
        if whitespace_normalized_match {
            return true;
        }

        // Allow for some fuzziness in matching by checking content similarity
        similarity_score(actual, expected) >= FUZZY_MATCH_THRESHOLD
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
        let context_pairs = self.extract_context_pairs(operations);

        if context_pairs.is_empty() {
            return Some(default_pos);
        }

        // Search in a reasonable range around the expected position
        let search_range = self.calculate_search_range(start_from, default_pos, lines.len());
        let mut best_result = None;

        // For each potential starting position
        for pos in search_range.0..search_range.1 {
            let score_info = self.calculate_position_score(lines, pos, &context_pairs);

            // Calculate overall score for this position
            if score_info.matches > 0 {
                let avg_score = score_info.total_score / score_info.matches as f64;
                let match_ratio = score_info.matches as f64 / context_pairs.len() as f64;
                let combined_score = avg_score * match_ratio;

                if combined_score > best_result.as_ref().map_or(0.0, |r: &MatchResult| r.score) {
                    best_result = Some(MatchResult {
                        position: pos,
                        score: combined_score,
                    });
                }
            }
        }

        // Only consider it a match if the score is above threshold
        best_result
            .filter(|r| r.score > COMBINED_SCORE_THRESHOLD)
            .map(|r| r.position)
    }

    /// Extract context line pairs (position, content) from operations
    fn extract_context_pairs<'a>(&self, operations: &'a [Operation]) -> Vec<(usize, &'a str)> {
        operations
            .iter()
            .enumerate()
            .filter_map(|(i, op)| {
                if let Operation::Context(line) = op {
                    Some((i, line.as_str()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Calculate search range based on starting positions
    fn calculate_search_range(
        &self,
        start_from: usize,
        default_pos: usize,
        max_len: usize,
    ) -> (usize, usize) {
        let start_search = start_from.saturating_sub(SEARCH_RANGE);
        let end_search = (default_pos + SEARCH_RANGE).min(max_len);
        (start_search, end_search)
    }

    /// Calculate position score for fuzzy matching
    fn calculate_position_score(
        &self,
        lines: &[&str],
        pos: usize,
        context_pairs: &[(usize, &str)],
    ) -> ScoreInfo {
        let mut total_score = 0.0;
        let mut matches = 0;

        // Check each context line
        for (op_index, context_line) in context_pairs {
            let target_line = pos + op_index - context_pairs[0].0;
            if target_line >= lines.len() {
                continue;
            }

            let score = similarity_score(lines[target_line], context_line);
            if score > FUZZY_MATCH_THRESHOLD {
                total_score += score;
                matches += 1;
            }
        }

        ScoreInfo {
            total_score,
            matches,
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
        let context_lines = self.extract_context_line_contents(context_ops);

        if context_lines.is_empty() {
            return Some(default_pos);
        }

        // Look for the context pattern in the file
        let search_range = self.calculate_search_range(start_from, default_pos, lines.len());

        // Try different matching strategies in order of strictness
        self.find_exact_context_match(lines, &context_lines, search_range)
            .or_else(|| self.find_fuzzy_context_match(lines, &context_lines, search_range))
            .or_else(|| self.find_partial_context_match(lines, &context_lines, search_range))
    }

    /// Extract context line contents from operations
    fn extract_context_line_contents<'a>(&self, context_ops: &[&'a Operation]) -> Vec<&'a str> {
        context_ops
            .iter()
            .map(|op| match op {
                Operation::Context(line) => line.as_str(),
                _ => unreachable!(),
            })
            .collect()
    }

    /// Find exact matches for context lines
    fn find_exact_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: (usize, usize),
    ) -> Option<usize> {
        for i in search_range.0..search_range.1 {
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
        None
    }

    /// Find fuzzy matches for context lines using similarity scoring
    fn find_fuzzy_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: (usize, usize),
    ) -> Option<usize> {
        let mut best_match = None;
        let mut best_score = 0.0;

        for i in search_range.0..search_range.1 {
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
            if avg_score > FUZZY_MATCH_THRESHOLD && avg_score > best_score {
                best_score = avg_score;
                best_match = Some(i);
            }
        }

        best_match
    }

    /// Find partial matches for context lines using flexible matching
    fn find_partial_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: (usize, usize),
    ) -> Option<usize> {
        let mut best_match = None;
        let mut best_partial_score = 0;

        for i in search_range.0..search_range.1 {
            if i + context_lines.len() > lines.len() {
                continue;
            }

            // Count how many lines match with increasing leniency
            let mut match_count = 0;
            for (j, context) in context_lines.iter().enumerate() {
                if self.is_flexible_line_match(lines[i + j], context) {
                    match_count += 1;
                }
            }

            let match_ratio = match_count as f64 / context_lines.len() as f64;
            if match_ratio >= LENIENT_MATCH_THRESHOLD && match_count > best_partial_score {
                best_partial_score = match_count;
                best_match = Some(i);
            }
        }

        best_match
    }

    /// Check if a line matches with flexible criteria
    fn is_flexible_line_match(&self, actual: &str, expected: &str) -> bool {
        let exact_match = actual == expected;
        let trimmed_match = actual.trim() == expected.trim();
        let whitespace_normalized = normalize_whitespace(actual) == normalize_whitespace(expected);
        let fuzzy_match = similarity_score(actual, expected) > LENIENT_MATCH_THRESHOLD;

        exact_match || trimmed_match || whitespace_normalized || fuzzy_match
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
        return calculate_length_based_score(a, b, PREFIX_MATCH_SCORE);
    }

    // Check if one string contains the other (not just as prefix)
    if a.contains(b) || b.contains(a) {
        return calculate_length_based_score(a, b, SUBSTRING_MATCH_SCORE);
    }

    calculate_jaccard_similarity(a, b)
}

/// Calculate score based on length ratio
fn calculate_length_based_score(a: &str, b: &str, base_score: f64) -> f64 {
    let max_len = a.len().max(b.len()) as f64;
    let min_len = a.len().min(b.len()) as f64;
    base_score + ((1.0 - base_score) * (min_len / max_len))
}

/// Calculate Jaccard similarity between string word sets
fn calculate_jaccard_similarity(a: &str, b: &str) -> f64 {
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

/// Helper struct for match results
struct MatchResult {
    position: usize,
    score: f64,
}

/// Helper struct for collecting score information
struct ScoreInfo {
    total_score: f64,
    matches: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Differ;

    #[test]
    fn test_apply_patch() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2 modified\nline3\nline4";

        let differ = Differ::new(old, new);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_reverse_patch() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3\nnew line";

        let differ = Differ::new(old, new);
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

        let differ = Differ::new(patch_content, patch_target);
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

        let differ = Differ::new(patch_content, patch_target);
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

        let differ = Differ::new(patch_content, patch_target);
        let patch = differ.generate();

        let patcher = Patcher::new(patch);
        let result = patcher.apply(old, false).unwrap();

        // The result should preserve "with extra stuff" part since we're keeping
        // the original content for context lines
        let expected = "start\n  line1  \nmodified line\nline3 with extra stuff\nend";
        assert_eq!(result, expected);
    }
}
