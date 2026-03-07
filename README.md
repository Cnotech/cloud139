# cloud139

139云盘（移动云盘）命令行客户端，支持文件上传、下载、管理等操作。

## 功能特性

- 账号登录与Token管理
- 文件列表查看
- 文件上传/下载
- 文件删除
- 目录创建
- 文件移动/重命名
- 文件复制

## 快速开始

### 安装

```bash
# 克隆项目
git clone https://github.com/your-repo/mobile-cloud-cli.git
cd mobile-cloud-cli

# 编译
cargo build --release

# 可执行文件位于 target/release/cloud139.exe
```

### 登录

```bash
cloud139 login -t <YOUR_TOKEN>
```

Token 需要在使用浏览器登陆移动云盘后打开开发者工具获取，可参考：[AList 文档](https://alistgo.com/zh/guide/drivers/139.html#%E6%96%B0%E4%B8%AA%E4%BA%BA%E4%BA%91)

### 基本使用

```bash
# 列出文件
cloud139 ls

# 上传文件
cloud139 upload ./local.txt /remote/path

# 下载文件
cloud139 download /remote/file.txt

# 永久删除
cloud139 rm /remote/file.txt --force

# 创建目录
cloud139 mkdir /remote/newfolder

# 移动文件
cloud139 mv /old/path /new/path

# 复制文件
cloud139 cp /source /destination

# 重命名
cloud139 rename /path/oldname newname
```

## 命令参考

### login

登录139云盘账号。

```bash
cloud139 login --token <TOKEN> [--storage-type <TYPE>] [--cloud-id <ID>]
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

| 参数 | 必填 | 说明 |
|------|------|------|
| 本地路径 | 是 | 要上传的本地文件路径 |
| 远程目录 | 否 | 云盘目标目录，默认 "/" |

**示例：**

```bash
cloud139 upload ./file.txt /
cloud139 upload ./folder/photo.jpg /backup/
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
cloud139 download /file.txt ./
cloud139 download /folder/photo.jpg ./downloads/
```

### rm

删除云盘文件或目录。

```bash
cloud139 rm <路径> [--force] [--permanent]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 路径 | - | 是 | 要删除的文件或目录路径 |
| --force | -f | 是 | 确认删除（必填） |
| --permanent | -p | 否 | 永久删除（不移动到回收站） |

**示例：**

```bash
cloud139 rm /file.txt -f
cloud139 rm /folder -f -p
```

### mkdir

在云盘创建目录。

```bash
cloud139 mkdir <路径>
```

**参数说明：**

| 参数 | 必填 | 说明 |
|------|------|------|
| 路径 | 是 | 新目录路径，格式: /父目录/新目录名 |

**示例：**

```bash
cloud139 mkdir /newfolder
cloud139 mkdir /parent/child
```

### mv

移动或重命名文件/目录。

```bash
cloud139 mv <源路径...> <目标路径>
```

**参数说明：**

| 参数 | 必填 | 说明 |
|------|------|------|
| 源路径 | 是 | 源文件路径（支持多个，用空格分隔） |
| 目标路径 | 是 | 目标路径 |

**示例：**

```bash
cloud139 mv /old.txt /new.txt
cloud139 mv /file1.txt /file2.txt /folder/
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

**示例：**

```bash
cloud139 cp /file.txt /backup/
cloud139 cp /file.txt /backup/ -m
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

## 配置文件

登录成功后，配置信息会保存在 `config/config.json` 文件中。

### 配置文件结构

```json
{
  "authorization": "...",
  "account": "13800138000",
  "storage_type": "personal_new",
  "cloud_id": null,
  "custom_upload_part_size": 0,
  "report_real_size": true,
  "use_large_thumbnail": false,
  "personal_cloud_host": "https://personal-kd-njs.yun.139.com/hcy",
  "refresh_token": "...",
  "token_expire_time": 1775454807088,
  "root_folder_id": null,
  "user_domain_id": null
}
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
mobile-cloud-cli/
├── Cargo.toml           # 项目配置
├── LICENSE              # MIT 许可证
├── config/
│   └── config.json      # 配置文件（运行时生成）
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

## 许可证

MIT License - see [LICENSE](LICENSE) 文件
