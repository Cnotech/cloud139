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

首先编译项目，确保测试的代码是最新的：
```bash
cargo build --release
```

使用提供的 token 登录：
```bash
./target/release/cloud139.exe login --token <token> --storage-type personal_new
```

检查并删除根目录下的遗留测试文件（如 README.md, Cargo.lock 等）：
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

### 3. 退出码校验规则

**通用规则**：如果命令未能正常执行，则程序退出码应当为 1。

具体场景：
- 边界情况（如文件不存在、目录不存在等）应返回退出码 1
- 用户未提供必要参数时应返回退出码 1
- 操作失败（如网络错误、API 错误等）应返回退出码 1
- 只有命令正常执行完成时才返回退出码 0

> 注：部分命令的 `--force` 参数会覆盖某些限制，此时即使有警告也可能返回 0（取决于具体实现）

### 4. 测试执行顺序

#### 阶段 1: 列表测试 (ls)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 1.1 | `./target/release/cloud139.exe ls /` | 能列出根目录内容 |
| 1.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 能列出空目录 |
| 1.3 | `./target/release/cloud139.exe ls /not_exist_dir` | **边界**：返回错误 |

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

#### 阶段 6: 重命名测试 (rename)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 6.1 | `./target/release/cloud139.exe rename /e2e_test_xxx/README.md README_copy.md` | 重命名成功 |
| 6.2 | `./target/release/cloud139.exe ls /e2e_test_xxx` | 应有 README_copy.md |
| 6.3 | `./target/release/cloud139.exe rename / new_name` | **边界**：不能重命名根目录 |
| 6.4 | `./target/release/cloud139.exe rename /not_exist.txt new.txt` | **边界**：文件不存在 |

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
| 9.1 | `./target/release/cloud139.exe rm /README_copy.md --yes` | 移到回收站 |
| 9.2 | `./target/release/cloud139.exe ls /` | README_copy.md 已删除 |
| 9.3 | `./target/release/cloud139.exe rm /not_exist.txt --yes` | **边界**：文件不存在 |
| 9.4 | `./target/release/cloud139.exe rm /Cargo.toml` | 不带 --yes 应提示确认 |
| 9.5 | `./target/release/cloud139.exe rm / --yes` | **边界**：不能删除根目录 |

### 4. 清理

测试完成后清理测试数据：
```bash
./target/release/cloud139.exe rm /e2e_test_xxx --yes
./target/release/cloud139.exe rm /README.md --yes
./target/release/cloud139.exe rm /Cargo.toml --yes
./target/release/cloud139.exe rm /e2e_random_{timestamp}.bin --yes
```

### 5. 生成报告

汇总所有测试结果，生成测试报告。

> **报告时应包含在执行过程中发现的潜在问题或风险**，如果有 SKILL 中没有清晰描述的情况，也应在报告中指出并建议添加到 SKILL 中。
