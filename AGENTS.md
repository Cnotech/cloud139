# cloud139 开发指南

## 代码边界

- `packages/cli`（`cloud139-cli`）负责参数、配置文件选择和凭据续期写回、输出及退出码。每个子命令在 `commands/` 下保留独立适配器，别名复用同一适配器。库 `packages/lib`（`cloud139`）负责认证、协议和文件操作，通过显式配置或路径接收输入。见[项目结构](docs/structure.md)。
- CLI 和库的操作入口仅支持个人云 `personal_new`。入口校验配置，内部复用已校验的配置；需要前置校验的命令在凭据续期和远端操作前检查参数。
- 目录模块使用 `<模块名>/mod.rs`，实现按职责放在对应文件。调用方优先从定义模块导入。见[代码约定](docs/style.md)。
- 自有代码注释和日志正文使用简体中文。使用各包的日志宏，调试字段使用稳定的英文 `key=value`。Token、Cookie 和其他凭据不得出现在日志中。见[日志与输出](docs/logging.md)。

## 验证

- 公开 API 或 CLI 行为测试放在对应包的 `tests/`，需要访问 crate 内部接口的独立测试放在 `src/tests/`。私有实现的单测可在对应源码文件中用 `#[cfg(test)]` 内联。
- 常用检查为 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked` 和 `cargo test --workspace --locked`。构建和定向测试见[开发命令](docs/commands.md)。
- CLI 命令变更后，立即核对 [cloud139-e2e-test 的 SKILL.md](.agents/skills/cloud139-e2e-test/SKILL.md)。检查命令示例、测试矩阵和预期结果是否覆盖变更后的参数、默认值及行为，有遗漏或过时内容就随本次变更一并更新。
- 用户要求在真实云盘测试时，使用 [cloud139-e2e-test 技能](.agents/skills/cloud139-e2e-test/SKILL.md)。

修改命令参数、默认值或行为时，对照 `--help` 和相关测试更新 [README.md](README.md)。格式见[命令文档约定](docs/command-docs-style.md)。
