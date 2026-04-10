# 项目结构

```
src/
├── main.rs                    # 程序入口
├── lib.rs                     # 库入口
├── cli/                       # CLI 层：命令行参数定义
│   ├── app.rs                 # CLI 应用定义 (Cli, Commands)
│   └── commands/              # 各命令的参数结构
│       ├── cp.rs              # 复制命令参数
│       ├── delete.rs          # 删除命令参数
│       ├── download.rs        # 下载命令参数
│       ├── list.rs            # 列表命令参数
│       ├── login.rs           # 登录命令参数
│       ├── mkdir.rs           # 创建目录命令参数
│       ├── mv.rs              # 移动命令参数
│       ├── rename.rs          # 重命名命令参数
│       └── upload.rs          # 上传命令参数
├── application/               # 应用层：业务逻辑服务
│   └── services/              # 业务服务实现
│       └── list_service.rs    # 列表服务
├── domain/                    # 领域层：核心业务模型
│   └── file_item.rs           # 文件项模型 (FileItem, EntryKind)
├── presentation/              # 展示层：输出格式化
│   ├── error.rs               # 错误格式化
│   └── renderers/             # 输出渲染器
│       └── list_renderer.rs   # 列表渲染器
├── commands/                  # 命令层：命令执行逻辑
│   ├── cp.rs                  # 复制命令
│   ├── delete.rs              # 删除命令
│   ├── dispatch.rs            # 命令分发
│   ├── download.rs            # 下载命令
│   ├── list.rs                # 列表命令
│   ├── login.rs               # 登录命令
│   ├── mkdir.rs               # 创建目录命令
│   ├── mv.rs                  # 移动命令
│   ├── rename.rs              # 重命名命令
│   └── upload/                # 上传命令
│       ├── family.rs          # 家庭云上传
│       ├── group.rs           # 群组云上传
│       ├── personal.rs        # 个人云上传
│       └── personal_parts.rs  # 个人云分片上传
├── client/                    # 基础设施：API 客户端
│   ├── api.rs                 # API 实现
│   ├── api_trait.rs           # API trait
│   ├── auth.rs                # 认证
│   ├── endpoints.rs           # API 端点
│   ├── error.rs               # 客户端错误
│   ├── headers.rs             # 请求头
│   └── storage_type.rs        # 存储类型
├── models/                    # 数据模型（API 请求/响应）
│   ├── auth.rs                # 认证模型
│   ├── batch_ops.rs           # 批量操作模型
│   ├── common.rs              # 通用模型
│   ├── list.rs                # 列表模型
│   ├── types.rs               # 类型定义
│   └── upload.rs              # 上传模型
├── config/                    # 配置管理
└── utils/                     # 工具函数
    ├── crypto.rs              # 加密工具
    ├── logger.rs              # 日志工具
    ├── rand.rs                # 随机数工具
    ├── time.rs                # 时间工具
    └── width.rs               # 宽度计算
```

## 架构分层说明

### 1. CLI 层 (cli/)
- 使用 clap 定义命令行参数结构
- 解析用户输入，传递给命令层

### 2. 命令层 (commands/)
- 作为适配器，连接 CLI 层和应用层
- 调用应用层服务执行业务逻辑
- 处理命令特定的逻辑

### 3. 应用层 (application/)
- 包含业务逻辑服务
- 协调领域对象和基础设施
- 实现核心业务用例

### 4. 领域层 (domain/)
- 核心业务模型
- 不依赖外部框架或基础设施
- 包含业务规则和验证

### 5. 展示层 (presentation/)
- 输出格式化
- 错误展示
- 用户界面相关逻辑

### 6. 基础设施层 (client/)
- API 客户端实现
- 外部服务交互
- 技术细节实现
