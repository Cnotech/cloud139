---
name: cloud139-e2e-test
description: Use when 需要对 cloud139 CLI 做完整端到端回归，且测试会创建大量本地与云端临时文件、配置文件和同步目录
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: testing
---

## 功能描述

对 139 云盘 CLI (cloud139) 进行完整的端到端测试，覆盖所有命令功能和边界情况。

## 执行模式

本技能必须采用“主代理编排 + sub agent 分阶段执行”的方式运行，不能由单个代理从头到尾串行硬跑。

主代理职责：
- 在开头和结尾执行 git 工作树检查
- 向用户索取 Token，并明确测试前置条件
- 维护测试台账：记录创建过的本地临时文件、目录、备份文件、云端测试目录、随机文件名、输出文件
- 将测试拆成多个阶段，分派给不同 sub agent
- 对共享状态做串行调度，避免多个 sub agent 同时操作同一个配置文件、同一个本地目录或同一个云端目录
- 汇总所有阶段结果，执行最终清理与报告

sub agent 职责：
- 只处理主代理分配给自己的阶段，不擅自扩展范围
- 回传实际执行的命令、退出码、发现的问题、产生的临时文件和后续清理义务
- 如果发现会污染其他阶段的共享状态，立即停止并回报主代理

并发规则：
- 对同一个云端目录、同一个本地临时目录、同一个配置文件的操作必须串行
- 只读检查可以并行，例如读取现有配置、读取 `ls` 输出、检查下载结果、检查 JSON 输出
- 会修改云端状态的阶段默认串行执行，除非主代理能证明目标路径完全隔离

建议拆分的 sub agent 阶段：
1. 预检与登录：工作树检查通过后，检查配置文件、构建、登录、准备测试目录
2. 基础命令测试：`ls`、`upload`、`download`
3. 变更类命令测试：`cp`、`rename`、`mv`、`mkdir`、`rm`
4. `sync` 专项测试
5. 清理与工作树复核

## 使用场景

- 测试 cloud139 CLI 所有功能是否正常工作
- 验证边界情况处理是否正确
- 回归测试

## 执行流程

### 1. 开始前 git 工作树门禁

第一步必须执行：
```bash
git status --short
```

判定规则：
- 如果输出为空，才允许继续执行后续 E2E 流程
- 如果存在任何已修改、已删除、未跟踪文件，立即停止测试
- 主代理必须把脏工作树内容原样展示给用户，并明确要求用户先自行处理完成，再重新开始本技能
- 在工作树不干净时，禁止继续执行 `cargo build`、`login`、上传下载、创建临时目录、清理动作，避免把旧脏状态和本轮测试副作用混在一起

### 2. 收集信息

首先询问用户获取以下信息：
- **139 云盘登录 Token**：从浏览器开发者工具获取

### 2.1 建立测试台账

主代理在分派 sub agent 前，先建立测试台账并持续更新。至少记录：
- 本地配置文件位置与备份位置
- 本轮创建的云端测试目录名，例如 `e2e_test_{timestamp}`
- 本轮创建的本地临时目录、输出文件、随机文件
- `sync` 阶段使用的本地源目录和目标目录
- 每个 sub agent 新增的临时文件、目录、备份文件
- 预期在清理阶段被删除或恢复的对象

台账的目的不是做装饰，而是保证最后能根据 git 工作树和台账反推“哪些是遗留临时文件，哪些可能是误删或误改”。

### 3. 配置文件约定

测试本 SKILL 时，配置文件统一使用 `cloud139rc.toml`。

读取优先级：
1. `--config <PATH>` 指定路径
2. 当前工作目录下的 `cloud139rc.toml`
3. 全局路径 `~/.config/cloud139/cloud139rc.toml`

写入规则：
- `login --config <PATH>`：写入指定路径
- 未指定 `--config` 且当前工作目录已存在 `cloud139rc.toml`：写回当前工作目录
- 其他情况：写入全局路径 `~/.config/cloud139/cloud139rc.toml`

执行 E2E 前，先确认并记录当前环境中以下文件是否存在，避免误删用户真实配置：
```bash
pwd
ls -la ./cloud139rc.toml
ls -la ~/.config/cloud139/cloud139rc.toml
```

如果本地或全局配置文件已存在，先备份再测试：
```bash
cp ./cloud139rc.toml ./cloud139rc.toml.e2e.bak 2>/dev/null || true
cp ~/.config/cloud139/cloud139rc.toml ~/.config/cloud139/cloud139rc.toml.e2e.bak 2>/dev/null || true
```


> ⚠️ **Windows Git Bash 用户注意**：在 Windows 的 Git Bash 环境下直接执行 `./cloud139.exe ls /` 会将 `/` 解析为 Windows 根目录（如 `C:`），导致 API 调用失败。请配置环境变量 `MSYS_NO_PATHCONV=1`。

首先编译项目，确保测试的代码是最新的：
```bash
cargo build --release
```

使用提供的 token 登录：
```bash
./target/release/cloud139.exe login --token <token> --storage-type personal_new
```

> ℹ️ **登录后置校验**：`login` 命令在保存配置后会自动执行一次 `ls /` 验证 Token 实际可用性。
> - 若成功，输出 `Token 验证成功!`
> - 若失败，会输出警告提示 Token 可能已过期，并以退出码 1 终止，**不会**保存配置

检查并删除**云端**根目录下的遗留测试文件（如 README.md, Cargo.lock 等）：
```bash
./target/release/cloud139.exe ls /
# 如果存在遗留测试文件，执行删除
./target/release/cloud139.exe rm /README.md --yes
./target/release/cloud139.exe rm /Cargo.lock --yes
```

创建一个随机命名的测试目录，格式：`e2e_test_{timestamp}`
```bash
./target/release/cloud139.exe mkdir /e2e_test_xxx
```

准备**固定名称**的 `ls` 分页测试目录 `/e2e_ls_paging_test`（若已存在且文件足够则跳过创建，便于复用）：
```bash
# 创建目录（已存在则忽略错误）
./target/release/cloud139.exe mkdir /e2e_ls_paging_test 2>/dev/null || true

# 获取当前文件数量（简单估算，根据实际输出调整）
file_count=$(./target/release/cloud139.exe ls /e2e_ls_paging_test 2>/dev/null | grep -c 'txt' || echo 0)

if [ "$file_count" -lt 25 ]; then
    for i in $(seq 1 25); do
        echo "paging_test_content_$i" > "/tmp/e2e_paging_file_$i.txt"
        ./target/release/cloud139.exe upload "/tmp/e2e_paging_file_$i.txt" /e2e_ls_paging_test/ 2>/dev/null || true
    done
    rm -f /tmp/e2e_paging_file_*.txt
fi
```

### 4. 退出码校验规则

**通用规则**：如果命令未能正常执行，则程序退出码应当为 1。

具体场景：
- 边界情况（如文件不存在、目录不存在等）应返回退出码 1
- 用户未提供必要参数时应返回退出码 1
- 操作失败（如网络错误、API 错误等）应返回退出码 1
- 只有命令正常执行完成时才返回退出码 0

> 注：部分命令的 `--force` 参数会覆盖某些限制，此时即使有警告也可能返回 0（取决于具体实现）

### 5. 测试执行顺序

#### 阶段 1: 登录测试 (login)

##### 阶段 1.1: 配置文件 E2E 测试

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 1.1.1 | `rm -f ./cloud139rc.toml && rm -f ~/.config/cloud139/cloud139rc.toml` | 准备干净环境（仅在已完成备份后执行） |
| 1.1.2 | `./target/release/cloud139.exe login --token <valid_token> --config ./e2e_config_override.toml` | `login` 成功，配置写入 `./e2e_config_override.toml` |
| 1.1.3 | `ls -la ./e2e_config_override.toml && ls -la ./cloud139rc.toml && ls -la ~/.config/cloud139/cloud139rc.toml` | 仅 override 文件存在；当前目录与全局路径不应新增配置 |
| 1.1.4 | `./target/release/cloud139.exe --config ./e2e_config_override.toml ls /` | 使用 override 配置读取成功 |
| 1.1.5 | `cp ./e2e_config_override.toml ./cloud139rc.toml && rm -f ./e2e_config_override.toml` | 准备“当前目录配置优先”场景 |
| 1.1.6 | `./target/release/cloud139.exe ls /` | 未传 `--config` 时，优先读取当前目录 `./cloud139rc.toml` |
| 1.1.7 | `mkdir -p ~/.config/cloud139 && cp ./cloud139rc.toml ~/.config/cloud139/cloud139rc.toml && rm -f ./cloud139rc.toml` | 准备“全局配置回退”场景 |
| 1.1.8 | `./target/release/cloud139.exe ls /` | 当前目录无配置时，回退读取全局配置 |
| 1.1.9 | `./target/release/cloud139.exe login --token <expired_or_invalid_token> --config ./e2e_invalid.toml` | **边界**：登录校验失败，退出码为 1，`./e2e_invalid.toml` 不应残留 |
| 1.1.10 | `ls -la ./e2e_invalid.toml` | 验证失败登录未保留 override 配置 |
| 1.1.11 | `cp ~/.config/cloud139/cloud139rc.toml ./cloud139rc.toml && ./target/release/cloud139.exe login --token <valid_token>` | 当前目录已有 `cloud139rc.toml` 时，成功登录后应写回当前目录，而不是写全局 |
| 1.1.12 | `ls -la ./cloud139rc.toml && ls -la ~/.config/cloud139/cloud139rc.toml` | 验证本地配置仍存在；全局配置未被新路径替代 |

##### 阶段 1.2: 基础登录校验

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 1.2.1 | `./target/release/cloud139.exe login --token <expired_or_invalid_token>` | **边界**：`ls /` 校验失败，输出 Token 可能已过期的警告，退出码为 1，配置文件被删除 |
| 1.2.2 | `./target/release/cloud139.exe login --token <valid_token>` | 登录成功：内部自动执行 `ls /`，输出 `Token 验证成功!`，退出码为 0 |

> **步骤 1.2.1 说明**：可以通过篡改有效 token 的最后几位字符来模拟无效 token。预期输出包含：
> ```
> ⚠ ls / 执行失败，Token 可能已过期或无效: ...
> ⚠ 请重新获取 Token 后再次登录
> ```

#### 阶段 2: 列表测试 (ls)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 2.1 | `./target/release/cloud139.exe ls /` | 能列出根目录内容 |
| 2.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 能列出空目录 |
| 2.3 | `./target/release/cloud139.exe ls /not_exist_dir` | **边界**：返回错误 |
| 2.4 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 1 -s 10` | 分页：只返回第1页10条记录；总条目数应为25 |
| 2.5 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 2 -s 10` | 分页：返回第2页10条记录，条目与 2.4 不重复 |
| 2.6 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 3 -s 10` | 分页：返回剩余约5条记录 |
| 2.7 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 1 -s 25` | 分页：单页返回全部25条记录，验证总数 |
| 2.8 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -o ls_result.json` | 结果输出为 JSON 文件，文件内容有效且包含目录条目 |
| 2.9 | `cat ls_result.json` | 验证 JSON 文件结构正确（包含 `name`、`type`、`size` 等字段） |
| 2.10 | `./target/release/cloud139.exe ls / -v debug` | 全局 `-v` 选项生效，控制台应输出 `DEBUG` 级别日志 |

#### 阶段 3: 上传测试 (upload)

测试上传当前目录的 `README.md` 和 `Cargo.toml`：

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 3.1 | `./target/release/cloud139.exe upload README.md /` | 上传到根目录 |
| 3.2 | `./target/release/cloud139.exe upload Cargo.toml /` | 上传到根目录 |
| 3.3 | `./target/release/cloud139.exe upload README.md /e2e_test_xxx/` | 上传到测试目录 |
| 3.4 | `./target/release/cloud139.exe upload Cargo.toml /e2e_test_xxx/` | 上传到测试目录 |
| 3.5 | `./target/release/cloud139.exe upload not_exist_file.txt /` | **边界**：本地文件不存在 |
| 3.6 | `./target/release/cloud139.exe upload README.md /not_exist_dir/` | **边界**：远程目录不存在 |
| 3.7 | `./target/release/cloud139.exe upload README.md /` | **边界**：上传同名文件，云端已存在；应提示警告且退出码为1 |
| 3.8 | `./target/release/cloud139.exe upload README.md / --force` | 强制上传，云端会自动重命名 |
| 3.9 | 生成并上传随机1MB+文件 | 随机数据文件，验证哈希一致性 |

**步骤 3.9 详细操作**：

首先在本地生成一个带时间戳的随机1MB文件（Windows和Unix命令不同）：

**Windows (PowerShell)**：
```powershell
$timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$filename = "e2e_random_$timestamp.bin"
$size = 1MB
$r = New-Object Random
$b = [byte[]]::new($size)
$r.NextBytes($b)
[IO.File]::WriteAllBytes($filename, $b)
# 计算本地哈希
$localHash = (Get-FileHash $filename -Algorithm SHA256).Hash
Write-Output "Local: $localHash"
# 上传
./target/release/cloud139.exe upload $filename /
# 提取响应中的哈希进行对比
```

**Unix (Linux/macOS/WSL2)**：
```bash
timestamp=$(date +%Y%m%d_%H%M%S)
filename="e2e_random_$timestamp.bin"
dd if=/dev/urandom of="$filename" bs=1M count=1
localHash=$(sha256sum "$filename" | cut -d' ' -f1)
echo "Local: $localHash"
./target/release/cloud139.exe upload "$filename" /
```

验证上传响应中的 `contentHash` 与本地计算的一致。

清理本地测试文件：
```bash
rm -f e2e_random_*.bin
```

#### 阶段 4: 列表验证

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 4.1 | `./target/release/cloud139.exe ls /` | 应包含 README.md, Cargo.toml |
| 4.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应包含上传的两个文件 |

#### 阶段 5: 下载测试 (download)

> 请注意在下载完成后检查本地文件是否存在、文件大小是否与云端一致


首先创建本地临时测试目录：
```bash
mkdir -p cloud139_e2e_download_test
```

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 5.1 | `./target/release/cloud139.exe download /README.md` | 下载成功（默认文件名） |
| 5.2 | `./target/release/cloud139.exe download /e2e_test_xxx/Cargo.toml` | 下载成功 |
| 5.3 | `./target/release/cloud139.exe download /README.md ./cloud139_e2e_download_test/` | 下载到指定目录（保持原名） |
| 5.4 | `./target/release/cloud139.exe download /e2e_test_xxx/Cargo.toml ./cloud139_e2e_download_test/custom_name.toml` | 下载并重命名 |
| 5.5 | `ls ./cloud139_e2e_download_test/` | 验证文件已保存 |
| 5.6 | `./target/release/cloud139.exe download /not_exist.txt` | **边界**：文件不存在 |
| 5.7 | `./target/release/cloud139.exe download /e2e_test_xxx` | **边界**：不能下载目录 |
| 5.8 | `./target/release/cloud139.exe download /Cargo.toml ./non-exist-dir-1/` | **边界**：自动创建目录并成功下载文件 |
| 5.9 | `./target/release/cloud139.exe download /README.md ./non-exist-dir-2/custom.txt` | **边界**：自动创建目录并成功下载文件 |
| 5.10 | 下载随机文件并验证哈希一致性 | 下载阶段 3.9 上传的随机文件，与本地哈希比对 |

**步骤 5.10 详细操作**：

首先从阶段 3.9 获取上传后的文件名（格式：`e2e_random_{timestamp}.bin`），然后下载并验证哈希：

**Windows (PowerShell)**：
```powershell
# 找到阶段 3.9 生成的文件名（根据时间戳推断）
$timestamp = "20260319_003824"  # 需根据实际情况调整
$filename = "e2e_random_$timestamp.bin"
# 下载文件
./target/release/cloud139.exe.exe download /$filename ./
# 计算本地下载文件的哈希
$downloadedHash = (Get-FileHash $filename -Algorithm SHA256).Hash
Write-Output "Downloaded: $downloadedHash"
# 注意：阶段 3.9 已将本地随机文件的哈希记录，可直接对比
```

**Unix (Linux/macOS/WSL2)**：
```bash
timestamp=$(date +%Y%m%d_%H%M%S)
filename="e2e_random_$timestamp.bin"
./target/release/cloud139.exe download "$filename" ./
downloadedHash=$(sha256sum "$filename" | cut -d' ' -f1)
echo "Downloaded: $downloadedHash"
# 阶段 3.9 已将本地随机文件的哈希记录在 $localHash 变量中
```

**验证方法**：
- 下载文件后计算其 SHA256 哈希
- 与阶段 3.9 记录在日志中的 `$localHash` 对比
- 二者应完全一致，确保上传下载过程数据完整性

测试完成后清理本地临时目录：
```bash
rm -rf cloud139_e2e_download_test
```

#### 阶段 6: 复制测试 (cp)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 6.1 | `./target/release/cloud139.exe cp /Cargo.toml /e2e_test_xxx/` | 复制到测试目录；**注意**：个人云下若目标目录已存在同名文件，需使用 `--force` 才能继续，云端会自动重命名 |
| 6.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 3 个文件（含自动重命名的文件） |
| 6.3 | `./target/release/cloud139.exe cp /not_exist.txt /tmp` | **边界**：源文件不存在 |
| 6.4 | `./target/release/cloud139.exe cp /README.md /not_exist_dir/` | **边界**：目标目录不存在 |
| 6.5 | `./target/release/cloud139.exe cp /Cargo.toml /e2e_test_xxx/` | **边界**：复制同名文件，云端已存在；应提示警告且退出码为1 |
| 6.6 | `./target/release/cloud139.exe cp /Cargo.toml /e2e_test_xxx/ --force` | 强制复制，云端会自动重命名 |

#### 阶段 7: 重命名测试 (rename)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 7.1 | `./target/release/cloud139.exe rename /e2e_test_xxx/README.md README_copy.md` | 重命名文件成功 |
| 7.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 README_copy.md |
| 7.3 | `./target/release/cloud139.exe rename / new_name` | **边界**：不能重命名根目录 |
| 7.4 | `./target/release/cloud139.exe rename /not_exist.txt new.txt` | **边界**：文件不存在 |
| 7.5 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/rename_dir_src` | 准备：创建待重命名的目录 |
| 7.6 | `./target/release/cloud139.exe rename /e2e_test_xxx/rename_dir_src rename_dir_dst` | 重命名目录成功 |
| 7.7 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 rename_dir_dst，无 rename_dir_src |
| 7.8 | `./target/release/cloud139.exe rename /e2e_test_xxx/rename_dir_dst rename_dir_dst` | **边界**：重命名为同名；当前实现允许重命名为同名（返回退出码 0） |
| 7.9 | `./target/release/cloud139.exe rename /e2e_test_xxx/not_exist_dir new_dir` | **边界**：目录不存在 |

#### 阶段 8: 移动测试 (mv)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 8.1 | `./target/release/cloud139.exe mv /e2e_test_xxx/README_copy.md /` | 移动到根目录 |
| 8.2 | `./target/release/cloud139.exe ls /` | 应有 README_copy.md |
| 8.3 | `./target/release/cloud139.exe ls /e2e_test_xxx` | README_copy.md 已移出 |
| 8.4 | `./target/release/cloud139.exe mv /README_copy.md /not_exist_dir/` | **边界**：目标不存在 |
| 8.5 | `./target/release/cloud139.exe mv / /somewhere` | **边界**：不能移动根目录 |
| 8.6 | `./target/release/cloud139.exe mv /README.md /e2e_test_xxx/` | **边界**：移动到已有同名文件的目录；当前实现个人云下会直接移动成功（云端自动重命名），退出码为 0 |
| 8.7 | `./target/release/cloud139.exe mv /README.md /e2e_test_xxx/ --force` | 强制移动，云端会自动重命名 |
| 8.8 | `echo "mv_multi_1" > mv_multi_1.txt && ./target/release/cloud139.exe upload mv_multi_1.txt /e2e_test_xxx/ && echo "mv_multi_2" > mv_multi_2.txt && ./target/release/cloud139.exe upload mv_multi_2.txt /e2e_test_xxx/` | 准备：上传多个文件用于批量移动测试 |
| 8.9 | `./target/release/cloud139.exe mv /e2e_test_xxx/mv_multi_1.txt /e2e_test_xxx/mv_multi_2.txt /e2e_test_xxx/subdir/` | 同时移动多个文件到目标目录 |
| 8.10 | `./target/release/cloud139.exe ls /e2e_test_xxx/subdir` | 目标目录应包含 mv_multi_1.txt 和 mv_multi_2.txt |
| 8.11 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 源目录中应无 mv_multi_1.txt 和 mv_multi_2.txt |
| 8.12 | `./target/release/cloud139.exe mv /e2e_test_xxx/subdir/mv_multi_1.txt /e2e_test_xxx/subdir/mv_multi_2.txt /not_exist_dir/` | **边界**：目标目录不存在 |
| 8.13 | `./target/release/cloud139.exe mv /e2e_test_xxx/subdir/mv_multi_1.txt /not_exist.txt /e2e_test_xxx/` | **边界**：多个源路径中有一个不存在 |

#### 阶段 9: 创建目录测试 (mkdir)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 9.1 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/subdir` | 创建子目录 |
| 9.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 subdir |
| 9.3 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/subdir` | **边界**：目录已存在，云端已存在；应提示警告且退出码为1 |
| 9.4 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/subdir --force` | 强制创建，云端会自动重命名 |
| 9.5 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/not_exist/child` | **边界**：父目录不存在 |

#### 阶段 10: 删除测试 (rm)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.1 | `echo "test" > test_delete.txt && ./target/release/cloud139.exe upload test_delete.txt /e2e_test_xxx/` | 准备删除测试文件 |
| 10.2 | `./target/release/cloud139.exe rm /e2e_test_xxx/test_delete.txt --yes` | 移到回收站 |
| 10.3 | `./target/release/cloud139.exe ls /e2e_test_xxx` | test_delete.txt 已删除 |
| 10.4 | `./target/release/cloud139.exe rm /not_exist.txt --yes` | **边界**：文件不存在 |
| 10.5 | `./target/release/cloud139.exe rm /Cargo.toml` | 不带 --yes 应提示确认 |
| 10.6 | `./target/release/cloud139.exe rm / --yes` | **边界**：不能删除根目录 |
| 10.7 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/rm_empty_dir` | 准备：创建待删除的空目录 |
| 10.8 | `./target/release/cloud139.exe rm /e2e_test_xxx/rm_empty_dir --yes` | 删除空目录成功 |
| 10.9 | `./target/release/cloud139.exe ls /e2e_test_xxx` | rm_empty_dir 已删除 |
| 10.10 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/rm_nonempty_dir && echo "dir_file" > rm_dir_file.txt && ./target/release/cloud139.exe upload rm_dir_file.txt /e2e_test_xxx/rm_nonempty_dir/` | 准备：创建非空目录 |
| 10.11 | `./target/release/cloud139.exe rm /e2e_test_xxx/rm_nonempty_dir --yes` | 删除非空目录成功 |
| 10.12 | `./target/release/cloud139.exe ls /e2e_test_xxx` | rm_nonempty_dir 已删除 |
| 10.13 | `./target/release/cloud139.exe rm /e2e_test_xxx/not_exist_dir --yes` | **边界**：目录不存在 |

#### 阶段 11: 同步测试 (sync)

> **sync 命令说明**：`sync` 命令参考 rsync 语义，支持本地与云端之间的单向同步。同步方向完全由 SRC/DEST 参数位置决定。
> - 本地 → 云端：`sync ./local cloud:/remote`
> - 云端 → 本地：`sync cloud:/remote ./local`
> - 云端路径以 `cloud:` 前缀标识，之后直接跟云端路径（如 `cloud:/backup`）
> - **执行顺序要求**：阶段 11 的各子步骤应串行执行；不要对同一个云端目标目录并发执行多条 `sync` 命令，否则会互相污染结果（例如触发云端自动重命名）

##### 阶段 11 环境准备

```bash
# 创建本地同步测试目录结构
mkdir -p cloud139_e2e_sync_src/subdir
echo "file1 content" > cloud139_e2e_sync_src/file1.txt
echo "file2 content" > cloud139_e2e_sync_src/file2.txt
echo "sub content"   > cloud139_e2e_sync_src/subdir/sub.txt

# 创建云端目标目录
./target/release/cloud139.exe mkdir /e2e_test_xxx/sync_target

# 创建本地下载目标目录（供云端→本地测试使用）
mkdir -p cloud139_e2e_sync_dst
```

##### 11.1 基础上传同步（本地 → 云端）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.1.1 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target` | 不带 `-r` 时仅同步源目录根下文件，子目录被跳过；按当前准备数据，应传输 `file1.txt`、`file2.txt` 共 2 个文件 |
| 11.1.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | 在 11.1.1 之后递归同步，应补传 `subdir/sub.txt` 1 个文件；若跳过 11.1.1 直接执行，则会首次全量同步 3 个文件 |
| 11.1.3 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 云端应有 file1.txt、file2.txt；应有 subdir 目录 |
| 11.1.4 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target/subdir` | 云端 subdir 应有 sub.txt |
| 11.1.5 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | **增量同步**：文件未变化，应全部 Skip，输出 `0 个文件传输, 3 个跳过` |
| 11.1.6 | 在源目录下创建空目录 `empty_dir` 后执行 `sync ... -r` | 云端应出现空目录 `empty_dir` |

##### 11.2 --dry-run 演习模式

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.2.1 | 修改本地文件：`echo "modified" > cloud139_e2e_sync_src/file1.txt` | 准备 |
| 11.2.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r -n` | 演习模式：输出操作计划（包含 `(DRY RUN)` 前缀），不实际传输；退出码为 0 |
| 11.2.3 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证云端 file1.txt **未被修改**（dry-run 不应产生实际变更） |

##### 11.3 增量同步（内容变化）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.3.1 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | file1.txt 已修改，应只传输 1 个文件，其余 2 个 Skip；输出 `1 个文件传输, 2 个跳过` |

##### 11.4 --delete 删除多余文件

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.4.1 | 删除本地文件：`rm cloud139_e2e_sync_src/file2.txt` | 准备 |
| 11.4.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | 不带 --delete：file2.txt 仍保留在云端；输出 `0 个文件传输, 2 个跳过` |
| 11.4.3 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --delete -n` | dry-run 下应显示 `*deleting` 标记，不实际删除 |
| 11.4.4 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --delete` | 实际删除：云端 file2.txt 应被移除 |
| 11.4.5 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证云端最终已无 file2.txt；若删除后立即 `ls` 仍短暂可见，等待片刻后重试一次，或执行 `rm /e2e_test_xxx/sync_target/file2.txt --yes` 应返回“文件不存在” |


| 11.4.6 | 删除本地空目录 `empty_dir` 后执行 `sync ... -r --delete` | 云端空目录应被删除 |

##### 11.5 --exclude 排除规则

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.5.1 | 准备：`echo "secret" > cloud139_e2e_sync_src/.env && echo "build" > cloud139_e2e_sync_src/output.log` | 准备排除测试文件 |
| 11.5.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --exclude "*.log" --exclude ".env"` | `.env` 和 `output.log` 应被跳过，不传输到云端 |
| 11.5.3 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证云端无 `.env`、无 `output.log` |
| 11.5.4 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --exclude "subdir"` | 排除整个子目录 |
| 11.5.5 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证 subdir 目录及其内容未被同步 |

##### 11.6 --checksum 精确对比

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.6.1 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --checksum` | 使用哈希对比，内容未变的文件应全部 Skip；观察终端输出，显示警告（需扫描 checksum 耗时） |
| 11.6.2 | 准备一个"同大小不同内容"的文件后执行 `sync ... -r --checksum` | 应识别为变化并传输，而不是 Skip |

##### 11.7 --jobs 并发控制

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.7.1 | 准备多文件：`for i in $(seq 1 8); do echo "content $i" > cloud139_e2e_sync_src/batch_$i.txt; done` | 准备 8 个文件 |
| 11.7.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r -j 2` | 限制 2 并发，8 个新文件全部上传成功；退出码为 0 |
| 11.7.3 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r -j 8` | 8 并发，全部 Skip（已存在）；退出码为 0 |

##### 11.8 云端 → 本地（下载方向）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.8.1 | `./target/release/cloud139.exe sync cloud:/e2e_test_xxx/sync_target ./cloud139_e2e_sync_dst -r` | 首次全量下载，云端所有文件下载到本地 |
| 11.8.2 | `ls ./cloud139_e2e_sync_dst/` | 验证本地有 file1.txt、subdir/ 等 |
| 11.8.3 | `./target/release/cloud139.exe sync cloud:/e2e_test_xxx/sync_target ./cloud139_e2e_sync_dst -r` | **增量**：无变化，全部 Skip |
| 11.8.4 | 修改本地文件：`echo "local modified" > ./cloud139_e2e_sync_dst/file1.txt` | 准备 |
| 11.8.5 | `./target/release/cloud139.exe sync cloud:/e2e_test_xxx/sync_target ./cloud139_e2e_sync_dst -r` | 云端 file1.txt 覆盖本地被修改版本；输出 `1 个文件传输` |

##### 11.9 路径参数错误处理（边界）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 11.9.1 | `./target/release/cloud139.exe sync ./local1 ./local2 -r` | **边界**：两端均为本地路径，应立即报错，退出码 2，提示使用 `cp`/`mv` 等系统工具 |
| 11.9.2 | `./target/release/cloud139.exe sync cloud:/src cloud:/dst -r` | **边界**：两端均为云端路径，应立即报错，退出码 2 |
| 11.9.3 | `./target/release/cloud139.exe sync ./not_exist_src cloud:/e2e_test_xxx/sync_target -r` | **边界**：本地源目录不存在，扫描失败，退出码 2 |
| 11.9.4 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/not_exist_remote_dir -r` | **边界**：云端目标目录不存在，扫描/创建失败，退出码 2 |

##### 阶段 11 清理

```bash
# 删除本地临时目录
rm -rf cloud139_e2e_sync_src
rm -rf cloud139_e2e_sync_dst
rm -f cloud139_e2e_sync_src/.env cloud139_e2e_sync_src/output.log  # 已在 rm -rf 中处理

# 删除云端 sync 测试目录（在本文第 6 节总清理中统一删除 e2e_test_xxx）
```

### 6. 清理与工作树复核

> ⚠️ **重要警告**：清理时**绝对不要删除本地的项目核心文件**（如 `Cargo.toml`、`README.md`）。下载测试会在当前目录覆盖这些文件，但这是正常行为，不需要清理。需要清理的是**云端**的测试文件。

清理阶段必须由专门的 cleanup sub agent 执行，主代理负责审核其结果。

清理本地临时 JSON 输出文件：
```bash
rm -f ls_result.json
rm -f ./e2e_config_override.toml ./e2e_invalid.toml
```

测试完成后清理**云端**的测试数据：
```bash
./target/release/cloud139.exe rm /e2e_test_xxx --yes
./target/release/cloud139.exe rm /README.md --yes
./target/release/cloud139.exe rm /Cargo.toml --yes
./target/release/cloud139.exe rm /e2e_random_{timestamp}.bin --yes
```

清理**本地**的测试临时文件（仅限测试过程中生成的临时文件）：
```bash
rm -rf cloud139_e2e_download_test
rm -rf non-exist-dir-1 non-exist-dir-2
rm -f e2e_random_*.bin
rm -f test_delete.txt
# sync 测试产生的临时目录
rm -rf cloud139_e2e_sync_src
rm -rf cloud139_e2e_sync_dst
```

**注意**：以下文件是本地项目核心文件，**绝对不能删除**：
- `Cargo.toml` - Rust 项目配置文件
- `README.md` - 项目说明文档
- `Cargo.lock` - 依赖锁定文件

如果测试前备份过配置文件，测试结束后恢复：
```bash
mv -f ./cloud139rc.toml.e2e.bak ./cloud139rc.toml 2>/dev/null || true
mv -f ~/.config/cloud139/cloud139rc.toml.e2e.bak ~/.config/cloud139/cloud139rc.toml 2>/dev/null || true
```

完成上述清理后，主代理必须再次检查 git 工作树：
```bash
git status --short
```

如果工作树为空，说明本地副作用已清理干净，可以进入报告阶段。

如果工作树仍然不为空，必须继续排查，不能直接宣布测试完成。排查顺序如下：

1. 先区分未跟踪文件、已修改文件、已删除文件：
```bash
git status --short
git diff --name-status
```

2. 若是未跟踪文件，优先判断是否属于本轮遗留临时文件。重点排查这些典型对象：
- `ls_result.json`
- `e2e_config_override.toml`
- `e2e_invalid.toml`
- `cloud139rc.toml.e2e.bak`
- `cloud139_e2e_download_test/`
- `cloud139_e2e_sync_src/`
- `cloud139_e2e_sync_dst/`
- `non-exist-dir-1/`
- `non-exist-dir-2/`
- `e2e_random_*.bin`
- 其他台账中登记过的临时文件或目录

3. 若是已修改或已删除的受跟踪文件，按“可能误删/误改”处理，重点检查：
- `README.md`
- `Cargo.toml`
- `Cargo.lock`
- `cloud139rc.toml`
- 其他本仓库受跟踪的源码、文档、配置文件

4. 对可疑的受跟踪变更，主代理必须结合测试台账确认来源：
- 如果明确是本轮 E2E 造成的覆盖、下载、副作用或误删，先恢复到测试前状态，再重新检查工作树
- 如果无法证明来源，停止自动处理，向用户报告具体路径和风险，不要擅自回滚

5. 只有当主代理完成“遗留临时文件”和“误删/误改内容”排查后，才允许进入最终报告

### 7. 生成报告

汇总所有测试结果，生成测试报告。

> **报告时应包含在执行过程中发现的潜在问题或风险**，如果有 SKILL 中没有清晰描述的情况，也应在报告中指出并建议添加到 SKILL 中。
