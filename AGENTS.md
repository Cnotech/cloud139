# AGENTS.md - cloud139 开发指南

本文档为 AI Agent 提供开发指导。

## 快速开始

### 构建与测试

```bash
# Debug 构建
cargo build

# 运行所有测试
cargo test

# 代码质量检查
cargo clippy
```

更多命令见 [docs/commands.md](docs/commands.md)

### 项目结构

```
src/
├── main.rs               # 程序入口
├── lib.rs                # 库入口
├── cli/                  # CLI 层：命令行参数定义
│   ├── app.rs            # CLI 应用定义
│   └── commands/         # 各命令的参数结构
├── application/          # 应用层：业务逻辑服务
│   └── services/         # 业务服务实现
├── domain/               # 领域层：核心业务模型
├── presentation/         # 展示层：输出格式化
│   ├── error.rs          # 错误格式化
│   └── renderers/        # 输出渲染器
├── commands/             # 命令层：命令执行逻辑
├── client/               # 基础设施：API 客户端
├── models/               # 数据模型（API 请求/响应）
├── config/               # 配置管理
└── utils/                # 工具函数
```

详细结构见 [docs/structure.md](docs/structure.md)

### 架构分层

- **CLI 层 (cli/)**: 定义命令行参数结构，使用 clap 解析
- **应用层 (application/)**: 业务逻辑服务，协调领域对象和基础设施
- **领域层 (domain/)**: 核心业务模型，不依赖外部
- **展示层 (presentation/)**: 输出格式化、错误展示
- **命令层 (commands/)**: 适配器，连接 CLI 层和应用层
- **基础设施层 (client/)**: API 客户端、外部服务交互

### 代码风格

参考现有代码风格，见 [docs/style.md](docs/style.md)

### 常用依赖

clap, reqwest, tokio, serde, thiserror 等

详细依赖见 [docs/dependencies.md](docs/dependencies.md)

## 开发注意事项

- 139 云盘 API 区分三种存储类型: PersonalNew, Family, Group
- 某些操作在不同存储类型下行为不同
- 遵循分层架构，避免跨层依赖

详细注意事项见 [docs/notes.md](docs/notes.md)

## 日志打印规范

项目在 `src/utils/logger.rs` 中定义了统一的日志宏和辅助函数，禁止使用 `println!`/`eprintln!`/`log::` 进行日志输出。

### 日志宏（直接使用）

| 宏 | 颜色 | 用途 | 输出 |
|---|---|---|---|
| `success!` | 绿 | 操作成功 | stdout |
| `step!` | 蓝 | 操作步骤提示 | stdout |
| `info!` | 青 | 用户提示信息 | stdout |
| `warn!` | 黄 | 警告 | stdout |
| `error!` | 红 | 错误 | stderr |
| `debug!` | 灰 | 仅在 `verbose=debug` 时输出 | stdout |

**使用规则：**
- 用户能看到的提示 → `success!`/`step!`/`info!`
- 需要注意但不阻断 → `warn!`
- 错误 → `error!`
- 仅调试时需要（API 响应详情、Token 刷新等） → `debug!`
- 不要在日志消息前硬编码级别标签（如 "警告:"、"错误:"），宏已自带颜色标签

### 进度条感知日志函数

在有进度条的场景下，必须使用 `pb_*` 或 `mp_*` 系列函数，确保输出在进度条上方而不破坏布局。

**单进度条（`Option<ProgressBar>`）：**

| 函数 | 用途 |
|---|---|
| `pb_debug(msg, &pb)` | 调试信息（需 `-v debug`） |
| `pb_info(msg, &pb)` | 提示信息 |
| `pb_step(msg, &pb)` | 步骤提示 |
| `pb_success(msg, &pb)` | 成功提示 |
| `pb_warn(msg, &pb)` | 警告 |
| `pb_error(msg, &pb)` | 错误 |

- `pb = None` 时自动降级为对应宏直接打印
- 定义在 `src/utils/logger.rs`，使用 `use crate::utils::logger::{pb_step, ...}` 导入

**多进度条（`&MultiProgress`）：**

| 函数 | 用途 |
|---|---|
| `mp_debug(msg, mp)` | 调试信息 |
| `mp_info(msg, mp)` | 提示信息 |
| `mp_step(msg, mp)` | 步骤提示 |
| `mp_success(msg, mp)` | 成功提示 |
| `mp_warn(msg, mp)` | 警告 |
| `mp_error(msg, mp)` | 错误 |

- 定义在 `src/utils/logger.rs`，使用 `use crate::utils::logger::mp_error;` 导入

## 相关文档

- [README.md](../README.md): 项目说明
- [.agents/skills/cloud139-e2e-test](../.agents/skills/cloud139-e2e-test/SKILL.md): E2E 测试流程
