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
# 运行 workspace 的所有测试
cargo test --workspace

# 运行单个测试
cargo test --workspace explicit_config_paths_keep_parallel_accounts_separate

# 运行 CLI 命令测试
cargo test -p cloud139 --bin cloud139 commands_test

# 运行集成测试
cargo test -p cloud139-core --test api_test

# 显示测试输出
cargo test --workspace -- --nocapture
```

## 代码检查

```bash
# Clippy 检查
cargo clippy --workspace

# 格式化检查
cargo fmt --all -- --check

# 格式化代码
cargo fmt --all
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
