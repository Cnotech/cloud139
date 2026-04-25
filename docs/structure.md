# 项目结构

```
src/
├── main.rs                    # 程序入口
├── lib.rs                     # 库入口
├── cli/                       # CLI 层：命令行参数定义
│   ├── app.rs                 # CLI 应用定义 (Cli, Commands)
│   └── mod.rs                 # CLI 模块导出
├── application/               # 应用层：业务逻辑服务
│   └── services/              # 业务服务实现
│       ├── copy_service.rs    # 复制服务
│       ├── delete_service.rs  # 删除服务
│       ├── download_service.rs # 下载服务
│       ├── list_service.rs    # 列表服务
│       ├── login_service.rs   # 登录服务
│       ├── mkdir_service.rs   # 创建目录服务
│       ├── move_service.rs    # 移动服务
│       ├── rename_service.rs  # 重命名服务
│       ├── sync_service.rs    # 同步服务（差异计算）
│       ├── sync_executor.rs   # 同步执行器
│       ├── upload_service.rs  # 上传服务调度
│       └── upload/            # 上传子模块（按存储类型拆分）
│           ├── family.rs      # 家庭云上传
│           ├── group.rs       # 群组云上传
│           ├── mod.rs         # 上传模块导出
│           ├── personal.rs    # 个人云上传
│           └── personal_parts.rs # 个人云分片上传
├── domain/                    # 领域层：核心业务模型
│   ├── file_item.rs           # 文件项模型 (FileItem, EntryKind)
│   ├── mod.rs                 # 领域模块导出
│   ├── storage_type.rs        # 存储类型枚举 (StorageType)
│   └── sync_item.rs           # 同步项模型 (SyncItem, SyncAction)
├── presentation/              # 展示层：输出格式化
│   ├── list_renderer.rs       # 列表渲染器
│   ├── mod.rs                 # 展示模块导出
│   ├── progress.rs            # 进度条辅助
│   └── sync_renderer.rs       # 同步结果渲染器
├── commands/                  # 命令层：命令执行逻辑
│   ├── cp.rs                  # 复制命令
│   ├── delete.rs              # 删除命令
│   ├── download.rs            # 下载命令
│   ├── list.rs                # 列表命令
│   ├── login.rs               # 登录命令
│   ├── mkdir.rs               # 创建目录命令
│   ├── mod.rs                 # 命令模块导出
│   ├── mv.rs                  # 移动命令
│   ├── rename.rs              # 重命名命令
│   ├── sync.rs                # 同步命令
│   └── upload.rs              # 上传命令
├── client/                    # 基础设施：API 客户端
│   ├── api.rs                 # API 实现
│   ├── api_trait.rs           # API trait 定义
│   ├── auth.rs                # 认证相关
│   ├── endpoints.rs           # API 端点常量
│   ├── error.rs               # 客户端错误
│   ├── headers.rs             # 请求头构造
│   └── mod.rs                 # 客户端模块导出
├── models/                    # 数据模型（API 请求/响应）
│   ├── auth.rs                # 认证模型
│   ├── batch_ops.rs           # 批量操作模型
│   ├── common.rs              # 通用模型
│   ├── list.rs                # 列表模型
│   ├── mod.rs                 # 模型模块导出
│   └── upload.rs              # 上传模型
├── config/                    # 配置管理
│   └── mod.rs
└── utils/                     # 工具函数
    ├── crypto.rs              # 加密/解密工具
    ├── logger.rs              # 日志宏与进度条封装
    ├── mod.rs                 # 工具模块导出
    ├── path.rs                # 路径解析辅助
    ├── rand.rs                # 随机数工具
    ├── time.rs                # 时间工具
    └── width.rs               # 终端宽度计算
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
