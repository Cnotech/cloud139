# 构建与测试命令

## 构建

```bash
# Debug 构建
cargo build

# Release 构建
cargo build --release
```

## 测试

```bash
# 运行所有测试
cargo test

# 运行单个测试
cargo test test_parse_path_root

# 运行指定测试文件
cargo test --test commands_test

# 运行集成测试
cargo test --test api_test

# 显示测试输出
cargo test -- --nocapture
```

## 代码检查

```bash
# Clippy 检查
cargo clippy

# 格式化检查
cargo fmt --check

# 格式化代码
cargo fmt
```

## 开发常用命令

```bash
# 运行程序
cargo run -- --help

# 运行特定命令
cargo run -- ls /

# 清理构建缓存
cargo clean
```

## 命令列表

### sync

同步本地目录和云端目录。

```bash
cloud139 sync <SRC> <DEST> [OPTIONS]
```

`SRC` 和 `DEST` 决定同步方向。以 `cloud:` 开头的路径是云端路径，其他路径是本地路径。

```bash
# 本地到云端
cloud139 sync ./backup cloud:/backup -r --delete -j 8

# 云端到本地
cloud139 sync cloud:/photos ./photos -r -n

# 排除特定文件
cloud139 sync . cloud:/project -r --exclude .git/** --exclude target/**

# 使用校验和比较
cloud139 sync ./data cloud:/data -r --checksum
```

**参数说明：**

| 参数 | 简写 | 默认值 | 说明 |
|------|------|--------|------|
| --recursive | -r | false | 递归同步子目录 |
| --dry-run | -n | false | 演习模式，只输出操作计划 |
| --delete | - | false | 删除目标中源没有的文件 |
| --checksum | - | false | 用 SHA-1 校验和替代大小和修改时间做对比 |
| --exclude | - | [] | 排除匹配的路径，可多次指定 |
| --jobs | -j | 4 | 并发传输数量上限 |
