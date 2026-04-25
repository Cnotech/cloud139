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
│   ├── app.rs            # CLI 应用定义 (Commands, Cli)
│   └── mod.rs            # CLI 模块导出
├── application/          # 应用层：业务逻辑服务
│   └── services/         # 业务服务实现
│       ├── copy_service.rs
│       ├── delete_service.rs
│       ├── download_service.rs
│       ├── list_service.rs
│       ├── login_service.rs
│       ├── mkdir_service.rs
│       ├── move_service.rs
│       ├── rename_service.rs
│       ├── sync_service.rs
│       ├── sync_executor.rs
│       ├── upload_service.rs
│       └── upload/       # 上传子模块（按存储类型拆分）
│           ├── family.rs
│           ├── group.rs
│           ├── mod.rs
│           ├── personal.rs
│           └── personal_parts.rs
├── domain/               # 领域层：核心业务模型
│   ├── file_item.rs      # 文件项模型 (FileItem, EntryKind)
│   ├── storage_type.rs   # 存储类型枚举 (StorageType)
│   └── sync_item.rs      # 同步项模型 (SyncItem, SyncAction)
├── presentation/         # 展示层：输出格式化
│   ├── list_renderer.rs  # 列表渲染器
│   ├── sync_renderer.rs  # 同步结果渲染器
│   └── progress.rs       # 进度条辅助
├── commands/             # 命令层：命令执行逻辑（各命令 Args + execute）
│   ├── cp.rs
│   ├── delete.rs
│   ├── download.rs
│   ├── list.rs
│   ├── login.rs
│   ├── mkdir.rs
│   ├── mod.rs
│   ├── mv.rs
│   ├── rename.rs
│   ├── sync.rs
│   └── upload.rs
├── client/               # 基础设施：API 客户端
│   ├── api.rs            # API 实现
│   ├── api_trait.rs      # API trait 定义
│   ├── auth.rs           # 认证相关
│   ├── endpoints.rs      # API 端点常量
│   ├── error.rs          # 客户端错误
│   ├── headers.rs        # 请求头构造
│   └── mod.rs
├── models/               # 数据模型（API 请求/响应）
│   ├── auth.rs
│   ├── batch_ops.rs
│   ├── common.rs
│   ├── list.rs
│   ├── mod.rs
│   └── upload.rs
├── config/               # 配置管理
│   └── mod.rs
└── utils/                # 工具函数
    ├── crypto.rs         # 加密/解密工具
    ├── logger.rs         # 日志宏与进度条封装
    ├── mod.rs
    ├── path.rs           # 路径解析辅助
    ├── rand.rs           # 随机数工具
    ├── time.rs           # 时间工具
    └── width.rs          # 终端宽度计算
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

### 日志打印规范

- 禁止直接使用 `println!`/`eprintln!`/`log::`，统一使用 `src/utils/logger.rs` 提供的日志宏
- 常用宏：`success!`（绿，成功）、`step!`（蓝，步骤）、`info!`（青，提示）、`warn!`（黄，警告）、`error!`（红，错误）、`debug!`（灰，调试）
- 进度条场景下使用 `pb_*` / `mp_*` 系列函数，避免破坏进度条布局

详细规范见 [docs/logging.md](docs/logging.md)

## 相关文档

- [README.md](../README.md): 项目说明
- [.agents/skills/cloud139-e2e-test](../.agents/skills/cloud139-e2e-test/SKILL.md): E2E 测试流程
- [docs/command-docs-style.md](docs/command-docs-style.md): 命令文档编写规范
