use diffpatch::{myers_diff, Diff};
use std::fmt;

/// A simple diff collector that outputs changes in a custom format
struct CustomDiffer {
    changes: Vec<Change>,
}

/// Represents a type of change between sequences
enum Change {
    Equal(usize, usize, usize),
    Delete(usize, usize, usize),
    Insert(usize, usize, usize),
}

impl CustomDiffer {
    fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    fn print_changes(&self, a: &[&str], b: &[&str]) {
        println!("Changes between sequences:");
        println!("-------------------------");

        for change in &self.changes {
            match change {
                Change::Equal(old_idx, new_idx, count) => {
                    println!("Equal: {} lines", count);
                    for i in 0..*count {
                        println!("  {}: {}", old_idx + i, a[*old_idx + i]);
                    }
                }
                Change::Delete(old_idx, count, _) => {
                    println!("Delete: {} lines", count);
                    for i in 0..*count {
                        println!("- {}: {}", old_idx + i, a[*old_idx + i]);
                    }
                }
                Change::Insert(old_idx, new_idx, count) => {
                    println!("Insert: {} lines", count);
                    for i in 0..*count {
                        println!("+ {}: {}", new_idx + i, b[*new_idx + i]);
                    }
                }
            }
            println!();
        }
    }
}

impl Diff for CustomDiffer {
    type Error = CustomDiffError;

    fn equal(&mut self, old_idx: usize, new_idx: usize, count: usize) -> Result<(), Self::Error> {
        self.changes.push(Change::Equal(old_idx, new_idx, count));
        Ok(())
    }

    fn delete(&mut self, old_idx: usize, count: usize, new_idx: usize) -> Result<(), Self::Error> {
        self.changes.push(Change::Delete(old_idx, count, new_idx));
        Ok(())
    }

    fn insert(&mut self, old_idx: usize, new_idx: usize, count: usize) -> Result<(), Self::Error> {
        self.changes.push(Change::Insert(old_idx, new_idx, count));
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug)]
struct CustomDiffError(String);

impl fmt::Display for CustomDiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Custom diff error: {}", self.0)
    }
}

impl std::error::Error for CustomDiffError {}

fn main() {
    // Define two sequences to compare
    let a = "The quick brown fox\njumps over the\nlazy dog.\nThis is a test.\nAnother line.";
    let b = "The quick brown fox\njumped over the\nlazy dog!\nThis is a test.\nA new line.\nAnother line.";

    let a_lines: Vec<&str> = a.lines().collect();
    let b_lines: Vec<&str> = b.lines().collect();

    // Create a custom differ
    let mut differ = CustomDiffer::new();

    // Apply Myers diff algorithm
    match myers_diff(
        &mut differ,
        &a_lines,
        0,
        a_lines.len(),
        &b_lines,
        0,
        b_lines.len(),
    ) {
        Ok(_) => {
            println!("Successfully computed diff\n");
            // Print the results
            differ.print_changes(&a_lines, &b_lines);
        }
        Err(e) => {
            eprintln!("Error computing diff: {}", e);
        }
    }

    // You can also compare other types of data
    println!("\nComparing lists of integers:");
    println!("-------------------------");
    let nums_a = vec![1, 2, 3, 4, 5, 6];
    let nums_b = vec![1, 2, 10, 4, 8, 6, 7];

    let mut differ = CustomDiffer::new();

    if myers_diff(
        &mut differ,
        &nums_a,
        0,
        nums_a.len(),
        &nums_b,
        0,
        nums_b.len(),
    )
    .is_ok()
    {
        println!("Diff between integer sequences:");
        for change in differ.changes {
            match change {
                Change::Equal(old_idx, new_idx, count) => {
                    print!("Equal:   ");
                    for i in 0..count {
                        print!("{} ", nums_a[old_idx + i]);
                    }
                    println!();
                }
                Change::Delete(old_idx, count, _) => {
                    print!("Delete:  ");
                    for i in 0..count {
                        print!("{} ", nums_a[old_idx + i]);
                    }
                    println!();
                }
                Change::Insert(_, new_idx, count) => {
                    print!("Insert:  ");
                    for i in 0..count {
                        print!("{} ", nums_b[new_idx + i]);
                    }
                    println!();
                }
            }
        }
    }
}
