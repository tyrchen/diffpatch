![](https://github.com/tyrchen/diffpatch/workflows/build/badge.svg)

# Diffpatch

一个用于生成和应用Git风格统一差异补丁的Rust库。

## 教程

参见[教程](./tutorial/zh/index.md)。

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
diffpatch = { version = "0.1.0", default-features = false }
```

或安装CLI工具：

```bash
cargo install diffpatch
```

## 库使用

### 生成补丁

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

### 应用补丁

```rust
use diffpatch::{Differ, Patcher};

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

### 使用Myers差异算法

该库提供了一个低级Myers差异算法实现，可用于任何数据类型：

```rust
use diffpatch::{Diff, myers_diff};

// 为自定义比较器实现Diff特性
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

    // 计算两个序列之间的差异
    myers_diff(&mut differ, &old, 0, old.len(), &new, 0, new.len()).unwrap();
}
```

查看[myers_diff.rs](examples/myers_diff.rs)示例以获取更完整的演示。

### 处理多文件补丁

```rust
use diffpatch::{MultifilePatch, MultifilePatcher};
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
```

## CLI使用

### 生成补丁

```bash
diffpatch generate --old original_file.txt --new modified_file.txt --output patch.diff
```

### 应用补丁

```bash
diffpatch apply --patch patch.diff --file original_file.txt --output result.txt
```

### 反向应用补丁

```bash
diffpatch apply --patch patch.diff --file modified_file.txt --output original.txt --reverse
```

### 应用多文件补丁

```bash
diffpatch apply-multi --patch changes.patch [--directory /path/to/target] [--reverse]
```

## 数据结构

- `Patch`：表示两个文件之间的完整差异
- `Chunk`：表示连续的更改部分
- `Operation`：表示差异中的单行（添加、删除或上下文）
- `MultifilePatch`：多个文件的补丁集合
- `MultifilePatcher`：将多个补丁应用到文件
- `Diff`：用于实现自定义差异逻辑的特性
- `myers_diff`：将高效的Myers算法应用于自定义序列类型的函数

## 限制

- 对各种差异格式的支持有限（专注于git风格的差异）

## 许可证

本项目根据MIT条款分发。

详情参见[LICENSE](LICENSE.md)。

版权所有 2025 Tyr Chen
