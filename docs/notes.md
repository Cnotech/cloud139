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

- 139 云盘 API 区分三种存储类型: `PersonalNew`, `Family`, `Group`
- 存储类型定义在 `domain/storage_type.rs`，不在 `client/` 下
- 某些操作在不同存储类型下行为不同（如重命名、批量移动）
- 登录 Token 需要从浏览器开发者工具获取
- API 客户端同时使用了 `reqwest`（异步）和 `ureq`（同步）两种 HTTP 客户端

## 上传实现

- 上传逻辑按存储类型拆分到 `application/services/upload/` 下
- `personal_parts.rs` 处理个人云分片上传
- 上传服务调度器在 `upload_service.rs` 中根据 `StorageType` 分发

## 测试

- 单元测试写在对应源文件内的 `#[cfg(test)]` 模块
- 集成测试放在 `tests/` 目录（如有）
- 测试可能需要真实 API 调用或 mock（使用 `httpmock`、`mockall`）

## 日志与输出

- 用户面向信息使用中文
- 调试日志推荐使用英文和 `key=value` 格式
- 进度条场景必须使用 `pb_*` / `mp_*` 系列函数
- 敏感信息（Token、Cookie）不得打印到日志
