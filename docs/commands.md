# 构建与测试命令

## 构建

```bash
# 调试构建
cargo build

# 发布构建
cargo build --release
```

## 测试

```bash
# 运行工作区的所有测试
cargo test --workspace

# 运行单个测试
cargo test --workspace explicit_config_paths_keep_parallel_accounts_separate

# 运行 CLI 命令测试
cargo test -p cloud139-cli --bin cloud139 commands_test

# 运行集成测试
cargo test -p cloud139 --test api_test

# 显示测试输出
cargo test --workspace -- --nocapture
```

## 代码检查

```bash
# 检查工作区源码及测试目标
cargo clippy --workspace --all-targets --locked

# 检查 Rust 源文件行数
python3 scripts/check-rust-file-length.py

# 格式化检查
cargo fmt --all -- --check

# 格式化代码
cargo fmt --all
```

CI 在 Linux、macOS 和 Windows 上运行 Clippy。单文件行数检查在 Linux CI 上独立运行，对 `packages/*/src/` 下总行数超过 500 的 Rust 文件发出告警，跳过独立测试目录。超过阈值时可以检查文件里是否有能够独立拆分的职责。

## 开发常用命令

```bash
# 运行程序
cargo run -- --help

# 运行特定命令
cargo run -- ls /

# 清理构建缓存
cargo clean
```
