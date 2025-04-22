use crate::{Chunk, Error, Operation, Patch};
use std::borrow::Cow;
use tracing::{debug, warn};

// Constants for search and matching
const SEARCH_RANGE: usize = 50;
const FUZZY_MATCH_THRESHOLD: f64 = 0.7;
const LENIENT_MATCH_THRESHOLD: f64 = 0.6;
const PREFIX_MATCH_SCORE: f64 = 0.8;
const SUBSTRING_MATCH_SCORE: f64 = 0.75;

/// Applies a `Patch` to content.
#[derive(Debug)]
pub struct Patcher {
    patch: Patch,
}

/// Represents the result of a fuzzy match attempt.
#[derive(Debug)]
struct MatchResult {
    position: usize,
    score: f64,
}

impl Patcher {
    /// Creates a new `Patcher` instance for the given `Patch`.
    pub fn new(patch: Patch) -> Self {
        Self { patch }
    }

    /// Applies the patch to the provided content.
    ///
    /// # Arguments
    ///
    /// * `content` - The original content (as a string slice) to patch.
    /// * `reverse` - If `true`, applies the patch in reverse (reverting changes).
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The patched content.
    /// * `Err(Error)` - If the patch cannot be applied cleanly.
    pub fn apply(&self, content: &str, reverse: bool) -> Result<String, Error> {
        let lines: Vec<&str> = content.lines().collect();
        let estimated_capacity = content
            .len()
            .saturating_add(self.estimate_patch_size_delta());
        let mut result = String::with_capacity(estimated_capacity);
        let mut current_line_index = 0;
        let mut first_line_written = true;

        for chunk in &self.patch.chunks {
            let (expected_start_line, operations_cow) =
                self.prepare_chunk_operations(chunk, reverse);
            let operations = operations_cow.as_ref();

            let actual_start_line = self.find_chunk_start_position(
                &lines,
                current_line_index,
                expected_start_line,
                operations,
            )?;

            current_line_index = self.append_lines_until(
                &lines,
                current_line_index,
                actual_start_line,
                &mut result,
                &mut first_line_written,
            )?;

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

        // Ensure final newline is present if the original content had one
        // and the result doesn't already end with one.
        if content.ends_with('\n') && !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }

    /// Estimates the change in total content size based on Add/Remove operations.
    /// This helps pre-allocate the result String more accurately.
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
    /// Returns a Cow to avoid cloning operations when not reversing.
    fn prepare_chunk_operations<'a>(
        &self,
        chunk: &'a Chunk,
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

    /// Reverses the operations in a chunk for applying a patch in reverse.
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

    /// Appends lines from the source `lines` slice to the `result` String until the `target_line_index`.
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

    /// Appends all remaining lines from the source `lines` slice to the `result` String.
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

    /// Applies the operations within a single chunk (add, remove, context) directly to the result String.
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

    // --- Context Matching Logic Consolidation ---

    /// Checks if an actual line from the content matches an expected context line from the patch
    /// using increasingly lenient matching strategies and a given threshold.
    fn lines_match_flexibly(actual: &str, expected: &str, fuzzy_threshold: f64) -> bool {
        // 1. Exact match
        if actual == expected {
            return true;
        }

        // 2. Trimmed whitespace match
        if actual.trim() == expected.trim() {
            return true;
        }

        // 3. Normalize whitespace (trim and collapse multiple spaces)
        // Avoid normalization allocation if strings are identical after trim
        let normalized_actual = normalize_whitespace(actual);
        let normalized_expected = normalize_whitespace(expected);
        if normalized_actual == normalized_expected {
            return true;
        }

        // 4. Fuzzy match based on content similarity
        // Use normalized versions for fuzzy matching as well
        similarity_score(&normalized_actual, &normalized_expected) >= fuzzy_threshold
    }

    // --- Chunk Position Finding Logic ---

    /// Finds the best starting line index in the `lines` slice to apply a chunk's operations.
    /// Tries exact context matching first, then fuzzy matching.
    fn find_chunk_start_position(
        &self,
        lines: &[&str],
        search_start_index: usize,
        expected_start_line: usize,
        operations: &[Operation],
    ) -> Result<usize, Error> {
        // Extract only the leading context lines from the chunk's operations for positioning.
        // Hunks are located based on the lines *before* the first change.
        let context_lines: Vec<&str> = operations
            .iter()
            .take_while(|op| matches!(op, Operation::Context(_)))
            .map(|op| match op {
                Operation::Context(line) => line.as_str(),
                _ => unreachable!(),
            })
            .collect();

        self.find_best_match_position(
            lines,
            search_start_index,
            expected_start_line,
            &context_lines,
        )
    }

    /// Searches for the best position to apply a chunk based on context lines.
    fn find_best_match_position(
        &self,
        lines: &[&str],
        search_start_index: usize,
        expected_start_line: usize,
        context_lines: &[&str],
    ) -> Result<usize, Error> {
        // Handle edge case: Applying patch to an empty file or only additions at start
        if expected_start_line == 0 && (lines.is_empty() || context_lines.is_empty()) {
            return Ok(0);
        }

        // If context is empty, but not handled above (e.g. applying to non-empty file at line > 0)
        if context_lines.is_empty() {
            if expected_start_line <= lines.len() {
                return Ok(expected_start_line);
            } else {
                // Error if expected start is out of bounds
                return Err(Error::ApplyError(format!(
                    "Cannot apply hunk with no context: Expected start line {} is out of bounds (content length {})",
                    expected_start_line + 1,
                    lines.len()
                )));
            }
        }

        // If expecting to start at line 0, try matching context there first explicitly.
        if expected_start_line == 0 {
            // Check if at least the first context line matches at line 0
            if !context_lines.is_empty() && !lines.is_empty() {
                let mut matches_at_zero = true;
                // Check as many context lines as possible without going out of bounds
                for (i, expected_ctx) in context_lines.iter().enumerate() {
                    if i >= lines.len() {
                        // Cannot check beyond the actual content length
                        break;
                    }
                    if !Self::lines_match_flexibly(lines[i], expected_ctx, LENIENT_MATCH_THRESHOLD)
                    {
                        // Use flexible match
                        matches_at_zero = false;
                        break;
                    }
                }
                if matches_at_zero {
                    return Ok(0); // Found sufficient match starting at line 0
                }
            }
            // If it doesn't match exactly at 0, continue to fuzzy search below
        }

        // Define the search range around the expected position
        let half_range = SEARCH_RANGE / 2;
        let search_range_start = expected_start_line
            .saturating_sub(half_range)
            .max(search_start_index);
        // Ensure end doesn't exceed file length limit for windowing
        let file_len = lines.len();
        let search_range_end =
            (expected_start_line + half_range + context_lines.len()).min(file_len);
        // Ensure the range is valid (start <= end)
        let search_range = if search_range_start > search_range_end {
            search_range_start..search_range_start
        } else {
            search_range_start..search_range_end
        };

        // Strategy 1: Find exact match (or whitespace normalized match)
        if let Some(pos) = self.find_exact_context_match(lines, context_lines, search_range.clone())
        {
            return Ok(pos);
        }

        // Strategy 2: Find best fuzzy match based on similarity score
        if let Some(pos) = self.find_fuzzy_context_match(lines, context_lines, search_range.clone())
        {
            warn!(
                "Warning: Patch applied using fuzzy matching (expected line {}, found at {}).",
                expected_start_line + 1,
                pos + 1
            );
            return Ok(pos);
        }

        // Strategy 3: Find best partial match (lenient)
        if let Some(pos) =
            self.find_partial_context_match(lines, context_lines, search_range.clone())
        {
            warn!(
                "Warning: Patch applied using lenient partial matching (expected line {}, found at {}).",
                expected_start_line + 1,
                pos + 1
            );
            return Ok(pos);
        }

        // Strategy 4: If all else fails, try the exact expected position with lenient checks
        if expected_start_line + context_lines.len() <= lines.len() {
            let mut matches_leniently = true;
            for (i, expected_ctx) in context_lines.iter().enumerate() {
                if !Self::lines_match_flexibly(
                    lines[expected_start_line + i],
                    expected_ctx,
                    LENIENT_MATCH_THRESHOLD,
                ) {
                    matches_leniently = false;
                    break;
                }
            }
            if matches_leniently {
                warn!(
                    "Warning: Patch applied at expected position ({}) using lenient context check.",
                    expected_start_line + 1
                );
                return Ok(expected_start_line);
            }
        }

        // If no match found after all strategies
        Err(Error::ApplyError(format!(
            "Cannot find hunk starting near line {}",
            expected_start_line + 1
        )))
    }

    /// Attempts to find an exact match for the sequence of context lines within the search range.
    /// Also considers matches where only whitespace differs.
    fn find_exact_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: std::ops::Range<usize>,
    ) -> Option<usize> {
        if context_lines.is_empty() {
            return None;
        }

        lines
            .windows(context_lines.len())
            .enumerate()
            .skip(search_range.start)
            .take(search_range.end - search_range.start)
            .find_map(|(index, window)| {
                let is_match = window
                    .iter()
                    .zip(context_lines.iter())
                    .all(|(actual, expected)| {
                        *actual == *expected
                            || actual.trim() == expected.trim()
                            || normalize_whitespace(actual) == normalize_whitespace(expected)
                    });
                if is_match {
                    Some(index)
                } else {
                    None
                }
            })
    }

    /// Attempts to find the best fuzzy match for the sequence of context lines within the search range.
    /// Uses similarity scoring and returns the position with the highest average score above a threshold.
    fn find_fuzzy_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: std::ops::Range<usize>,
    ) -> Option<usize> {
        if context_lines.is_empty() {
            return None;
        }

        let mut best_match: Option<MatchResult> = None;

        lines
            .windows(context_lines.len())
            .enumerate()
            .skip(search_range.start)
            .take(search_range.end - search_range.start)
            .for_each(|(index, window)| {
                let total_score: f64 = window
                    .iter()
                    .zip(context_lines.iter())
                    .map(|(actual, expected)| similarity_score(actual, expected))
                    .sum();

                let avg_score = total_score / context_lines.len() as f64;

                debug!("Fuzzy match score at index {}: {:.4}", index, avg_score);
                if avg_score >= FUZZY_MATCH_THRESHOLD
                    && (best_match.is_none() || avg_score > best_match.as_ref().unwrap().score)
                {
                    best_match = Some(MatchResult {
                        position: index,
                        score: avg_score,
                    });
                }
            });

        best_match.map(|m| m.position)
    }

    /// Attempts to find a partial match where a significant portion of context lines match leniently.
    fn find_partial_context_match(
        &self,
        lines: &[&str],
        context_lines: &[&str],
        search_range: std::ops::Range<usize>,
    ) -> Option<usize> {
        if context_lines.is_empty() {
            return None;
        }

        let mut best_match: Option<(usize, usize)> = None;

        lines
            .windows(context_lines.len())
            .enumerate()
            .skip(search_range.start)
            .take(search_range.end - search_range.start)
            .for_each(|(index, window)| {
                let match_count = window
                    .iter()
                    .zip(context_lines.iter())
                    .filter(|(actual, expected)| {
                        Self::lines_match_flexibly(actual, expected, LENIENT_MATCH_THRESHOLD)
                    })
                    .count();

                let match_ratio = match_count as f64 / context_lines.len() as f64;
                debug!(
                    "Lenient match count at index {}: {}, ratio: {:.4}",
                    index, match_count, match_ratio
                );
                if match_ratio >= LENIENT_MATCH_THRESHOLD
                    && (best_match.is_none() || match_count > best_match.as_ref().unwrap().1)
                {
                    best_match = Some((index, match_count));
                }
            });

        best_match.map(|(pos, _)| pos)
    }
}

// --- String Similarity & Normalization Helpers ---

/// Normalizes whitespace in a string slice:
/// 1. Trims leading/trailing whitespace.
/// 2. Collapses multiple internal whitespace characters into a single space.
fn normalize_whitespace(text: &str) -> Cow<str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed("");
    }

    let mut needs_normalization = false;
    let mut last_char_was_whitespace = false;
    if text.len() != trimmed.len() {
        needs_normalization = true;
    } else {
        for c in trimmed.chars() {
            if c.is_whitespace() {
                if last_char_was_whitespace {
                    needs_normalization = true;
                    break;
                }
                last_char_was_whitespace = true;
            } else {
                last_char_was_whitespace = false;
            }
        }
    }

    if !needs_normalization {
        return Cow::Borrowed(trimmed);
    }

    let mut result = String::with_capacity(trimmed.len());
    last_char_was_whitespace = false;
    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !last_char_was_whitespace {
                result.push(' ');
                last_char_was_whitespace = true;
            }
        } else {
            result.push(c);
            last_char_was_whitespace = false;
        }
    }
    Cow::Owned(result)
}

/// Calculates a similarity score between two strings (0.0 to 1.0).
/// Prioritizes prefix/substring matches, then falls back to Jaccard similarity on words.
fn similarity_score(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let norm_a = normalize_whitespace(a);
    let norm_b = normalize_whitespace(b);
    if norm_a == norm_b {
        return 0.95;
    }

    if norm_a.is_empty() || norm_b.is_empty() {
        return 0.0;
    }

    let norm_a_ref = norm_a.as_ref();
    let norm_b_ref = norm_b.as_ref();

    if norm_a_ref.starts_with(norm_b_ref) || norm_b_ref.starts_with(norm_a_ref) {
        return calculate_length_based_score(norm_a_ref, norm_b_ref, PREFIX_MATCH_SCORE);
    }
    if norm_a_ref.contains(norm_b_ref) || norm_b_ref.contains(norm_a_ref) {
        return calculate_length_based_score(norm_a_ref, norm_b_ref, SUBSTRING_MATCH_SCORE);
    }

    calculate_jaccard_similarity(norm_a_ref, norm_b_ref)
}

/// Calculates a score boosted by a base score, adjusted by the length ratio.
fn calculate_length_based_score(a: &str, b: &str, base_score: f64) -> f64 {
    let len_a = a.len() as f64;
    let len_b = b.len() as f64;
    if len_a == 0.0 || len_b == 0.0 {
        return 0.0;
    }
    let max_len = len_a.max(len_b);
    let min_len = len_a.min(len_b);
    base_score + ((1.0 - base_score) * (min_len / max_len))
}

/// Calculates Jaccard similarity based on common words between two strings.
fn calculate_jaccard_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;

    let words_a: HashSet<&str> = a.split_whitespace().collect();
    let words_b: HashSet<&str> = b.split_whitespace().collect();

    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    let intersection_size = words_a.intersection(&words_b).count() as f64;
    let union_size = words_a.union(&words_b).count() as f64;

    if union_size == 0.0 {
        1.0
    } else {
        intersection_size / union_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Chunk, Differ}; // Need Differ and Chunk for setup

    fn create_test_patch(old: &str, new: &str) -> Patch {
        Differ::new(old, new).generate()
    }

    #[test]
    fn test_apply_simple_modification() {
        let old_content = "line1\nline2\nline3\nline4";
        let new_content = "line1\nline2 modified\nline3\nline4";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(old_content, false).unwrap();
        assert_eq!(result, new_content);
    }

    #[test]
    fn test_apply_addition() {
        let old_content = "line1\nline3";
        let new_content = "line1\nline2 added\nline3";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(old_content, false).unwrap();
        assert_eq!(result, new_content);
    }

    #[test]
    fn test_apply_deletion() {
        let old_content = "line1\nline2 to delete\nline3";
        let new_content = "line1\nline3";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(old_content, false).unwrap();
        assert_eq!(result, new_content);
    }

    #[test]
    fn test_apply_at_start() {
        let old_content = "line2\nline3";
        let new_content = "line1 added\nline2\nline3";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);
        let result = patcher
            .apply(old_content, false)
            .expect("Applying patch at start failed");
        assert_eq!(result, new_content);
    }

    #[test]
    fn test_apply_at_end() {
        let old_content = "line1\nline2";
        let new_content = "line1\nline2\nline3 added";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(old_content, false).unwrap();
        assert_eq!(result, new_content);
    }

    #[test]
    fn test_reverse_simple_modification() {
        let old_content = "line1\nline2\nline3";
        let new_content = "line1\nmodified\nline3\nnew line";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);

        // Apply forward first to ensure patch is valid
        let forward_result = patcher.apply(old_content, false).unwrap();
        assert_eq!(forward_result, new_content);

        // Apply backward/reverse
        let backward_result = patcher.apply(new_content, true).unwrap();
        assert_eq!(backward_result, old_content);
    }

    #[test]
    fn test_reverse_addition() {
        let old_content = "line1\nline3";
        let new_content = "line1\nline2 added\nline3";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);
        let backward_result = patcher.apply(new_content, true).unwrap();
        assert_eq!(backward_result, old_content);
    }

    #[test]
    fn test_reverse_deletion() {
        let old_content = "line1\nline2 to delete\nline3";
        let new_content = "line1\nline3";
        let patch = create_test_patch(old_content, new_content);
        let patcher = Patcher::new(patch);
        let backward_result = patcher.apply(new_content, true).unwrap();
        assert_eq!(backward_result, old_content);
    }

    #[test]
    fn test_apply_with_offset_context() {
        // Content has headers, patch was generated without them
        let original_content = "header1\nheader2\nline1\nline2\nline3\nline4\nfooter";
        let patch_source = "line1\nline2\nline3\nline4";
        let patch_target = "line1\nline2 modified\nline3\nline4";
        let expected_result = "header1\nheader2\nline1\nline2 modified\nline3\nline4\nfooter";

        let patch = create_test_patch(patch_source, patch_target);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(original_content, false).unwrap();
        assert_eq!(result, expected_result);
    }

    #[test]
    fn test_apply_with_whitespace_diff_context() {
        // Context lines in content have different whitespace than in patch
        let original_content = "start\n  line1  \nline2\n  line3 \nend"; // Note extra spaces
        let patch_source = "line1\nline2\nline3";
        let patch_target = "line1\nmodified line\nline3";
        let expected_result = "start\n  line1  \nmodified line\n  line3 \nend";

        let patch = create_test_patch(patch_source, patch_target);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(original_content, false).unwrap();
        assert_eq!(result, expected_result);
    }

    #[test]
    fn test_apply_preserves_original_context_content() {
        // Content context line has extra info not in patch context line
        let original_content = "start\nline1\nline2\nline3 with extra stuff\nend";
        let patch_source = "line1\nline2\nline3"; // Patch context is simpler
        let patch_target = "line1\nmodified line\nline3";
        let expected_result = "start\nline1\nmodified line\nline3 with extra stuff\nend"; // Should keep original line3

        let patch = create_test_patch(patch_source, patch_target);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(original_content, false).unwrap();
        assert_eq!(result, expected_result);
    }

    #[test]
    fn test_apply_fails_on_major_context_mismatch() {
        let original_content = "start\ncompletely different line\nline2\nline3\nend";
        let patch_source = "line1\nline2\nline3";
        let patch_target = "line1\nmodified line\nline3";

        let patch = create_test_patch(patch_source, patch_target);
        let patcher = Patcher::new(patch);
        let result = patcher.apply(original_content, false);

        assert!(result.is_err());
        assert!(matches!(result.err().unwrap(), Error::ApplyError(_)));
    }

    #[test]
    fn test_apply_fails_if_line_not_found_for_remove() {
        let original_content = "line1";
        let patch = Patch {
            // Manually create patch
            preamble: None,
            old_file: "a".into(),
            new_file: "b".into(),
            chunks: vec![Chunk {
                old_start: 0,
                old_lines: 2, // Expects 2 lines to remove
                new_start: 0,
                new_lines: 0,
                operations: vec![
                    Operation::Remove("line1".into()),
                    Operation::Remove("line2".into()), // This line doesn't exist in the original content
                ],
            }],
        };
        let patcher = Patcher::new(patch);
        let result = patcher.apply(original_content, false);
        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            Error::LineNotFound { line_num: 2 }
        ));
    }

    #[test]
    fn test_apply_fails_if_line_not_found_for_context() {
        let original_content = "line1";
        let patch = Patch {
            // Manually create patch
            preamble: None,
            old_file: "a".into(),
            new_file: "b".into(),
            chunks: vec![Chunk {
                old_start: 0,
                old_lines: 2, // Expects 2 context lines
                new_start: 0,
                new_lines: 2,
                operations: vec![
                    Operation::Context("line1".into()),
                    Operation::Context("line2".into()), // This line doesn't exist
                ],
            }],
        };
        let patcher = Patcher::new(patch);
        let result = patcher.apply(original_content, false);
        assert!(result.is_err());
        let err = result.err().unwrap();
        // The patcher::apply calls find_chunk_start_position first. It finds line1 at index 0.
        // Then apply_chunk_operations processes Operation::Context("line1"), ok. Index becomes 1.
        // Then it tries Operation::Context("line2"). Inside apply_chunk_operations,
        // it checks `current_line_index >= lines.len()`. Here 1 >= 1 is true.
        // So it returns Err(Error::LineNotFound { line_num: 1 + 1 }) -> LineNotFound { line_num: 2 }.
        assert!(
            matches!(err, Error::LineNotFound { line_num: 2 }),
            "Expected LineNotFound {{ line_num: 2 }}, got {:?}",
            err
        );
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("\thello\nworld "), "hello world");
        assert_eq!(normalize_whitespace("nochange"), "nochange");
        assert_eq!(normalize_whitespace("   "), "");
        assert_eq!(normalize_whitespace(""), "");

        // Test borrowing
        let s1 = "nochange";
        let normalized_s1 = normalize_whitespace(s1);
        assert!(
            matches!(normalized_s1, Cow::Borrowed(_)),
            "Should borrow for nochange"
        );
        assert_eq!(normalized_s1.as_ref(), s1);

        // Test owning
        let s2 = "  needs change  ";
        let normalized_s2 = normalize_whitespace(s2);
        assert!(
            matches!(normalized_s2, Cow::Owned(_)),
            "Should own for changed string"
        );
        assert_eq!(normalized_s2.as_ref(), "needs change");
    }

    #[test]
    fn test_similarity_score() {
        assert_eq!(similarity_score("abc", "abc"), 1.0);
        assert!(similarity_score("  abc ", "abc") > 0.9); // Normalized match
                                                          // Jaccard score for ("abc", "def") vs ("abc", "xyz") is 1 / 3 = 0.333...
        let score = similarity_score("abc def", "abc xyz");
        assert!(
            (score - (1.0 / 3.0)).abs() < f64::EPSILON,
            "Expected Jaccard score ~0.333, got {}",
            score
        );
        assert!(similarity_score("abcdef", "abc") > PREFIX_MATCH_SCORE); // Prefix
        assert!(similarity_score("xyz abc def", "abc") > SUBSTRING_MATCH_SCORE); // Substring
        assert_eq!(similarity_score("", "abc"), 0.0);
        assert_eq!(similarity_score("abc", ""), 0.0);
        assert_eq!(similarity_score("", ""), 1.0);
    }
}
