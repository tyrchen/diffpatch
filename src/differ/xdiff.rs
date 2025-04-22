use crate::differ::{Change, DiffAlgorithm};
use crate::{Differ, Patch};
use std::cmp::{max, min};

use super::{create_patch, handle_empty_files, process_changes_to_chunks};

// Constants based on xdiffi.c
const XDL_MAX_COST_MIN: usize = 256;
const XDL_HEUR_MIN_COST: usize = 256;
const XDL_SNAKE_CNT: usize = 20;
const XDL_K_HEUR: usize = 4;
// Sentinel value for K-vectors, equivalent to -1 in C
const NEG_ONE: isize = -1;
// Sentinel value for K-vectors, equivalent to XDL_LINE_MAX in C
const LINE_MAX: isize = isize::MAX / 2; // Use a large value, avoid overflow

/// Represents the algorithm environment/heuristic parameters
#[derive(Clone, Copy)]
struct AlgoEnv {
    mxcost: usize,
    snake_cnt: usize,
    heur_min: usize,
    need_min: bool,
}

/// Represents a potential split point found by the algorithm
#[derive(Clone, Copy, Debug)]
struct SplitPoint {
    old_idx: usize, // i1 in C
    new_idx: usize, // i2 in C
    min_lo: bool,   // Flag indicating if minimal check needed for the first part
    min_hi: bool,   // Flag indicating if minimal check needed for the second part
}

/// XDiff differ implementation based on LibXDiff algorithm
pub struct XDiffDiffer<'a> {
    differ: &'a Differ,
}

impl<'a> XDiffDiffer<'a> {
    /// Create a new XDiffDiffer from a base Differ instance
    pub fn new(differ: &'a Differ) -> Self {
        Self { differ }
    }

    /// Implementation of the XDiff algorithm based on xdl_do_diff and xdl_recs_cmp
    fn xdiff(&self, old_lines: &[&str], new_lines: &[&str]) -> Vec<Change> {
        let old_len = old_lines.len();
        let new_len = new_lines.len();

        // Create hash vectors for faster comparison
        let old_hash: Vec<u64> = old_lines.iter().map(|&line| self.hash_line(line)).collect();
        let new_hash: Vec<u64> = new_lines.iter().map(|&line| self.hash_line(line)).collect();

        // Initialize change markers
        // Note: C uses 1-based indexing in rchg internally, but markers are applied to 0-based lines.
        // Rust uses 0-based indexing consistently.
        let mut old_changes = vec![false; old_len];
        let mut new_changes = vec![false; new_len];

        // Allocate K vectors (forward and backward paths)
        let ndiags = old_len + new_len + 3;
        let k_vec_size = 2 * ndiags + 2; // Total size needed
        let mut kvd = vec![0isize; k_vec_size]; // Store as isize to handle potential large coords

        // Calculate the offset for indexing K-vectors (diagonals can be negative)
        // k = old_idx - new_idx
        // offset allows mapping k to a non-negative vec index: index = k + offset
        let k_offset = new_len + 1; // Matches `xe->xdf2.nreff + 1` in C

        // Calculate heuristic parameters
        // bogosqrt approximation: sqrt(N) - adjust if needed
        let approx_sqrt = (ndiags as f64).sqrt() as usize;
        let mxcost = max(approx_sqrt, XDL_MAX_COST_MIN);
        let env = AlgoEnv {
            mxcost,
            snake_cnt: XDL_SNAKE_CNT,
            heur_min: XDL_HEUR_MIN_COST,
            need_min: false, // TODO: Integrate XDF_NEED_MINIMAL flag if available
        };

        // Run the recursive comparison
        let result = self.compare_recursive(
            &old_hash,
            &mut old_changes,
            0,
            old_len,
            &new_hash,
            &mut new_changes,
            0,
            new_len,
            &mut kvd,
            k_offset,
            ndiags,
            env,
        );

        if result.is_err() {
            // Handle error case - maybe return empty changes or panic
            eprintln!("XDiff algorithm failed.");
            return vec![];
        }

        // Build change script from the markers
        self.build_script(&old_changes, &new_changes, old_len, new_len)
    }

    /// Recursive comparison function based on xdl_recs_cmp
    #[allow(clippy::too_many_arguments)]
    fn compare_recursive(
        &self,
        old_hash: &[u64],
        old_changes: &mut [bool],
        mut old_start: usize,
        mut old_end: usize,
        new_hash: &[u64],
        new_changes: &mut [bool],
        mut new_start: usize,
        mut new_end: usize,
        kvd: &mut [isize], // Combined buffer for forward and backward vectors
        k_offset: usize,   // Offset to map diagonal k to vector index
        ndiags: usize,     // Size of one K-vector part (for slicing)
        env: AlgoEnv,
    ) -> Result<(), ()> {
        // Shrink the box by skipping common prefixes
        while old_start < old_end
            && new_start < new_end
            && old_hash[old_start] == new_hash[new_start]
        {
            old_start += 1;
            new_start += 1;
        }
        // Shrink the box by skipping common suffixes
        while old_start < old_end
            && new_start < new_end
            && old_hash[old_end - 1] == new_hash[new_end - 1]
        {
            old_end -= 1;
            new_end -= 1;
        }

        // Base cases: If one dimension is empty, mark all lines in the other as changed
        if old_start == old_end {
            if new_start < new_end {
                // Use iterator slice assignment for conciseness
                new_changes[new_start..new_end]
                    .iter_mut()
                    .for_each(|c| *c = true);
            }
            return Ok(());
        } else if new_start == new_end {
            if old_start < old_end {
                // Use iterator slice assignment for conciseness
                old_changes[old_start..old_end]
                    .iter_mut()
                    .for_each(|c| *c = true);
            }
            return Ok(());
        }

        // Divide: Find the split point using the core algorithm
        let (kvdf_slice, kvdb_slice) = kvd.split_at_mut(ndiags);
        let split_result = self.find_split_point(
            old_hash, old_start, old_end, new_hash, new_start, new_end, kvdf_slice, kvdb_slice,
            k_offset, env,
        );

        match split_result {
            Ok(split) => {
                // Conquer: Recursively compare the sub-problems
                // Note: Pass split.min_lo/min_hi as need_min for subproblems
                let env_lo = AlgoEnv {
                    need_min: split.min_lo,
                    ..env
                };
                let env_hi = AlgoEnv {
                    need_min: split.min_hi,
                    ..env
                };

                self.compare_recursive(
                    old_hash,
                    old_changes,
                    old_start,
                    split.old_idx,
                    new_hash,
                    new_changes,
                    new_start,
                    split.new_idx,
                    kvd, // Pass the full buffer again
                    k_offset,
                    ndiags,
                    env_lo,
                )?;

                self.compare_recursive(
                    old_hash,
                    old_changes,
                    split.old_idx,
                    old_end,
                    new_hash,
                    new_changes,
                    split.new_idx,
                    new_end,
                    kvd, // Pass the full buffer again
                    k_offset,
                    ndiags,
                    env_hi,
                )?;

                Ok(())
            }
            Err(_) => {
                // Handle split error - mark remaining as changed? Or propagate error?
                // For now, propagate error.
                Err(())
            }
        }
    }

    /// Core splitting algorithm based on xdl_split
    #[allow(clippy::too_many_arguments)]
    fn find_split_point(
        &self,
        old_hash: &[u64],
        old_start: usize,
        old_end: usize,
        new_hash: &[u64],
        new_start: usize,
        new_end: usize,
        kvdf: &mut [isize], // Forward K-vector part
        kvdb: &mut [isize], // Backward K-vector part
        k_offset: usize,    // Offset for diagonal indexing
        env: AlgoEnv,
    ) -> Result<SplitPoint, ()> {
        // Cast usize to isize for calculations involving diagonals
        let old_start_i = old_start as isize;
        let old_end_i = old_end as isize;
        let new_start_i = new_start as isize;
        let new_end_i = new_end as isize;

        // Calculate diagonal range and midpoints
        let dmin: isize = old_start_i - new_end_i;
        let dmax: isize = old_end_i - new_start_i;
        let fmid: isize = old_start_i - new_start_i;
        let bmid: isize = old_end_i - new_end_i;
        let odd: bool = (fmid - bmid) % 2 != 0;

        // K-vector boundaries for forward and backward searches
        let mut fmin: isize = fmid;
        let mut fmax: isize = fmid;
        let mut bmin: isize = bmid;
        let mut bmax: isize = bmid;

        // Initialize K-vectors at midpoints
        // Map diagonal k to vector index: idx = k + k_offset
        kvdf[(fmid + k_offset as isize) as usize] = old_start_i;
        kvdb[(bmid + k_offset as isize) as usize] = old_end_i;

        // Initialize sentinel values for boundaries
        kvdf[(fmid - 1 + k_offset as isize) as usize] = NEG_ONE;
        kvdf[(fmid + 1 + k_offset as isize) as usize] = NEG_ONE;
        kvdb[(bmid - 1 + k_offset as isize) as usize] = LINE_MAX;
        kvdb[(bmid + 1 + k_offset as isize) as usize] = LINE_MAX;

        for ec in 1.. {
            // Edit cost
            let mut got_snake = false;

            // --- Forward Pass ---
            // Extend diagonal domain
            if fmin > dmin {
                fmin -= 1;
                kvdf[(fmin - 1 + k_offset as isize) as usize] = NEG_ONE; // Extend boundary sentinel
            } else {
                fmin += 1;
            }
            if fmax < dmax {
                fmax += 1;
                kvdf[(fmax + 1 + k_offset as isize) as usize] = NEG_ONE; // Extend boundary sentinel
            } else {
                fmax -= 1;
            }

            // Iterate through forward diagonals
            for d in (fmin..=fmax).rev().step_by(2) {
                let k_idx = (d + k_offset as isize) as usize;
                let km1_idx = (d - 1 + k_offset as isize) as usize;
                let kp1_idx = (d + 1 + k_offset as isize) as usize;

                let mut i1: isize = // current old_idx
                    if kvdf[km1_idx] >= kvdf[kp1_idx] { kvdf[km1_idx] + 1 } else { kvdf[kp1_idx] };
                let prev_i1 = i1;
                let mut i2: isize = i1 - d; // current new_idx

                // Follow the snake (diagonal match)
                while i1 < old_end_i
                    && i2 < new_end_i
                    && old_hash[i1 as usize] == new_hash[i2 as usize]
                {
                    i1 += 1;
                    i2 += 1;
                }

                if (i1 - prev_i1) as usize > env.snake_cnt {
                    got_snake = true;
                }
                kvdf[k_idx] = i1;

                // Check for overlap with backward path
                if odd && d >= bmin && d <= bmax {
                    let bk_idx = (d + k_offset as isize) as usize;
                    if kvdb[bk_idx] <= i1 {
                        return Ok(SplitPoint {
                            old_idx: i1 as usize,
                            new_idx: i2 as usize,
                            min_lo: true,
                            min_hi: true,
                        });
                    }
                }
            }

            // --- Backward Pass ---
            // Extend diagonal domain
            if bmin > dmin {
                bmin -= 1;
                kvdb[(bmin - 1 + k_offset as isize) as usize] = LINE_MAX;
            } else {
                bmin += 1;
            }
            if bmax < dmax {
                bmax += 1;
                kvdb[(bmax + 1 + k_offset as isize) as usize] = LINE_MAX;
            } else {
                bmax -= 1;
            }

            // Iterate through backward diagonals
            for d in (bmin..=bmax).rev().step_by(2) {
                let k_idx = (d + k_offset as isize) as usize;
                let km1_idx = (d - 1 + k_offset as isize) as usize;
                let kp1_idx = (d + 1 + k_offset as isize) as usize;

                let mut i1: isize = // current old_idx (from end)
                    if kvdb[km1_idx] < kvdb[kp1_idx] { kvdb[km1_idx] } else { kvdb[kp1_idx] - 1 };
                let prev_i1 = i1;
                let mut i2: isize = i1 - d; // current new_idx (from end)

                // Follow the snake backward
                while i1 > old_start_i
                    && i2 > new_start_i
                    && old_hash[(i1 - 1) as usize] == new_hash[(i2 - 1) as usize]
                {
                    i1 -= 1;
                    i2 -= 1;
                }

                if (prev_i1 - i1) as usize > env.snake_cnt {
                    got_snake = true;
                }
                kvdb[k_idx] = i1;

                // Check for overlap with forward path
                if !odd && d >= fmin && d <= fmax {
                    let fk_idx = (d + k_offset as isize) as usize;
                    if i1 <= kvdf[fk_idx] {
                        return Ok(SplitPoint {
                            old_idx: i1 as usize,
                            new_idx: i2 as usize,
                            min_lo: true,
                            min_hi: true,
                        });
                    }
                }
            }

            // --- Heuristics and Cutoffs (if not need_min) ---
            if !env.need_min {
                // Heuristic: Check for good snakes if cost exceeds threshold
                if got_snake && ec > env.heur_min {
                    let mut best_v: isize = 0;
                    let mut best_split: Option<SplitPoint> = None;

                    // Check forward diagonals for interesting paths
                    for d in (fmin..=fmax).rev().step_by(2) {
                        let dd = (d - fmid).abs(); // Distance from middle diagonal
                        let i1 = kvdf[(d + k_offset as isize) as usize];
                        let i2 = i1 - d;
                        let v = (i1 - old_start_i) + (i2 - new_start_i) - dd; // Score

                        if v > (XDL_K_HEUR * ec) as isize
                            && v > best_v
                            && old_start_i + env.snake_cnt as isize <= i1
                            && i1 < old_end_i
                            && new_start_i + env.snake_cnt as isize <= i2
                            && i2 < new_end_i
                        {
                            // Check if it's actually a snake end
                            let mut is_snake = true;
                            for k in 1..=env.snake_cnt {
                                if i1 < k as isize
                                    || i2 < k as isize
                                    || old_hash[(i1 - k as isize) as usize]
                                        != new_hash[(i2 - k as isize) as usize]
                                {
                                    is_snake = false;
                                    break;
                                }
                            }
                            if is_snake {
                                best_v = v;
                                best_split = Some(SplitPoint {
                                    old_idx: i1 as usize,
                                    new_idx: i2 as usize,
                                    min_lo: true,
                                    min_hi: false,
                                });
                            }
                        }
                    }
                    if let Some(split) = best_split {
                        return Ok(split);
                    }

                    // Check backward diagonals for interesting paths
                    best_v = 0; // Reset best_v
                    best_split = None;
                    for d in (bmin..=bmax).rev().step_by(2) {
                        let dd = (d - bmid).abs();
                        let i1 = kvdb[(d + k_offset as isize) as usize];
                        let i2 = i1 - d;
                        let v = (old_end_i - i1) + (new_end_i - i2) - dd;

                        if v > (XDL_K_HEUR * ec) as isize
                            && v > best_v
                            && old_start_i < i1
                            && i1 <= old_end_i - env.snake_cnt as isize
                            && new_start_i < i2
                            && i2 <= new_end_i - env.snake_cnt as isize
                        {
                            // Check if it's actually a snake start (looking forward)
                            let mut is_snake = true;
                            for k in 0..env.snake_cnt {
                                if i1 + k as isize >= old_end_i
                                    || i2 + k as isize >= new_end_i
                                    || old_hash[(i1 + k as isize) as usize]
                                        != new_hash[(i2 + k as isize) as usize]
                                {
                                    is_snake = false;
                                    break;
                                }
                            }
                            if is_snake {
                                best_v = v;
                                best_split = Some(SplitPoint {
                                    old_idx: i1 as usize,
                                    new_idx: i2 as usize,
                                    min_lo: false,
                                    min_hi: true,
                                });
                            }
                        }
                    }
                    if let Some(split) = best_split {
                        return Ok(split);
                    }
                }

                // Cutoff: Max cost reached, find furthest reaching point
                if ec >= env.mxcost {
                    let mut fbest_val = -1;
                    let mut fbest_i1 = NEG_ONE;

                    for d in (fmin..=fmax).rev().step_by(2) {
                        let mut i1 = min(kvdf[(d + k_offset as isize) as usize], old_end_i);
                        let mut i2 = i1 - d;
                        if i2 > new_end_i {
                            // Adjust if outside bounds
                            i1 = new_end_i + d;
                            i2 = new_end_i;
                        }
                        if fbest_val < i1 + i2 {
                            fbest_val = i1 + i2;
                            fbest_i1 = i1;
                        }
                    }

                    let mut bbest_val = LINE_MAX;
                    let mut bbest_i1 = LINE_MAX;

                    for d in (bmin..=bmax).rev().step_by(2) {
                        let mut i1 = max(old_start_i, kvdb[(d + k_offset as isize) as usize]);
                        let mut i2 = i1 - d;
                        if i2 < new_start_i {
                            // Adjust if outside bounds
                            i1 = new_start_i + d;
                            i2 = new_start_i;
                        }
                        if i1 + i2 < bbest_val {
                            bbest_val = i1 + i2;
                            bbest_i1 = i1;
                        }
                    }

                    // Compare forward best and backward best
                    if (old_end_i + new_end_i - bbest_val)
                        < (fbest_val - (old_start_i + new_start_i))
                    {
                        // Forward path reached further relatively
                        return Ok(SplitPoint {
                            old_idx: fbest_i1 as usize,
                            new_idx: (fbest_val - fbest_i1) as usize,
                            min_lo: true,
                            min_hi: false,
                        });
                    } else {
                        // Backward path reached further relatively
                        return Ok(SplitPoint {
                            old_idx: bbest_i1 as usize,
                            new_idx: (bbest_val - bbest_i1) as usize,
                            min_lo: false,
                            min_hi: true,
                        });
                    }
                }
            }
            // If need_min is true, we skip heuristics and continue until overlap or error
            else if env.need_min && ec >= env.mxcost {
                // Avoid infinite loop if need_min is true and no overlap found within cost limit
                // This condition isn't explicitly in C's xdl_split loop, but needed for safety
                eprintln!("XDiff: Max cost reached in minimal mode without finding overlap.");
                return Err(()); // Indicate failure
            }
        } // End main loop (ec)

        // Should not be reached if logic is correct, but needed for compiler
        Err(())
    }

    /// Simple hash function for lines (FNV-1a)
    fn hash_line(&self, line: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in line.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Build a change script from the comparison results (marks changes)
    /// This function seems compatible with the new approach using bool arrays.
    fn build_script(
        &self,
        old_changes: &[bool],
        new_changes: &[bool],
        old_len: usize,
        new_len: usize,
    ) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut i1 = 0; // old index
        let mut i2 = 0; // new index

        while i1 < old_len || i2 < new_len {
            if i1 < old_len && i2 < new_len && !old_changes[i1] && !new_changes[i2] {
                // Equal lines (find run)
                let start_i1 = i1;
                let start_i2 = i2;
                while i1 < old_len && i2 < new_len && !old_changes[i1] && !new_changes[i2] {
                    // In the original Myers/XDiff context, we'd check hash equality here,
                    // but rely on the change markers generated by compare_recursive.
                    i1 += 1;
                    i2 += 1;
                }
                // Add individual Equal changes for process_changes_to_chunks
                for k in 0..(i1 - start_i1) {
                    changes.push(Change::Equal(start_i1 + k, start_i2 + k));
                }
            } else {
                // Find consecutive changed lines in old
                let start_del = i1;
                while i1 < old_len && old_changes[i1] {
                    i1 += 1;
                }
                if i1 > start_del {
                    changes.push(Change::Delete(start_del, i1 - start_del));
                }

                // Find consecutive changed lines in new
                let start_ins = i2;
                while i2 < new_len && new_changes[i2] {
                    i2 += 1;
                }
                if i2 > start_ins {
                    changes.push(Change::Insert(start_ins, i2 - start_ins));
                }

                // If we haven't advanced but there are still lines, it means
                // we hit the end of one file's changes but not the other's sequence.
                // The loop condition `i1 < old_len || i2 < new_len` handles advancing.
                if i1 == start_del && i2 == start_ins {
                    // This should only happen if we hit the end of both files simultaneously
                    // after processing changes, or if there's an error state.
                    // Break to prevent infinite loop if something went wrong.
                    if i1 >= old_len && i2 >= new_len {
                        break;
                    } else {
                        // If only one file has remaining lines, they must be changes
                        // that weren't marked (error in compare_recursive?) or we are at the end.
                        if i1 < old_len && !old_changes[i1] {
                            i1 += 1;
                        }
                        if i2 < new_len && !new_changes[i2] {
                            i2 += 1;
                        }
                        // Avoid infinite loops if stuck on unmarked changes
                        if i1 == start_del && i2 == start_ins {
                            break;
                        }
                    }
                }
            }
        }
        // Post-processing (merging) is handled outside this function if needed,
        // but process_changes_to_chunks expects individual changes.
        // The old post_process_changes function is removed as it merged changes
        // which is not the desired input for process_changes_to_chunks.
        changes
    }

    // Removed compare_files
    // Removed find_longest_common_subsequence
    // Removed post_process_changes (merging logic interferes with chunk processing)
}

impl DiffAlgorithm for XDiffDiffer<'_> {
    /// Generate a patch between the old and new content using the XDiff algorithm
    fn generate(&self) -> Patch {
        let old_lines: Vec<&str> = self.differ.old.lines().collect();
        let new_lines: Vec<&str> = self.differ.new.lines().collect();

        // Handle special cases for empty files
        if let Some(patch) = handle_empty_files(&old_lines, &new_lines) {
            return patch;
        }

        // Find the line-level changes using the XDiff implementation
        let changes = self.xdiff(&old_lines, &new_lines);

        // Process the changes into chunks with context
        let chunks =
            process_changes_to_chunks(&changes, &old_lines, &new_lines, self.differ.context_lines);

        // Create the final patch
        create_patch(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{differ::DiffAlgorithmType, Patcher};

    // Keeping existing tests - they should still pass if the algorithm is correct,
    // though the exact chunking might differ slightly from the previous LCS impl.

    #[test]
    fn test_simple_xdiff() {
        let old = "line1\\nline2\\nline3";
        let new = "line1\\nline2\\nline3";

        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        // Check if the generated patch can revert the change
        // Since it's identical, the patch should be empty
        assert!(
            patch.chunks.is_empty(),
            "Patch should be empty for identical files"
        );
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, old); // Applying empty patch should yield original
    }

    #[test]
    fn test_xdiff_add_line() {
        let old = "line1\\nline2\\nline3";
        let new = "line1\\nline2\\nline3\\nline4";

        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_xdiff_remove_line() {
        let old = "line1\\nline2\\nline3";
        let new = "line1\\nline3";

        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_xdiff_complex_changes() {
        let old = "line1\\nline2\\nline3\\nline4\\nline5";
        let new = "line1\\nmodified\\nline3\\nadded\\nline5";

        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let xdiff = XDiffDiffer::new(&differ);
        let patch = xdiff.generate();

        assert!(!patch.chunks.is_empty());
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_xdiff_trailing_newline_change() {
        let old = "a\\nb\\nc";
        let new = "a\\nb\\nc\\n";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let patch = differ.generate();
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);

        let old = "a\\nb\\nc\\n";
        let new = "a\\nb\\nc";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let patch = differ.generate();
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_xdiff_leading_change() {
        let old = "a\\nb\\nc";
        let new = "x\\na\\nb\\nc";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let patch = differ.generate();
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_xdiff_middle_change() {
        let old = "a\\nb\\nc\\nd";
        let new = "a\\nx\\ny\\nd";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let patch = differ.generate();
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_empty_to_non_empty() {
        let old = "";
        let new = "line1\\nline2";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let patch = differ.generate();
        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn test_non_empty_to_empty() {
        let old = "line1\\nline2";
        let new = "";
        let differ = Differ::new_with_algorithm(old, new, DiffAlgorithmType::XDiff);
        let patch = differ.generate();
        assert_eq!(patch.chunks.len(), 1);
        let result = Patcher::new(patch).apply(old, false).unwrap();
        assert_eq!(result, new);
    }
}
