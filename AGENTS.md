# AGENTS.md - cloud139 开发指南

本文档为 AI Agent 提供开发指导。

## 快速开始

### 构建与测试

```bash
# 调试构建
cargo build

# 运行所有测试
cargo test --workspace

# 代码质量检查
cargo clippy --workspace
```

更多命令见 [docs/commands.md](docs/commands.md)

### 项目结构

Package `cloud139-cli` 位于 `packages/cli/`。参数定义、命令适配、展示、配置路径和跨进程锁分别在该 package 的 `src/cli.rs`、`src/commands/`、`src/presentation/` 和 `src/config/` 中。

库 package `cloud139` 位于 `packages/lib/`，负责配置读写、认证、API 请求和文件操作。CLI 显式传递配置或路径，并将服务结果转换成命令行输出。详见 [项目结构](docs/structure.md)。

### 代码风格

参考现有代码风格，见 [docs/style.md](docs/style.md)

### 注释语言

- 项目编写的普通注释、文档注释、测试注释和代码示例中的注释统一使用简体中文。代码标识符、协议字段、技术术语和工具识别标记按需保留原文。

### 模块导入

- 优先用 `use` 从定义所在的模块直接导入类型、函数和常量。公开 API 优先直接定义在其对外入口。
- 尽量避免 `pub use` 和 `pub(crate) use` 等重导出。发现为了缩短路径、提供重复入口或跨模块转发而引入的重导出时，将调用方迁移到定义模块。
- 平台实现选择、导出宏从调用方访问依赖等确需重导出的场景，使用最小可见范围并就地注明原因。

### 常用依赖

clap, reqwest, tokio, serde, thiserror 等

详细依赖见 [docs/dependencies.md](docs/dependencies.md)

## 开发注意事项

- 当前 CLI 仅支持个人云，配置使用 `personal_new`
- 遵循分层架构，避免跨层依赖

详细注意事项见 [docs/notes.md](docs/notes.md)

### 测试文件编写规范

- 新增测试优先放在对应 package 的测试目录统一管理。通过公开 API 或 CLI 入口验证行为的测试放在 `tests/`，需要访问 crate 内部接口的测试放在 `src/tests/`。
- 测试私有函数或方法时，允许在同一源码文件中，在对应函数或 `impl` 块下方用 `#[cfg(test)]` 模块内联编写单测。
- 禁止在实现代码所在目录散放独立测试文件，例如 `src/config/tests.rs` 或 `src/config/foo_test.rs`。

### 日志打印规范

- 禁止直接使用 `println!`/`eprintln!`/`log::`，统一使用各 package 的日志宏。CLI 由 `packages/cli/src/logger.rs` 输出，库由 `packages/lib/src/utils/logger.rs` 将日志交给宿主。
- 常用宏：`success!`（绿，成功）、`step!`（蓝，步骤）、`info!`（青，提示）、`warn!`（黄，警告）、`error!`（红，错误）、`debug!`（灰，调试）
- 自有日志正文使用简体中文，调试字段沿用稳定的英文 `key=value`。
- CLI 日志输出会协调进度条重绘。库通过传入的进度回调报告传输进展。

详细规范见 [docs/logging.md](docs/logging.md)

## 相关文档

- [README.md](../README.md): 项目说明
- [.agents/skills/cloud139-e2e-test](../.agents/skills/cloud139-e2e-test/SKILL.md): E2E 测试流程
- [docs/command-docs-style.md](docs/command-docs-style.md): 命令文档编写规范
