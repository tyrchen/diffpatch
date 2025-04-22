use crate::{Chunk, Diff, Differ, Operation, Patch};
use std::cmp::{max, min};
use std::ops::Index;

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

        // Find the line-level changes using the Myers diff algorithm
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
                                if consecutive_equals >= self.context_lines {
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
                        }
                        Change::Insert(n, count) => {
                            new_start = min(new_start, n);
                            new_end = max(new_end, n + count);
                        }
                    }
                }

                // Handle cases where no actual changes were found
                if old_start == usize::MAX {
                    old_start = 0;
                }
                if new_start == usize::MAX {
                    new_start = 0;
                }

                // Expand to include context lines before
                let context_before = self.context_lines;
                let adjusted_old_start = if old_start >= context_before {
                    old_start - context_before
                } else {
                    0
                };
                let adjusted_new_start = if new_start >= context_before {
                    new_start - context_before
                } else {
                    0
                };

                // Create the chunk
                let mut operations = Vec::new();
                let mut old_idx = adjusted_old_start;
                let mut new_idx = adjusted_new_start;

                // Add leading context
                while old_idx < old_start && old_idx < old_lines.len() {
                    operations.push(Operation::Context(old_lines[old_idx].to_string()));
                    old_idx += 1;
                    new_idx += 1;
                }

                // Add the changes
                for i in change_start..min(change_end, changes.len()) {
                    match changes[i] {
                        Change::Equal(o, n) => {
                            if o == old_idx && n == new_idx {
                                operations.push(Operation::Context(old_lines[o].to_string()));
                                old_idx += 1;
                                new_idx += 1;
                            }
                        }
                        Change::Delete(o, count) => {
                            if o == old_idx {
                                for j in 0..count {
                                    if o + j < old_lines.len() {
                                        operations
                                            .push(Operation::Remove(old_lines[o + j].to_string()));
                                    }
                                }
                                old_idx += count;
                            }
                        }
                        Change::Insert(n, count) => {
                            if n == new_idx {
                                for j in 0..count {
                                    if n + j < new_lines.len() {
                                        operations
                                            .push(Operation::Add(new_lines[n + j].to_string()));
                                    }
                                }
                                new_idx += count;
                            }
                        }
                    }
                }

                // Add trailing context
                let context_after = self.context_lines;
                let old_limit = min(old_end + context_after, old_lines.len());
                while old_idx < old_limit {
                    operations.push(Operation::Context(old_lines[old_idx].to_string()));
                    old_idx += 1;
                    new_idx += 1;
                }

                // Finalize the chunk
                chunks.push(Chunk {
                    old_start: adjusted_old_start,
                    old_lines: old_idx - adjusted_old_start,
                    new_start: adjusted_new_start,
                    new_lines: new_idx - adjusted_new_start,
                    operations,
                });

                // Move to the next set of changes
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

    /// Helper method to find the next matching line
    fn find_next_match(
        &self,
        old_lines: &[&str],
        new_lines: &[&str],
        max_look_ahead: usize,
    ) -> (usize, usize) {
        if old_lines.is_empty() || new_lines.is_empty() {
            return (old_lines.len(), new_lines.len());
        }

        // Try to find the best match within the look-ahead window
        let old_look_ahead = min(old_lines.len(), max_look_ahead);
        let new_look_ahead = min(new_lines.len(), max_look_ahead);

        for i in 1..=old_look_ahead {
            for j in 1..=new_look_ahead {
                if old_lines[i - 1] == new_lines[j - 1] {
                    return (i - 1, j - 1);
                }
            }
        }

        // If no match is found, return default values
        (min(old_lines.len(), 1), min(new_lines.len(), 1))
    }

    /// Modulo operation that handles negative numbers correctly
    fn modulo(a: isize, b: usize) -> usize {
        let b = b as isize;
        (((a % b) + b) % b) as usize
    }

    /// Implementation of Myers' diff algorithm to generate an edit script
    /// between two sequences
    fn myers_diff<T: PartialEq>(&self, old: &[T], new: &[T]) -> Vec<Edit> {
        // If either sequence is empty, handle it directly
        if old.is_empty() {
            if !new.is_empty() {
                return vec![Edit::Insert(new.len())];
            }
            return Vec::new();
        } else if new.is_empty() {
            return vec![Edit::Delete(old.len())];
        }

        // Apply Myers algorithm to find shortest edit path
        let n = old.len();
        let m = new.len();
        let edit_path = self.shortest_edit_path(old, new);

        // Convert edit path to edit script
        let mut script = Vec::new();
        let mut i = 0;
        let mut j = 0;

        for (next_i, next_j) in edit_path {
            if next_i > i && next_j > j {
                // Diagonal move (equal elements)
                let equal_count = min(next_i - i, next_j - j);
                script.push(Edit::Equal(equal_count));
                i += equal_count;
                j += equal_count;
            }

            if next_i > i {
                // Horizontal move (deletion from old)
                script.push(Edit::Delete(next_i - i));
                i = next_i;
            }

            if next_j > j {
                // Vertical move (insertion from new)
                script.push(Edit::Insert(next_j - j));
                j = next_j;
            }
        }

        // If we haven't reached the end, add final operations
        if i < n && j < m {
            let equal_count = min(n - i, m - j);
            script.push(Edit::Equal(equal_count));
            i += equal_count;
            j += equal_count;
        }

        if i < n {
            script.push(Edit::Delete(n - i));
        }

        if j < m {
            script.push(Edit::Insert(m - j));
        }

        // Merge consecutive operations of the same type
        let mut merged_script = Vec::new();
        let mut current_edit: Option<Edit> = None;

        for edit in script {
            match (current_edit, edit) {
                (Some(Edit::Equal(c1)), Edit::Equal(c2)) => {
                    current_edit = Some(Edit::Equal(c1 + c2));
                }
                (Some(Edit::Insert(c1)), Edit::Insert(c2)) => {
                    current_edit = Some(Edit::Insert(c1 + c2));
                }
                (Some(Edit::Delete(c1)), Edit::Delete(c2)) => {
                    current_edit = Some(Edit::Delete(c1 + c2));
                }
                (Some(e), new_e) => {
                    merged_script.push(e);
                    current_edit = Some(new_e);
                }
                (None, e) => {
                    current_edit = Some(e);
                }
            }
        }

        if let Some(edit) = current_edit {
            merged_script.push(edit);
        }

        merged_script
    }

    /// Find the shortest edit path between two sequences using Myers' diff algorithm
    fn shortest_edit_path<T: PartialEq>(&self, old: &[T], new: &[T]) -> Vec<(usize, usize)> {
        let n = old.len();
        let m = new.len();
        let max_edit = n + m;

        // Initialize vector to store furthest reaching D-paths
        let mut v = vec![0; 2 * max_edit + 1];
        // Store the best path for each k-line
        let mut paths: Vec<Vec<(usize, usize)>> = vec![Vec::new(); 2 * max_edit + 1];

        for d in 0..=max_edit {
            for k in (-(d as isize)..=(d as isize)).step_by(2) {
                let k_index = (k + max_edit as isize) as usize;

                // Decide whether to go down or right
                let mut x =
                    if k == -(d as isize) || (k != d as isize && v[k_index - 1] < v[k_index + 1]) {
                        v[k_index + 1] // Go down
                    } else {
                        v[k_index - 1] + 1 // Go right
                    };

                let mut y = (x as isize - k) as usize;

                // Store the starting point
                let mut path =
                    if k == -(d as isize) || (k != d as isize && v[k_index - 1] < v[k_index + 1]) {
                        paths[k_index + 1].clone()
                    } else {
                        let mut p = paths[k_index - 1].clone();
                        p.push((
                            v[k_index - 1],
                            ((v[k_index - 1] as isize) - (k - 1)) as usize,
                        ));
                        p
                    };

                // Follow diagonal moves (snakes)
                while x < n && y < m && old[x] == new[y] {
                    x += 1;
                    y += 1;
                }

                // Save furthest reaching x for this k-line
                v[k_index] = x;

                // Save the path
                path.push((x, y));
                paths[k_index] = path;

                // Check if we've reached the bottom right corner
                if x >= n && y >= m {
                    return paths[k_index].clone();
                }
            }
        }

        // This should not happen if the algorithm is correctly implemented
        Vec::new()
    }
}

/// Change type representing operations for generating patches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Equal(usize, usize),  // (old_index, new_index)
    Delete(usize, usize), // (old_index, count)
    Insert(usize, usize), // (new_index, count)
}

/// Edit operation representing the type of change required
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    /// Elements are equal (no change needed)
    Equal(usize),
    /// Insert elements from the new sequence
    Insert(usize),
    /// Delete elements from the old sequence
    Delete(usize),
}

/// Myers' diff algorithm. Diff `e`, between indices `e0` (included)
/// and `e1` (excluded), on the one hand, and `f`, between indices
/// `f0` (included)` and `f1` (excluded), on the other hand.
pub fn diff<S: Index<usize> + ?Sized, T: Index<usize> + ?Sized, D: Diff>(
    d: &mut D,
    e: &S,
    e0: usize,
    e1: usize,
    f: &T,
    f0: usize,
    f1: usize,
) -> Result<(), D::Error>
where
    T::Output: PartialEq<S::Output>,
{
    diff_offsets(d, e, e0, e1, f, f0, f1)?;
    d.finish()
}

/// Implementation of Myers algorithm for the Diff trait
pub(crate) fn diff_offsets<D: Diff + ?Sized, S: Index<usize> + ?Sized, T: Index<usize> + ?Sized>(
    diff: &mut D,
    e: &S,
    i: usize,
    i_: usize,
    f: &T,
    j: usize,
    j_: usize,
) -> Result<(), D::Error>
where
    T::Output: PartialEq<S::Output>,
{
    if i_ > i && j_ > j {
        let n = i_ - i;
        let m = j_ - j;
        let l = (n + m) as isize;
        let z = (2 * min(n, m) + 2) as usize;
        let w = n as isize - m as isize;
        let mut g = vec![0; z as usize];
        let mut p = vec![0; z as usize];
        for h in 0..=(l / 2 + l % 2) {
            macro_rules! search {
                ($e: expr, $c: expr, $d: expr) => {
                    let (k0, k1) = {
                        let (m, n) = (m as isize, n as isize);
                        (-(h - 2*max(0, h - m)), h-2*max(0, h-n)+1)
                    };
                    for k in (k0..k1).step_by(2) {
                        let mut a: usize = if k == -h || k != h && $c[Differ::modulo(k-1, z)] < $c[Differ::modulo(k+1, z)] {
                            $c[Differ::modulo(k+1, z)]
                        } else {
                            $c[Differ::modulo(k-1, z)] + 1
                        };
                        let mut b = (a as isize - k) as usize;
                        let (s, t) = (a, b);
                        while a < n && b < m && {
                            let (e_i, f_i) = if $e { (a, b) } else { (n - a - 1, m - b - 1) };
                            f[j + f_i] == e[i + e_i]
                        } {
                            a += 1;
                            b += 1;
                        }
                        $c[Differ::modulo(k, z)] = a;
                        let bound = if $e { h-1 } else { h };
                        if (l%2 == 1) == $e
                            && w-k >= -bound && w-k <= bound
                            && $c[Differ::modulo(k, z)]+$d[Differ::modulo(w-k, z)] >= n
                        {
                            let (x, y, u, v) = if $e {
                                (s, t, a, b)
                            } else {
                                (n-a, m-b, n-s, m-t)
                            };
                            if h + bound > 1 || (x != u && y != v) {
                                diff_offsets(diff, e, i, i+x, f, j, j+y)?;
                                if x != u {
                                    diff.equal(i + x, j + y, u-x)?;
                                }
                                diff_offsets(diff, e, i+u, i_, f, j+v, j_)?;
                                return Ok(())
                            } else if m > n {
                                diff.equal(i, j, n)?;
                                diff.insert(i+n, j+n, m-n)?;
                                return Ok(())
                            } else if m < n {
                                diff.equal(i, j, m)?;
                                diff.delete(i+m, n-m, j+m)?;
                                return Ok(())
                            } else {
                                return Ok(())
                            }
                        }
                    }
                }
            }
            search!(true, g, p);
            search!(false, p, g);
        }
    }

    // Handle special cases
    if i_ > i {
        diff.delete(i, i_ - i, j)?
    } else if j_ > j {
        diff.insert(i, j, j_ - j)?
    }

    Ok(())
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

    #[test]
    fn test_myers_modulo() {
        assert_eq!(Differ::modulo(-11, 10), 9);
        assert_eq!(Differ::modulo(23, 7), 2);
        assert_eq!(Differ::modulo(-12, 6), 0);
    }
}
