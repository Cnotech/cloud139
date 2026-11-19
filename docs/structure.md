# 项目结构

```
cloud139/
├── Cargo.toml                    # Workspace 与 CLI package
├── src/
│   ├── main.rs                    # CLI 入口
│   ├── cli/                       # clap 参数定义
│   ├── commands/                  # 调用服务、映射退出码
│   ├── config/                    # CLI 配置路径、续期写回及跨进程锁
│   ├── logger.rs                  # 终端日志输出
│   └── presentation/             # JSON、文本与进度条
└── crates/cloud139-core/
    ├── Cargo.toml                 # 库 package
    ├── src/
    │   ├── lib.rs                 # 库入口
    │   ├── config.rs              # 显式路径的配置读写
    │   ├── client/                # API 请求与认证
    │   ├── services/              # 文件操作及同步
    │   ├── domain/                # 业务数据
    │   ├── models/                # API 数据模型
    │   └── utils/                 # 日志、重试及通用函数
    └── tests/                     # 库集成测试
```

CLI 单向依赖 `cloud139-core`。CLI 每次调用选择配置文件，持锁更新凭据，再把加载好的配置交给服务。库接受调用方传入的配置，返回结构化结果，并通过 `log` 和进度回调向宿主报告进展。
