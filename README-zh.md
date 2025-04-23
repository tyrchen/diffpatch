![](https://github.com/tyrchen/patcher/workflows/build/badge.svg)

# Patcher

一个用于生成和应用Git风格统一差异补丁的Rust库。

## 教程

参见[教程](./tutorial/zh/README.md)。

## 特性

- 从原始内容和修改后的内容生成补丁
- 向前和向后应用补丁到内容
- 从文本格式解析补丁
- 支持多文件补丁
- 用于生成和应用补丁的命令行界面
- 高效的Myers差异算法实现
- 可自定义的差异实现，适用于任何数据类型

## 安装

添加到您的Cargo.toml：

```toml
[dependencies]
patcher = { version = "0.1.0", default-features = false }
```

或安装CLI工具：

```bash
cargo install patcher
```

## 库使用

### 生成补丁

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

### 应用补丁

```rust
use patcher::{DiffAlgorithm, Differ, PatchAlgorithm, Patcher};

fn main() {
    let old_content = "line1\nline2\nline3\nline4";
    let new_content = "line1\nline2 modified\nline3\nline4";

    // 生成补丁
    let differ = Differ::new(old_content, new_content);
    let patch = differ.generate();

    // 将补丁应用到原始内容
    let patcher = Patcher::new(patch);
    let result = patcher.apply(old_content, false).unwrap();

    assert_eq!(result, new_content);
}
```

### 解析补丁

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

### 处理多文件补丁

```rust
use patcher::{MultifilePatch, MultifilePatcher};
use std::path::Path;

fn main() {
    // 从文件解析多文件补丁
    let patch_path = Path::new("changes.patch");
    let multipatch = MultifilePatch::parse_from_file(patch_path).unwrap();

    // 将所有补丁应用到当前目录中的文件
    let patcher = MultifilePatcher::new(multipatch);
    let written_files = patcher.apply_and_write(false).unwrap();

    println!("Updated files: {:?}", written_files);
}

## 数据结构

- `Patch`：表示两个文件之间的完整差异
- `Chunk`：表示连续的更改部分
- `Operation`：表示差异中的单行（添加、删除或上下文）
- `MultifilePatch`：多个文件的补丁集合
- `MultifilePatcher`：将多个补丁应用到文件
- `Diff`：用于实现自定义差异逻辑的特性

## 限制

- 对各种差异格式的支持有限（专注于git风格的差异）

## 许可证

本项目根据MIT条款分发。

详情参见[LICENSE](LICENSE.md)。

版权所有 2025 Tyr Chen
