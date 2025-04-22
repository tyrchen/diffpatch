# Differ

Please help to make differ a directory structure like this:

```bash
differ/
├── mod.rs
├── myers.rs
├── naive.rs
└── README.md
```

Extract `pub fn generate(&self) -> Patch` into a trait, move existing code to naive.rs, move myers related code to myers.rs and implement the trait for all the algorithms.

Please also add tests for all the algorithms.
