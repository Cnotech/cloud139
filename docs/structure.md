# 项目结构

```
src/
├── main.rs                    # 程序入口
├── lib.rs                     # 库入口
├── cli/                       # CLI 层：命令行参数定义
├── application/               # 应用层：业务逻辑服务
│   └── services/              # 业务服务实现
│       ├── login / list / download / upload / delete
│       ├── mkdir / mv / cp / rename / sync
│       └── upload/            # 上传子模块（按存储类型拆分）
├── domain/                    # 领域层：核心业务模型
├── presentation/              # 展示层：输出格式化、进度条
├── commands/                  # 命令层：命令执行逻辑
├── client/                    # 基础设施：API 客户端
├── models/                    # 数据模型（API 请求/响应）
├── config/                    # 配置管理
└── utils/                     # 工具函数
```

## 架构分层说明

### 1. CLI 层 (cli/)
- 使用 clap 定义命令行参数结构
- `app.rs` 中定义 `Cli` 和 `Commands` 枚举
- 解析用户输入，传递给命令层

### 2. 命令层 (commands/)
- 包含各命令的 Args 定义和 thin execute 适配器
- 连接 CLI 层和应用层
- 调用应用层服务执行业务逻辑

### 3. 应用层 (application/)
- 包含业务逻辑服务
- 协调领域对象和基础设施
- 实现核心业务用例
- `upload/` 子模块按存储类型拆分上传逻辑

### 4. 领域层 (domain/)
- 核心业务模型
- 不依赖外部框架或基础设施
- 包含业务规则和验证

### 5. 展示层 (presentation/)
- 输出格式化
- 进度条展示
- 用户界面相关逻辑

### 6. 基础设施层 (client/)
- API 客户端实现
- 外部服务交互
- 技术细节实现
