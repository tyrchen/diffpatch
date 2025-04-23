![](https://github.com/tyrchen/diffpatch/workflows/build/badge.svg)

# Diffpatch

A Rust library for generating and applying Git-style unified diff patches. See [中文说明](README-zh.md).

## Tutorial

See [Tutorial](./tutorial/en/README.md).

## Features

- Generate patches from original and modified content
- Apply patches to content, both forward and in reverse
- Parse patches from text format
- Support for multi-file patches
- Command-line interface for generating and applying patches
- Efficient Myers diff algorithm implementation
- Customizable diff implementation for any data type

## Installation

Add to your Cargo.toml:

```toml
[dependencies]
diffpatch = { version = "0.1.0", default-features = false }
```

Or install the CLI tool:

```bash
cargo install diffpatch
```

## Library Usage

### Generate a Patch

```rust
use diffpatch::Differ;

fn main() {
    let old_content = "line1\nline2\nline3\nline4";
    let new_content = "line1\nline2 modified\nline3\nline4";

    let differ = Differ::new(old_content, new_content);
    let patch = differ.generate();

    println!("{}", patch);
}
```

### Apply a Patch

```rust
use diffpatch::{Differ, Patcher};

fn main() {
    let old_content = "line1\nline2\nline3\nline4";
    let new_content = "line1\nline2 modified\nline3\nline4";

    // Generate a patch
    let differ = Differ::new(old_content, new_content);
    let patch = differ.generate();

    // Apply it to the original content
    let patcher = Patcher::new(patch);
    let result = patcher.apply(old_content, false).unwrap();

    assert_eq!(result, new_content);
}
```

### Parse a Patch

```rust
use diffpatch::Patch;

fn main() {
    let patch_content = "\
--- a/file.txt
+++ b/file.txt
@@ -1,4 +1,4 @@
 line1
-line2
+line2 modified
 line3
 line4
";

    let patch = Patch::parse(patch_content).unwrap();

    println!("Original file: {}", patch.old_file);
    println!("Modified file: {}", patch.new_file);
    println!("Number of chunks: {}", patch.chunks.len());
}
```

### Using the Myers Diff Algorithm

The library provides a low-level Myers diff algorithm implementation that can be used with any data type:

```rust
use diffpatch::{Diff, myers_diff};

// Implement the Diff trait for your custom differ
struct MyDiffer;

impl Diff for MyDiffer {
    type Error = String;

    fn equal(&mut self, old_idx: usize, new_idx: usize, count: usize) -> Result<(), Self::Error> {
        println!("Equal: {} elements at old index {} and new index {}", count, old_idx, new_idx);
        Ok(())
    }

    fn delete(&mut self, old_idx: usize, count: usize, new_idx: usize) -> Result<(), Self::Error> {
        println!("Delete: {} elements at old index {}", count, old_idx);
        Ok(())
    }

    fn insert(&mut self, old_idx: usize, new_idx: usize, count: usize) -> Result<(), Self::Error> {
        println!("Insert: {} elements at new index {}", count, new_idx);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        println!("Diff complete");
        Ok(())
    }
}

fn main() {
    let old = vec![1, 2, 3, 4, 5];
    let new = vec![1, 2, 10, 4, 8];

    let mut differ = MyDiffer;

    // Calculate diff between the two sequences
    myers_diff(&mut differ, &old, 0, old.len(), &new, 0, new.len()).unwrap();
}
```

See the [myers_diff.rs](examples/myers_diff.rs) example for a more complete demonstration.

### Working with Multi-file Patches

```rust
use diffpatch::{MultifilePatch, MultifilePatcher};
use std::path::Path;

fn main() {
    // Parse a multi-file patch from a file
    let patch_path = Path::new("changes.patch");
    let multipatch = MultifilePatch::parse_from_file(patch_path).unwrap();

    // Apply all patches to files in the current directory
    let patcher = MultifilePatcher::new(multipatch);
    let written_files = patcher.apply_and_write(false).unwrap();

    println!("Updated files: {:?}", written_files);
}
```

## CLI Usage

### Generate a Patch

```bash
diffpatch generate --old original_file.txt --new modified_file.txt --output patch.diff
```

### Apply a Patch

```bash
diffpatch apply --patch patch.diff --file original_file.txt --output result.txt
```

### Apply a Patch in Reverse

```bash
diffpatch apply --patch patch.diff --file modified_file.txt --output original.txt --reverse
```

### Apply a Multi-file Patch

```bash
diffpatch apply-multi --patch changes.patch [--directory /path/to/target] [--reverse]
```

## Data Structures

- `Patch`: Represents a complete diff between two files
- `Chunk`: Represents a contiguous section of changes
- `Operation`: Represents a single line in a diff (addition, deletion, or context)
- `MultifilePatch`: Collection of patches for multiple files
- `MultifilePatcher`: Applies multiple patches to files
- `Diff`: Trait for implementing custom diff logic
- `myers_diff`: Function to apply the efficient Myers algorithm to custom sequence types

## Limitations

- Limited support for various diff formats (focuses on git-style diffs)

## License

This project is distributed under the terms of MIT.

See [LICENSE](LICENSE.md) for details.

Copyright 2025 Tyr Chen
