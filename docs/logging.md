# 控制台输出与日志规范

> 目标：统一 CLI 控制台输出风格，确保信息美观、清晰、状态表达准确，符合现代命令行工具最佳实践。

## 1. 核心原则

- **禁止裸打印**：代码中禁止使用 `println!` / `eprintln!` / `log::` 系列进行输出，统一使用 `src/utils/logger.rs` 提供的宏与辅助函数。
- **状态语义化**：每条输出必须让玩家（用户）明确知道当前是「正在进行」「已完成」「需要关注」还是「出错了」。
- **视觉一致性**：同一级别使用统一颜色与前缀，避免各行其是。
- **进度条安全**：在含进度条（`ProgressBar` / `MultiProgress`）的交互中，必须通过 `pb_*` / `mp_*` 函数输出，防止日志撕裂进度条。
- **克制输出**：非必要不刷屏，非调试不暴露内部实现细节（Token、URL 参数、完整 JSON 等）。
- **国际化语境**：本项目面向中文用户，**面向用户的提示信息使用中文**，代码内部调试日志可使用英文。

## 2. 日志级别与使用场景

| 宏 / 函数 | 颜色 | 输出流 | 使用场景 |
|---|---|---|---|
| `success!` | 绿 (`\x1b[32m`) | stdout | 操作已成功完成，给用户正向反馈 |
| `step!` | 蓝 (`\x1b[34m`) | stdout | 标记一个操作步骤或阶段性进展 |
| `info!` | 青 (`\x1b[36m`) | stdout | 普通提示信息，如配置路径、等待原因 |
| `warn!` | 黄 (`\x1b[33m`) | stdout | 警告：不会阻断流程，但用户应当知晓 |
| `error!` | 红 (`\x1b[31m`) | stderr | 错误：操作失败或无法继续，需用户干预 |
| `debug!` | 灰 (`\x1b[90m`) | stdout | 调试信息：仅在 `-v debug` 时输出 |

### 2.1 场景示例

```rust
// ✅ 正确：用 step! 提示当前阶段
step!("正在计算同步差异...");

// ✅ 正确：用 success! 反馈结果
success!("下载完成: {}", filename);

// ✅ 正确：用 warn! + error! 组合提示风险并要求确认
warn!("此操作将永久删除文件，无法恢复！");
error!("请使用 --force 参数确认继续");

// ❌ 错误：直接用 println! 裸露输出
println!("登录成功");

// ❌ 错误：在普通信息里暴露内部详情
info!("API 响应: {:?}", full_json);
```

## 3. 消息格式规范

### 3.1 基础格式

- **不要重复级别标签**：宏已自带带颜色的 `success` / `step` / `info` / `warn` / `error` / `debug` 前缀，消息文本中不要再写「错误:」「警告:」等字样。
- **避免句尾多余标点**：单行简短消息不加句号；多句或完整段落可保留句号。
- **保持主动语态**：「下载完成」优于「文件已被下载」；「正在连接」优于「连接中」。
- **关键数据后置**：将文件名、路径、数量等变量放在句子后半段，方便扫描。

```rust
// ✅ 正确
success!("同步完成: 上传 3 个, 删除 1 个");
step!("开始下载到: {}", local_path);
error!("请使用 --force 参数确认继续");

// ❌ 错误：重复标签 + 被动语态 + 多余句号
success!("成功: 文件已被下载完成。");
error!("错误: 请输入 --force 参数。");
```

### 3.2 多语言与内部调试

| 受众 | 语言 | 示例 |
|---|---|---|
| 终端用户 | 中文 | `success!("登录成功")` |
| 开发者调试 | 英文（推荐） | `debug!("token refreshed, expires_at={}", ts)` |

## 4. 进度条感知日志

当命令使用 `indicatif` 进度条时，普通宏会直接覆盖或撕裂进度条渲染区域，必须使用以下封装函数。

### 4.1 单进度条 (`Option<ProgressBar>`)

使用 `pb_*` 系列函数；当传入 `None` 时自动降级为对应宏。

```rust
use crate::utils::logger::{pb_step, pb_success, pb_error};

pub async fn download(pb: &Option<ProgressBar>) -> Result<()> {
    pb_step("开始下载...", pb);
    // ... 执行下载 ...
    pb_success("下载完成!", pb);
    Ok(())
}
```

| 函数 | 条件输出 | 说明 |
|---|---|---|
| `pb_debug(msg, &pb)` | 仅在 `-v debug` | 调试日志 |
| `pb_info(msg, &pb)` | 始终 | 普通提示 |
| `pb_step(msg, &pb)` | 始终 | 步骤提示 |
| `pb_success(msg, &pb)` | 始终 | 成功提示 |
| `pb_warn(msg, &pb)` | 始终 | 警告 |
| `pb_error(msg, &pb)` | 始终 | 错误 |

### 4.2 多进度条 (`&MultiProgress`)

并发下载/上传等多任务场景使用 `mp_*` 系列。

```rust
use crate::utils::logger::mp_info;
use indicatif::MultiProgress;

let mp = MultiProgress::new();
mp_info("所有任务已启动", &mp);
```

| 函数 | 说明 |
|---|---|
| `mp_debug(msg, mp)` | 调试（需 `-v debug`） |
| `mp_info(msg, mp)` | 普通提示 |
| `mp_step(msg, mp)` | 步骤提示 |
| `mp_success(msg, mp)` | 成功提示 |
| `mp_warn(msg, mp)` | 警告 |
| `mp_error(msg, mp)` | 错误 |

## 5. 错误信息规范

- **输出到 stderr**：所有错误必须使用 `error!`（已内置 `eprintln!`），确保管道安全。
- **用户可行动**：错误信息应暗示用户下一步能做什么，而非仅陈述失败。
- **不暴露原始技术栈**：面向用户的错误应转化为中文业务语义，原始错误通过 `debug!` 保留。

```rust
// ✅ 正确
error!("登录失败: Token 已过期，请重新执行 login 命令");
debug!("原始错误: {:?}", api_err);

// ❌ 错误：直接抛出原始错误给用户
error!("reqwest error: {:?}", api_err);
```

## 6. 调试信息规范

- **默认不可见**：`debug!` 仅在命令行指定 `-v debug` 时输出，生产环境保持静默。
- **结构化优先**：调试日志推荐 `key=value` 格式，便于 grep 与日志分析。
- **敏感信息脱敏**：调试日志中不得打印用户 Token、Cookie、密码等凭证，可打印脱敏后的前 4 位或哈希值。

```rust
// ✅ 正确
debug!("upload_part: file_id={}, part={}/{}", file_id, i, total);
debug!("token_prefix={}", &token[..4.min(token.len())]);

// ❌ 错误：泄露敏感信息
debug!("headers: {:?}", headers_with_cookie);
```

## 7. 进度条展示规范

除文本日志外，凡涉及大文件上传/下载、批量同步等耗时操作，**应当**提供进度条。

### 7.1 进度条使用原则

- **有始有终**：进度条创建后必须调用 `finish()` / `finish_with_message()`，避免僵尸进度条。
- **粒度合理**：分片上传时按分片数更新，而非每秒刷新；总进度与当前分片进度二选一，避免双层进度条视觉混乱。
- **失败清理**：任务异常退出时，调用 `abandon_with_message()` 或打印错误后 finish，不要让进度条卡住。
- **与日志共存**：进度条刷新期间的所有文本输出，必须通过 `pb_*` / `mp_*` 系列函数。

### 7.2 样式建议

使用 `indicatif` 默认风格或项目统一定制的样式模板，保持视觉一致：

```rust
let pb = ProgressBar::new(total_size);
pb.set_style(
    ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-")
);
```

## 8. 正反例速查表

| 场景 | ✅ 正确 | ❌ 错误 |
|---|---|---|
| 普通提示 | `info!("配置文件已保存到: {}", path)` | `println!("配置文件已保存到: {}", path)` |
| 操作成功 | `success!("复制成功")` | `println!("复制成功");` |
| 步骤提示 | `step!("正在解析目录结构...")` | `info!("步骤1: 正在解析目录结构...")` |
| 警告 | `warn!("云端已存在同名文件")` | `println!("警告: 云端已存在同名文件")` |
| 错误 | `error!("请使用 --force 参数确认继续")` | `println!("错误: 请使用 --force 参数确认继续")` |
| 调试 | `debug!("diff_result: {:?}", diff)` | `info!("diff_result: {:?}", diff)` |
| 进度条中输出 | `pb_step("正在连接...", &pb)` | `step!("正在连接...")` |
| 重复标签 | `error!("文件不存在")` | `error!("错误: 文件不存在")` |

## 9. 参考与最佳实践

- [12-Factor App: Logs](https://12factor.net/logs) — 将日志视为事件流
- [Command Line Interface Guidelines: Output](https://clig.dev/#output) — 现代 CLI 输出设计
- [indicatif 文档](https://docs.rs/indicatif) — Rust 进度条库

---

> 规范维护：当新增日志宏或调整颜色语义时，同步更新本文档与 `src/utils/logger.rs`。
