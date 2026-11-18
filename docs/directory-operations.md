# 目录操作实现与验证

## 协议依据与限制

使用项目 main `353bbf79bb54585742133247b6309234be089de9` 已有个人盘端点，并核对公开的 [AlistGo/alist 139 driver](https://github.com/AlistGo/alist/blob/main/drivers/139/driver.go) 与 [目录列表实现](https://github.com/AlistGo/alist/blob/main/drivers/139/util.go)（核对日期 2026-11-17）。只借助公开代码核验请求字段，没有使用真实 139 凭证或请求真实盘数据。

- 个人盘使用 `/file/create`、`/file/batchCopy`、`/file/batchMove`、`/file/update` 和 `/recyclebin/batchTrash`。
- 家庭盘列表使用 `catalogID`、`pageInfo.pageNum`，目录创建使用 `createCloudDoc` 的 `docLibName` 和真实父目录路径。文件和目录的 ID 必须从列表逐层解析，不能用用户路径代替 ID。
- 家庭盘分页 `totalCount` 在参考实现中可能表示当前页数量，因此一直读取到空页；重复 ID/名称会明确报错，且有页数上限。
- 群组列表使用真实 catalog ID、服务端 path、`startNumber/endNumber`；目录移动提交 `catalogList` 中的真实 ID，目录回收站删除使用完整服务端路径。
- 家庭盘移动、家庭/群组复制缺少可验证实现，明确拒绝；家庭盘目录重命名被参考实现标为不支持，保留明确错误。永久删除端点未验证，所有后端拒绝，绝不转为回收站删除后声称永久删除。
- 群组目录操作要求配置中已有可靠的 `root_folder_id`。不自动生成或猜测路径。

## 映射和兼容行为

`upload ./dir /dest` 保留目录本身，落在 `/dest/dir`；`upload ./dir/ /dest` 仅上传内容，落在 `/dest`。两种映射均保留空目录。复用 sync 本地扫描和底层单文件上传，始终计算并提交文件哈希，不采用 `compute_diff` 的大小判断。已有同名文件默认报错；个人盘 `--force` 沿用既有文件上传行为。家庭/群组同名文件明确拒绝，不猜测覆盖协议。

`download -r /dir ./new-name` 将目录内容存入 `./new-name`；目标已是目录或参数末尾有 `/` 时，在其下创建源目录名。远程根目录默认保存到 `download`。不带 `-r` 下载目录失败。现有普通文件可以写入，但不能通过符号链接写入，也不能把目录覆盖为文件。递归计划先完整扫描并校验名称、重复名称、本地类型冲突与现有路径的符号链接，再开始写入。

递归下载根据目标文件系统检测大小写及 Unicode 等名称冲突。每个文件完整下载到同目录临时文件后才替换目标，单个文件断流会保留该文件原内容。

`mkdir -p` 创建所有缺失父目录，已有目录成功，同名文件失败；不带 `-p` 要求父目录存在，且最终目录不存在。旧 `mkdir --force` 仅保留参数解析，不再允许自动改名。创建后重新解析并检查目录名称和类型，避免误用重命名后的 ID。

`ls -R` 返回所有目录的完整内容，包括空目录；JSON 为目录对象数组。普通 `ls` 的 JSON 保持原对象格式。个人盘保留游标保护，家庭/群组拒绝重复页；递归遍历拒绝重复目录 ID、深度超过 256 和条目超过 100000。

所有变更操作都规范化 `/`、重复斜杠、`.` 和 `..`，再检查根目录和自身/子目录。远程名称按跨平台本地路径约束检查（包括 Windows 设备名、反斜杠、冒号、控制字符）。复制/移动先预检全部源、目标目录和名称冲突，再发出变更。`--force` 不覆盖目录类型冲突。批量响应中的条目级 `success: false` 会使操作失败。

递归任务可能部分完成；发生错误返回失败，不自动回滚。除下述个人盘目录上传的逐文件处理外，遇到错误停止执行。家庭/群组变更 API 返回任务接受，不代表异步任务已经完成。路径检查拒绝已存在的 symlink；本工具不提供与其他进程并发替换本地路径的沙箱隔离保证。

个人盘目录上传会继续处理后续普通文件，最后汇总成功、超限未传和其他失败，有未完成项返回非零；目录创建等结构性错误仍停止。个人盘上传采用 5 GiB（5,368,709,120 字节，包含边界值）的保守上限，并非对所有账号权益的保证；家庭/群组上传不应用此规则。单文件在计算哈希前检查，执行前重新检查本地大小。

本地到个人盘 sync 在完整源清单上计算差异，再把需要上传的超限项标为 `UploadBlocked`。不先过滤源清单，避免 `--delete` 误删旧文件；如果同名云端条目是目录，其后代的删除动作也受保护。超限项单独计数，不计为成功或普通跳过。dry-run 显示原因且返回 0，实际执行有超限项时返回 1。下载及未变化的大文件不受上传限制。

## 离线与 mock 测试

`tests/directory_operations_test.rs` 使用状态化 loopback HTTP mock，覆盖目录与内容映射、缺失父目录、空目录、同大小文件不跳过、目录下载需 `-r`、全部分页、游标和目录循环、根别名、自身/子目录、目标类型、文件冲突、部分失败、恶意名称、Unix symlink。

`tests/legacy_directory_test.rs` 注入 mock 传输层，验证家庭盘第二页名称解析、真实 catalog ID、父路径、两后端重复分页、群组移动/重命名/回收站删除请求形状、家庭目录重命名拒绝、错误响应与条目级部分失败。

测试不读取真实配置。完整测试前先审查测试中已有网络调用；所有凭证为虚构，使用临时 HOME/XDG_CONFIG_HOME，HTTP_PROXY/HTTPS_PROXY/ALL_PROXY 指向不可用 loopback 端口，并仅对 localhost/127.0.0.1/::1 免代理。Cargo 先单独获取锁定依赖，然后使用 `--locked --offline` 测试。仓库 E2E 技能需要真实 Token 和云端写入，本次不执行。

```bash
cargo fmt --all -- --check
cargo test --locked --offline --no-fail-fast
cargo clippy --locked --offline --all-targets
```

CI 的 Unit Tests workflow 当前只触发 `main` push；新分支推送后没有 CI run 不能视为 CI 通过。未创建 PR、未合并。

## 本次验证结果

在隔离 Linux checkout 使用官方 Rust 1.100.0 stable：`cargo fmt --all -- --check` 通过；`cargo test --locked --offline --no-fail-fast` 共 630 项通过、0 失败、0 忽略（其中新增目录回归 26 项）；`cargo clippy --locked --offline --all-targets` 通过，保留仓库已有测试风格与未使用依赖提示。本次未运行真实云端 E2E，未在 macOS/Windows 实机执行测试。

## 目录 E2E 计划入口

[cloud139-e2e-test 技能](../.agents/skills/cloud139-e2e-test/SKILL.md) 已按目录功能更新分阶段用例，包括目录/内容映射、逐文件 SHA256 与空目录、分页 JSON 兼容、变更冲突及失败不变、各后端支持范围、异步有界轮询和本轮对象清理。其[本地夹具工具](../.agents/skills/cloud139-e2e-test/scripts/fixture_tool.py) 只生成虚构数据和目录清单，不调用云盘。

用户请求真实执行时，由主代理生成并登记本轮精确 UUID 子树，采用独立本地临时目录和显式配置，不读写真实全局配置；不得用固定分页目录、根目录既有文件或仓库核心文件作夹具。根目录变更、永久删除拒绝、恶意响应及故障注入仅在离线 mock 中验证。编写这些用例不代表它们已经在真实盘执行；上面的 630 项结果属于目录实现的单元/mock 回归，不能当作真实 E2E 结果。
