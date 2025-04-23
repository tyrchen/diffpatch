# Patchers

This module contains implementations of patching algorithms that apply patches to text content.

## Overview

The `patcher` module provides different strategies for applying patches to text content:

- **NaivePatcher**: A fast, simple implementation that requires exact context matches
- **SimilarPatcher**: A sophisticated implementation using fuzzy matching for more robust patching

Both patchers implement the `PatchAlgorithm` trait, providing a consistent interface for applying patches.

## Features

### NaivePatcher

- Applies patches at exact line numbers specified in the patch
- Requires perfect matches for context lines
- High performance with minimal overhead
- Best for situations where exact context matching is sufficient

### SimilarPatcher

- Uses fuzzy matching to find the best location for patches
- Handles whitespace differences, minor typos, and formatting variations
- Searches within a configurable range to find optimal patch locations
- Provides multiple fallback strategies for robust patching
- Best for situations where flexibility is more important than raw performance

## Usage

```rust
use patcher::{Differ, Patch};
use patcher::patcher::{PatchAlgorithm, NaivePatcher, SimilarPatcher};

// Generate a patch
let old_content = "line1\nline2\nline3";
let new_content = "line1\nline2 modified\nline3";
let differ = Differ::new(old_content, new_content);
let patch = differ.generate();

// Apply patch with NaivePatcher (faster but strict)
let naive_patcher = NaivePatcher::new(&patch);
let result = naive_patcher.apply(old_content, false).unwrap();
assert_eq!(result, new_content);

// Apply patch with SimilarPatcher (more flexible but slower)
let similar_patcher = SimilarPatcher::new(&patch);
let result = similar_patcher.apply(old_content, false).unwrap();
assert_eq!(result, new_content);

// Apply patches in reverse (to revert changes)
let reversed_result = naive_patcher.apply(new_content, true).unwrap();
assert_eq!(reversed_result, old_content);
```

## Performance Considerations

For detailed performance analysis and comparison between the implementations, see [Performance Comparison](./perf.md).

## When to Choose Each Implementation

- **NaivePatcher**: When performance is critical and you can guarantee that the content to be patched will exactly match the context in the patch.

- **SimilarPatcher**: When robustness is more important than performance, especially when:
  - Content may have minor variations from what the patch expects
  - Line numbers might have shifted slightly
  - Whitespace differences exist
  - Exact context matching would fail but semantically equivalent matches would work

## Implementation Details

Both patchers handle:

- Adding lines
- Removing lines
- Context verification
- Reverse patch application (to undo changes)
- Preserving final newlines

The key difference is in how they locate and verify context lines before applying changes.
