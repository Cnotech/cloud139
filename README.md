# cloud139

139云盘（移动云盘）命令行客户端，支持文件上传、下载、管理等操作。

> 注意：当前仅实现了个人云功能，不支持家庭云/和家亲/群组。

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

个人盘文件操作统一使用 PC 客户端标识；已有配置和从浏览器获取的 Token 继续沿用，登录命令与自动续期流程不变，无需安装官方 PC 客户端。家庭盘和群组盘仍使用各自原有协议。

> 如果粘贴 Token 后提示 `Invalid token`，可尝试去除 `Basic ` 前缀，或在 Token 前后增加单引号（`'`）
>
> 更多详情可参考：[AList 文档](https://alistgo.com/zh/guide/drivers/139.html#%E6%96%B0%E4%B8%AA%E4%BA%BA%E4%BA%91)

### 基本使用

登录群组盘时，使用 `--cloud-id` 和 `--root-folder-id` 提供已核实的群组 ID 与根目录路径。对同一账号、同一群组重新登录时，可沿用目标配置中的根目录路径。登录校验通过后才会保存新配置。

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

目录上传复用 sync 的本地扫描及底层上传，不使用 sync 的同大小跳过规则；已有同名文件默认报错。个人盘 `--force` 沿用现有单文件上传策略。目录与文件类型冲突始终报错。

`ls -R` 忽略页码选择，读取每个目录的全部分页；JSON 输出为目录对象数组（包括空目录），每个对象含 `path`、`total` 和 `items`。普通 `ls` 保持单目录分页输出格式。游标、重复分页或目录 ID 循环均报错。

目录创建不自动改名。复制、移动、重命名的目录重名冲突即使加 `--force` 也会拒绝；文件同名仅在个人盘沿用 `--force` 行为。所有后端禁止根目录变更，路径先规范化，复制/移动禁止自身与子目录目标。非法跨平台名称、服务端路径穿越名称和本地符号链接会被拒绝。

| 操作 | 个人盘 | 家庭盘 | 群组盘 |
|------|--------|--------|--------|
| 目录上传、下载、mkdir -p、ls -R、rm -r | 支持 | 支持 | 支持，需已有可靠的 root_folder_id |
| cp 文件/目录 | 支持多源 | 协议未核实，明确拒绝 | 协议未核实，明确拒绝 |
| mv 文件/目录 | 支持多源 | 协议未核实，明确拒绝 | 支持单源 |
| rename 文件 | 支持 | 支持 | 支持 |
| rename 目录 | 支持 | 网页协议不支持，明确拒绝 | 支持 |
| --permanent | 拒绝 | 拒绝 | 拒绝 |

家庭/群组上传不支持强制覆盖同名文件。群组盘若缺少已解析的 `root_folder_id` 会报错，不猜测根路径。家庭/群组的异步任务成功响应只表示已接受，需再次 `ls` 核对结果。递归任务失败会返回非零退出码；此前条目可能已经完成，且不会自动回滚。详见 [目录操作与测试说明](docs/directory-operations.md)。

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

个人盘统一使用 PC 客户端标识，不再固定按 5 GiB 拦截文件，单文件可上传大小由服务器按账号权益决定。目录上传遇到权益不足或其他文件失败时会记录原因与服务端错误码，继续处理其余文件，最后汇总成功与失败数量；有失败项时退出码为 1。目录创建等准备错误仍停止执行。

```bash
cloud139 upload <本地路径> [远程目录] [-f]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 本地路径 | - | 是 | 本地文件或目录；末尾 / 表示仅上传目录内容 |
| 远程目录 | - | 否 | 云盘目标目录，默认：/ |
| --force | -f | 否 | 强制继续，如果云端存在同名文件则自动重命名 |

**示例：**

```bash
# 上传文件到根目录
cloud139 upload ./file.txt /

# 上传文件到指定目录
cloud139 upload ./folder/photo.jpg /backup/

# 强制上传，同名文件自动重命名
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

删除云盘文件或目录。

```bash
cloud139 rm [-r] --yes <路径>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 路径 | - | 是 | 要删除的文件或目录路径 |
| --yes | -y | 是 | 确认删除 |
| --recursive | -r | 目录必需 | 确认递归删除目录 |
| --permanent | -p | 否 | 当前明确拒绝：没有可靠的永久删除协议 |

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
| --force | -f | 否 | 仅保留解析兼容；不会自动改名或覆盖同名条目 |

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
| --force | -f | 否 | 强制继续，如果云端存在同名文件则自动重命名 |

**示例：**

```bash
# 移动单个文件
cloud139 mv /old.txt /existing-directory/

# 移动多个文件到目录
cloud139 mv /file1.txt /file2.txt /folder/

# 强制移动，同名文件自动重命名
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
| --force | -f | 否 | 强制继续，如果云端存在同名文件则自动重命名 |

**示例：**

```bash
# 复制单个文件到指定目录
cloud139 cp /file.txt /backup/

# 复制多个文件到目录
cloud139 cp /file1.txt /file2.txt /backup/

# 强制复制（云端自动重命名）
cloud139 cp /file.txt /backup/ -f
```

### rename

重命名文件或目录。

```bash
cloud139 rename <源路径> <新名称>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 源文件或目录路径 |
| 新名称 | - | 是 | 新名称 |

**示例：**

```bash
# 重命名文件
cloud139 rename /oldname.txt newname.txt

# 重命名目录
cloud139 rename /folder/old newname
```

### sync

同步本地目录和云端目录。

个人盘上传采用 PC 客户端标识，单文件大小由服务器按账号权益校验。同步先上传临时副本，上传完成并确认新文件后才替换云端同名条目；服务器拒绝、传输失败或完成确认失败时，旧条目不会先被删除。同名云端目录的后代也会保留到替换成功，`--delete` 不会提前删除这些内容。更新期间需留出同时保存新旧两份内容的空间；失败时可能保留临时副本或旧文件备份，错误会注明名称。文件失败后其他任务继续，最终有失败项返回退出码 1。`-n` 仅展示计划、正常返回 0，不创建上传任务，也不预判账号的实际上传上限。

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
| --dry-run | -n | 否 | 演习模式，只输出操作计划，不执行实际同步 |
| --delete | - | 否 | 删除目标中源没有的文件或空目录 |
| --exclude | - | 否 | 排除匹配的路径（glob 模式），可多次指定 |
| --jobs | -j | 否 | 并发传输数量上限，默认：4 |

**示例：**

```bash
# 本地到云端同步（递归、删除目标多余文件、8并发）
cloud139 sync ./backup cloud:/backup -r --delete -j 8

# 云端到本地同步（先演习预览）
cloud139 sync cloud:/photos ./photos -r -n

# 排除特定文件或目录
cloud139 sync . cloud:/project -r -n --exclude .git/** --exclude target/**
```

## 配置文件

登录成功后，配置信息默认保存到 `~/.config/cloud139/cloud139rc.toml`。

读取时按以下优先级查找：

1. 通过全局参数 `--config <PATH>` 指定的路径
2. 当前工作目录下的 `cloud139rc.toml`
3. 全局路径 `~/.config/cloud139/cloud139rc.toml`

`login` 时如果指定了 `--config <PATH>`，则将配置文件写入该路径；否则写入全局路径（若当前工作目录已存在 `cloud139rc.toml`，则沿用该文件）。

### 配置文件结构

```toml
authorization = "..."
account = "13800138000"
storage_type = "personal_new"
cloud_id = null
custom_upload_part_size = 0
report_real_size = true
use_large_thumbnail = false
personal_cloud_host = "https://personal-kd-njs.yun.139.com/hcy"
refresh_token = "..."
token_expire_time = 1775454807088
root_folder_id = null
user_domain_id = null
```

### 字段说明

| 字段 | 类型 | 说明 |
|------|------|------|
| `authorization` | String | 授权Token，用于API请求认证 |
| `account` | String | 登录账号（手机号） |
| `storage_type` | String | 存储类型：personal_new（个人云）、family（家庭云）、group（和家亲/群组） |
| `cloud_id` | String/null | 云盘ID，家庭云/和家亲时需要设置 |
| `custom_upload_part_size` | Number | 自定义分片大小（字节），0表示使用默认值 |
| `report_real_size` | Boolean | 上传时是否报告真实文件大小 |
| `use_large_thumbnail` | Boolean | 是否使用大缩略图 |
| `personal_cloud_host` | String | 个人云API服务器地址 |
| `refresh_token` | String | 刷新Token，用于自动续期 |
| `token_expire_time` | Number | Token过期时间（毫秒时间戳） |
| `root_folder_id` | String/null | 根目录ID |
| `user_domain_id` | String/null | 用户域ID |

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
