---
name: cloud139-e2e-test
description: 139 云盘 CLI 完整 E2E 测试流程，覆盖所有命令功能和边界情况
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: testing
---

## 功能描述

对 139 云盘 CLI (cloud139) 进行完整的端到端测试，覆盖所有命令功能和边界情况。

## 使用场景

- 测试 cloud139 CLI 所有功能是否正常工作
- 验证边界情况处理是否正确
- 回归测试

## 执行流程

### 1. 收集信息

首先询问用户获取以下信息：
- **139 云盘登录 Token**：从浏览器开发者工具获取

### 2. 环境准备

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

### 3. 退出码校验规则

**通用规则**：如果命令未能正常执行，则程序退出码应当为 1。

具体场景：
- 边界情况（如文件不存在、目录不存在等）应返回退出码 1
- 用户未提供必要参数时应返回退出码 1
- 操作失败（如网络错误、API 错误等）应返回退出码 1
- 只有命令正常执行完成时才返回退出码 0

> 注：部分命令的 `--force` 参数会覆盖某些限制，此时即使有警告也可能返回 0（取决于具体实现）

### 4. 测试执行顺序

#### 阶段 0: 登录测试 (login)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 0.1 | `./target/release/cloud139.exe login --token <expired_or_invalid_token>` | **边界**：`ls /` 校验失败，输出 Token 可能已过期的警告，退出码为 1，配置文件被删除 |
| 0.2 | `./target/release/cloud139.exe login --token <valid_token>` | 登录成功：内部自动执行 `ls /`，输出 `Token 验证成功!`，退出码为 0 |

> **步骤 0.1 说明**：可以通过篡改有效 token 的最后几位字符来模拟无效 token。预期输出包含：
> ```
> ⚠ ls / 执行失败，Token 可能已过期或无效: ...
> ⚠ 请重新获取 Token 后再次登录
> ```

#### 阶段 1: 列表测试 (ls)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 1.1 | `./target/release/cloud139.exe ls /` | 能列出根目录内容 |
| 1.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 能列出空目录 |
| 1.3 | `./target/release/cloud139.exe ls /not_exist_dir` | **边界**：返回错误 |
| 1.4 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 1 -s 10` | 分页：只返回第1页10条记录；总条目数应为25 |
| 1.5 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 2 -s 10` | 分页：返回第2页10条记录，条目与 1.4 不重复 |
| 1.6 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 3 -s 10` | 分页：返回剩余约5条记录 |
| 1.7 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -p 1 -s 25` | 分页：单页返回全部25条记录，验证总数 |
| 1.8 | `./target/release/cloud139.exe ls /e2e_ls_paging_test -o ls_result.json` | 结果输出为 JSON 文件，文件内容有效且包含目录条目 |
| 1.9 | `cat ls_result.json` | 验证 JSON 文件结构正确（包含 `name`、`type`、`size` 等字段） |
| 1.10 | `./target/release/cloud139.exe ls / -v debug` | 全局 `-v` 选项生效，控制台应输出 `DEBUG` 级别日志 |

#### 阶段 2: 上传测试 (upload)

测试上传当前目录的 `README.md` 和 `Cargo.toml`：

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 2.1 | `./target/release/cloud139.exe upload README.md /` | 上传到根目录 |
| 2.2 | `./target/release/cloud139.exe upload Cargo.toml /` | 上传到根目录 |
| 2.3 | `./target/release/cloud139.exe upload README.md /e2e_test_xxx/` | 上传到测试目录 |
| 2.4 | `./target/release/cloud139.exe upload Cargo.toml /e2e_test_xxx/` | 上传到测试目录 |
| 2.5 | `./target/release/cloud139.exe upload not_exist_file.txt /` | **边界**：本地文件不存在 |
| 2.6 | `./target/release/cloud139.exe upload README.md /not_exist_dir/` | **边界**：远程目录不存在 |
| 2.7 | `./target/release/cloud139.exe upload README.md /` | **边界**：上传同名文件，云端已存在；应提示警告且退出码为1 |
| 2.8 | `./target/release/cloud139.exe upload README.md / --force` | 强制上传，云端会自动重命名 |
| 2.9 | 生成并上传随机1MB+文件 | 随机数据文件，验证哈希一致性 |

**步骤 2.9 详细操作**：

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

#### 阶段 3: 列表验证

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 3.1 | `./target/release/cloud139.exe ls /` | 应包含 README.md, Cargo.toml |
| 3.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应包含上传的两个文件 |

#### 阶段 4: 下载测试 (download)

> 请注意在下载完成后检查本地文件是否存在、文件大小是否与云端一致


首先创建本地临时测试目录：
```bash
mkdir -p cloud139_e2e_download_test
```

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 4.1 | `./target/release/cloud139.exe download /README.md` | 下载成功（默认文件名） |
| 4.2 | `./target/release/cloud139.exe download /e2e_test_xxx/Cargo.toml` | 下载成功 |
| 4.3 | `./target/release/cloud139.exe download /README.md ./cloud139_e2e_download_test/` | 下载到指定目录（保持原名） |
| 4.4 | `./target/release/cloud139.exe download /e2e_test_xxx/Cargo.toml ./cloud139_e2e_download_test/custom_name.toml` | 下载并重命名 |
| 4.5 | `ls ./cloud139_e2e_download_test/` | 验证文件已保存 |
| 4.6 | `./target/release/cloud139.exe download /not_exist.txt` | **边界**：文件不存在 |
| 4.7 | `./target/release/cloud139.exe download /e2e_test_xxx` | **边界**：不能下载目录 |
| 4.8 | `./target/release/cloud139.exe download /Cargo.toml ./non-exist-dir-1/` | **边界**：自动创建目录并成功下载文件 |
| 4.9 | `./target/release/cloud139.exe download /README.md ./non-exist-dir-2/custom.txt` | **边界**：自动创建目录并成功下载文件 |
| 4.10 | 下载随机文件并验证哈希一致性 | 下载阶段2.9上传的随机文件，与本地哈希比对 |

**步骤 4.10 详细操作**：

首先从阶段2.9获取上传后的文件名（格式：`e2e_random_{timestamp}.bin`），然后下载并验证哈希：

**Windows (PowerShell)**：
```powershell
# 找到阶段2.9生成的文件名（根据时间戳推断）
$timestamp = "20260319_003824"  # 需根据实际情况调整
$filename = "e2e_random_$timestamp.bin"
# 下载文件
./target/release/cloud139.exe.exe download /$filename ./
# 计算本地下载文件的哈希
$downloadedHash = (Get-FileHash $filename -Algorithm SHA256).Hash
Write-Output "Downloaded: $downloadedHash"
# 注意：阶段2.9已将本地随机文件的哈希记录，可直接对比
```

**Unix (Linux/macOS/WSL2)**：
```bash
timestamp=$(date +%Y%m%d_%H%M%S)
filename="e2e_random_$timestamp.bin"
./target/release/cloud139.exe download "$filename" ./
downloadedHash=$(sha256sum "$filename" | cut -d' ' -f1)
echo "Downloaded: $downloadedHash"
# 阶段2.9已将本地随机文件的哈希记录在 $localHash 变量中
```

**验证方法**：
- 下载文件后计算其 SHA256 哈希
- 与阶段2.9记录在日志中的 `$localHash` 对比
- 二者应完全一致，确保上传下载过程数据完整性

测试完成后清理本地临时目录：
```bash
rm -rf cloud139_e2e_download_test
```

#### 阶段 5: 复制测试 (cp)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 5.1 | `./target/release/cloud139.exe cp /Cargo.toml /e2e_test_xxx/` | 复制到测试目录（云端自动重命名） |
| 5.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 3 个文件（含自动重命名的文件） |
| 5.3 | `./target/release/cloud139.exe cp /not_exist.txt /tmp` | **边界**：源文件不存在 |
| 5.4 | `./target/release/cloud139.exe cp /README.md /not_exist_dir/` | **边界**：目标目录不存在 |
| 5.5 | `./target/release/cloud139.exe cp /Cargo.toml /e2e_test_xxx/` | **边界**：复制同名文件，云端已存在；应提示警告且退出码为1 |
| 5.6 | `./target/release/cloud139.exe cp /Cargo.toml /e2e_test_xxx/ --force` | 强制复制，云端会自动重命名 |
| 5.7 | `./target/release/cloud139.exe cp /Cargo.toml /e2e_test_xxx/ -m` | **已知问题**：`--merge` 参数在个人云场景下**未实现**（`cp_personal` 中 `_merge` 被忽略），行为与不带 `-m` 相同，遇到同名文件报错退出码 1 |

#### 阶段 6: 重命名测试 (rename)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 6.1 | `./target/release/cloud139.exe rename /e2e_test_xxx/README.md README_copy.md` | 重命名文件成功 |
| 6.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 README_copy.md |
| 6.3 | `./target/release/cloud139.exe rename / new_name` | **边界**：不能重命名根目录 |
| 6.4 | `./target/release/cloud139.exe rename /not_exist.txt new.txt` | **边界**：文件不存在 |
| 6.5 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/rename_dir_src` | 准备：创建待重命名的目录 |
| 6.6 | `./target/release/cloud139.exe rename /e2e_test_xxx/rename_dir_src rename_dir_dst` | 重命名目录成功 |
| 6.7 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 rename_dir_dst，无 rename_dir_src |
| 6.8 | `./target/release/cloud139.exe rename /e2e_test_xxx/rename_dir_dst rename_dir_dst` | **边界**：重命名为同名（或目标已存在），应提示警告且退出码为 1 |
| 6.9 | `./target/release/cloud139.exe rename /e2e_test_xxx/not_exist_dir new_dir` | **边界**：目录不存在 |

#### 阶段 7: 移动测试 (mv)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 7.1 | `./target/release/cloud139.exe mv /e2e_test_xxx/README_copy.md /` | 移动到根目录 |
| 7.2 | `./target/release/cloud139.exe ls /` | 应有 README_copy.md |
| 7.3 | `./target/release/cloud139.exe ls /e2e_test_xxx` | README_copy.md 已移出 |
| 7.4 | `./target/release/cloud139.exe mv /README_copy.md /not_exist_dir/` | **边界**：目标不存在 |
| 7.5 | `./target/release/cloud139.exe mv / /somewhere` | **边界**：不能移动根目录 |
| 7.6 | `./target/release/cloud139.exe mv /README.md /e2e_test_xxx/` | **边界**：移动到已有同名文件的目录，云端已存在；应提示警告且退出码为1 |
| 7.7 | `./target/release/cloud139.exe mv /README.md /e2e_test_xxx/ --force` | 强制移动，云端会自动重命名 |
| 7.8 | `echo "mv_multi_1" > mv_multi_1.txt && ./target/release/cloud139.exe upload mv_multi_1.txt /e2e_test_xxx/ && echo "mv_multi_2" > mv_multi_2.txt && ./target/release/cloud139.exe upload mv_multi_2.txt /e2e_test_xxx/` | 准备：上传多个文件用于批量移动测试 |
| 7.9 | `./target/release/cloud139.exe mv /e2e_test_xxx/mv_multi_1.txt /e2e_test_xxx/mv_multi_2.txt /e2e_test_xxx/subdir/` | 同时移动多个文件到目标目录 |
| 7.10 | `./target/release/cloud139.exe ls /e2e_test_xxx/subdir` | 目标目录应包含 mv_multi_1.txt 和 mv_multi_2.txt |
| 7.11 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 源目录中应无 mv_multi_1.txt 和 mv_multi_2.txt |
| 7.12 | `./target/release/cloud139.exe mv /e2e_test_xxx/subdir/mv_multi_1.txt /e2e_test_xxx/subdir/mv_multi_2.txt /not_exist_dir/` | **边界**：目标目录不存在 |
| 7.13 | `./target/release/cloud139.exe mv /e2e_test_xxx/subdir/mv_multi_1.txt /not_exist.txt /e2e_test_xxx/` | **边界**：多个源路径中有一个不存在 |

#### 阶段 8: 创建目录测试 (mkdir)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 8.1 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/subdir` | 创建子目录 |
| 8.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 subdir |
| 8.3 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/subdir` | **边界**：目录已存在，云端已存在；应提示警告且退出码为1 |
| 8.4 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/subdir --force` | 强制创建，云端会自动重命名 |
| 8.5 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/not_exist/child` | **边界**：父目录不存在 |

#### 阶段 9: 删除测试 (rm)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 9.0 | `echo "test" > test_delete.txt && ./target/release/cloud139.exe upload test_delete.txt /e2e_test_xxx/` | 准备删除测试文件 |
| 9.1 | `./target/release/cloud139.exe rm /e2e_test_xxx/test_delete.txt --yes` | 移到回收站 |
| 9.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | test_delete.txt 已删除 |
| 9.3 | `./target/release/cloud139.exe rm /not_exist.txt --yes` | **边界**：文件不存在 |
| 9.4 | `./target/release/cloud139.exe rm /Cargo.toml` | 不带 --yes 应提示确认 |
| 9.5 | `./target/release/cloud139.exe rm / --yes` | **边界**：不能删除根目录 |
| 9.6 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/rm_empty_dir` | 准备：创建待删除的空目录 |
| 9.7 | `./target/release/cloud139.exe rm /e2e_test_xxx/rm_empty_dir --yes` | 删除空目录成功 |
| 9.8 | `./target/release/cloud139.exe ls /e2e_test_xxx` | rm_empty_dir 已删除 |
| 9.9 | `./target/release/cloud139.exe mkdir /e2e_test_xxx/rm_nonempty_dir && echo "dir_file" > rm_dir_file.txt && ./target/release/cloud139.exe upload rm_dir_file.txt /e2e_test_xxx/rm_nonempty_dir/` | 准备：创建非空目录 |
| 9.10 | `./target/release/cloud139.exe rm /e2e_test_xxx/rm_nonempty_dir --yes` | 删除非空目录成功 |
| 9.11 | `./target/release/cloud139.exe ls /e2e_test_xxx` | rm_nonempty_dir 已删除 |
| 9.12 | `./target/release/cloud139.exe rm /e2e_test_xxx/not_exist_dir --yes` | **边界**：目录不存在 |

#### 阶段 10: 同步测试 (sync)

> **sync 命令说明**：`sync` 命令参考 rsync 语义，支持本地与云端之间的单向同步。同步方向完全由 SRC/DEST 参数位置决定。
> - 本地 → 云端：`sync ./local cloud:/remote`
> - 云端 → 本地：`sync cloud:/remote ./local`
> - 云端路径以 `cloud:` 前缀标识，之后直接跟云端路径（如 `cloud:/backup`）
> - **执行顺序要求**：阶段 10 的各子步骤应串行执行；不要对同一个云端目标目录并发执行多条 `sync` 命令，否则会互相污染结果（例如触发云端自动重命名）

##### 阶段 10 环境准备

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

##### 10.1 基础上传同步（本地 → 云端）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.1.1 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target` | 不带 `-r` 时仅同步源目录根下文件，子目录被跳过；按当前准备数据，应传输 `file1.txt`、`file2.txt` 共 2 个文件 |
| 10.1.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | 在 10.1.1 之后递归同步，应补传 `subdir/sub.txt` 1 个文件；若跳过 10.1.1 直接执行，则会首次全量同步 3 个文件 |
| 10.1.3 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 云端应有 file1.txt、file2.txt；应有 subdir 目录 |
| 10.1.4 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target/subdir` | 云端 subdir 应有 sub.txt |
| 10.1.5 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | **增量同步**：文件未变化，应全部 Skip，输出 `0 个文件传输, 3 个跳过` |
| 10.1.6 | 在源目录下创建空目录 `empty_dir` 后执行 `sync ... -r` | 云端应出现空目录 `empty_dir` |

##### 10.2 --dry-run 演习模式

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.2.1 | 修改本地文件：`echo "modified" > cloud139_e2e_sync_src/file1.txt` | 准备 |
| 10.2.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r -n` | 演习模式：输出操作计划（包含 `(DRY RUN)` 前缀），不实际传输；退出码为 0 |
| 10.2.3 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证云端 file1.txt **未被修改**（dry-run 不应产生实际变更） |

##### 10.3 增量同步（内容变化）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.3.1 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | file1.txt 已修改，应只传输 1 个文件，其余 2 个 Skip；输出 `1 个文件传输, 2 个跳过` |

##### 10.4 --delete 删除多余文件

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.4.1 | 删除本地文件：`rm cloud139_e2e_sync_src/file2.txt` | 准备 |
| 10.4.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r` | 不带 --delete：file2.txt 仍保留在云端；输出 `0 个文件传输, 2 个跳过` |
| 10.4.3 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --delete -n` | dry-run 下应显示 `*deleting` 标记，不实际删除 |
| 10.4.4 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --delete` | 实际删除：云端 file2.txt 应被移除 |
| 10.4.5 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证云端最终已无 file2.txt；若删除后立即 `ls` 仍短暂可见，等待片刻后重试一次，或执行 `rm /e2e_test_xxx/sync_target/file2.txt --yes` 应返回“文件不存在” |


| 10.4.6 | 删除本地空目录 `empty_dir` 后执行 `sync ... -r --delete` | 云端空目录应被删除 |

##### 10.5 --exclude 排除规则

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.5.1 | 准备：`echo "secret" > cloud139_e2e_sync_src/.env && echo "build" > cloud139_e2e_sync_src/output.log` | 准备排除测试文件 |
| 10.5.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --exclude "*.log" --exclude ".env"` | `.env` 和 `output.log` 应被跳过，不传输到云端 |
| 10.5.3 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证云端无 `.env`、无 `output.log` |
| 10.5.4 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --exclude "subdir"` | 排除整个子目录 |
| 10.5.5 | `./target/release/cloud139.exe ls /e2e_test_xxx/sync_target` | 验证 subdir 目录及其内容未被同步 |

##### 10.6 --checksum 精确对比

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.6.1 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r --checksum` | 使用哈希对比，内容未变的文件应全部 Skip；观察终端输出，显示警告（需扫描 checksum 耗时） |
| 10.6.2 | 准备一个"同大小不同内容"的文件后执行 `sync ... -r --checksum` | 应识别为变化并传输，而不是 Skip |

##### 10.7 --jobs 并发控制

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.7.1 | 准备多文件：`for i in $(seq 1 8); do echo "content $i" > cloud139_e2e_sync_src/batch_$i.txt; done` | 准备 8 个文件 |
| 10.7.2 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r -j 2` | 限制 2 并发，8 个新文件全部上传成功；退出码为 0 |
| 10.7.3 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/e2e_test_xxx/sync_target -r -j 8` | 8 并发，全部 Skip（已存在）；退出码为 0 |

##### 10.8 云端 → 本地（下载方向）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.8.1 | `./target/release/cloud139.exe sync cloud:/e2e_test_xxx/sync_target ./cloud139_e2e_sync_dst -r` | 首次全量下载，云端所有文件下载到本地 |
| 10.8.2 | `ls ./cloud139_e2e_sync_dst/` | 验证本地有 file1.txt、subdir/ 等 |
| 10.8.3 | `./target/release/cloud139.exe sync cloud:/e2e_test_xxx/sync_target ./cloud139_e2e_sync_dst -r` | **增量**：无变化，全部 Skip |
| 10.8.4 | 修改本地文件：`echo "local modified" > ./cloud139_e2e_sync_dst/file1.txt` | 准备 |
| 10.8.5 | `./target/release/cloud139.exe sync cloud:/e2e_test_xxx/sync_target ./cloud139_e2e_sync_dst -r` | 云端 file1.txt 覆盖本地被修改版本；输出 `1 个文件传输` |

##### 10.9 路径参数错误处理（边界）

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 10.9.1 | `./target/release/cloud139.exe sync ./local1 ./local2 -r` | **边界**：两端均为本地路径，应立即报错，退出码 2，提示使用 `cp`/`mv` 等系统工具 |
| 10.9.2 | `./target/release/cloud139.exe sync cloud:/src cloud:/dst -r` | **边界**：两端均为云端路径，应立即报错，退出码 2 |
| 10.9.3 | `./target/release/cloud139.exe sync ./not_exist_src cloud:/e2e_test_xxx/sync_target -r` | **边界**：本地源目录不存在，扫描失败，退出码 2 |
| 10.9.4 | `./target/release/cloud139.exe sync ./cloud139_e2e_sync_src cloud:/not_exist_remote_dir -r` | **边界**：云端目标目录不存在，扫描/创建失败，退出码 2 |

##### 阶段 10 清理

```bash
# 删除本地临时目录
rm -rf cloud139_e2e_sync_src
rm -rf cloud139_e2e_sync_dst
rm -f cloud139_e2e_sync_src/.env cloud139_e2e_sync_src/output.log  # 已在 rm -rf 中处理

# 删除云端 sync 测试目录（在阶段 4 总清理中统一删除 e2e_test_xxx）
```

### 4. 清理

> ⚠️ **重要警告**：清理时**绝对不要删除本地的项目核心文件**（如 `Cargo.toml`、`README.md`）。下载测试会在当前目录覆盖这些文件，但这是正常行为，不需要清理。需要清理的是**云端**的测试文件。

清理本地临时 JSON 输出文件：
```bash
rm -f ls_result.json
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

### 5. 生成报告

汇总所有测试结果，生成测试报告。

> **报告时应包含在执行过程中发现的潜在问题或风险**，如果有 SKILL 中没有清晰描述的情况，也应在报告中指出并建议添加到 SKILL 中。
