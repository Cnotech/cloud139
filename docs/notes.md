# 开发注意事项

## 强制要求

- 禁止直接使用 `println!` / `eprintln!` / `log::`，统一使用各 package 的日志宏
- 错误类型需要实现 `Error` trait（推荐使用 `thiserror`）
- async 函数使用 `tokio` 运行时
- 配置文件使用 TOML 格式

## 架构约束

- 遵循分层架构，避免跨层依赖
- CLI 参数在 `cli.rs` 中定义，由命令层处理执行
- 命令层 (`commands/`) 作为适配器，连接 CLI 和应用层
- 应用层 (`packages/lib/src/services/`) 包含核心业务逻辑
- 文件和同步使用的业务类型位于 `models/file_item.rs` 和 `models/sync_item.rs`，只依赖标准库
- 展示层 (`presentation/`) 只处理输出格式化

## API 相关

- CLI 和 API 客户端使用个人云协议。
- 命令入口校验 `storage_type` 为 `personal_new`，通过参数前置检查后按需续期。
- 登录 Token 从浏览器开发者工具获取。
- HTTP 请求使用 `reqwest`。

## 上传实现

- `services/upload.rs` 协调文件和目录上传，处理分片读取与进度。
- `client/api/upload.rs` 构造个人云上传 API 请求并解析结果。

## 测试

- 测试文件的目录和内联单测例外见 [AGENTS.md 的测试文件编写规范](../AGENTS.md#测试文件编写规范)
- 测试可能需要真实 API 调用或 mock（使用 `httpmock`）

## 日志与输出

- CLI 自有日志正文使用简体中文，包括调试日志
- 调试字段使用稳定的英文 `key=value`，详见 [日志规范](logging.md)
- CLI 日志输出会协调进度条重绘，库通过进度回调报告传输进展
- 敏感信息（Token、Cookie）不得打印到日志
