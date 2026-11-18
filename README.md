# cloud139

139云盘（移动云盘）命令行客户端，支持文件上传、下载、管理等操作。

> 当前仅支持个人云。家庭云、和家亲、群组盘及其他存储类型统一返回“未实现：当前仅支持个人云”。

## 功能特性

- 账号登录
- 文件列表查看
- 文件上传/下载
- 文件删除
- 目录创建
- 文件移动
- 文件复制
- 文件重命名
- 目录同步

## 快速开始

### 安装

从 [GitHub Releases](https://github.com/Cnotech/cloud139/releases) 下载对应平台的预编译二进制文件，然后添加到系统 PATH 中。

或者从源码编译：

```bash
# 克隆项目
git clone https://github.com/Cnotech/cloud139.git
cd cloud139

# 编译
cargo build -r

# 可执行文件位于 target/release/cloud139
```

### 登录

```bash
cloud139 login -t <YOUR_TOKEN>
```

Token 需要使用浏览器登陆移动云盘后打开开发者工具获取，具体步骤如下：
- 打开浏览器，登录[移动云盘网页版](https://yun.139.com/)
- 打开开发者工具（F12 或右键点击 -> 检查）
- 切换到`应用（Application）`标签页，点击`存储（Storage）`-`Cookie`-`https://yun.139.com`
- 找到 `authorization` 项，复制其值即可

> 如果粘贴 Token 后提示 `Invalid token`，可尝试去除 `Basic ` 前缀，或在 Token 前后增加单引号（`'`）
>
> 更多详情可参考：[AList 文档](https://alistgo.com/zh/guide/drivers/139.html#%E6%96%B0%E4%B8%AA%E4%BA%BA%E4%BA%91)

### 从 2.x 升级到 3.0

- 创建目录时去掉 `mkdir -f` 或 `mkdir --force`。需要同时创建父目录时使用 `mkdir -p`。
- 登录使用 `cloud139 login -t <TOKEN>`。旧脚本可移除 `--storage-type`、`--cloud-id` 和 `--root-folder-id`。
- `rm --yes` 将文件移到回收站，目录另加 `-r`。`--permanent` 已移除。
- `sync --jobs` 接受 1 到 32，默认 4。
- 个人云配置的 `storage_type` 使用 `personal_new`。重新登录可生成新格式的配置，移除旧的 `cloud_id`、`root_folder_id`、`user_domain_id` 和 `report_real_size` 字段。

### 基本使用

登录校验通过后保存配置。

```bash
# 列出根目录文件
cloud139 ls /

# 列出指定目录文件
cloud139 ls /myfolder

# 上传文件到根目录
cloud139 upload ./file.txt /

# 上传文件到指定目录
cloud139 upload ./photo.jpg /backup/

# 下载文件到当前目录
cloud139 download /file.txt

# 下载文件到指定路径
cloud139 download /file.txt ./downloads/myfile.txt

# 删除文件
cloud139 rm /file.txt -y

# 删除目录
cloud139 rm -r --yes /folder

# 创建目录
cloud139 mkdir /parent/newFolder

# 移动单个文件
cloud139 mv /old.txt /existing-directory/

# 移动多个文件到目录
cloud139 mv /file1.txt /file2.txt /folder/

# 复制单个文件到指定目录
cloud139 cp /file.txt /backup/

# 复制多个文件到目录
cloud139 cp /file1.txt /file2.txt /backup/

# 重命名文件
cloud139 rename /oldname.txt newname.txt

# 本地到云端同步（递归）
cloud139 sync ./local/path cloud:/remote/path -r

# 云端到本地同步（递归）
cloud139 sync cloud:/remote/path ./local/path -r
```

## 目录操作

```bash
# 上传目录本身：/backup/photos/<子项>；自动创建缺失目录
cloud139 upload ./photos /backup
# 仅上传内容：/backup/<子项>（注意本地参数末尾 /）
cloud139 upload ./photos/ /backup
# 目录下载到新的 ./photos；已有目标目录或末尾 / 表示在其中保留源目录名
cloud139 download -r /backup/photos ./photos
cloud139 download -r /backup/photos ./downloads/
cloud139 ls -R /backup
cloud139 ls -R /backup -o tree.json
cloud139 mkdir -p /a/b/empty
cloud139 cp /backup/photos /existing-destination/
cloud139 mv /backup/photos /existing-destination/
cloud139 rm -r --yes /a
```

目录上传复用 sync 的本地扫描及底层上传，不使用 sync 的同大小跳过规则；已有同名文件默认报错。`--force` 允许上传同名文件，保留云端为新文件分配的名称。目录与文件类型冲突始终报错。

`ls -R` 忽略页码选择，读取每个目录的全部分页；JSON 输出为目录对象数组（包括空目录），每个对象含 `path`、`total` 和 `items`。普通 `ls` 保持单目录分页输出格式。游标、重复分页或目录 ID 循环均报错。

目录创建不自动改名。复制、移动、重命名的目录重名冲突即使加 `--force` 也会拒绝；文件同名可使用 `--force`。禁止根目录变更，路径先规范化，复制/移动禁止自身与子目录目标。非法跨平台名称、服务端路径穿越名称和本地符号链接会被拒绝。

个人盘支持目录上传、下载、`mkdir -p`、`ls -R`、`rm -r`、多源复制和移动，以及文件和目录重命名。递归任务失败会返回非零退出码，此前成功的条目会保留。详见 [目录操作与测试说明](docs/directory-operations.md)。

## 全局选项

| 参数 | 简写 | 默认值 | 说明 |
|------|------|--------|------|
| --verbose | -v | info | 日志级别 (trace, debug, info, warn, error) |
| --config | -c | - | 指定配置文件路径。读取时优先使用该路径；`login` 时也写入该路径 |

## 命令参考

### login

登录139云盘账号。

```bash
cloud139 login -t <TOKEN>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| --token | -t | 是 | 授权Token |

**示例：**

```bash
# 登录
cloud139 login -t your_authorization_token
```

### ls

列出云盘文件。

```bash
cloud139 ls [-R] [-p <页码>] [-s <数量>] [-o <输出文件>] [路径]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 路径 | - | 否 | 文件路径，默认：/ |
| --page | -p | 否 | 页码，默认：1 |
| --page-size | -s | 否 | 每页数量，默认：100 |
| --recursive | -R | 否 | 递归列出全部目录和分页 |
| --output | -o | 否 | 将结果输出为JSON到指定文件 |

**示例：**

```bash
# 列出根目录
cloud139 ls

# 列出指定目录
cloud139 ls /myfolder

# 分页查看，每页50条，查看第2页
cloud139 ls / -p 2 -s 50

# 将结果输出为JSON文件
cloud139 ls / -o result.json
```

### upload

上传文件或递归上传本地目录到云盘。保留空目录，自动创建缺失的云端目标目录。

单个文件的上传大小上限因账号而异。超过上限时，报错会显示文件大小和云盘返回的错误码。

上传目录时，单个文件失败后仍会处理其余文件，最后汇总成功和失败数量。有文件上传失败时，退出码为 1。创建目录失败会停止上传。

```bash
cloud139 upload <本地路径> [远程目录] [-f]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 本地路径 | - | 是 | 本地文件或目录；末尾 / 表示仅上传目录内容 |
| 远程目录 | - | 否 | 云盘目标目录，默认：/ |
| --force | -f | 否 | 允许上传同名文件，由云端为新文件分配名称。文件与目录同名时会报错 |

**示例：**

```bash
# 上传文件到根目录
cloud139 upload ./file.txt /

# 上传文件到指定目录
cloud139 upload ./folder/photo.jpg /backup/

# 上传同名文件，由云端为新文件分配名称
cloud139 upload ./file.txt / -f
```

### download

从云盘下载文件，使用 `-r / --recursive` 递归下载目录并保留空目录。不带 `-r` 下载目录会报错。

```bash
cloud139 download [-r] <远程路径> [本地路径]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 远程路径 | - | 是 | 云盘文件或目录路径 |
| --recursive | -r | 否 | 递归下载目录 |
| 本地路径 | - | 否 | 本地保存路径，默认：当前目录的同名文件 |

**示例：**

```bash
# 下载文件到当前目录
cloud139 download /file.txt

# 下载文件到指定路径
cloud139 download /folder/photo.jpg ./downloads/
```

### rm

将云盘文件或目录移到回收站。

```bash
cloud139 rm [-r] --yes <路径>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 路径 | - | 是 | 要删除的文件或目录路径 |
| --yes | -y | 是 | 确认删除 |
| --recursive | -r | 目录必需 | 确认递归删除目录 |

**示例：**

```bash
# 删除文件
cloud139 rm /file.txt -y

# 删除目录
cloud139 rm -r --yes /folder
```

### mkdir

在云盘创建目录。

```bash
cloud139 mkdir [-p] <路径>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 路径 | - | 是 | 新目录路径，格式：/父目录/新目录名 |
| --parents | -p | 否 | 递归创建父目录；已有目录成功，同名文件报错 |

**示例：**

```bash
# 创建目录
cloud139 mkdir /newfolder

# 递归创建嵌套目录；重复执行成功
cloud139 mkdir -p /parent/child
```

### mv

移动文件或目录。

```bash
cloud139 mv <源路径...> <目标路径> [-f]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 源文件或目录路径；个人盘支持多个，用空格分隔 |
| 目标路径 | - | 是 | 必须是已有目录，不能是文件或缺失路径 |
| --force | -f | 否 | 目标目录已有同名文件时，仍提交移动。涉及目录的同名冲突会报错 |

**示例：**

```bash
# 移动单个文件
cloud139 mv /old.txt /existing-directory/

# 移动多个文件到目录
cloud139 mv /file1.txt /file2.txt /folder/

# 目标目录已有同名文件时仍提交移动
cloud139 mv /file.txt /folder/ -f
```

### cp

复制文件或目录。

```bash
cloud139 cp <源路径...> <目标路径> [-f]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 源文件或目录路径；个人盘支持多个，用空格分隔 |
| 目标路径 | - | 是 | 必须是已有目录，不能是文件或缺失路径 |
| --force | -f | 否 | 目标目录已有同名文件时，仍提交复制。涉及目录的同名冲突会报错 |

**示例：**

```bash
# 复制单个文件到指定目录
cloud139 cp /file.txt /backup/

# 复制多个文件到目录
cloud139 cp /file1.txt /file2.txt /backup/

# 目标目录已有同名文件时仍提交复制
cloud139 cp /file.txt /backup/ -f
```

### rename

重命名文件或目录。

```bash
cloud139 rename [-f] <源路径> <新名称>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 源文件或目录路径 |
| 新名称 | - | 是 | 新名称 |
| --force | -f | 否 | 新名称已被其他文件使用时，仍提交重命名。涉及目录的同名冲突会报错 |

**示例：**

```bash
# 重命名文件
cloud139 rename /oldname.txt newname.txt

# 重命名目录
cloud139 rename /folder/old newname
```

### sync

同步本地目录和云端目录。

同步到云盘时，先上传并确认新文件，再替换云端的同名文件或目录。即使启用了 `--delete`，待替换目录及其中的内容也会保留到替换成功。更新期间，云盘需要同时容纳新旧两份内容。替换失败时，报错会列出保留的临时副本或备份。

单个文件传输失败后，同步会继续处理其他文件。执行计划时如有操作失败，退出码为 1。

用文件替换云端同名目录需要 `-r`，且该目录及其后代都须纳入本轮同步范围。替换前会复核该目录的内容。本地目标出现文件与目录类型冲突，或本地目录与云端同名文件冲突时，同步会保留原条目并报告错误。云端到本地同步还会在传输前检查大小写及 Unicode 名称冲突。`sync --delete` 会保留 `.cloud139-upload-` 和 `.cloud139-backup-` 开头的恢复条目，以及包含这些条目的目录。替换失败后可按报错提示核对并手动恢复这些副本。

普通 `--delete` 在建目录和传输全部成功后才开始清理。清理逐项复核扫描时的对象，先处理子项，再处理确认已空的目录。发现变更或删除失败会停止后续清理。单文件可上传同步到云端目录，使用目录作为源才能启用 `--delete`。

`--verify` 用 SHA-256 识别大小相同但内容不同的文件。跳过前复核远端文件，替换前校验新上传的临时副本。远端未提供可比的 SHA-256 时，同步会读取该文件的远端内容计算哈希。这可能增加下载流量和耗时。校验模式固定串行。

`--verify --delete` 只清理初始扫描中目标端多余的文件。所有校验和更新通过后，同步会复核保留文件和待删除文件的 ID，再开始清理。空源、被排除规则筛空的源会拒绝执行。清理开始后如果服务端拒绝某次删除，已完成的删除保留，剩余删除停止。

`--json` 在执行或预览结束后向标准输出写一行同步报告，日志写到标准错误。普通同步和 `--verify` 都支持这个选项。

- `source` 和 `destination` 是命令中的源路径和目标路径。`changes` 列出目标端已确认的文件变更，包含相对路径 `path` 和操作 `action`（`upload`、`download` 或 `delete`）。用文件替换云端同名目录时，原目录下移出目标路径的文件也计入删除。
- `-n` 将 `dryRun` 设为 `true`，预览操作写在 `plan` 中，`changes` 为空。待内容校验的文件用 `verify` 标记，上传、下载和删除的计数表示计划中的文件数。
- `objects` 是源文件数，`uploaded`、`downloaded`、`deleted` 和 `skipped` 按文件计数。`hashMissing` 记录因缺少可比哈希而需要读取云端内容的文件数，`seconds` 是执行或预览用时。
- `failures` 每项包含 `path`、`error` 和 `retries`。执行中有失败时仍会报告已确认的变更，并返回退出码 1。参数、配置或初始扫描报错时，通过标准错误和退出码查看原因。

```bash
cloud139 sync <源路径> <目标路径> [选项]
```

**同步方向：**

> 云端路径需使用 `cloud:` 前缀标识。

- **本地 → 云端**：`cloud139 sync <本地路径> cloud:/<远程路径>`
- **云端 → 本地**：`cloud139 sync cloud:/<远程路径> <本地路径>`

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 本地路径或云端路径（cloud:/path） |
| 目标路径 | - | 是 | 本地路径或云端路径 |
| --recursive | -r | 否 | 递归同步子目录，空目录也会同步 |
| --dry-run | -n | 否 | 只预览同步计划。预览成功时退出码为 0 |
| --delete | - | 否 | 删除目标中源没有的文件或空目录 |
| --exclude | - | 否 | 排除匹配的路径（glob 模式），可多次指定 |
| --jobs | -j | 否 | 同时传输的文件数，范围 1 到 32。默认 4，校验模式固定为 1 |
| --verify | - | 否 | 校验内容并修复不一致的文件 |
| --json | - | 否 | 以 JSON 报告本次同步的文件变更 |

**示例：**

```bash
# 本地到云端同步（递归、删除目标多余文件、8并发）
cloud139 sync ./backup cloud:/backup -r --delete -j 8

# 校验备份内容，确认全部更新成功后清理多余文件
cloud139 sync ./backup cloud:/backup -r --verify --delete

# 同步并保存文件变更报告
cloud139 sync ./backup cloud:/backup -r --json > sync-report.json

# 预览云端到本地的同步计划
cloud139 sync cloud:/photos ./photos -r -n

# 排除特定文件或目录
cloud139 sync . cloud:/project -r -n --exclude .git/** --exclude target/**
```

## 配置文件

登录成功后，配置信息默认保存到当前工作目录的 `cloud139rc.toml`。

读取时按以下优先级查找：

1. 通过全局参数 `--config <PATH>` 指定的路径
2. 当前工作目录下的 `cloud139rc.toml`
3. 全局路径 `~/.config/cloud139/cloud139rc.toml`

`login` 时如果指定了 `--config <PATH>`，则将配置文件写入该路径。

### 配置文件结构

```toml
authorization = "..."
account = "13800138000"
storage_type = "personal_new"
custom_upload_part_size = 0
use_large_thumbnail = false
personal_cloud_host = "https://personal-kd-njs.yun.139.com/hcy"
refresh_token = "..."
token_expire_time = 1775454807088
```

### 字段说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `authorization` | String | 授权Token，用于API请求认证 |
| `account` | String | 登录账号（手机号） |
| `storage_type` | String | 必填，个人云使用 `personal_new` |
| `custom_upload_part_size` | Number | 自定义分片大小（字节），0表示使用默认值 |
| `use_large_thumbnail` | Boolean | 是否使用大缩略图 |
| `personal_cloud_host` | String | 个人云API服务器地址 |
| `refresh_token` | String | 刷新Token，用于自动续期 |
| `token_expire_time` | Number | Token过期时间（毫秒时间戳） |

## 项目结构

```
cloud139/
├── Cargo.toml           # 项目配置
├── LICENSE              # MIT 许可证
├── cloud139rc.toml      # 本地配置文件（可选，存在时优先生效）
└── src/
    ├── main.rs          # 入口文件
    ├── lib.rs           # 库入口
    ├── client/          # API客户端
    │   ├── api.rs       # API接口
    │   ├── auth.rs      # 认证模块
    │   └── mod.rs
    ├── commands/        # 命令实现
    │   ├── login.rs
    │   ├── list.rs
    │   ├── upload.rs
    │   ├── download.rs
    │   ├── delete.rs
    │   ├── mkdir.rs
    │   ├── mv.rs
    │   ├── cp.rs
    │   ├── rename.rs
    │   └── mod.rs
    ├── config/          # 配置管理
    │   └── mod.rs
    ├── models/          # 数据模型
    │   ├── types.rs
    │   └── mod.rs
    └── utils/           # 工具函数
        ├── crypto.rs    # 加密相关
        ├── logger.rs    # 日志
        ├── width.rs     # 终端宽度
        └── mod.rs
```

## 技术栈

- [Rust](https://www.rust-lang.org/) - 编程语言
- [clap](https://docs.rs/clap/) - CLI 参数解析
- [reqwest](https://docs.rs/reqwest/) - HTTP 客户端
- [tokio](https://tokio.rs/) - 异步运行时
- [serde](https://serde.rs/) - 序列化/反序列化
- [aes-gcm](https://docs.rs/aes-gcm/) - 加密
- [chrono](https://chrono.rs/) - 日期时间处理
- [directories](https://docs.rs/directories/) - 目录路径处理

## 许可证

MIT License - see [LICENSE](LICENSE) 文件
