# 项目结构

```
cloud139/
├── Cargo.toml                     # Workspace 清单
└── packages/
    ├── cli/
    │   ├── Cargo.toml             # cloud139-cli 命令行 package
    │   ├── src/
    │   │   ├── main.rs            # CLI 入口
    │   │   ├── cli.rs             # clap 参数定义
    │   │   ├── commands/          # 调用服务、映射退出码
    │   │   ├── config/            # 配置路径、续期写回及跨进程锁
    │   │   ├── error.rs           # CLI 错误类型
    │   │   ├── logger.rs          # 终端日志输出
    │   │   └── presentation/     # JSON、文本与进度条
    │   └── tests/                 # CLI 集成测试
    └── lib/
        ├── Cargo.toml             # cloud139 库 package
        ├── src/
        │   ├── lib.rs             # 库入口
        │   ├── config/            # 配置类型、个人云校验与 TOML 读写
        │   ├── client/            # API 请求与认证
        │   ├── services/          # 文件操作及同步
        │   ├── error.rs           # 库错误类型和共用错误构造
        │   ├── models/            # API 数据模型和业务类型
        │   ├── progress.rs        # 传输进度接口
        │   ├── protocol.rs        # 云盘端点、远程返回码和其他协议固定值
        │   └── utils/             # 日志、重试及通用函数
        └── tests/                 # 库集成测试
```

CLI 单向依赖 `cloud139`。CLI 每次调用选择配置文件，持锁更新凭据，再把加载好的配置交给服务。库接受调用方传入的配置，返回结构化结果，并通过 `log` 和 `progress.rs` 定义的进度回调向宿主报告进展。

`main.rs` 使用 `cli.rs` 中的定义解析参数，再调用 `commands::dispatch::execute` 分发命令。命令适配器直接导入参数类型和需要的渲染模块。配置路径由 `config::paths` 决定，`config::store` 负责持锁保存配置和写回续期结果。

`client/api/headers.rs` 定义路由和个人云请求头及客户端标识。`client/api/route.rs` 负责路由查询，`client/api/personal/` 负责个人云目录和文件请求。对外的文件查询与下载链接接口位于 `client/api/files.rs`，上传请求位于 `client/api/upload.rs`。Token 解析和续期分别位于 `client/auth/token.rs` 和 `client/auth/refresh.rs`。HTTP 客户端和签名请求发送位于 `client/http.rs`，共用的 API 响应判断位于 `client/api/response.rs`。

`models/file_item.rs` 定义列表服务返回的文件条目，`models/sync_item.rs` 定义同步扫描、计划和结果用的类型。云端请求和响应的数据模型按接口用途放在 `models` 的其他文件中。

`services/sync/` 中的 `plan.rs` 定义同步请求和计划，调用 `scan.rs` 收集条目、`diff.rs` 生成操作，并通过 `executor.rs` 或 `verify.rs` 执行计划。`publish.rs` 负责同步时校验临时文件并替换目标。`report.rs` 负责生成报告和格式化操作明细。本地和云端扫描共用 `services::path_filter` 的路径过滤规则，同步和目录上传共用 `services::local_scan` 的本地扫描。

`services/upload/mod.rs` 定义上传入口和普通文件上传流程。`services/upload/` 中的 `directory.rs` 负责递归上传目录，`transfer.rs` 负责计算哈希和上传分片，供普通上传和同步共用。
