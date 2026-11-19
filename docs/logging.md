# 日志与输出

业务代码统一通过所在包的宏记录日志。CLI 日志器渲染终端输出并协调 `MultiProgress` 重绘，库日志通过 `log` 交给宿主。实现见 [CLI 日志器](../packages/cli/src/logger.rs)和[库日志器](../packages/lib/src/utils/logger.rs)。

| 宏 | 用途 |
| --- | --- |
| `step!` | 操作阶段 |
| `info!` | 普通提示 |
| `success!` | 操作成功 |
| `warn!` | 值得关注的情况 |
| `error!` | 执行失败 |
| `debug!` | 排查细节，CLI 使用 `-v debug` 开启 |

- 自有消息用简体中文说明动作或结果。调试字段用稳定的英文 `key=value`，例如 `status=403`。日志宏自带级别前缀。
- 各级日志都应排除凭据和带签名的 URL。记录请求或响应时按需提取安全字段，例如 HTTP 方法和状态码。
- CLI 默认将错误写入 stderr，其余日志写入 stdout。`sync --json` 将日志写入 stderr，stdout 专用于 JSON，进度条关闭。命令结果由 `presentation/` 渲染，写入 stdout 时使用 CLI 的 `print!`。颜色由 `anstream` 根据输出流处理，可用 `NO_COLOR` 关闭。
- 库通过 [`TransferProgress`](../packages/lib/src/progress.rs)报告传输进度。进度按字节更新，重试从头开始时重置，传输结束时收尾。CLI 的进度条实现见 [`presentation/progress.rs`](../packages/cli/src/presentation/progress.rs)。
