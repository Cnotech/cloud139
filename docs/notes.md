# 开发注意事项

## 强制要求

- 禁止直接使用 `println!` / `eprintln!` / `log::`，统一使用 `src/utils/logger.rs` 提供的日志宏
- 错误类型需要实现 `Error` trait（推荐使用 `thiserror`）
- async 函数使用 `tokio` 运行时
- 配置文件使用 TOML 格式

## 架构约束

- 遵循分层架构，避免跨层依赖
- CLI 层 (`cli/`) 只负责参数定义，不直接调用基础设施
- 命令层 (`commands/`) 作为适配器，连接 CLI 和应用层
- 应用层 (`application/`) 包含核心业务逻辑
- 领域层 (`domain/`) 不依赖外部框架
- 展示层 (`presentation/`) 只处理输出格式化

## API 相关

- CLI 和 API 客户端使用个人云协议。
- 加载操作配置时校验 `storage_type` 为 `personal_new`。
- 登录 Token 从浏览器开发者工具获取。
- HTTP 请求使用 `reqwest`。

## 上传实现

- `services/upload_service.rs` 协调文件和目录上传。
- `services/upload/` 处理个人云上传流程、分片读取和进度。
- `client/api/upload.rs` 构造上传 API 请求并解析结果。

## 测试

- 单元测试写在对应源文件内的 `#[cfg(test)]` 模块
- 集成测试放在 `tests/` 目录（如有）
- 测试可能需要真实 API 调用或 mock（使用 `httpmock`）

## 日志与输出

- CLI 自有日志正文使用简体中文，包括调试日志
- 调试字段使用稳定的英文 `key=value`，详见 [日志规范](logging.md)
- 进度条场景必须使用 `pb_*` / `mp_*` 系列函数
- 敏感信息（Token、Cookie）不得打印到日志
