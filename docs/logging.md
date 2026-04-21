# 日志打印规范

项目在 `src/utils/logger.rs` 中定义了统一的日志宏和辅助函数，禁止使用 `println!`/`eprintln!`/`log::` 进行日志输出。

## 日志宏（直接使用）

| 宏 | 颜色 | 用途 | 输出 |
|---|---|---|---|
| `success!` | 绿 | 操作成功 | stdout |
| `step!` | 蓝 | 操作步骤提示 | stdout |
| `info!` | 青 | 用户提示信息 | stdout |
| `warn!` | 黄 | 警告 | stdout |
| `error!` | 红 | 错误 | stderr |
| `debug!` | 灰 | 仅在 `verbose=debug` 时输出 | stdout |

**使用规则：**
- 用户能看到的提示 → `success!`/`step!`/`info!`
- 需要注意但不阻断 → `warn!`
- 错误 → `error!`
- 仅调试时需要（API 响应详情、Token 刷新等） → `debug!`
- 不要在日志消息前硬编码级别标签（如 "警告:"、"错误:"），宏已自带颜色标签

## 进度条感知日志函数

在有进度条的场景下，必须使用 `pb_*` 或 `mp_*` 系列函数，确保输出在进度条上方而不破坏布局。

**单进度条（`Option<ProgressBar>`）：**

| 函数 | 用途 |
|---|---|
| `pb_debug(msg, &pb)` | 调试信息（需 `-v debug`） |
| `pb_info(msg, &pb)` | 提示信息 |
| `pb_step(msg, &pb)` | 步骤提示 |
| `pb_success(msg, &pb)` | 成功提示 |
| `pb_warn(msg, &pb)` | 警告 |
| `pb_error(msg, &pb)` | 错误 |

- `pb = None` 时自动降级为对应宏直接打印
- 定义在 `src/utils/logger.rs`，使用 `use crate::utils::logger::{pb_step, ...}` 导入

**多进度条（`&MultiProgress`）：**

| 函数 | 用途 |
|---|---|
| `mp_debug(msg, mp)` | 调试信息 |
| `mp_info(msg, mp)` | 提示信息 |
| `mp_step(msg, mp)` | 步骤提示 |
| `mp_success(msg, mp)` | 成功提示 |
| `mp_warn(msg, mp)` | 警告 |
| `mp_error(msg, mp)` | 错误 |

- 定义在 `src/utils/logger.rs`，使用 `use crate::utils::logger::mp_error;` 导入
