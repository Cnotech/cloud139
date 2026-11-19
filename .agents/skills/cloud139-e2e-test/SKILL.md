---
name: cloud139-e2e-test
description: 编写、维护或执行 cloud139 CLI 端到端回归，核对目录映射、内容完整性、后端支持范围和清理。用户要求真实测试时使用本轮独立云端子树。
---

## 功能描述与执行边界

覆盖 login、ls、upload、download、cp、rename、mv、mkdir、rm 和个人盘 sync。按[目录实现说明](../../../docs/directory-operations.md)及当前源码核对预期。按用户请求选择维护文档、离线验证或真实测试。

- **仅编写/维护用例**：只编辑技能及必要文档，静态核对命令、链接和夹具工具。不得索取、读取或使用真实 Token，不登录、不访问真实云盘、不创建或清理云端数据。
- **离线验证**：使用虚构配置、隔离 HOME 和 loopback mock。根目录变更、已移除参数的解析拒绝、恶意服务端名称、符号链接、循环分页和部分失败注入只在离线 mock 中测试。
- **真实 E2E**：用户要求在真实云盘执行测试或复测时，主代理按请求的个人云用例范围生成并登记本轮 UUID 子树，直接执行所需的登录校验、根目录查重、夹具操作和清理。根目录列表只用于检查本轮 UUID 名称。沿用用户指定的测试限制，只有缺少执行所需的信息时才询问用户。
- 所有真实变更必须限制在本轮登记的子树内。不向真实根目录发出删除、移动、复制根等负向操作；不按文件名清理任何既有用户对象，不复用固定分页目录，不下载到仓库工作目录覆盖源码或文档。

## 执行模式

真实 E2E 保留“主代理编排 + sub agent 分阶段执行”，不能单代理从头到尾串行硬跑。仅维护文档时不启动真实 E2E 阶段。

主代理在开头和结尾执行工作树门禁；真实测试使用用户安全提供的 Token；维护台账、后端支持矩阵和阶段依赖；串行调度共享状态；审核 cleanup sub agent 的对象清单及结果，汇总报告。不得打开用户真实全局配置或让 sub agent 自行扩大作用域。

阶段 sub agent 只执行已分配步骤，接收明确的配置路径、云端子树、本地目录和允许的读写动作；回传用例 ID、脱敏命令、退出码、内容验证证据、创建对象和清理义务。碰到共享状态冲突、超时或来源不明对象立即停止该阶段。

分派阶段：1 预检与登录；2–5 列表/上传/下载；6–10 变更类命令；11 sync；12 专门 cleanup sub agent。相同配置文件、同一本地目录或云端目录的写操作必须串行；只读核验或确认完全不相交的夹具可并行。前置夹具失败时，依赖用例记为“阻塞”，不得记作通过。

## 1. 工作树门禁、隔离与台账

真实执行第一步（在仓库根目录）：

```bash
git status --short
```

工作树有改动时，使用干净 checkout 测试。需要测试尚未提交的改动时，在独立 checkout 保存并核对待测快照，确认检出干净后构建。保留原工作树，避免自动 stash、reset 或清理未知文件。无法得到可验证的快照时报告阻塞。

门禁通过后记录仓库绝对路径 `REPO`、提交 SHA 和构建二进制绝对路径 `BIN`。如需构建，在干净 checkout 使用 `cargo build --locked --release`，不要为运行本文修改业务代码。为本轮生成独立 UUID、本地临时目录和云端子树，并在发起真实命令前登记：

```bash
RUN_ID=$(python3 -c 'import uuid; print(uuid.uuid4().hex)')
LOCAL=$(mktemp -d "${TMPDIR:-/tmp}/cloud139-e2e-${RUN_ID}.XXXXXX")
LOCAL=$(python3 -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$LOCAL")
CLOUD="/cloud139-e2e-${RUN_ID}-personal"
CONFIG="$LOCAL/profile/cloud139rc.toml"
FIX="$LOCAL/fixtures"
TOOL="$REPO/.agents/skills/cloud139-e2e-test/scripts/fixture_tool.py"
mkdir -p "$LOCAL/profile" "$LOCAL/logs" "$LOCAL/manifests" "$LOCAL/downloads" "$LOCAL/work"
chmod 700 "$LOCAL" "$LOCAL/profile"
python3 "$TOOL" init "$FIX"
python3 "$TOOL" manifest "$FIX/tree" --output "$LOCAL/manifests/tree.json"
python3 "$TOOL" manifest "$FIX/paged" --output "$LOCAL/manifests/paged.json"
python3 "$TOOL" manifest "$FIX/empty" --output "$LOCAL/manifests/empty.json"
cd "$LOCAL/work"
c139() { "$BIN" --config "$CONFIG" "$@"; }
```

`REPO` 和 `BIN` 由主代理在台账中明确赋值后才使用；`BIN` 必须指向本轮要验证的构建。Windows 使用 Git Bash 并设置 `MSYS_NO_PATHCONV=1`；路径参数使用 `/`，不把本文 shell 代码直接作为 PowerShell 代码执行。

台账至少包含：RUN_ID、测试请求、限制及精确 CLOUD、后端/账号脱敏标识、配置与临时 HOME 路径、提交/二进制、每阶段负责人、每条用例的前置状态/预期/实际退出码、日志/JSON/哈希清单、确认创建过的云端对象及其 ID（若接口可见）、部分完成或结果不明对象、清理状态。不得记录 Token 或完整配置内容。每个夹具在上传前登记，API 结果不明时记为“待核验”，不凭名称就加入可删除清单。

[本地夹具工具](scripts/fixture_tool.py) 只生成测试数据与内容清单，不调用 CLI 或网络。

**夹具约定**：`tree` 含 3 个文件、多层子目录、`empty` 和 `nested/empty`；`paged` 根目录含 121 个文件和 `child`、`empty` 两个目录（普通 ls 的 total 为 123），`child` 含另 121 个文件；`same-v1/same.bin` 为 `AAAA`，`same-v2/same.bin` 为 `BBBB`（都是 4 字节，SHA256 不同）；`duplicates/left` 和 `right` 各含一个同名但内容不同的 `shared.txt`。这些都是工具生成的测试数据，不能改用仓库核心文件或用户文件。

## 2. 支持矩阵与判定规则

| 能力 | personal_new |
|------|--------------|
| upload 目录/内容、download -r、mkdir -p、ls -R、rm -r | 支持 |
| cp 文件/非空目录 | 单源和多源 |
| mv 文件/非空目录 | 单源和多源 |
| rename 文件 | 支持 |
| rename 目录（包括同名 no-op） | 支持 |
| 已存在文件加 --force | upload/cp/mv/rename 沿用已有个人盘策略，必须核实实际结果 |
| 已存在目录/文件与目录类型冲突加 --force | 仍拒绝 |
| sync | 支持 |

配置错误和执行失败返回退出码 1。缺少必需参数等 clap 解析错误返回 2，sync 的方向和初始扫描错误也返回 2。不支持用例必须实际观察到对应拒绝且夹具不变，不能把一次网络错误当成“不支持”通过。

### 2.1 内容和“不变”判定

以本地完整目录清单为 oracle，不能只看大小、日志“成功”或文件总数：

```bash
python3 "$TOOL" manifest "$LOCAL/downloads/actual-tree" --output "$LOCAL/manifests/actual-tree.json"
python3 "$TOOL" compare "$LOCAL/manifests/tree.json" "$LOCAL/manifests/actual-tree.json"
```

manifest 同时比较所有相对目录路径（含空目录）、文件路径、字节数和逐文件 SHA256。清单存放在被扫描目录之外。每次云端断言均先 `ls -R -o` 核对名称/类型，再下载到本轮**新的、不存在的**核验路径生成清单。已有本地目标会触发追加源目录名的映射，不能混用作核验路径。

表中的“快照不变”表示：变更前拍下用例所属阶段子树的递归 JSON、下载清单；失败后重新拍摄并比较规范化后的路径/类型/大小/哈希，排除 modified 时间等不稳定字段。源和目标均要检查。远端 listing 没有 hash 时必须下载计算 SHA256。真实负向操作仅限本轮测试夹具；不得把根目录或服务端故障注入混进来。

### 2.2 有界轮询与异步后端

所有真实变更必须验证最终状态。退出码 0 或“任务已接受”只说明提交成功。

- 主代理为每个步骤设单调时钟截止时间，默认 120 秒、每 2 秒只读复查，最多 60 次；大批量分页夹具可在运行前明确记录更长的有限上限，禁止无限轮询。
- 用 `c139 ls -R "$STAGE" -o "$LOCAL/logs/步骤ID-尝试次数.json"` 检查所属阶段子树；目录被删除/移动/改名时查询仍存在的父目录，不能把任何网络错误都视为“不存在”。总清理查询根列表时只检查本轮 UUID 名称是否消失，不处理其他条目。
- 出现预期名称/类型后，下载到 `$LOCAL/downloads/步骤ID-尝试次数/tree` 这种每次全新的路径，比较完整目录清单及 SHA256。预期缺失的条目也要由成功的父列表证明。
- 最终谓词连续两次满足才通过；失败后“不变”同样复查。只轮询读操作，不能为了“等待”重复 cp/mv/rm/upload，不能补发 rm 来证明前一个 rm 已完成。
- 到期未收敛记为失败或结果不明，保留输出和台账，停止依赖变更；不重试不确定的写操作，不自动回滚。清理时先核对对象归属和在途任务，无法确认的对象报告待人工处理。

## 3. 分阶段测试矩阵

以下命令用于用户请求的真实测试，执行前须满足前置条件；`c139` 始终携带隔离 CONFIG。所有云端路径都在 CLOUD 内。每个阶段使用独立 `s02`–`s11` 子树，成功/重名/多源用例使用不同目标。

### 阶段 1：登录、配置与创建本轮根

| 步骤 | 命令/操作 | 验证点 |
|------|-----------|--------|
| 1.1 | 在 LOCAL 下的子进程创建独立 HOME/XDG_CONFIG_HOME；Windows 同时隔离 USERPROFILE，若无法证明平台默认配置目录位于 LOCAL，则配置回退用例改做离线单测 | 不读、不备份、不修改真实全局配置；不在父进程重写用户 HOME |
| 1.2 | `c139 login --token "$TOKEN"` | Token 使用用户安全提供并指定用于测试的凭证，禁用 shell xtrace、不记录参数值；只在新 CONFIG 上操作，观察内置只读 ls 校验 |
| 1.3 | `cargo test --locked --offline --test storage_support_test`（离线） | 使用虚构配置验证非个人云存储类型在网络访问前被拒绝 |
| 1.4 | 配置优先级：在独立 LOCAL/work 子目录分别放置显式 CONFIG、当前目录配置和临时 HOME 下的 `.config/cloud139/cloud139rc.toml`，用可区分的虚构标记配置和 loopback mock 验证显式 > 当前目录 > 临时全局；验证 login 写回来源 | 这部分默认离线，含失败登录不落盘、刷新回写和配置并发；不使用真实账户反复失败登录，不篡改真实 Token 来试服务端 |
| 1.5 | `c139 ls / -o "$LOCAL/logs/before-root.json"`，只检查新 UUID 名称不存在；`c139 mkdir "$CLOUD"` | CLOUD 已登记；若同名已存在立即停止，不复用、不改名、不删除它；成功创建并轮询确认后登记本轮根所有权 |
| 1.6 | `c139 ls "$CLOUD" -o "$LOCAL/logs/empty-root.json"` | 空目录、total 0；只创建本轮根，不清理任何历史对象 |

登录首先校验根目录列表，成功后原子保存隔离 CONFIG。失败应保留既有配置，首次登录失败不落盘，此行为通过 1.4 的离线用例验证。Token/完整配置不进入报告或仓库。后端准备失败时，只报告阻塞，不改真实用户配置来绕过。

### 阶段 2：ls、ls -R 和完整分页

前置：`c139 upload "$FIX/paged" "$CLOUD/s02"`，轮询清单与本地 paged 一致；令 `PAGE="$CLOUD/s02/paged"`。这批共 242 个小文件只在本轮新子树内生成，禁止复用固定远端目录。

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 2.1 | `c139 ls "$PAGE" -p 1 -s 50 -o "$LOCAL/logs/ls-page1.json"` | 0；JSON 顶层是对象，path/page/page_size/total/items 保持普通 ls 格式；total=123、items=50 |
| 2.2 | 分别 `c139 ls "$PAGE" -p 2 -s 50 -o "$LOCAL/logs/ls-page2.json"` 和 `c139 ls "$PAGE" -p 3 -s 50 -o "$LOCAL/logs/ls-page3.json"` | 0；50/23 个条目，无重复、并集与夹具根条目一致；数据不变期间执行，禁止按不稳定时间顺序假定固定文件名 |
| 2.3 | `c139 ls -R "$PAGE" -o "$LOCAL/logs/ls-tree.json"` | 0；文本含每个完整目录路径；JSON 顶层为数组，各对象含 path/total/items，含根、child 和空目录 empty；242 个文件恰好出现一次 |
| 2.4 | `c139 ls -R "$PAGE" -p 2 -s 1 -o "$LOCAL/logs/ls-tree-small-page.json"` | 0；递归模式忽略显示页码/数量选择，仍返回全部内容，规范化后与 2.3 一致；child 的 121 个文件也跨 API 默认 100 条分页完整返回 |
| 2.5 | `c139 ls "$PAGE/empty" -o "$LOCAL/logs/ls-empty.json"` | 0；普通 JSON 对象，total=0、items=[]；递归结果也包含该目录 |
| 2.6 | `c139 ls "$CLOUD/s02/not-created"` | 1；明确不存在，未创建新路径 |
| 2.7 | `"$BIN" -v debug --config "$CONFIG" ls "$PAGE"` | 0；全局 verbose 放子命令前，调试输出不泄露 Token |

递归 JSON 核验需把每个目录 path 相对 PAGE 化，将 items 的 name/type/size 与 paged manifest 投影比较；同时确认目录结果 path 唯一、条目名称唯一、所有空目录存在。普通分页三页并集比较同样用 path/name/type/size，不能把时间字段变化当成重复。

### 阶段 3：upload 目录与内容映射

每行独立目标均事先确认不存在。基准 tree manifest 在上传前固定，失败前后快照覆盖各自 s03 子树。

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 3.1 | `c139 upload "$FIX/tree" "$CLOUD/s03/keep"` | 0；目标是 `$CLOUD/s03/keep/tree`，不把文件直接放进 keep；逐文件哈希、多层目录及两处空目录与基准完全一致 |
| 3.2 | `c139 upload "$FIX/tree/" "$CLOUD/s03/contents"` | 0；目标内容直接在 contents，下方不多一层 tree；完整清单相同 |
| 3.3 | `c139 upload "$FIX/empty" "$CLOUD/s03/empty-container"`；另用 `c139 upload "$FIX/empty/" "$CLOUD/s03/empty-contents"` | 0；分别存在 empty-container/empty 和 empty-contents，均为空目录；即使没有文件也建立映射根 |
| 3.4 | `c139 upload "$FIX/tree" "$CLOUD/s03/missing/parents/dest"` | 0；自动创建缺失目标及所有父目录，落在 dest/tree，不能自动改名或丢空目录 |
| 3.5 | 准备 `c139 mkdir -p "$CLOUD/s03/same"`；`c139 upload "$FIX/same-v1/same.bin" "$CLOUD/s03/same"` | 0；下载证明确为 AAAA；保存原始 SHA256 |
| 3.6 | `c139 upload "$FIX/same-v2/same.bin" "$CLOUD/s03/same"` | 1；同名默认拒绝，AAAA 及命名快照不变；不得因大小相同返回成功却什么也没做 |
| 3.7 | `c139 upload --force "$FIX/same-v2/same.bin" "$CLOUD/s03/same"` | 个人盘允许提交；返回 0 后必须下载证明 BBBB 的 SHA256 确实出现在原名或明确记录的服务端新名称下，登记实际重名策略，不能只看日志或同样的 4 字节大小。 |
| 3.8 | `c139 upload "$FIX/tree-as-file/tree" "$CLOUD/s03/file-conflict"` 后 `c139 upload "$FIX/tree" "$CLOUD/s03/file-conflict"` | 前者 0；后者 1，文件 tree 不能变目录且其内容不变；即使 --force 也拒绝，快照无多余目录 |
| 3.9 | `c139 mkdir -p "$CLOUD/s03/dir-conflict/same.bin"` 后 `c139 upload --force "$FIX/same-v2/same.bin" "$CLOUD/s03/dir-conflict"` | 后者 1；同名目录不能变文件，目录清单不变 |
| 3.10 | `c139 upload "$FIX/not-created.bin" "$CLOUD/s03/keep"` | 1；本地源不存在；普通单文件上传到缺失远端目录也仍拒绝：`c139 upload "$FIX/files/a.txt" "$CLOUD/s03/not-created-dir"`，不能误套目录自动建父语义 |

| 3.11 | `c139 upload "$FIX/same-v1/" "$CLOUD/s03/same-directory"`，随后 `c139 upload "$FIX/same-v2/" "$CLOUD/s03/same-directory"` | 第一条 0；第二条 1，完整快照仍为 AAAA；验证目录路径也不能按同大小静默 Skip |
| 3.12 | `c139 upload --force "$FIX/same-v2/" "$CLOUD/s03/same-directory"` | 允许提交但必须以逐文件 SHA256 证明 BBBB 确实被处理，名称策略按 3.7 登记 |

### 阶段 4：上传后的完整性基线

| 步骤 | 命令/操作 | 验证点 |
|------|-----------|--------|
| 4.1 | `c139 ls -R "$CLOUD/s03" -o "$LOCAL/logs/upload-baseline.json"` | 每个目标映射、文件/目录类型、空目录均正确，没有意外自动改名目录 |
| 4.2 | 将 3.1、3.2、3.4 的有效目录分别下载到新的 LOCAL/downloads 基线路径，用 fixture_tool manifest/compare | 逐文件 SHA256、完整目录清单一致；上传返回内容哈希不是唯一证据 |
| 4.3 | 核对同大小变更 3.6/3.7 和所有冲突目标的前后清单 | 默认拒绝不变；强制成功不能静默跳过内容变化；重名附加对象均记入台账 |

### 阶段 5：download -r、目标映射与本地冲突

前置：独立上传 `c139 upload "$FIX/tree" "$CLOUD/s05"`，设置 `DLSRC="$CLOUD/s05/tree"`；验证源与基准一致。所有本地目标均在 LOCAL/downloads 下，绝不在仓库执行默认下载。

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 5.1 | `c139 download "$DLSRC/root.txt" "$LOCAL/downloads/renamed-file.txt"` | 0；单文件指定名称，SHA256 与 fixture root.txt 相同 |
| 5.2 | 在新的 LOCAL/work/default-file 子目录执行 `c139 download "$DLSRC/root.txt"` | 0；默认名 root.txt 仅写临时工作目录，内容一致 |
| 5.3 | `c139 download -r "$DLSRC" "$LOCAL/downloads/new-tree"` | 0；new-tree 原本不存在，直接包含 root.txt/nested/empty，不多 tree 层；完整 manifest 与基准一致 |
| 5.4 | 先创建空本地 existing；`c139 download -r "$DLSRC" "$LOCAL/downloads/existing"` | 0；已有目录映射为 existing/tree，逐文件 hash 和空目录一致 |
| 5.5 | `c139 download -r "$DLSRC" "$LOCAL/downloads/trailing/"` | 0；trailing 原本不存在也映射为 trailing/tree，完整 manifest 一致 |
| 5.6 | `c139 download "$CLOUD/s05/not-created.bin" "$LOCAL/downloads/missing.bin"` | 1；明确远程不存在，本地 missing.bin 不应创建 |
| 5.7 | `c139 download "$DLSRC" "$LOCAL/downloads/no-recursive"`；随后 `c139 download -r "$DLSRC" "$LOCAL/downloads/with-recursive"` | 前者 1 且无目标输出；后者 0 且完整 tree 清单一致，不再把目录下载统一写成不支持 |
| 5.8 | `c139 download -r "$DLSRC/empty" "$LOCAL/downloads/empty-new"` | 0；empty-new 是空目录，不能漏建或产生文件 |
| 5.9 | 分别准备冲突文件 conflict-new，以及已有 conflict-parent/tree/empty 文件，再下载到对应目标（见下文） | 两种情况均为 1；文件与目录冲突应在写入前拒绝，本地原始文件和全部清单不变 |
| 5.10 | 单文件下载到新建父目录路径和末尾 `/` 路径 | 自动建本地父目录；与远端原文件 SHA256 一致；使用不同新目标，避免一次创建的目录改变后续映射 |

5.9 的准确准备：创建 `$LOCAL/downloads/conflict-new` 作为**普通文件**后下载同名目录应失败且文件不变；另创建 `$LOCAL/downloads/conflict-parent/tree/empty` 普通文件，再执行 `c139 download -r "$DLSRC" "$LOCAL/downloads/conflict-parent"`，由于已有目录会追加 tree，恰好在 tree/empty 触发冲突。两种情况分开记录；不能用一个已存在目录当作“新目标”。

### 阶段 6：cp 非空目录、多源与预检

前置：`c139 upload "$FIX/tree" "$CLOUD/s06/source"`，`c139 upload "$FIX/files/" "$CLOUD/s06/files"`，`c139 upload "$FIX/duplicates/" "$CLOUD/s06/dup"`。分别创建全新 dest-single、dest-multi、dest-conflict；令 `CP_SRC="$CLOUD/s06/source/tree"`。禁止在单源测试已占用的目标里直接要求多源无冲突成功。

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 6.1 | `c139 mkdir -p "$CLOUD/s06/dest-single"` | 0；创建独立空目标，确认状态 |
| 6.2 | `c139 cp "$CP_SRC" "$CLOUD/s06/dest-single"` | 0，目标 dest-single/tree 与基准完整一致、源不变 |
| 6.3 | `c139 mkdir -p "$CLOUD/s06/dest-multi"` | 0；独立空目标，与 6.2 不共享 |
| 6.4 | `c139 cp "$CLOUD/s06/files/a.txt" "$CLOUD/s06/files/b.txt" "$CLOUD/s06/dest-multi"` | 0，两个文件都复制且源仍在，SHA256 一致 |
| 6.5 | `c139 cp "$CP_SRC" "$CP_SRC"`；`c139 cp --force "$CP_SRC" "$CP_SRC/nested"` | 1；禁止目录复制到自身和子目录，source 整体清单不变 |
| 6.6 | `c139 cp "$CP_SRC" "$CLOUD/s06/files/blocker.bin"`；另以阶段内未创建目标作 target | 1；目标文件/缺失目标被拒绝；不把目标参数当新文件名 |
| 6.7 | 先 `c139 upload "$FIX/tree" "$CLOUD/s06/dest-conflict"` 准备非空 tree，再 `c139 cp --force "$CP_SRC" "$CLOUD/s06/dest-conflict"` | 1；目录同名不能合并、覆盖或自动改名，源/目标完整快照不变 |
| 6.8 | 先 `c139 mkdir -p "$CLOUD/s06/file-conflict"`、`c139 upload "$FIX/tree-as-file/tree" "$CLOUD/s06/file-conflict"`，再 `c139 cp --force "$CP_SRC" "$CLOUD/s06/file-conflict"` | 1；文件/目录同名类型冲突不变 |
| 6.9 | 先 `c139 mkdir -p "$CLOUD/s06/file-repeat"`，复制 a.txt 到该目标，再不带 --force 重复复制 | 1；同名文件预检拒绝，快照不变 |
| 6.10 | 在 6.9 同一目标加 --force | 允许提交；0 后轮询校验新增副本实际名称/哈希及源不变，登记服务端重名结果，不预先写死名称 |
| 6.11 | `c139 cp --force "$CLOUD/s06/dup/left/shared.txt" "$CLOUD/s06/dup/right/shared.txt" "$CLOUD/s06/dest-multi"` | 1；多个源 basename 相同，即使 --force 也拒绝，不能沿用旧“自动改名成功”预期 |
| 6.12 | 先创建独立空 dest-preflight，再把 files/a.txt 与阶段内不存在源混合复制到该已有目录 | 1；全部预检后才变更，不允许已有源先被复制；双方完整快照不变 |

### 阶段 7：rename 非空目录与冲突

前置：向 s07/source 上传 tree，向 s07/files 上传 files 的内容。另依次用 `c139 upload "$FIX/tree" "$CLOUD/s07/conflict"`、`c139 upload "$FIX/tree/" "$CLOUD/s07/conflict/taken"` 和 `c139 upload "$FIX/files/blocker.bin" "$CLOUD/s07/conflict"`，准备非空 tree、非空 taken 和普通文件 blocker.bin；这些夹具与成功用例分离。

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 7.1 | `c139 rename "$CLOUD/s07/files/a.txt" renamed-a.txt` | 0，新名文件 hash 不变，旧名消失 |
| 7.2 | `c139 rename "$CLOUD/s07/source/tree" renamed-tree` | 0，完整非空目录与空子目录保留，新路径下 manifest 与基准一致，旧路径消失 |
| 7.3 | `c139 rename "$CLOUD/s07/source/renamed-tree" renamed-tree` | 0；已有目录同名 no-op，清单不变 |
| 7.4 | `c139 rename "$CLOUD/s07/not-created" new-name` | 1；源不存在，不能把它当 no-op 成功 |
| 7.5 | `c139 rename --force "$CLOUD/s07/conflict/tree" taken` | 1；非空目录同名冲突不变，不能强制合并 |
| 7.6 | `c139 rename --force "$CLOUD/s07/conflict/tree" blocker.bin` | 1；目录不能覆盖同名文件，双方不变 |
| 7.7 | `c139 mkdir -p "$CLOUD/s07/files/taken-dir"` 后 `c139 rename --force "$CLOUD/s07/files/b.txt" taken-dir` | rename 为 1；文件不能替换目录，双方快照不变；非法名称测试转离线 mock |
| 7.8 | `c139 rename "$CLOUD/s07/files/renamed-a.txt" renamed-a.txt` | 0，hash 和名称清单不变 |

### 阶段 8：mv 非空目录、多源与预检

前置：向 s08/source 上传 tree，向 s08/files 上传 files 内容，并 mkdir -p 创建独立 dest-dir、dest-multi 目标。另用目录上传分别在 s08/conflict-source 和 s08/conflict-dir 下创建非空 tree；在先 mkdir -p 创建的 s08/conflict-file 中上传 FIX/tree-as-file/tree 普通文件。用 `c139 upload "$FIX/files/" "$CLOUD/s08/same-source"` 和 `c139 upload "$FIX/other-files/" "$CLOUD/s08/same-name"` 准备同名 a.txt、不同哈希的源/目标。所有目标均在 CLOUD 下，不把文件搬进真实根目录。

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 8.1 | `c139 mv "$CLOUD/s08/source/tree" "$CLOUD/s08/dest-dir"` | 0，轮询旧路径消失，dest-dir/tree 完整清单与基准一致 |
| 8.2 | 在 dest-dir/tree下载和遍历 | 验证多层文件哈希和全部空目录，没有遗漏内容 |
| 8.3 | `c139 mv "$CLOUD/s08/files/a.txt" "$CLOUD/s08/files/b.txt" "$CLOUD/s08/dest-multi"` | 0，两源消失，目标两文件哈希一致 |
| 8.4 | 对专门的 conflict-source/tree 执行 mv 到自身以及其 nested 子目录，包含 --force 变体 | 1；清单不变 |
| 8.5 | 用阶段内文件 blocker.bin 或未创建路径作 mv target | 1；拒绝无效目标，源不动 |
| 8.6 | `c139 mv "$CLOUD/s08/same-source/a.txt" "$CLOUD/s08/same-name"` | 1；当前预检要求拒绝，源和目标哈希均不变，不再期待普通 mv 自动改名 |
| 8.7 | 在 8.6 后加 --force | 允许提交，0 后源消失、目标原文件和实际新名副本按 API 策略核验并登记 |
| 8.8 | 用 conflict-source/tree 为源，分别执行 mv --force 到 s08/conflict-dir 和 s08/conflict-file | 1；目录/目录及目录/文件冲突都不变 |
| 8.9 | 两个有效但 basename 同名的源一起移动；另测有效源+不存在源 | 均为 1；--force 不能放开多源同名；不得移动部分有效源，快照不变 |
| 8.10 | 将某文件移到它当前所在父目录 | 1；同目录冲突/无效操作，文件不应丢失或变名 |

### 阶段 9：mkdir -p、幂等与文件冲突

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 9.1 | 先 `c139 mkdir -p "$CLOUD/s09"`，再 `c139 mkdir "$CLOUD/s09/single"` | 0；只创建预期目录 |
| 9.2 | `c139 mkdir -p "$CLOUD/s09/missing/a/b/empty"` | 0；多层父目录和叶目录完整创建，最终为空目录 |
| 9.3 | 重复 9.2，并以等价路径 `$CLOUD/s09/missing//a/./b/empty/` 再执行 -p | 0；幂等，目录清单不变、无改名兄弟目录；仅使用保持在 CLOUD 内的等价路径 |
| 9.4 | 重复 `c139 mkdir "$CLOUD/s09/single"` | 1，已有目录报错，目录清单无新增项 |
| 9.5 | `c139 mkdir "$CLOUD/s09/not-created/child"` | 1；不带 -p 时父目录不存在必须报错，快照不变 |
| 9.6 | 先上传 blocker.bin 到 s09，再 `c139 mkdir -p "$CLOUD/s09/blocker.bin/child"` | 1；父节点是文件，原文件 hash 不变，无 child |
| 9.7 | `c139 mkdir -p "$CLOUD/s09/blocker.bin"` | 1，叶节点是文件，原文件哈希与目录清单不变 |

### 阶段 10：rm 显式 -r、--yes 和只删除本轮对象

前置：`c139 upload "$FIX/files" "$CLOUD/s10"`、`c139 upload "$FIX/empty" "$CLOUD/s10"`、`c139 upload "$FIX/tree" "$CLOUD/s10"`，并验证完整快照。所有删除仅针对已登记的本轮夹具。

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.1 | `c139 rm "$CLOUD/s10/files/a.txt"` | 1；缺 --yes，提示确认且文件内容不变 |
| 10.2 | `c139 rm --yes "$CLOUD/s10/files/a.txt"` | 0；只移入回收站；轮询成功父列表确认 a.txt 消失，其他文件不变 |
| 10.3 | `c139 rm --yes "$CLOUD/s10/not-created.bin"` | 1；明确不存在，不能误当成功 |
| 10.4 | `c139 rm -r "$CLOUD/s10/tree"` | 1；有 -r 也仍须 --yes，完整目录不变 |
| 10.5 | 已移除的旧参数 | 转阶段 13 进行 CLI 解析测试 |
| 10.6 | 根目录别名删除/移动安全负向 | 仅离线 mock；不向真实根目录发出测试操作 |
| 10.7 | `c139 ls -R "$CLOUD/s10/empty" -o "$LOCAL/logs/rm-empty-before.json"` | 0；确认该空目录是本轮创建且仍为空 |
| 10.8 | `c139 rm --yes "$CLOUD/s10/empty"` | 1；缺 -r 必须拒绝，空目录仍在、快照不变 |
| 10.9 | `c139 rm -r --yes "$CLOUD/s10/empty"` | 0；有界轮询父列表确认该空目录消失，其他条目不变 |
| 10.10 | 拍摄 s10/tree 完整清单和哈希 | 证明它是含文件/空子目录的本轮非空夹具 |
| 10.11 | `c139 rm --yes "$CLOUD/s10/tree"` | 1；非空目录缺 -r 也必须拒绝，全部内容不变 |
| 10.12 | `c139 rm -r --yes "$CLOUD/s10/tree"` | 0；轮询确认非空目录消失，不能只把任务接受算删除完成 |
| 10.13 | `c139 rm -r --yes "$CLOUD/s10/not-created-dir"` | 1；源不存在，其他对象不变 |

### 阶段 11：sync 回归（个人盘），与 upload 规则分开

设置 `SYNCSRC="$LOCAL/sync-source"`（从 FIX/sync-source 复制）、`SYNCDST="$LOCAL/sync-download"`（新目标）、`SYNC_REMOTE="$CLOUD/s11/target"`（先 mkdir -p）。空目录也纳入 manifest，不按脆弱的日志 Skip 数量作为唯一 oracle。

| 步骤 | 命令/操作 | 验证点 |
|------|-----------|--------|
| 11.1 | `c139 sync "$SYNCSRC" "cloud:$SYNC_REMOTE"`，再加 -r 执行 | 首次只传根下 2 文件；递归后补齐 subdir/sub.txt 和 empty；云端完整 manifest 与本地一致，0 |
| 11.2 | 再 `c139 sync -r "$SYNCSRC" "cloud:$SYNC_REMOTE"` | 0；无新增传输、完整清单不变；不把目录 Skip 统计误写成固定的 3 个文件 |
| 11.3 | 将 file1.txt 改为明显不同**字节数**，记录修改前云端 hash；`c139 sync -r -n "$SYNCSRC" "cloud:$SYNC_REMOTE"` | 0；有 DRY RUN 计划，下载/哈希核验云端内容未变，不能只看 ls 文件大小/时间 |
| 11.4 | 不带 -n 执行上一同步 | 0；仅预期内容更新，最终 manifest 一致。sync 本地→云端仍按大小判断，同大小内容变化可能 Skip；不能把 upload 的规则套在 sync 上 |
| 11.5 | 从 SYNCSRC 删除 file2.txt 和空目录 empty；先正常同步，再 `c139 sync -r --delete -n "$SYNCSRC" "cloud:$SYNC_REMOTE"` | 0；不带 --delete 时云端保留额外对象；dry-run 出现删除计划但内容不变 |
| 11.6 | `c139 sync -r --delete "$SYNCSRC" "cloud:$SYNC_REMOTE"` | 0；只删除本轮 target 内额外对象，轮询及 manifest 确认；不额外补发 rm 来判断完成 |
| 11.7 | 在新的 exclude-source 中生成假的 .env、output.log、subdir，再同步到**新的空 exclude-target**，传 `--exclude '*.log' --exclude '.env' --exclude 'subdir'` | 0；被排除条目不存在；旧目标已存在的 subdir 不会仅因 exclude 自动消失，不能用旧目标测试“未上传” |
| 11.8 | 在独立 jobs-source 生成 8 个小文件，对新 jobs-target 使用 `-r -j 2`，随后 `-r -j 8` | 0；首次完整传输后再无新增内容，hash 清单一致，不对同目标同时启动两次 sync |
| 11.9 | `c139 sync -r "cloud:$SYNC_REMOTE" "$SYNCDST"`，重复执行 | 0；下载清单一致、第二次无新增传输；若本地已存在旧目标，单独确认映射而不复用 download 的追加目录规则 |
| 11.10 | 将 SYNCDST/file1.txt 改为不同大小后再执行 11.9 | 0；恢复云端正确内容，逐文件 SHA256 一致；不使用仅改同大小内容的夹具期待强制覆盖 |
| 11.11 | `c139 sync "$LOCAL/not-created" "cloud:$SYNC_REMOTE" -r`；两端都是本地或都是本轮 CLOUD 路径的方向错误 | 2；扫描/方向失败，不创建远端对象；执行错误退出码 1 的故障注入留在 mock |
| 11.12 | 个人盘以 CLOUD/s11/not-created-dir 为云端 target 执行 sync | 当前云端根扫描要求目标已存在：2，未创建目标；不要将 upload 自动建父目录的预期套在这里 |

### 阶段 12：清理、收尾核验与报告

cleanup sub agent 先审核台账、登记的精确 CLOUD、创建成功证据和在途任务。只有能证明由本轮创建的对象才可清理。若写操作结果不明或出现外来对象，暂停该对象清理并报告，不能按类似名称猜测归属。

| 步骤 | 命令/操作 | 验证点 |
|------|-----------|--------|
| 12.1 | 对照台账和本轮根列表重新读取 CLOUD 的递归清单 | 所有实际副作用均已登记；无范围外对象；将脱敏报告和哈希证据保存到用户指定位置或当前任务可访问的安全位置 |
| 12.2 | `c139 rm -r --yes "$CLOUD"` | 仅在确认本轮根归属且用户请求未限制总清理时执行，使用 -r 清理本轮目录 |
| 12.3 | 有界轮询成功根列表，检查本轮 UUID 名称已消失 | 只检查 CLOUD 的缺失；根列表的其他条目不读内容、不变更；超时如实记录残留，不盲目重复删除 |
| 12.4 | 审核 LOCAL 确为本轮创建的规范化临时根，再清理其中配置、锁文件、下载、输出和夹具 | 只处理台账中的本地对象；不触碰真实全局配置，无需备份/恢复用户配置；凭证文件按安全要求移除，日志不得含 Token |
| 12.5 | 在 REPO 再执行 `git status --short`、`git diff --name-status` | 与测试前干净基线一致；未知变更/删除保留并报告，不自动 reset 或 git clean，不把不明变化当正常下载覆盖 |

最终报告列出：测试提交与构建、获准后端与 UUID 子树、各阶段通过/失败/阻塞/未跑、实际退出码、逐文件 SHA256/目录清单证据、异步轮询次数和耗时、支持限制、部分失败/未知结果、清理完成和残留对象、工作树前后状态。**真实未执行就明确写“真实云端 E2E 未执行”**；以前的单元测试通过数不能冒充本轮 E2E 结果。

### 阶段 13：仅离线 mock 的安全与故障矩阵

此阶段不需要也不得使用真实 Token。隔离 HOME/XDG_CONFIG_HOME（Windows 同时检查 USERPROFILE），虚构配置只指向 loopback；审查测试源后，仅运行已确认不会访问真实盘的测试。需要安装/下载依赖时单独完成，再 `--locked --offline`。HTTP_PROXY/HTTPS_PROXY/ALL_PROXY 指向不可用 loopback 端口，NO_PROXY 只放 localhost/127.0.0.1/::1；真实盘访问仍按用户请求执行。

| 步骤 | 命令/用例 | 验证点 |
|------|-----------|--------|
| 13.1 | `cargo test --locked --offline --test directory_operations_test` | 根路径别名和服务端根 ID 别名的 rm/mv/cp/rename 拒绝，确认 mock 没有变更请求 |
| 13.2 | `cargo test --locked --offline --test directory_operations_test removed_options_fail_before_config_changes` | 已移除的参数在解析阶段返回退出码 2，原配置不变 |
| 13.3 | 上述目录用例及 `cargo test -p cloud139-core --locked --offline --test pagination_cursor_guard_test` | 重复/循环游标、目录 ID 循环均报错；不将不完整列表当成功 |
| 13.4 | 恶意服务端文件名、绝对路径、穿越、Windows 设备名、源/目标 symlink | 拒绝前不写出本轮本地临时根、不覆盖受保护文件；symlink 相关平台限制明确记录 |
| 13.5 | 上传中途失败、批量条目 success=false、HTTP 错误/下载断流、无数据或失败列表 | 失败返回非零并准确报告已完成部分；不静默 Skip，不把错误当空目录，不自动重试不确定写请求 |
| 13.6 | Config 优先级、失败登录、刷新/并发写回；CLI 缺参数 | 全用虚构凭证/loopback 和临时配置验证，缺参数为 clap 退出码 2；不通过修改真实 Token 测试失败 |

部分条目可能需要补充 mock 才能完整覆盖。执行者必须逐项核对现有测试断言；缺少断言时写“未覆盖/待补 mock”，不得声称本文矩阵已经全部自动化或全部执行。阶段 13 可在真实 E2E 之前单独运行。
