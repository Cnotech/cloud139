---
name: bump-version
description: 更新项目版本号，更新依赖，提交并打标签
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: versioning
---

## 功能描述

更新 cloud139 项目的版本号，包括：
- 根据用户选择增加 major、minor 或 patch 版本位
- 更新 Cargo.toml 中的版本号
- 运行 cargo update -w 更新依赖
- 提交 Cargo.toml 和 Cargo.lock
- 创建并推送 Git tag

## 执行流程

### 1. 读取当前版本

读取项目根目录的 `Cargo.toml` 文件，解析并打印当前的版本号（格式：`x.y.z`）。

### 2. 询问用户

询问用户新的版本号是增加 major、minor 还是 patch 位，并为每一个选项预览选择后版本号会升级到多少：
- **major**: 主版本号增加（如 1.1.0 → 2.0.0）
- **minor**: 次版本号增加（如 1.1.0 → 1.2.0）
- **patch**: 补丁版本号增加（如 1.1.0 → 1.1.1）

### 3. 更新 Cargo.toml

根据用户选择更新版本号：
- 选择 major: x+1.0.0
- 选择 minor: x.y+1.0
- 选择 patch: x.y.z+1

使用 Edit 工具将新版本号写入 Cargo.toml。

### 4. 运行 cargo update

在项目根目录执行：
```bash
cargo update -w
```

这会更新 Cargo.lock 中 workspace 依赖的版本。

### 5. Git 提交

提交 Cargo.toml 和 Cargo.lock：
```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to v{x.y.z}"
```

### 6. 创建并推送 Tag

创建 Git tag 并推送：
```bash
git tag v{x.y.z}
git push
git push --tags
```

## 错误处理

- 如果 tag `v{x.y.z}` 已存在，报错并询问用户如何处理（跳过、删除旧 tag、或终止操作）
- 如果 git push 失败，检查网络连接或远程仓库状态

## 验证

操作完成后，显示：
- 新版本号
- 提交的 commit hash
- 创建的 tag 名称
