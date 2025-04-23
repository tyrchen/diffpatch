![](https://github.com/tyrchen/patcher/workflows/build/badge.svg)

# Patcher

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
patcher = "0.2.1"
```

## Library Usage

### Generate a Patch

```rust
use patcher::{DiffAlgorithm, Differ};

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
use patcher::{DiffAlgorithm, Differ, PatchAlgorithm, Patcher};

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
use patcher::Patch;

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

### Working with Multi-file Patches

```rust
use patcher::{MultifilePatch, MultifilePatcher};
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
