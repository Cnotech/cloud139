# 项目结构

```
cloud139/
├── Cargo.toml                     # Workspace 清单
└── packages/
    ├── cli/
    │   ├── Cargo.toml             # cloud139-cli 命令行 package
    │   ├── src/
    │   │   ├── main.rs            # CLI 入口
    │   │   ├── cli/               # clap 参数定义
    │   │   ├── commands/          # 调用服务、映射退出码
    │   │   ├── config/            # 配置路径、续期写回及跨进程锁
    │   │   ├── logger.rs          # 终端日志输出
    │   │   └── presentation/     # JSON、文本与进度条
    │   └── tests/                 # CLI 集成测试
    └── lib/
        ├── Cargo.toml             # cloud139 库 package
        ├── src/
        │   ├── lib.rs             # 库入口
        │   ├── config.rs          # 显式路径的配置读写
        │   ├── client/            # API 请求与认证
        │   ├── services/          # 文件操作及同步
        │   ├── domain/            # 业务数据
        │   ├── models/            # API 数据模型
        │   ├── protocol.rs        # 云盘端点、远程返回码和其他协议固定值
        │   └── utils/             # 日志、重试及通用函数
        └── tests/                 # 库集成测试
```

CLI 单向依赖 `cloud139`。CLI 每次调用选择配置文件，持锁更新凭据，再把加载好的配置交给服务。库接受调用方传入的配置，返回结构化结果，并通过 `log` 和进度回调向宿主报告进展。

`client/api/common/` 实现路由查询、Token 续期等共用接口，与 `client/api/storage/` 中按云盘类型划分的接口并列。两者共用 `client/http/` 提供的 HTTP 客户端、签名请求和请求头拼装。`client/api/mod.rs` 统一重导出调用入口。
