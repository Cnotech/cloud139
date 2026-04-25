# 常用依赖

## 运行时依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| clap | 4.6 | CLI 参数解析 |
| reqwest | 0.13 | HTTP 客户端 |
| ureq | 3.2 | 同步 HTTP 客户端 |
| tokio | 1.51 | 异步运行时 |
| tokio-stream | 0.1 | 异步流处理 |
| futures-util | 0.3 | Future 工具 |
| serde | 1.0 | 序列化/反序列化 |
| serde_json | 1 | JSON 处理 |
| toml | 1.1 | TOML 配置文件解析 |
| thiserror | 2.0 | 错误类型定义 |
| anyhow | 1 | 通用错误处理 |
| chrono | 0.4 | 时间处理 |
| indicatif | 0.17 | 进度条 |
| async-trait | 0.1 | 异步 trait |
| directories | 5 | 目录路径处理 |
| base64 | 0.22 | Base64 编解码 |
| hex | 0.4 | 十六进制编解码 |
| aes | 0.8 | AES 加密 |
| aes-gcm | 0.10 | AES-GCM 认证加密 |
| sha1 | 0.11 | SHA-1 哈希 |
| sha2 | 0.11 | SHA-256 哈希 |
| md-5 | 0.11 | MD5 哈希 |
| cipher | 0.5 | 加密算法抽象 |
| cbc | 0.1 | CBC 模式 |
| digest | 0.11 | 哈希算法抽象 |
| generic-array | 1.3 | 泛型数组 |
| typenum | 1.17 | 类型级数字 |
| rand | 0.8 | 随机数生成 |
| regex | 1 | 正则表达式 |
| url | 2 | URL 解析 |
| urlencoding | 2 | URL 编码 |
| glob | 0.3 | 全局匹配 |
| quick-xml | 0.39 | XML 解析/序列化 |
| serde-xml-rs | 0.8 | XML 反序列化 |
| filetime | 0.2 | 文件时间戳处理 |

## 开发依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| tempfile | 3 | 临时文件/目录（测试） |
| mockall | 0.14 | Mock 框架（测试） |
| httpmock | 0.8 | HTTP Mock 服务器（测试） |
| serde_json | 1 | JSON 处理（测试） |
