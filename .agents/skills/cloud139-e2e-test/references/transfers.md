# 列表、上传和下载

使用[真实测试环境](real-run.md)。云端路径均在 `$CLOUD` 内。默认下载只在临时工作目录执行。

## 阶段 2 · 列表与分页

上传 `"$FIX/paged"` 到 `"$CLOUD/s02"`，核对清单后设 `PAGE="$CLOUD/s02/paged"`。夹具根含 121 个文件和 2 个目录，`child` 另含 121 个文件，`empty` 为空。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 2.1 | `c139 ls "$PAGE" -p 1 -s 50 -o "$LOCAL/logs/ls-page1.json"` | 0。JSON 是含 `path/page/page_size/total/items` 的对象，`total=123`，50 项。 |
| 2.2 | 用不同输出文件查询第 2、3 页，仍用 `-s 50`。 | 0。分别 50、23 项。列表稳定时，三页无重复，并集等于夹具根清单。按集合比较，避免假定某个文件必定在某页。 |
| 2.3 | `c139 ls -R "$PAGE" -o "$LOCAL/logs/ls-tree.json"` | 0。文本含各目录完整路径。JSON 为含 `path/total/items` 的目录对象数组，包括 `empty`。242 个文件各出现一次。 |
| 2.4 | 在 2.3 的命令加 `-p 2 -s 1`，写到另一文件。 | 0。递归模式忽略两个分页选项，结果与 2.3 相同，`child` 的 121 个文件齐全。 |
| 2.5 | 普通 `ls -o` 查询 `"$PAGE/empty"`。 | 0。JSON 为对象，`total=0`、`items=[]`。 |
| 2.6 | `c139 ls "$CLOUD/s02/not-created"` | 1。报告目录不存在。 |
| 2.7 | 在隔离环境用 `"$BIN" -v debug --config "$CONFIG" ls "$PAGE"`。 | 0。调试日志可见，输出不含 Token 或签名链接。 |

递归结果把目录 `path` 转成相对 `PAGE` 的路径，再比较目录与条目的路径、名称、类型、大小。哈希用下载结果核对。

## 阶段 3 · 上传目录、空目录和重名

令 `UP="$CLOUD/s03"`。每条独立用例使用未占用的目标。失败前后对比清单。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 3.1 | `c139 upload "$FIX/tree" "$UP/keep"` | 0。结果在 `keep/tree`，与 `tree` 完整清单一致。 |
| 3.2 | `c139 upload "$FIX/tree/" "$UP/contents"` | 0。内容直接放在 `contents`，保留两个空目录。 |
| 3.3 | 分别上传 `"$FIX/empty"` 到 `"$UP/empty-container"`，`"$FIX/empty/"` 到 `"$UP/empty-contents"`。 | 0。结果分别是空的 `empty-container/empty` 和 `empty-contents`。 |
| 3.4 | `c139 upload "$FIX/tree" "$UP/missing/parents/dest"` | 0。创建缺失的目录，`dest/tree` 与夹具清单一致。 |
| 3.5 | 建好 `"$UP/same"` 后上传 `"$FIX/same-v1/same.bin"`。 | 0。下载内容为 AAAA，记录 SHA-256。 |
| 3.6 | 向同一目标上传 `"$FIX/same-v2/same.bin"`。 | 1。原名仍为 AAAA，清单不变。 |
| 3.7 | 重做 3.6，增加 `--force`。 | 0。原名为 AAAA，新名称为 BBBB。下载分别核对 SHA-256，登记新名称。 |
| 3.8 | 建好 `"$UP/file-conflict"`，将 `"$FIX/tree-as-file/tree"` 上传到其中。再上传 `"$FIX/tree"` 到同一目录，各试一次默认和 `--force`。 | 首次 0，两个目录上传均为 1。普通文件 `tree` 及目录清单不变。 |
| 3.9 | `c139 mkdir -p "$UP/dir-conflict/same.bin"` 后用 `--force` 上传 `"$FIX/same-v2/same.bin"` 到 `"$UP/dir-conflict"`。 | 1。目标的同名目录与清单不变。 |
| 3.10 | 尝试上传不存在的本地文件。另将现有单文件 `"$FIX/files/a.txt"` 上传到缺失的 `"$UP/not-created-dir"`。 | 均为 1。本地源缺失或远端目标目录缺失，原云端清单不变。 |
| 3.11 | 先上传 `"$FIX/same-v1/"` 到 `"$UP/same-directory"`，再上传 `"$FIX/same-v2/"` 到同一处。 | 首次 0，第二次 1。目录内仍只有 AAAA。 |
| 3.12 | 用 `--force` 重做 3.11 的第二次上传。 | 0。按 3.7 核对两个名称和 SHA-256。 |

## 阶段 4 · 上传结果的内容基线

| 用例 | 操作与预期 |
| --- | --- |
| 4.1 | `c139 ls -R "$CLOUD/s03" -o "$LOCAL/logs/upload-baseline.json"`。确认目标层级、类型、空目录和新名称。 |
| 4.2 | 把 3.1、3.2、3.4 的结果分别下载到新路径。逐一与 `tree` 的本地清单比较。 |
| 4.3 | 比对所有冲突用例的前后清单，确认失败后原内容保留，强制上传后能找到新内容。新对象都记入台账。 |

## 阶段 5 · 下载与本地冲突

上传 `"$FIX/tree"` 到 `"$CLOUD/s05"` 并核对清单。令 `DLSRC="$CLOUD/s05/tree"` 和 `DL="$LOCAL/downloads"`。

| 用例 | 操作 | 预期 |
| --- | --- | --- |
| 5.1 | `c139 download "$DLSRC/root.txt" "$DL/renamed-file.txt"` | 0。新文件的 SHA-256 与夹具一致。 |
| 5.2 | 在独立临时工作子目录执行 `c139 download "$DLSRC/root.txt"`。 | 0。写入同名文件，内容一致。 |
| 5.3 | `c139 download -r "$DLSRC" "$DL/new-tree"`，目标尚不存在。 | 0。内容直接写入 `new-tree`，清单一致。 |
| 5.4 | 建好空目录 `"$DL/existing"` 后下载目录到该路径。 | 0。结果在 `existing/tree`，清单一致。 |
| 5.5 | 下载目录到尚不存在的 `"$DL/trailing/"`。 | 0。结果在 `trailing/tree`，清单一致。 |
| 5.6 | 下载不存在的远端文件到 `"$DL/missing.bin"`。 | 1。目标文件未创建。 |
| 5.7 | 尝试下载 `"$DLSRC"` 到新路径，另用 `-r` 下载到另一个新路径。 | 前者 1 且不创建目标，后者 0 且完整清单一致。 |
| 5.8 | `c139 download -r "$DLSRC/empty" "$DL/empty-new"` | 0。目标是空目录。 |
| 5.9 | 分别用普通文件占住目标 `"$DL/conflict-new"` 和递归目标中的 `"$DL/conflict-parent/tree/empty"`，再下载到 `conflict-new` 和已存在的 `conflict-parent`。 | 均为 1。写入前拒绝冲突，原文件和全部清单不变。 |
| 5.10 | 将 `root.txt` 分别下载到缺少父目录的 `"$DL/new-parent/renamed.txt"` 和以斜杠结尾的 `"$DL/new-output/"`。 | 0。创建各自父目录，结果分别是 `renamed.txt` 与 `new-output/root.txt`，SHA-256 一致。 |
