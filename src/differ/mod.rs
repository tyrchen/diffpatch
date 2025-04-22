use crate::Diff;
use crate::Patch;
use std::ops::Index;

mod myers;
mod naive;

pub use myers::MyersDiffer;
pub use naive::NaiveDiffer;

/// Trait for different diffing algorithms
pub trait DiffAlgorithm {
    /// Generate a patch between the old and new content
    fn generate(&self) -> Patch;
}

/// The base Differ struct that implements diffing algorithms
pub struct Differ {
    pub(crate) old: String,
    pub(crate) new: String,
    pub(crate) context_lines: usize,
}

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

    /// Generate a patch using the naive diffing algorithm (default)
    pub fn generate(&self) -> Patch {
        let naive = NaiveDiffer::new(self);
        naive.generate()
    }
}

/// Implementation of diff for comparing sequences
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
    diff_offsets(d, e, e0, e1 - e0, f, f0, f1 - f0)
}

/// Internal implementation of diff using offsets
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
    let n = i_;
    let m = j_;

    if n == 0 && m == 0 {
        return diff.finish();
    }

    if n > 0 && m == 0 {
        diff.delete(i, n, j)?;
        return diff.finish();
    }

    if n == 0 && m > 0 {
        diff.insert(i, j, m)?;
        return diff.finish();
    }

    if n == 1 && m == 1 {
        if f[j] == e[i] {
            diff.equal(i, j, 1)?;
        } else {
            diff.delete(i, 1, j)?;
            diff.insert(i + 1, j, 1)?;
        }
        return diff.finish();
    }

    // Find the middle snake
    let middle = find_middle_snake(e, i, i_, f, j, j_);
    if middle.d == 0 {
        // The sequences are identical
        diff.equal(i, j, n)?;
        return diff.finish();
    }

    if middle.snake_len == 0 {
        if n == 1 {
            // Delete the single element from e
            diff.delete(i, 1, j)?;
        } else if m == 1 {
            // Insert the single element from f
            diff.insert(i, j, 1)?;
        } else {
            // Divide and conquer
            let i_mid = middle.source_mid;
            let j_mid = middle.target_mid;

            // Recurse on the left segment
            if i_mid > 0 || j_mid > 0 {
                diff_offsets(diff, e, i, i_mid - i, f, j, j_mid - j)?;
            }

            // Recurse on the right segment
            if i_mid < i + i_ || j_mid < j + j_ {
                diff_offsets(diff, e, i_mid, i + i_ - i_mid, f, j_mid, j + j_ - j_mid)?;
            }
        }
        return diff.finish();
    }

    // We have a snake in the middle. Recursively diff the parts to the left and right
    let i_mid = middle.source_mid;
    let j_mid = middle.target_mid;
    let snake_len = middle.snake_len;

    // Recurse left of the snake
    if i_mid > i || j_mid > j {
        diff_offsets(diff, e, i, i_mid - i, f, j, j_mid - j)?;
    }

    // Process the snake itself
    diff.equal(i_mid, j_mid, snake_len)?;

    // Recurse right of the snake
    let i_end = i_mid + snake_len;
    let j_end = j_mid + snake_len;
    if i_end < i + i_ || j_end < j + j_ {
        diff_offsets(diff, e, i_end, i + i_ - i_end, f, j_end, j + j_ - j_end)?;
    }

    diff.finish()
}

/// Stores the result of finding a middle snake
struct MiddleSnake {
    /// Edit distance
    d: usize,
    /// Midpoint in the source sequence
    source_mid: usize,
    /// Midpoint in the target sequence
    target_mid: usize,
    /// Length of the snake (diagonal)
    snake_len: usize,
}

/// Find the middle snake in the shortest edit script (SES)
fn find_middle_snake<S: Index<usize> + ?Sized, T: Index<usize> + ?Sized>(
    e: &S,
    i: usize,
    i_: usize,
    f: &T,
    j: usize,
    j_: usize,
) -> MiddleSnake
where
    T::Output: PartialEq<S::Output>,
{
    let n = i_;
    let m = j_;
    let max_d = n + m;

    // Adjust for odd/even total length
    let delta = n as isize - m as isize;
    let odd = delta % 2 != 0;

    // Initialize arrays for forward and backward search
    let mut v_f = vec![0; 2 * max_d + 1];
    let mut v_b = vec![0; 2 * max_d + 1];

    // Initialize the forward search
    v_f[1 + max_d] = 0;

    // Initialize the backward search
    v_b[1 + max_d] = 0;

    // Search loop, for each edit distance d
    for d in 0..=max_d {
        let d_isize = d as isize;

        // Forward search
        for k in (-d_isize..=d_isize).step_by(2) {
            let k_minus_idx = ((k - 1) + max_d as isize) as usize;
            let k_plus_idx = ((k + 1) + max_d as isize) as usize;
            let k_idx = (k + max_d as isize) as usize;

            let mut x = if k == -d_isize || (k != d_isize && v_f[k_minus_idx] < v_f[k_plus_idx]) {
                v_f[k_plus_idx]
            } else {
                v_f[k_minus_idx] + 1
            };

            let mut y = (x as isize - k) as usize;

            // Follow diagonal as far as possible
            let mut snake_len = 0;
            while x < n && y < m {
                let e_val = &e[i + x];
                let f_val = &f[j + y];
                if !f_val.eq(e_val) {
                    break;
                }
                x += 1;
                y += 1;
                snake_len += 1;
            }

            v_f[k_idx] = x;

            // Check if paths overlap, which means we found the shortest edit script
            if odd && (k - delta).abs() <= (d_isize - 1) {
                let k_delta_idx = ((k - delta) + max_d as isize) as usize;
                if v_f[k_idx] + v_b[k_delta_idx] >= n {
                    return MiddleSnake {
                        d: 2 * d - 1,
                        source_mid: i + x - snake_len,
                        target_mid: j + (x as isize - k - snake_len as isize) as usize,
                        snake_len,
                    };
                }
            }
        }

        // Backward search
        for k in (-d_isize..=d_isize).step_by(2) {
            let k_minus_idx = ((k - 1) + max_d as isize) as usize;
            let k_plus_idx = ((k + 1) + max_d as isize) as usize;
            let k_idx = (k + max_d as isize) as usize;

            let mut x = if k == -d_isize || (k != d_isize && v_b[k_minus_idx] < v_b[k_plus_idx]) {
                v_b[k_plus_idx]
            } else {
                v_b[k_minus_idx] + 1
            };

            let mut y = (x as isize - k) as usize;

            // Follow diagonal as far as possible
            let mut snake_len = 0;
            while x < n && y < m {
                let e_idx = i + (n - x - 1);
                let f_idx = j + (m - y - 1);
                let e_val = &e[e_idx];
                let f_val = &f[f_idx];
                if !f_val.eq(e_val) {
                    break;
                }
                x += 1;
                y += 1;
                snake_len += 1;
            }

            v_b[k_idx] = x;

            // Check if paths overlap, which means we found the shortest edit script
            if !odd && (k + delta).abs() <= d_isize {
                let k_delta_idx = ((k + delta) + max_d as isize) as usize;
                if v_b[k_idx] + v_f[k_delta_idx] >= n {
                    // Calculate the snake coordinates for the backward path
                    let source_mid = n - (v_b[k_idx] - snake_len);
                    let target_mid = m - ((v_b[k_idx] as isize - k - snake_len as isize) as usize);
                    return MiddleSnake {
                        d: 2 * d,
                        source_mid: i + source_mid,
                        target_mid: j + target_mid,
                        snake_len,
                    };
                }
            }
        }
    }

    // If we get here, one sequence is a subset of the other
    MiddleSnake {
        d: max_d,
        source_mid: i,
        target_mid: j,
        snake_len: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Patcher;

    #[test]
    fn test_different_algorithms_produce_valid_patches() {
        let old = "line1\nline2\nline3\nline4";
        let new = "line1\nline2 modified\nline3\nline4";

        // Create a differ
        let differ = Differ::new(old, new);

        // Test naive algorithm
        let naive = NaiveDiffer::new(&differ);
        let naive_patch = naive.generate();
        let naive_result = Patcher::new(naive_patch).apply(old, false).unwrap();
        assert_eq!(naive_result, new);

        // Test Myers algorithm
        let myers = MyersDiffer::new(&differ);
        let myers_patch = myers.generate();
        let myers_result = Patcher::new(myers_patch).apply(old, false).unwrap();
        assert_eq!(myers_result, new);
    }

    #[test]
    fn test_complex_diff_comparison() {
        let old = "This is a test file\nwith multiple lines\nthat will be modified\nin various ways\nto test the diff algorithms\nend of file";
        let new = "This is a changed test file\nwith multiple modified lines\nthat will be completely changed\nand some lines removed\nto test the diff algorithms\nnew line at end\nend of file";

        // Create a differ with more context lines
        let differ = Differ::new(old, new).context_lines(2);

        // Test both algorithms and make sure they both produce valid patches
        let naive = NaiveDiffer::new(&differ);
        let naive_patch = naive.generate();
        let naive_result = Patcher::new(naive_patch).apply(old, false).unwrap();
        assert_eq!(naive_result, new);

        let myers = MyersDiffer::new(&differ);
        let myers_patch = myers.generate();
        let myers_result = Patcher::new(myers_patch).apply(old, false).unwrap();
        assert_eq!(myers_result, new);
    }
}
