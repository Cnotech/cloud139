# 项目结构

## CLI

`cloud139-cli` 构建名为 `cloud139` 的可执行文件，单向依赖库 `cloud139`。工作区默认构建 CLI。两个包的直接依赖、feature 和平台条件分别见 [CLI Cargo.toml](../packages/cli/Cargo.toml) 和[库 Cargo.toml](../packages/lib/Cargo.toml)，锁定版本见 [Cargo.lock](../Cargo.lock)。

| 位置 | 职责 |
| --- | --- |
| [`src/main.rs`](../packages/cli/src/main.rs) 和 [`src/cli.rs`](../packages/cli/src/cli.rs) | 进程入口、clap 参数声明、日志初始化、退出码 |
| [`src/commands/`](../packages/cli/src/commands/) | `dispatch.rs` 负责分发和共用配置准备。各子命令适配参数、调用库服务并处理结果 |
| [`src/config/`](../packages/cli/src/config/) | 配置路径选择、持锁保存与续期写回 |
| [`src/presentation/`](../packages/cli/src/presentation/) | 文本、JSON 和终端进度条 |

CLI 使用显式的 `--config` 路径，省略时按当前目录、用户配置目录的顺序查找。需要续期时，CLI 对加载配置的同一路径持锁更新，随后重读并校验配置。删除和同步的参数前置检查在续期前执行。库操作入口校验外部配置的 `storage_type`，内部调用可复用 `PersonalConfig`。

## 库

| 位置 | 职责 |
| --- | --- |
| [`src/config/`](../packages/lib/src/config/) | TOML 读写、原子保存和个人云配置校验 |
| [`src/client/`](../packages/lib/src/client/) 和 [`src/protocol.rs`](../packages/lib/src/protocol.rs) | 认证、HTTP 与签名、个人云请求和协议常量 |
| [`src/services/`](../packages/lib/src/services/) | 文件和目录操作、传输与同步 |
| [`src/models/`](../packages/lib/src/models/) | 请求响应模型、文件条目、同步计划和结果所用类型 |
| [`src/progress.rs`](../packages/lib/src/progress.rs) | 库与宿主之间的传输进度接口 |

- 文件查询和下载链接从 `client::api::files` 进入，上传请求位于 `client::api::upload`，Token 解析和续期分别位于 `client::auth::token` 和 `client::auth::refresh`。
- 文件和目录上传在 `services::upload` 协调。目录上传和同步共用本地扫描，普通上传和同步共用分片传输。
- 同步由 `services::sync::plan` 校验方向、并发和排除规则，生成计划后交给执行器或校验流程。文件替换与清理的行为见[目录操作实现与验证](directory-operations.md)。
