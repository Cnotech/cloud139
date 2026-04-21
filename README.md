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

> 如果粘贴 Token 后提示 `Invalid token`，可尝试去除 `Basic ` 前缀，或在 Token 前后增加单引号（`'`）
> 
> 更多详情可参考：[AList 文档](https://alistgo.com/zh/guide/drivers/139.html#%E6%96%B0%E4%B8%AA%E4%BA%BA%E4%BA%91)
### 基本使用

```bash
# 列出文件
# ls [远程目录]
cloud139 ls /remote/path

# 上传文件
# upload [本地文件路径] [远程目录]
cloud139 upload ./local.txt /remote/path

# 下载文件
# download [远程文件路径] (本地保存路径)
cloud139 download /remote/file.txt
cloud139 download /remote/file.txt ./local/myfile.txt

# 删除文件（移动到回收站）
# rm [远程文件路径] --yes
cloud139 rm /remote/file.txt --yes

# 创建目录
# mkdir [远程新目录]
cloud139 mkdir /remote/newfolder

# 移动文件
# mv [远程源文件路径] [远程目标目录]
cloud139 mv /remote/source/file.txt /remote/destination

# 复制文件
# cp [远程源文件路径] [远程目标目录]
cloud139 cp /remote/source/file.txt /remote/destination

# 重命名
# rename [远程文件路径] [新文件名]
cloud139 rename /remote/oldname.txt newname.txt

# 同步
# sync [本地路径] [cloud:远程路径]
# sync [cloud:远程路径] [本地路径]
cloud139 sync ./local/path cloud:/remote/path
cloud139 sync cloud:/remote/path ./local/path -r
```

## 全局选项

| 参数 | 简写 | 默认值 | 说明 |
|------|------|--------|------|
| --verbose | -v | info | 日志级别 (trace, debug, info, warn, error) |

## 命令参考

### login

登录139云盘账号。

```bash
cloud139 login -t <TOKEN> [--storage-type <TYPE>] [--cloud-id <ID>]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| --token | -t | 是 | 授权Token |
| --storage-type | -s | 否 | 存储类型 (personal_new/family/group)，默认 personal_new |
| --cloud-id | -c | 否 | 云盘ID，家庭云/和家亲时需要 |

**示例：**

```bash
cloud139 login -t your_authorization_token
cloud139 login -t token -s family -c cloud123
```

### ls

列出云盘文件。

```bash
cloud139 ls [路径] [--page <N>] [--page-size <N>] [--output <FILE>]
```

**参数说明：**

| 参数 | 简写 | 说明 |
|------|------|------|
| 路径 | - | 文件路径，默认根目录 "/" |
| --page | -p | 页码，默认1 |
| --page-size | -s | 每页数量，默认100 |
| --output | -o | 将结果输出为JSON到指定文件 |

**示例：**

```bash
cloud139 ls
cloud139 ls /myfolder
cloud139 ls / -p 2 -s 50
cloud139 ls / -o result.json
```

### upload

上传文件到云盘。

```bash
cloud139 upload <本地路径> [远程目录]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 本地路径 | - | 是 | 要上传的本地文件路径 |
| 远程目录 | - | 否 | 云盘目标目录，默认 "/" |
| --force | -f | 否 | 强制继续，如果云端存在同名文件则自动重命名 |

**示例：**

```bash
cloud139 upload ./file.txt /
cloud139 upload ./folder/photo.jpg /backup/
cloud139 upload ./file.txt / --force
```

### download

从云盘下载文件。

```bash
cloud139 download <远程路径> [本地路径]
```

**参数说明：**

| 参数 | 必填 | 说明 |
|------|------|------|
| 远程路径 | 是 | 云盘文件路径 |
| 本地路径 | 否 | 本地保存路径，默认保存到当前目录的同名文件 |

**示例：**

```bash
cloud139 download /file.txt
cloud139 download /folder/photo.jpg ./downloads/
```

### rm

删除云盘文件或目录。

```bash
cloud139 rm <路径> [--yes]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 路径 | - | 是 | 要删除的文件或目录路径 |
| --yes | -y | 是 | 确认删除（必填） |

**示例：**

```bash
cloud139 rm /file.txt -f
cloud139 rm /folder -f
```

### mkdir

在云盘创建目录。

```bash
cloud139 mkdir <路径>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 路径 | - | 是 | 新目录路径，格式: /父目录/新目录名 |
| --force | -f | 否 | 强制继续，如果云端存在同名目录则自动重命名 |

**示例：**

```bash
cloud139 mkdir /newfolder
cloud139 mkdir /parent/child
cloud139 mkdir /newfolder --force
```

### mv

移动文件/目录。

```bash
cloud139 mv <源路径...> <目标路径>
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 源文件路径（支持多个，用空格分隔） |
| 目标路径 | - | 是 | 目标路径 |
| --force | -f | 否 | 强制继续，如果云端存在同名文件则自动重命名 |

**示例：**

```bash
cloud139 mv /old.txt /new.txt
cloud139 mv /file1.txt /file2.txt /folder/
cloud139 mv /file.txt /folder/ --force
```

### cp

复制文件或目录。

```bash
cloud139 cp <源路径> <目标目录> [--merge]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 源文件路径 |
| 目标目录 | - | 是 | 目标目录 |
| --merge | -m | 否 | 合并复制（覆盖目标中的同名文件） |
| --force | -f | 否 | 强制继续，如果云端存在同名文件则自动重命名 |

**示例：**

```bash
cloud139 cp /file.txt /backup/
cloud139 cp /file.txt /backup/ -m
cloud139 cp /file.txt /backup/ --force
```

### rename

重命名文件或目录。

```bash
cloud139 rename <源路径> <新名称>
```

**参数说明：**

| 参数 | 必填 | 说明 |
|------|------|------|
| 源路径 | 是 | 源文件路径 |
| 新名称 | 是 | 新名称 |

**示例：**

```bash
cloud139 rename /oldname.txt newname.txt
cloud139 rename /folder/old newname
```

### sync

同步本地目录和云端目录。

```bash
cloud139 sync <SRC> <DEST> [OPTIONS]
```

**同步方向：**

- **本地 → 云端**：`cloud139 sync <本地路径> cloud:/<远程路径>`
- **云端 → 本地**：`cloud139 sync cloud:/<远程路径> <本地路径>`

> 云端路径需使用 `cloud:` 前缀标识。

**文件比对逻辑：**

默认通过文件大小和修改时间判断文件是否需要同步。启用 `--checksum` 时，优先使用 SHA-256 校验和进行比对；如果云端文件缺少校验和信息，则自动回退到大小和时间比对。

**参数说明：**

| 参数 | 简写 | 说明 |
|------|------|------|
| 源路径 | - | 本地路径或云端路径（`cloud:/path`） |
| 目标路径 | - | 本地路径或云端路径 |
| --recursive | -r | 递归同步子目录，空目录也会同步 |
| --dry-run | -n | 演习模式，只输出操作计划，不执行实际同步 |
| --delete | - | 删除目标中源没有的文件或空目录 |
| --checksum | - | 优先用校验和（SHA-256）替代大小和修改时间做比对 |
| --exclude | - | 排除匹配的路径（glob 模式），可多次指定 |
| --jobs | -j | 并发传输数量上限，默认 4 |

**示例：**

```bash
# 本地到云端同步（递归、删除目标多余文件、8并发）
cloud139 sync ./backup cloud:/backup -r --delete -j 8

# 云端到本地同步（先演习预览）
cloud139 sync cloud:/photos ./photos -r -n

# 排除特定文件或目录
cloud139 sync . cloud:/project -r -n --exclude .git/** --exclude target/**

# 使用校验和进行精确比对
cloud139 sync ./docs cloud:/docs -r --checksum
```

## 配置文件

登录成功后，配置信息会保存在 `cloud139.toml` 文件中。

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
├── cloud139.toml        # 配置文件
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
- [env_logger](https://docs.rs/env_logger/) - 日志输出

## 许可证

MIT License - see [LICENSE](LICENSE) 文件
