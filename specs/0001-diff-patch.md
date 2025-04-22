# Diff Patch Algorithm

## Overview

The diff patch algorithm is a way to generate a patch for the changes in a file. A patch could then be applied to the original file to bring it to the new state.

Below is an example of a diff:

```diff
index 36730e7..e9116e3 100644
--- a/examples/server.rs
+++ b/examples/server.rs
@@ -5,17 +5,15 @@ use argon2::{
 };
 use axum::{
     Json, Router,
-    extract::{Path, Request, State},
+    extract::{Path, State},
     http::StatusCode,
-    middleware::{Next, from_fn_with_state},
-    response::Response,
     routing::{delete, get, post, put},
 };
 use axum_server::tls_rustls::RustlsConfig;
 use chrono::{DateTime, Utc};
 use clap::Parser;
 use dashmap::DashMap;
-use http::HeaderValue;
+use http::{Request, Response};
 use rand::rngs::OsRng;
 use serde::{Deserialize, Serialize};
 use std::{
@@ -74,17 +72,15 @@ struct AppStateInner {
     next_id: AtomicU64,
     users: DashMap<u64, User>,
     argon2: Argon2<'static>,
-    addr: SocketAddr,
 }

 impl AppState {
-    fn new(addr: impl Into<SocketAddr>) -> Self {
+    fn new() -> Self {
         Self {
             inner: Arc::new(AppStateInner {
                 next_id: AtomicU64::new(1),
                 users: DashMap::new(),
                 argon2: Argon2::default(),
-                addr: addr.into(),
             }),
         }
     }
```

## Implementation

The diff patch algorithm is implemented in the `diff_patch` module with the following data structures and methods:

```rust
struct Differ {
  old: String,
  new: String,
  ...
}

struct Patcher {
  patch: Patch,
  ...
}

struct Patch {
    ...
}


impl Differ {
  pub fn generate(&self) -> Patch {
    // ...
  }
}

impl Patcher {
  pub fn apply(&self, content: &str, reverse: bool) -> Result<String, Error> {
    // ...
  }
}
```

## Testing

Please
