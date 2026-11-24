# cloud139

139云盘（移动云盘）命令行客户端，支持文件上传、下载、管理等操作。

> 当前仅支持个人云。家庭云、和家亲、群组盘及其他存储类型会统一返回“未实现：当前仅支持个人云”。

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
cargo build --locked --release

# 可执行文件位于 target/release/cloud139，Windows 下为 cloud139.exe
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

> Token 可以带 `Basic ` 前缀。传入带空格的完整值时，用引号包住整个值。
>
> 更多详情可参考：[AList 文档](https://alistgo.com/zh/guide/drivers/139.html#%E6%96%B0%E4%B8%AA%E4%BA%BA%E4%BA%91)

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
```

目录也可以直接上传、下载、复制、移动和删除。上传路径末尾的 `/` 决定是否包含源目录名，下载时则根据目标路径决定写入位置。

```bash
# 上传到 /backup/photos，自动创建缺失的目录
cloud139 upload ./photos /backup

# 将 photos 的内容直接上传到 /backup
cloud139 upload ./photos/ /backup

# 将目录内容下载到尚不存在的 ./photos
cloud139 download -r /backup/photos ./photos

# 下载到 ./downloads/photos，保留源目录名
cloud139 download -r /backup/photos ./downloads/

# 递归列出目录并遍历全部分页
cloud139 ls -R /backup

# 将各目录的列表保存为 JSON
cloud139 ls -R /backup -o tree.json

# 创建目录及缺失的父目录
cloud139 mkdir -p /a/b/empty

# 复制目录到已有目录
cloud139 cp /backup/photos /existing-destination/

# 移动目录到已有目录
cloud139 mv /backup/photos /existing-destination/

# 将目录及其内容移到回收站
cloud139 rm -r --yes /a

# 将本地目录递归同步到云端
cloud139 sync ./local/path cloud:/remote/path -r

# 将云端目录递归同步到本地
cloud139 sync cloud:/remote/path ./local/path -r
```

目录上传会逐个处理文件，包括与云端同名且同大小的文件。已有同名文件默认报错。`--force` 允许云端为新文件分配名称。

`ls -R` 忽略页码和每页数量设置，读取每个目录的全部分页。使用 `-o` 时，JSON 文件包含目录对象数组（包括空目录），每个对象含 `path`、`total` 和 `items`。循环游标或重复目录 ID 会报错。

递归任务失败时，此前成功的条目会保留。路径检查、同名冲突和部分失败的处理见 [目录操作与验证](docs/directory-operations.md)。

## 全局选项

| 参数 | 简写 | 默认值 | 说明 |
|------|------|--------|------|
| --verbose | -v | info | 在子命令前使用 `-v debug` 开启调试日志 |
| --config | -c | - | 仅使用指定的配置文件。`login` 将配置写入该路径 |
| --log-file | - | - | 新建文件并写入调试日志，可放在子命令前后 |

`--log-file` 需要指定一个尚不存在的文件，父目录须已存在。日志路径应与配置、命令输出及本地传输目录分开。文件记录调试日志，终端仍由 `-v` 控制。`sync --json` 继续在 stdout 只输出一行 JSON。

```bash
# 记录同步扫描与执行阶段的 API 请求
cloud139 sync ./source cloud:/backup --verify --json --log-file /tmp/cloud139-verify.log
```

日志字段与计数方式见 [日志与输出](docs/logging.md)。

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
| 路径 | - | 否 | 远程目录路径，默认 `/` |
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

上传文件或递归上传本地目录到云盘。目录上传保留空目录，并创建缺失的云端目标目录。单文件上传要求目标目录已存在。

目录上传复用已解析的父目录 ID 和同名检查结果，并记录云端为本次上传分配的文件名。创建目录后会确认实际名称。文件写入失败后，下次处理同一父目录时会重新读取条目。

单个文件的上传大小上限因账号而异。超过上限时，报错会显示文件大小和云盘返回的错误码。

分片上传在持续发送数据时可以超过 30 秒。连续 30 秒未发送数据或收到分片响应时，报错会显示超时的分片编号。

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
| 本地路径 | - | 否 | 默认保存到当前目录下的同名文件或目录。下载云端根目录默认使用 `./download` |

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

移动文件或目录。一次移动多个条目时，共用预检期间已读取的目录分页。

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

复制文件或目录。一次复制多个条目时，共用预检期间已读取的目录分页。

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

在本地和云端之间单向同步目录，也可将本地单个文件同步到云端目录。云端路径使用 `cloud:` 前缀，例如 `cloud:/backup`，云端同步根目录须已存在。

```bash
cloud139 sync <源路径> <目标路径> [选项]
```

**参数说明：**

| 参数 | 简写 | 必填 | 说明 |
|------|------|------|------|
| 源路径 | - | 是 | 本地路径或云端目录 |
| 目标路径 | - | 是 | 本地路径或云端路径 |
| --recursive | -r | 否 | 递归同步子目录，空目录也会同步 |
| --dry-run | -n | 否 | 预览同步计划。扫描前仍可能按需续期凭据 |
| --delete | - | 否 | 源须为目录。同步成功后清理目标中多余的文件和空目录。与 `--verify` 同用时，要求同步范围内至少有一个源文件且只清理文件 |
| --exclude | - | 否 | 排除匹配的路径（glob 模式），可多次指定 |
| --jobs | -j | 否 | 同时传输的文件数，范围 1 到 32。默认 4，校验模式固定为 1 |
| --verify | - | 否 | 仅支持本地到云端，使用 SHA-256 校验内容并修复不一致的文件 |
| --json | - | 否 | 向标准输出写一行 JSON 报告，日志写到标准错误 |

**示例：**

```bash
# 递归同步到云端
cloud139 sync ./backup cloud:/backup -r

# 预览云端到本地的同步计划
cloud139 sync cloud:/photos ./photos -r -n

# 校验备份内容，更新成功后清理多余文件并保存报告
cloud139 sync ./backup cloud:/backup -r --verify --delete --json > sync-report.json
```

替换与清理规则、退出码和 JSON 报告字段见 [同步与清理](docs/directory-operations.md#同步与清理)。

## 配置文件

登录成功后，配置信息默认保存到当前工作目录的 `cloud139rc.toml`。

指定 `--config <PATH>` 时，CLI 只读取该路径，`login` 也写到该路径。省略该选项时，CLI 先查找当前目录下的 `cloud139rc.toml`，文件不存在时再查找 `~/.config/cloud139/cloud139rc.toml`。已找到的配置无效时，命令会报错。

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
| `personal_cloud_host` | String | 个人云 API 服务器地址，登录成功时写入 |
| `refresh_token` | String | 刷新Token，用于自动续期 |
| `token_expire_time` | Number | Token过期时间（毫秒时间戳） |

## 以 crate 形式使用

在 Rust 项目的 `Cargo.toml` 中加入 `cloud139` 库依赖，`path` 指向本地检出的 `packages/lib` 目录。异步接口在 Tokio 运行时中调用。

```toml
[dependencies]
cloud139 = { path = "../cloud139/packages/lib" }
tokio = { version = "1", features = ["macros", "rt"] }
```

以下示例读取 `cloud139 login` 生成的配置，并列出云盘目录的第一页。

```rust,no_run
use cloud139::config::Config;
use cloud139::services::list::{ListOptions, list};
use std::path::Path;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load_from_path(Path::new("cloud139rc.toml"))?;
    let options = ListOptions {
        path: "/",
        page: 1,
        page_size: 100,
    };
    for item in list(&config, options).await?.items {
        println!("{}", item.name);
    }
    Ok(())
}
```

`services::login::login` 也可直接用 Token 获取配置。登录和续期接口返回 `Config`，由调用方通过 `Config::save_to` 保存到指定路径。库通过 `log` 发出日志，多个进程读改写同一份配置文件时由调用方协调。
