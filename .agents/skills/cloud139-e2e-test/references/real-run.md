# 真实测试的准备、验证和清理

用户要求在真实个人云测试时使用。先确认测试范围，再打开对应的用例表。

## 工作区和分工

- 测试前后记录 `git status --short` 和 `git diff --name-status`。工作区有改动时，在独立检出中构建待测快照并记录 SHA。保留原工作区的改动。
- 主代理负责预检、登录、台账和报告。按阶段分派用例，2–5 为传输，6–10 为目录变更，11 为同步，12 由专门的代理清理。各阶段共用一处请求日志目录。阶段代理只在分配的配置、云端子树、本地目录和权限内操作，反馈用例编号、脱敏命令、退出码、文件证据和待清理项。共享配置或目标的写入串行，独立夹具可并行。
- 台账登记用户要求、UUID 和精确云端根、脱敏账号、测试提交和二进制、配置和临时目录、负责人、预期与实际退出码、清单与哈希、对象归属和 ID（若可见）、不确定结果和清理状态。创建前先登记预期对象，结果不明时标为待核验。

## 阶段 1 · 登录和创建本轮根

`REPO` 和 `BIN` 使用已核对的绝对路径。需要构建时运行 `cargo build --locked --release`。Windows 使用 Git Bash，设 `MSYS_NO_PATHCONV=1`，云端路径用 `/`。

```bash
RUN_ID=$(python3 -c 'import uuid; print(uuid.uuid4().hex)')
LOCAL=$(mktemp -d "${TMPDIR:-/tmp}/cloud139-e2e-${RUN_ID}.XXXXXX")
LOCAL=$(python3 -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$LOCAL")
CLOUD="/cloud139-e2e-${RUN_ID}-personal"
CONFIG="$LOCAL/profile/cloud139rc.toml"
FIX="$LOCAL/fixtures"
TOOL="$REPO/.agents/skills/cloud139-e2e-test/scripts/fixture_tool.py"
REQUEST_LOGS="$REPO/.agents/skills/cloud139-e2e-test/scripts/request_logs.py"
LOG_DIR="$LOCAL/logs/requests"
mkdir -p "$LOCAL/home" "$LOCAL/profile" "$LOG_DIR" "$LOCAL/manifests" "$LOCAL/downloads" "$LOCAL/work"
chmod 700 "$LOCAL" "$LOCAL/profile"
python3 "$TOOL" init "$FIX"
for fixture in tree paged empty; do
    python3 "$TOOL" manifest "$FIX/$fixture" --output "$LOCAL/manifests/$fixture.json"
done
cd "$LOCAL/work"
c139() {
    local options=()
    if [[ -n ${EXECUTE_LIST_LIMIT:-} ]]; then
        options+=(--execute-list-limit "$EXECUTE_LIST_LIMIT")
    fi
    env HOME="$LOCAL/home" XDG_CONFIG_HOME="$LOCAL/home/.config" USERPROFILE="$LOCAL/home" \
        python3 "$REQUEST_LOGS" run --directory "$LOG_DIR" --bin "$BIN" --config "$CONFIG" \
        --case "${CASE_ID:?请登记用例编号}" --purpose "${PURPOSE:-case}" \
        --expected-exit "${EXPECTED_EXIT:-0}" "${options[@]}" -- "$@"
}
```

| 用例 | 操作与预期 |
| --- | --- |
| 1.1 | 使用上面的独立配置和子进程用户目录。默认下载在临时工作目录中验证。无法确认平台默认配置目录已隔离时，把查找回退用例交给离线测试。 |
| 1.2 | 关闭 shell 命令跟踪，用用户提供的测试凭据执行 `CASE_ID=1.2 c139 login --token "$TOKEN"`。登录解析 Token、获取个人云路由并只读检查根目录列表，成功后原子保存配置。 |
| 1.3 | 非个人云配置的拒绝在[离线用例](offline.md#配置和认证)中验证。 |
| 1.4 | 配置选择、失败登录、续期写回和并发保存在[离线用例](offline.md#配置和认证)中验证。 |
| 1.5 | `c139 ls / -o "$LOCAL/logs/before-root.json"` 只查本轮 UUID 是否存在。名称未占用时执行 `c139 mkdir "$CLOUD"` 并轮询确认，登记归属。名称已占用时停止本轮创建。 |
| 1.6 | `c139 ls "$CLOUD" -o "$LOCAL/logs/empty-root.json"` 返回 0，`total=0`、`items=[]`。 |

## 判断操作结果

- 参数解析错误返回 2。配置或执行错误通常返回 1，sync 前置检查和计划错误返回 2。失败用例还要核对错误原因和前后状态，网络错误不能作为拒绝用例通过的证据。
- 用 `ls -R -o` 核对路径与类型。递归 JSON 不含内容哈希。下载到此前不存在的核验路径，再比对相对路径、空目录、大小和 SHA-256。清单放在被扫描目录外。单文件直接比较 SHA-256。
- 表中的「不变」表示重新取得源和目标的清单并比较，忽略修改时间等易变化的字段。确认对象消失时，检查仍存在的父目录的成功列表。
- 写操作只提交一次。默认使用单调时钟，最多 120 秒、每 2 秒只读复查一次，上限 60 次。较大的夹具可提前登记更长的有限上限。连续两次符合预期才通过，失败后的「不变」也一样。超时记录为失败或结果不明，暂停依赖的写操作。
- 本轮子树外只允许登录校验和对本轮 UUID 的根目录查重、收尾确认。发现共享状态冲突或来源不明的对象时暂停相关阶段。

```bash
python3 "$TOOL" manifest "$LOCAL/downloads/actual-tree" --output "$LOCAL/manifests/actual-tree.json"
python3 "$TOOL" compare "$LOCAL/manifests/tree.json" "$LOCAL/manifests/actual-tree.json"
```

## 日志收集与分析

用例表里的 `c139` 命令继承当前用例的 `CASE_ID`。准备、轮询、核验和清理分别登记 `PURPOSE=setup/poll/verify/cleanup`。预期失败的命令登记 `EXPECTED_EXIT`，例如 `CASE_ID=2.6 EXPECTED_EXIT=1 c139 ls "$CLOUD/s02/not-created"`。自定义运行器也调用 `request_logs.py run`，使用同一个 `LOG_DIR`。运行器只登记用例、用途、命令名和退出码，保留 CLI 的 stdout 和 stderr。

`--log-file` 每次新建文件。运行器为每条命令分配独立路径，避免并行覆盖。将有已知预算的用例提前登记，例如未变化文件的校验同步使用 `EXECUTE_LIST_LIMIT=0`，限制执行阶段的 `/file/list`。日志目录须位于传输和同步目录之外。

等所有阶段的 CLI 调用结束，包括清理与最后一次轮询，再运行汇总脚本。

```bash
python3 "$REQUEST_LOGS" analyze --directory "$LOG_DIR"
```

脚本输出 `combined.log` 和 `analysis.json`。报告前核对台账和日志覆盖，检查异常退出、日志缺失、警告、错误、失败或未结束的 HTTP 请求以及 429。将预期拒绝、已恢复的重试与异常分开。按用例、阶段和端点核对请求数、P95 耗时与每秒峰值。对照夹具中的文件数、目录分页、写入复核和轮询用途，定位逐文件重复列举或意外请求突发。已知频率预算时可传 `--peak-limit`。频率峰值用于定位问题，服务端限流以 HTTP 429 等实际证据确认。

报告写明日志覆盖、发现及判定依据。保留脱敏后的汇总日志和分析，归档前检查凭据或签名链接。

## 阶段 12 · 清理和报告

| 用例 | 操作与预期 |
| --- | --- |
| 12.1 | 清理代理对照台账和创建证据重新列出本轮子树，审核在途任务。发现外来对象或归属不明时暂停相关清理并报告。 |
| 12.2 | 用户未限制清理且已确认本轮根归属时，执行一次 `c139 rm -r --yes "$CLOUD"`。 |
| 12.3 | 只读轮询根列表，确认本轮 UUID 消失。超时记录残留。 |
| 12.4 | 完成整轮日志合并与分析，保留脱敏证据，核对临时根归属后清理其中的配置、锁、夹具和下载。 |
| 12.5 | 比对工作区前后状态。遇到来源不明的改动，保留并报告。 |

报告列出测试提交、实际用例和退出码、内容证据、轮询耗时、支持范围、部分失败、残留与清理结果。离线测试和文档核对的结果单独说明。
