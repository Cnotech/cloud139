# 日志与输出

业务代码统一通过所在包的宏记录日志。CLI 日志器渲染终端输出并协调 `MultiProgress` 重绘，库日志通过 `log` 交给宿主。实现见 [CLI 日志器](../packages/cli/src/logger.rs)和[库日志器](../packages/lib/src/utils/logger.rs)。

| 宏 | 用途 |
| --- | --- |
| `step!` | 操作阶段 |
| `info!` | 普通提示 |
| `success!` | 操作成功 |
| `warn!` | 值得关注的情况 |
| `error!` | 执行失败 |
| `debug!` | 排查细节，CLI 使用 `-v debug` 输出到终端，使用 `--log-file` 写入文件 |

- 自有消息用简体中文说明动作或结果。调试字段用稳定的英文 `key=value`，例如 `status=403`。日志宏自带级别前缀。
- 各级日志都应排除凭据和带签名的 URL。记录请求或响应时按需提取安全字段，例如 HTTP 方法和状态码。
- CLI 默认将错误写入 stderr，其余日志写入 stdout。`sync --json` 将日志写入 stderr，stdout 专用于 JSON，进度条关闭。命令结果由 `presentation/` 渲染，写入 stdout 时使用 CLI 的 `print!`。颜色由 `anstream` 根据输出流处理，可用 `NO_COLOR` 关闭。
- 库通过 [`TransferProgress`](../packages/lib/src/progress.rs)报告传输进度。进度按字节更新，重试从头开始时重置，传输结束时收尾。CLI 的进度条实现见 [`presentation/progress.rs`](../packages/cli/src/presentation/progress.rs)。

## 日志文件

`--log-file <PATH>` 为本次运行新建纯文本日志，使用毫秒时间戳 `ts_ms`。文件已存在、无法创建或与命令输入输出冲突时，CLI 在远端调用前以退出码 1 结束。文件包含 CLI 和库的日志，终端仍按原规则输出。服务端错误详情通过终端查看，文件保留错误级别、请求状态和命令退出码。运行中写入日志失败会提示一次，命令继续执行。

## 请求计数

每次调用请求封装时记录 `event=http_request_start`，重试也单独记录。按 `endpoint` 统计开始事件即可得到尝试次数。用 `request_id` 配对 `event=http_request_end` 查看 `status`、`elapsed_ms` 和 `outcome`。未获得响应时 `status=-`。请求被取消时可能只有开始事件。

签名 POST 的耗时包括 HTTP 响应和 JSON 解码，`outcome=ok` 表示这两步成功。续期请求记录读取响应文本的耗时。CDN 下载以收到响应头为止，分片上传记录到收到响应。端点来自协议常量或固定标签，请求头、请求体及带签名的地址均不进入请求日志。

`sync` 的 `event=sync_phase_start` 和 `event=sync_phase_end` 分别标记 `phase=scan` 与 `phase=execute`。检查执行阶段有多少次 `/file/list` 时，只统计该区间的开始事件。
