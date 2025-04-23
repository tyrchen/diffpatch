use crate::patcher::PatchAlgorithm;
use crate::{Error, Operation, Patch};
use levenshtein::levenshtein;
use std::borrow::Cow;
use std::cmp::min;

// Constants for fuzzy matching
const SEARCH_RANGE: usize = 50;
const FUZZY_MATCH_THRESHOLD: f64 = 0.7;
const LENIENT_MATCH_THRESHOLD: f64 = 0.6;

/// A more sophisticated patcher that uses fuzzy matching to find the best
/// location to apply patches when exact context doesn't match.
pub struct SimilarPatcher<'a> {
    patch: &'a Patch,
}

/// Represents the result of a fuzzy match attempt.
#[derive(Debug)]
struct MatchResult {
    position: usize,
    score: f64,
}

impl<'a> SimilarPatcher<'a> {
    pub fn new(patch: &'a Patch) -> Self {
        Self { patch }
    }
}

impl PatchAlgorithm for SimilarPatcher<'_> {
    fn apply(&self, content: &str, reverse: bool) -> Result<String, Error> {
        let lines: Vec<&str> = content.lines().collect();
        let estimated_capacity = content
            .len()
            .saturating_add(self.estimate_patch_size_delta());
        let mut result = String::with_capacity(estimated_capacity);
        let mut current_line_index = 0;
        let mut first_line_written = true;

        for chunk in &self.patch.chunks {
            let (expected_start_line_one_based, operations_cow) =
                self.prepare_chunk_operations(chunk, reverse);
            let operations = operations_cow.as_ref();

            // Ensure expected_start_line is 0-based for find_chunk_start_position
            let expected_start_line_zero_based = expected_start_line_one_based.saturating_sub(1);

            let actual_start_line = self.find_chunk_start_position(
                &lines,
                current_line_index,
                expected_start_line_zero_based,
                operations,
            )?;

            self.append_lines_until(
                &lines,
                current_line_index,
                actual_start_line,
                &mut result,
                &mut first_line_written,
            )?;

            // Update current line index
            current_line_index = actual_start_line;

            current_line_index = self.apply_chunk_operations_to_string(
                &lines,
                current_line_index,
                operations,
                &mut result,
                &mut first_line_written,
            )?;
        }

        self.append_remaining_lines(
            &lines,
            current_line_index,
            &mut result,
            &mut first_line_written,
        );

        // Ensure final newline is preserved if the original content had one
        if content.ends_with('\n') && !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }
}

impl SimilarPatcher<'_> {
    /// Estimates the change in total content size based on Add/Remove operations.
    fn estimate_patch_size_delta(&self) -> usize {
        self.patch.chunks.iter().fold(0, |acc, c| {
            let added_len: usize = c
                .operations
                .iter()
                .filter_map(|op| match op {
                    Operation::Add(s) => Some(s.len() + 1),
                    _ => None,
                })
                .sum();
            let removed_len: usize = c
                .operations
                .iter()
                .filter_map(|op| match op {
                    Operation::Remove(s) => Some(s.len() + 1),
                    _ => None,
                })
                .sum();
            acc.saturating_add(added_len).saturating_sub(removed_len)
        })
    }

    /// Prepares chunk operations based on whether the patch is being applied normally or in reverse.
    fn prepare_chunk_operations<'a>(
        &self,
        chunk: &'a crate::Chunk,
        reverse: bool,
    ) -> (usize, Cow<'a, [Operation]>) {
        if reverse {
            (
                chunk.new_start,
                Cow::Owned(self.reverse_operations(&chunk.operations)),
            )
        } else {
            (chunk.old_start, Cow::Borrowed(&chunk.operations))
        }
    }

    /// Reverses the operations for applying a patch in reverse.
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

    /// Appends lines from the source until the target line index.
    fn append_lines_until(
        &self,
        lines: &[&str],
        mut current_line_index: usize,
        target_line_index: usize,
        result: &mut String,
        first_line_written: &mut bool,
    ) -> Result<usize, Error> {
        while current_line_index < target_line_index {
            if current_line_index >= lines.len() {
                return Err(Error::ApplyError(format!(
                    "Calculated chunk start {} is beyond content length {}",
                    target_line_index + 1,
                    lines.len()
                )));
            }
            if !*first_line_written {
                result.push('\n');
            } else {
                *first_line_written = false;
            }
            result.push_str(lines[current_line_index]);
            current_line_index += 1;
        }
        Ok(current_line_index)
    }

    /// Appends all remaining lines to the result string.
    fn append_remaining_lines(
        &self,
        lines: &[&str],
        mut current_line_index: usize,
        result: &mut String,
        first_line_written: &mut bool,
    ) {
        while current_line_index < lines.len() {
            if !*first_line_written {
                result.push('\n');
            } else {
                *first_line_written = false;
            }
            result.push_str(lines[current_line_index]);
            current_line_index += 1;
        }
    }

    /// Applies the operations within a single chunk to the result string.
    fn apply_chunk_operations_to_string(
        &self,
        lines: &[&str],
        mut current_line_index: usize,
        operations: &[Operation],
        result: &mut String,
        first_line_written: &mut bool,
    ) -> Result<usize, Error> {
        for op in operations {
            match op {
                Operation::Context(expected_line) => {
                    if current_line_index >= lines.len() {
                        return Err(Error::LineNotFound {
                            line_num: current_line_index + 1,
                        });
                    }
                    let actual_line = lines[current_line_index];
                    if !Self::lines_match_flexibly(
                        actual_line,
                        expected_line,
                        FUZZY_MATCH_THRESHOLD,
                    ) {
                        return Err(Error::ApplyError(format!(
                            "Context mismatch at line {}: Expected '{}', got '{}'",
                            current_line_index + 1,
                            expected_line,
                            actual_line
                        )));
                    }
                    if !*first_line_written {
                        result.push('\n');
                    } else {
                        *first_line_written = false;
                    }
                    result.push_str(actual_line);
                    current_line_index += 1;
                }
                Operation::Add(line_to_add) => {
                    if !*first_line_written {
                        result.push('\n');
                    } else {
                        *first_line_written = false;
                    }
                    result.push_str(line_to_add);
                }
                Operation::Remove(_) => {
                    if current_line_index >= lines.len() {
                        return Err(Error::LineNotFound {
                            line_num: current_line_index + 1,
                        });
                    }
                    current_line_index += 1;
                }
            }
        }
        Ok(current_line_index)
    }

    /// Determines if two lines match with some flexibility, allowing for whitespace differences.
    fn lines_match_flexibly(actual: &str, expected: &str, fuzzy_threshold: f64) -> bool {
        // Check exact match first (common case, make it fast)
        if actual == expected {
            return true;
        }

        // Then check with normalized whitespace
        let actual_norm = normalize_whitespace(actual);
        let expected_norm = normalize_whitespace(expected);
        if actual_norm == expected_norm {
            return true;
        }

        // Finally check with similarity
        similarity_score(actual, expected) >= fuzzy_threshold
    }

    /// Finds the best position to start applying a chunk.
    fn find_chunk_start_position(
        &self,
        lines: &[&str],
        search_start_index: usize,
        expected_start_line: usize,
        operations: &[Operation],
    ) -> Result<usize, Error> {
        // Extract context lines for better fuzzy matching
        let context_lines: Vec<&str> = operations
            .iter()
            .filter_map(|op| match op {
                Operation::Context(line) => Some(line.as_str()),
                _ => None,
            })
            .collect();

        if context_lines.is_empty() {
            // No context lines, just use the expected position
            return Ok(expected_start_line);
        }

        // Try to find the best match for this chunk
        self.find_best_match_position(
            lines,
            search_start_index,
            expected_start_line,
            &context_lines,
        )
    }

    /// Finds the best position to match the context lines.
    fn find_best_match_position(
        &self,
        lines: &[&str],
        search_start_index: usize,
        expected_start_line: usize,
        context_lines: &[&str],
    ) -> Result<usize, Error> {
        // Try exact match at the expected position first
        if expected_start_line < lines.len() {
            let expected_end = expected_start_line + context_lines.len();
            if expected_end <= lines.len() {
                let mut exact_match = true;
                for (i, context) in context_lines.iter().enumerate() {
                    let line_index = expected_start_line + i;
                    if !Self::lines_match_flexibly(
                        lines[line_index],
                        context,
                        FUZZY_MATCH_THRESHOLD,
                    ) {
                        exact_match = false;
                        break;
                    }
                }
                if exact_match {
                    return Ok(expected_start_line);
                }
            }
        }

        // Define search range: try an expanding range around the expected position
        let min_search = search_start_index.max(expected_start_line.saturating_sub(SEARCH_RANGE));
        let max_search = min(
            lines.len().saturating_sub(context_lines.len()),
            expected_start_line.saturating_add(SEARCH_RANGE),
        );

        // First, try to find an exact match in the search range
        let search_range = min_search..max_search;
        if let Some(position) =
            self.find_exact_context_match(lines, context_lines, search_range.clone())
        {
            return Ok(position);
        }

        // Next, try fuzzy matching
        if let Some(position) =
            self.find_fuzzy_context_match(lines, context_lines, search_range.clone())
        {
            return Ok(position);
        }

        // Finally, try partial matching on a subset of context
        if let Some(position) = self.find_partial_context_match(lines, context_lines, search_range)
        {
            return Ok(position);
        }

        // If we still haven't found a good match, use the expected position but warn
        if expected_start_line < lines.len() {
            Ok(expected_start_line)
        } else {
            Err(Error::ApplyError(format!(
                "Failed to find matching context for chunk expected at line {}",
                expected_start_line + 1
            )))
        }
    }

    /// Tries to find an exact match for the context lines.
    fn find_exact_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: std::ops::Range<usize>,
    ) -> Option<usize> {
        for start_idx in search_range {
            if start_idx + context_lines.len() > lines.len() {
                continue;
            }

            let mut match_found = true;
            for (i, &context_line) in context_lines.iter().enumerate() {
                if lines[start_idx + i] != context_line {
                    match_found = false;
                    break;
                }
            }

            if match_found {
                return Some(start_idx);
            }
        }
        None
    }

    /// Tries to find a fuzzy match for the context lines.
    fn find_fuzzy_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: std::ops::Range<usize>,
    ) -> Option<usize> {
        let mut best_match: Option<MatchResult> = None;

        for start_idx in search_range {
            if start_idx + context_lines.len() > lines.len() {
                continue;
            }

            let mut total_score = 0.0;
            let mut all_above_threshold = true;

            for (i, &context_line) in context_lines.iter().enumerate() {
                let line_idx = start_idx + i;
                let score = similarity_score(lines[line_idx], context_line);

                if score < FUZZY_MATCH_THRESHOLD {
                    all_above_threshold = false;
                    break;
                }

                total_score += score;
            }

            if all_above_threshold {
                let avg_score = total_score / context_lines.len() as f64;
                if let Some(current_best) = &best_match {
                    if avg_score > current_best.score {
                        best_match = Some(MatchResult {
                            position: start_idx,
                            score: avg_score,
                        });
                    }
                } else {
                    best_match = Some(MatchResult {
                        position: start_idx,
                        score: avg_score,
                    });
                }
            }
        }

        best_match.map(|m| m.position)
    }

    /// Tries to find a partial match for a subset of the context lines.
    fn find_partial_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: std::ops::Range<usize>,
    ) -> Option<usize> {
        // Try with just the first few and last few context lines for a strong partial match
        let context_len = context_lines.len();
        if context_len < 2 {
            return None; // Not enough context to do meaningful partial matching
        }

        // For short context, just check the first line with a lower threshold
        if context_len == 2 {
            for start_idx in search_range {
                if start_idx >= lines.len() {
                    continue;
                }

                let score = similarity_score(lines[start_idx], context_lines[0]);
                if score >= LENIENT_MATCH_THRESHOLD {
                    return Some(start_idx);
                }
            }
            return None;
        }

        // For longer context, check first and last couple of lines
        let mut best_match: Option<MatchResult> = None;

        for start_idx in search_range {
            if start_idx + context_len > lines.len() {
                continue;
            }

            // Score the beginning lines
            let mut begin_score = 0.0;
            let begin_count = 2.min(context_len);
            for i in 0..begin_count {
                begin_score += similarity_score(lines[start_idx + i], context_lines[i]);
            }
            begin_score /= begin_count as f64;

            // Score the ending lines
            let mut end_score = 0.0;
            let end_count = 2.min(context_len);
            for i in 0..end_count {
                let context_idx = context_len - 1 - i;
                let line_idx = start_idx + context_len - 1 - i;
                end_score += similarity_score(lines[line_idx], context_lines[context_idx]);
            }
            end_score /= end_count as f64;

            // Combined score with higher weight on beginning
            let combined_score = (begin_score * 0.6) + (end_score * 0.4);
            if combined_score >= LENIENT_MATCH_THRESHOLD {
                if let Some(current_best) = &best_match {
                    if combined_score > current_best.score {
                        best_match = Some(MatchResult {
                            position: start_idx,
                            score: combined_score,
                        });
                    }
                } else {
                    best_match = Some(MatchResult {
                        position: start_idx,
                        score: combined_score,
                    });
                }
            }
        }

        best_match.map(|m| m.position)
    }
}

/// Normalizes whitespace in a string, collapsing multiple spaces into one.
fn normalize_whitespace(text: &str) -> Cow<str> {
    if !text.contains("  ") && !text.contains('\t') {
        return Cow::Borrowed(text);
    }

    let mut result = String::with_capacity(text.len());
    let mut last_was_space = false;

    for c in text.chars() {
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

    Cow::Owned(result)
}

/// Calculates a similarity score between two strings based on Levenshtein distance.
fn similarity_score(a: &str, b: &str) -> f64 {
    // Check for exact match
    if a == b {
        return 1.0;
    }

    // Check for normalized match (ignoring whitespace differences)
    let a_norm = normalize_whitespace(a);
    let b_norm = normalize_whitespace(b);
    if a_norm == b_norm {
        // High score for whitespace-only differences, allowing flexibility
        return 0.95;
    }

    let len_a = a.chars().count();
    let len_b = b.chars().count();

    // Handle empty strings
    if len_a == 0 && len_b == 0 {
        return 1.0; // Two empty strings are identical
    }
    if len_a == 0 || len_b == 0 {
        return 0.0; // Similarity is 0 if one string is empty and the other is not
    }

    let distance = levenshtein(a, b) as f64;
    let max_len = (len_a.max(len_b)) as f64;

    // Normalize distance into a similarity score (1.0 = identical, 0.0 = completely different)
    (1.0 - (distance / max_len)).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::differ::{DiffAlgorithm, Differ};

    #[test]
    fn test_apply_with_exact_match() {
        let old_content = "line1\nline2\nline3\nline4";
        let new_content = "line1\nline2 modified\nline3\nline4";

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Apply the patch
        let patcher = SimilarPatcher::new(&patch);
        let result = patcher.apply(old_content, false).unwrap();

        assert_eq!(result, new_content);
    }

    #[test]
    fn test_apply_with_whitespace_differences() {
        let old_content = "line1\nline2\nline3\nline4";
        let new_content = "line1\nline2 modified\nline3\nline4";
        let input_with_whitespace = "line1\n  line2  \nline3\nline4";

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Apply the patch to content with whitespace differences
        let patcher = SimilarPatcher::new(&patch);
        let result = patcher.apply(input_with_whitespace, false).unwrap();

        // The result should have the modified line without the original whitespace
        // as the patch operation replaces the line entirely.
        assert_eq!(result, "line1\nline2 modified\nline3\nline4");
    }

    #[test]
    fn test_apply_with_fuzzy_match() {
        let old_content = "line1\nline2\nline3\nline4";
        let new_content = "line1\nline2 modified\nline3\nline4";
        let similar_content = "line1\nlin2\nlin3\nline4"; // Slightly misspelled lines

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Apply the patch to content with fuzzy differences
        let patcher = SimilarPatcher::new(&patch);
        let result = patcher.apply(similar_content, false);

        // This should succeed due to fuzzy matching
        assert!(result.is_ok());
        // The result should have the same structure but with the modified line from the patch
        assert_eq!(result.unwrap(), "line1\nline2 modified\nlin3\nline4");
    }

    #[test]
    fn test_apply_reverse_with_fuzzy_match() {
        let old_content = "line1\nline2\nline3\nline4";
        let new_content = "line1\nline2 modified\nline3\nline4";
        let similar_new_content = "line1\nline2 modified\nlin3\nline4"; // Slight difference

        // Generate a patch
        let differ = Differ::new(old_content, new_content);
        let patch = differ.generate();

        // Apply the patch in reverse to content with fuzzy differences
        let patcher = SimilarPatcher::new(&patch);
        let result = patcher.apply(similar_new_content, true);

        // This should succeed due to fuzzy matching
        assert!(result.is_ok());
        // The result should have the original content structure but preserve the slight difference
        assert_eq!(result.unwrap(), "line1\nline2\nlin3\nline4");
    }
}
