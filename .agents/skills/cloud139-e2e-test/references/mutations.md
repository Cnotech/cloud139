# 复制、重命名、移动、建目录和回收

使用[真实测试环境](real-run.md)。每阶段用独立子树。成功、冲突、多源和缺失用例分开准备。比较前后清单，并轮询确认实际结果。

## 阶段 6 · 复制

令 `CP="$CLOUD/s06"`。上传 `tree` 到 `$CP/source`，将 `files/` 和 `duplicates/` 分别上传到 `$CP/files`、`$CP/dup`。目录源为 `"$CP/source/tree"`。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 6.1 | 创建空目标 `"$CP/dest-single"`。 | 0，轮询确认。 |
| 6.2 | 将目录源复制到 `dest-single`。 | 0，源仍在，目标 `dest-single/tree` 的清单一致。 |
| 6.3 | 单独创建空目标 `"$CP/dest-multi"`。 | 0，目标为空。 |
| 6.4 | 以 `REQUEST_LIMITS='/file/batchCopy=1'` 在一次命令中复制 `files/a.txt` 和 `files/b.txt` 到 `dest-multi`。 | 0，源与目标均有对应文件，哈希一致。日志中批量复制请求最多 1 次。 |
| 6.5 | 复制目录到自身，再加 `--force` 复制到其 `nested`。 | 均为 1，源不变。 |
| 6.6 | 用 `files/blocker.bin` 和另一个缺失路径分别作目标。 | 均为 1，源与目标状态不变。 |
| 6.7 | 在独立目标放同名非空 `tree`，再用 `--force` 复制目录源过去。 | 1，两个目录清单不变。 |
| 6.8 | 在另一目标放普通文件 `tree`，再用 `--force` 复制目录源过去。 | 1，目录和同名文件均不变。 |
| 6.9 | 将 `a.txt` 复制到新的 `file-repeat`，再重复复制。 | 首次 0，第二次 1，重复前后的快照一致。 |
| 6.10 | 在 6.9 的目标加 `--force` 重试同名文件。 | CLI 允许提交。服务端接受后核对原目标、新副本名称与哈希、源文件。 |
| 6.11 | 用 `--force` 将 `dup/left/shared.txt`、`dup/right/shared.txt` 一起复制到 `dest-multi`。 | 1，两个同名源与目标清单不变。 |
| 6.12 | 先建空目标 `dest-preflight`，混合提交一个有效源和一个缺失源。 | 1，有效源也未复制，目标仍为空。 |

## 阶段 7 · 重命名

令 `REN="$CLOUD/s07"`。上传 `tree` 到 `$REN/source`，将 `files/` 上传到 `$REN/files`。冲突夹具放在 `$REN/conflict`，其中有非空目录 `tree`、`taken` 和普通文件 `blocker.bin`。用目录上传创建这些父目录，再放单文件。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 7.1 | `c139 rename "$REN/files/a.txt" renamed-a.txt` | 0，旧名消失，新文件哈希一致。 |
| 7.2 | `c139 rename "$REN/source/tree" renamed-tree` | 0，旧路径消失，非空目录及其空子目录完整。 |
| 7.3 | 将 7.2 的结果再重命名为 `renamed-tree`。 | 0，清单不变。 |
| 7.4 | 重命名缺失的源为 `new-name`。 | 1，源与目标均未创建。 |
| 7.5 | 用 `--force` 将 `conflict/tree` 改为 `taken`。 | 1，两个目录不变。 |
| 7.6 | 用 `--force` 将 `conflict/tree` 改为 `blocker.bin`。 | 1，目录与文件不变。 |
| 7.7 | 创建 `files/taken-dir`，再用 `--force` 将 `files/b.txt` 改为该目录名。 | 1，文件与目录不变。 |
| 7.8 | 将 `files/renamed-a.txt` 改为现名。 | 0，文件哈希和清单不变。 |
| 7.9 | 在独立目录用 `files/a.txt`、`files/b.txt` 准备不同内容的文件。把前者改为后者名称，分别试默认和 `--force`。 | 默认返回 1，清单不变。`--force` 允许提交。服务端接受后确认源路径消失，按实际名称定位源内容并登记原目标的结果。 |

## 阶段 8 · 移动

令 `MV="$CLOUD/s08"`。上传 `tree` 到 `$MV/source`，将 `files/` 上传到 `$MV/files`，建好独立空目标 `dest-dir`、`dest-multi`。另准备 `conflict-source/tree`、`conflict-dir/tree` 两个非空目录，在 `conflict-file` 中放普通文件 `tree`。将 `files/` 与 `other-files/` 分别上传到 `same-source` 和 `same-name`，得到同名但内容不同的 `a.txt`。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 8.1 | `c139 mv "$MV/source/tree" "$MV/dest-dir"` | 0，旧路径消失，目标 `dest-dir/tree` 出现。 |
| 8.2 | 列出并下载 8.1 的目标。 | 哈希、多层目录和空目录与夹具一致。 |
| 8.3 | 以 `REQUEST_LIMITS='/file/batchMove=1'` 同时将 `files/a.txt`、`files/b.txt` 移到 `dest-multi`。 | 0，两个源消失，目标哈希一致。日志中批量移动请求最多 1 次。 |
| 8.4 | 将 `conflict-source/tree` 移到自身及其 `nested`，再检查 `--force` 变体。 | 均为 1，目录清单不变。 |
| 8.5 | 用 `files/blocker.bin` 和缺失路径分别作目标。 | 均为 1，源与目标状态不变。 |
| 8.6 | 将 `same-source/a.txt` 移到 `same-name`。 | 1，源和目标哈希不变。 |
| 8.7 | 在 8.6 加 `--force`。 | CLI 允许提交。服务端接受后检查源消失，核对目标的名称和对应哈希。 |
| 8.8 | 用 `--force` 将 `conflict-source/tree` 分别移到 `conflict-dir` 和 `conflict-file`。 | 均为 1，源与目标不变。 |
| 8.9 | 把 `duplicates/` 上传到 `dup`，建独立空目标。混合提交两个不同目录下的 `shared.txt`，再试有效源加缺失源。 | 均为 1。`--force` 下两个同名源也被拒绝。有效源未移动，目标仍为空。 |
| 8.10 | 把现有文件移到它自己的父目录。 | 1，文件不变。 |

## 阶段 9 · 创建目录

令 `MK="$CLOUD/s09"`。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 9.1 | `c139 mkdir -p "$MK"` 后执行 `c139 mkdir "$MK/single"`。 | 0，只创建指定目录。 |
| 9.2 | `c139 mkdir -p "$MK/missing/a/b/empty"` | 0，缺失的父目录齐全，叶目录为空。 |
| 9.3 | 重复 9.2，再以等价路径 `"$MK/missing//a/./b/empty/"` 执行。 | 均为 0，清单不变。 |
| 9.4 | 重复创建 `"$MK/single"`，不带 `-p`。 | 1，目录已存在，清单不变。 |
| 9.5 | 不带 `-p` 创建 `"$MK/not-created/child"`。 | 1，缺失的父目录未创建。 |
| 9.6 | 上传 `blocker.bin` 到 `$MK` 后执行 `c139 mkdir -p "$MK/blocker.bin/child"`。 | 1，父路径是文件，哈希不变。 |
| 9.7 | `c139 mkdir -p "$MK/blocker.bin"` | 1，叶路径是文件，清单不变。 |

## 阶段 10 · 回收文件与目录

令 `RM="$CLOUD/s10"`，上传 `files`、`empty`、`tree` 三个目录到其中并核对清单。删除只针对本轮已登记的夹具。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 10.1 | `c139 rm "$RM/files/a.txt"` | 1，提示 `--yes`，文件不变。 |
| 10.2 | `c139 rm --yes "$RM/files/a.txt"` | 0，确认文件从父列表消失，其他条目不变。 |
| 10.3 | 用 `--yes` 删除缺失的文件。 | 1，报不存在。 |
| 10.4 | `c139 rm -r "$RM/tree"` | 1，仍需 `--yes`，目录清单不变。 |
| 10.5 | 已移除参数见[离线用例 13.2](offline.md#命令参数和存储类型)。 | 解析失败，配置不变。 |
| 10.6 | 根别名见[离线用例 13.1](offline.md#路径分页与故障)。 | 核对拒绝与请求数。 |
| 10.7 | 用 `ls -R -o` 检查 `"$RM/empty"`。 | 0，确认它由本轮创建且为空。 |
| 10.8 | `c139 rm --yes "$RM/empty"` | 1，缺 `-r`，空目录仍在。 |
| 10.9 | `c139 rm -r --yes "$RM/empty"` | 0，轮询确认空目录消失。 |
| 10.10 | 获取 `"$RM/tree"` 的目录清单与文件哈希。 | 保留删除前证据，确认文件和空子目录都存在。 |
| 10.11 | `c139 rm --yes "$RM/tree"` | 1，缺 `-r`，完整清单不变。 |
| 10.12 | `c139 rm -r --yes "$RM/tree"` | 0，轮询确认非空目录消失。 |
| 10.13 | 用 `-r --yes` 删除缺失的目录。 | 1，报不存在，其他对象不变。 |
