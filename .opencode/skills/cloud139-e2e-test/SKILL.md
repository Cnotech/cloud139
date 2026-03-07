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

首先编译项目：
```bash
cargo build --release
```

使用提供的 token 登录：
```bash
./target/release/cloud139 login --token <token> --storage-type personal_new
```

创建一个随机命名的测试目录，格式：`e2e_test_{timestamp}`
```bash
./target/release/cloud139 mkdir /e2e_test_xxx
```

### 3. 测试执行顺序

#### 阶段 1: 列表测试 (ls)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 1.1 | `./target/release/cloud139 ls /` | 能列出根目录内容 |
| 1.2 | `./target/release/cloud139 ls /e2e_test_xxx` | 能列出空目录 |
| 1.3 | `./target/release/cloud139 ls /not_exist_dir` | **边界**：返回错误 |

#### 阶段 2: 上传测试 (upload)

测试上传当前目录的 `README.md` 和 `Cargo.toml`：

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 2.1 | `./target/release/cloud139 upload README.md /` | 上传到根目录 |
| 2.2 | `./target/release/cloud139 upload Cargo.toml /` | 上传到根目录 |
| 2.3 | `./target/release/cloud139 upload README.md /e2e_test_xxx/` | 上传到测试目录 |
| 2.4 | `./target/release/cloud139 upload Cargo.toml /e2e_test_xxx/` | 上传到测试目录 |
| 2.5 | `./target/release/cloud139 upload not_exist_file.txt /` | **边界**：本地文件不存在 |
| 2.6 | `./target/release/cloud139 upload README.md /not_exist_dir/` | **边界**：远程目录不存在 |

#### 阶段 3: 列表验证

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 3.1 | `./target/release/cloud139 ls /` | 应包含 README.md, Cargo.toml |
| 3.2 | `./target/release/cloud139 ls /e2e_test_xxx` | 应包含上传的两个文件 |

#### 阶段 4: 下载测试 (download)

> 请注意在下载完成后检查本地文件是否存在、文件大小是否与云端一致


首先创建本地临时测试目录：
```bash
mkdir -p cloud139_e2e_download_test
```

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 4.1 | `./target/release/cloud139 download /README.md` | 下载成功（默认文件名） |
| 4.2 | `./target/release/cloud139 download /e2e_test_xxx/Cargo.toml` | 下载成功 |
| 4.3 | `./target/release/cloud139 download /README.md ./cloud139_e2e_download_test/` | 下载到指定目录（保持原名） |
| 4.4 | `./target/release/cloud139 download /e2e_test_xxx/Cargo.toml ./cloud139_e2e_download_test/custom_name.toml` | 下载并重命名 |
| 4.5 | `ls ./cloud139_e2e_download_test/` | 验证文件已保存 |
| 4.6 | `./target/release/cloud139 download /not_exist.txt` | **边界**：文件不存在 |
| 4.7 | `./target/release/cloud139 download /e2e_test_xxx` | **边界**：不能下载目录 |
| 4.8 | `./target/release/cloud139 download /Cargo.toml ./non-exist-dir-1/` | **边界**：自动创建目录并成功下载文件 |
| 4.9 | `./target/release/cloud139 download /README.md ./non-exist-dir-2/custom.txt` | **边界**：自动创建目录并成功下载文件 |

测试完成后清理本地临时目录：
```bash
rm -rf cloud139_e2e_download_test
```

#### 阶段 5: 复制测试 (cp)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 5.1 | `./target/release/cloud139 cp /Cargo.toml /e2e_test_xxx/` | 复制到测试目录（云端自动重命名） |
| 5.2 | `./target/release/cloud139 ls /e2e_test_xxx` | 应有 3 个文件（含自动重命名的文件） |
| 5.3 | `./target/release/cloud139 cp /not_exist.txt /tmp` | **边界**：源文件不存在 |
| 5.4 | `./target/release/cloud139 cp /README.md /not_exist_dir/` | **边界**：目标目录不存在 |

#### 阶段 6: 重命名测试 (rename)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 6.1 | `./target/release/cloud139 rename /e2e_test_xxx/README.md README_copy.md` | 重命名成功 |
| 6.2 | `./target/release/cloud139 ls /e2e_test_xxx` | 应有 README_copy.md |
| 6.3 | `./target/release/cloud139 rename / new_name` | **边界**：不能重命名根目录 |
| 6.4 | `./target/release/cloud139 rename /not_exist.txt new.txt` | **边界**：文件不存在 |

#### 阶段 7: 移动测试 (mv)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 7.1 | `./target/release/cloud139 mv /e2e_test_xxx/README_copy.md /` | 移动到根目录 |
| 7.2 | `./target/release/cloud139 ls /` | 应有 README_copy.md |
| 7.3 | `./target/release/cloud139 ls /e2e_test_xxx` | README_copy.md 已移出 |
| 7.4 | `./target/release/cloud139 mv /README_copy.md /not_exist_dir/` | **边界**：目标不存在 |
| 7.5 | `./target/release/cloud139 mv / /somewhere` | **边界**：不能移动根目录 |

#### 阶段 8: 创建目录测试 (mkdir)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 8.1 | `./target/release/cloud139 mkdir /e2e_test_xxx/subdir` | 创建子目录 |
| 8.2 | `./target/release/cloud139 ls /e2e_test_xxx` | 应有 subdir |
| 8.3 | `./target/release/cloud139 mkdir /e2e_test_xxx/subdir` | **边界**：目录已存在 |
| 8.4 | `./target/release/cloud139 mkdir /e2e_test_xxx/not_exist/child` | **边界**：父目录不存在 |

#### 阶段 9: 删除测试 (rm)

| 步骤 | 命令 | 验证点 |
|------|------|--------|
| 9.1 | `./target/release/cloud139 rm /README_copy.md --yes` | 移到回收站 |
| 9.2 | `./target/release/cloud139 ls /` | README_copy.md 已删除 |
| 9.3 | `./target/release/cloud139 rm /not_exist.txt --yes` | **边界**：文件不存在 |
| 9.4 | `./target/release/cloud139 rm /Cargo.toml` | 不带 --yes 应提示确认 |
| 9.5 | `./target/release/cloud139 rm / --yes` | **边界**：不能删除根目录 |

### 4. 清理

测试完成后清理测试数据：
```bash
./target/release/cloud139 rm /e2e_test_xxx --yes
./target/release/cloud139 rm /README.md --yes
./target/release/cloud139 rm /Cargo.toml --yes
```

### 5. 生成报告

汇总所有测试结果，生成测试报告。

## 边界情况总结

| 场景 | 预期行为 |
|------|----------|
| 根目录 `/` 操作 | 删除/移动/重命名应被拒绝 |
| 不存在路径 | 返回错误信息 |
| 空目录 | 正常显示空列表 |
| 重名文件 | 按云端规则处理（自动重命名或覆盖） |
| 不带 --yes 删除 | 提示确认信息 |

## 测试文件

使用项目根目录下的文件进行测试：
- `README.md`
- `Cargo.toml`
