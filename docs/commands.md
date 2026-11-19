# 开发命令

在工作区根目录执行。

```sh
cargo build --locked
cargo check --workspace --all-targets --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked --no-fail-fast
```

`cargo build` 默认构建 CLI 及其依赖的库，加 `--release` 构建发布版本。CI 在 Linux、macOS 和 Windows 的 stable 工具链上运行 Clippy 与工作区测试。Linux 还运行 `python3 scripts/check-rust-file-length.py`，对超过 500 行的 Rust 源文件发出告警，跳过各包的 `src/tests/`。见 [CI 工作流](../.github/workflows/ut.yml)。

## 定向验证

```sh
# 检查命令帮助
cargo run --locked -- sync --help

# 运行 CLI 参数测试
cargo test --locked -p cloud139-cli --bin cloud139 commands_args_test

# 运行库的分页集成测试
cargo test --locked -p cloud139 --test api_path_paging_test
```

HTTP mock 测试需要监听本地回环端口。依赖已缓存时，可以给 Cargo 加 `--offline`，它限制 Cargo 获取依赖，测试代码的网络访问仍由运行环境控制。真实云盘回归按 [cloud139-e2e-test 技能](../.agents/skills/cloud139-e2e-test/SKILL.md)执行。
