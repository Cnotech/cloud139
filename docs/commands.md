# 构建与测试命令

## 构建

```bash
# Debug 构建
cargo build

# Release 构建
cargo build --release
```

## 测试

```bash
# 运行所有测试
cargo test

# 运行单个测试
cargo test test_parse_path_root

# 运行指定测试文件
cargo test --test commands_test

# 运行集成测试
cargo test --test api_test

# 显示测试输出
cargo test -- --nocapture
```

## 代码检查

```bash
# Clippy 检查
cargo clippy

# 格式化检查
cargo fmt --check

# 格式化代码
cargo fmt
```

## 开发常用命令

```bash
# 运行程序
cargo run -- --help

# 运行特定命令
cargo run -- ls /

# 清理构建缓存
cargo clean
```
