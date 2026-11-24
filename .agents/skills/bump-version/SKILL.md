---
name: bump-version
description: 同步更新 cloud139 库和 CLI 的工作区版本及锁文件，提交并打标签
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: versioning
---

## 功能描述

同步更新 `cloud139` 库和 `cloud139-cli` 的版本号：
- 根据用户选择增加 major、minor 或 patch 版本位
- 更新根目录 `Cargo.toml` 中的工作区版本和 CLI 对库的版本约束
- 运行 `cargo update --workspace`，将工作区版本写入 `Cargo.lock`
- 提交根目录的 `Cargo.toml` 和 `Cargo.lock`
- 创建并推送 Git tag

## 执行流程

### 1. 读取当前版本

读取根目录 `Cargo.toml` 的 `[workspace.package].version`，解析并打印当前版本号（格式 `x.y.z`）。库和 CLI 都通过 `version.workspace = true` 继承该值。

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

在根目录 `Cargo.toml` 中，将 `[workspace.package].version` 和 `[workspace.dependencies]` 下 `cloud139` 的 `version` 更新为相同的新版本号。依赖的版本约束用于打包 CLI，`path` 用于从工作区构建。

### 4. 运行 cargo update

在项目根目录执行：
```bash
cargo update --workspace
```

这会同步 `Cargo.lock` 中工作区包的版本，并尽量保留第三方依赖的锁定版本。需要升级第三方依赖时，按用户请求单独更新对应依赖。

### 5. Git 提交

提交根目录的 `Cargo.toml` 和 `Cargo.lock`：
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
