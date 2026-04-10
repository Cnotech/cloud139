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

## 相关文档

- [README.md](../README.md): 项目说明
- [.agents/skills/cloud139-e2e-test](../.agents/skills/cloud139-e2e-test/SKILL.md): E2E 测试流程
