# Patch multiple files

Tools like `git-diff` or `diff` can generate patch with multiple files. We should be able to apply these patches to multiple files at current directory at once.

## Implementation

Please build new data structures and methods utilizing existing `Differ` and `Patcher` structs and methods to support patching multiple files at once.

```rust
struct MultifilePatch {
  patches: Vec<Patch>,
  ...
}

struct MultifilePatcher {
  patches: Vec<Patch>,
  ...
}

struct PatchedFile {
  path: String,
  content: String,
}

impl MultifilePatcher {
  pub fn apply(&self, reverse: bool) -> Result<Vec<PatchedFile>, Error> {
    // ...
  }
}
```

## Testing

Please add enough unit tests to cover the `MultifilePatch` and `MultifilePatcher` functionalities. Make sure edge cases are also tested.

## Integration tests

Please write an integration test - you should clone the current repo to a temp folder, checkout to `diff-test1` branch, and apply the patch file `fixtures/diff-test1.diff` to test if it works as expected. You could use `tempfile` and `git2` crate to help with the test.
