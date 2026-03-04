# 移动云盘 CLI (139 Yun) 项目规划

## 1. 项目概述

基于 OpenList 的 139 网盘驱动（drivers/139），使用 Rust 实现一个命令行工具，提供登录、文件上传、下载、删除、列表等核心功能。

## 2. 项目结构

```
mobile-cloud-cli/
├── Cargo.toml                 # 项目配置和依赖
├── PROJECT.md                  # 本规划文档
├── src/
│   ├── main.rs                # CLI 入口 (clap)
│   ├── lib.rs                 # 库导出
│   ├── client/
│   │   ├── mod.rs             # 客户端模块
│   │   ├── auth.rs            # 登录/刷新令牌
│   │   └── api.rs             # HTTP 请求封装
│   ├── commands/
│   │   ├── mod.rs             # 命令模块
│   │   ├── login.rs           # login 子命令
│   │   ├── list.rs            # ls/list 子命令
│   │   ├── upload.rs          # upload 子命令
│   │   ├── download.rs        # download 子命令
│   │   └── delete.rs          # rm/delete 子命令
│   ├── models/
│   │   ├── mod.rs
│   │   └── types.rs           # 响应类型定义
│   └── config/
│       ├── mod.rs
│       └── store.rs           # 配置持久化
```

## 3. 功能规划

| 功能 | 说明 | 优先级 |
|------|------|--------|
| **登录** | 支持用户名密码+邮箱cookies登录，支持4种类型（personal_new, personal, family, group） | P0 |
| **令牌刷新** | 令牌有效期小于15天时自动刷新 | P0 |
| **列出文件** | 分页获取文件列表，区分文件夹和文件 | P0 |
| **文件上传** | 分片上传+秒传支持，显示进度 | P0 |
| **文件下载** | 获取下载链接，流式下载到本地 | P0 |
| **文件删除** | 移动到回收站 | P0 |

## 4. 认证流程（来自 Go 代码）

### 4.1 三步登录

```
Step 1: POST https://mail.10086.cn/Login/Login.ashx
        用户名密码登录 → 获取 sid 和 cguid

Step 2: GET  https://smsrebuild1.mail.10086.cn/setting/s?func=umc:getArtifact&sid=xxx
        换 artifact → 获取 dycpwd

Step 3: POST https://user-njs.yun.139.com/user/thirdlogin
        第三方登录（加密请求）→ 获取 authToken
```

### 4.2 授权令牌格式

```
Base64(pc:{account}:{authToken})
```

### 4.3 支持的存储类型

| 类型 | 常量 | 说明 |
|------|------|------|
| 个人云(新) | `personal_new` | 推荐使用新 API |
| 个人云 | `personal` | 旧版 API |
| 家庭云 | `family` | 家庭共享存储 |
| 群组云 | `group` | 企业/团队存储 |

## 5. 核心 API 端点

| 操作 | 端点 |
|------|------|
| 查询路由策略 | `https://user-njs.yun.139.com/user/route/qryRoutePolicy` |
| 个人云文件列表 | `{PersonalCloudHost}/file/list` |
| 个人云上传 | `{PersonalCloudHost}/file/create` |
| 个人云下载链接 | `{PersonalCloudHost}/file/getDownloadUrl` |
| 家庭云文件列表 | `https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList` |
| 群组云文件列表 | `https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList` |
| 令牌刷新 | `https://aas.caiyun.feixin.10086.cn/tellin/authTokenRefresh.do` |

## 6. Rust 依赖

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }  # CLI 参数解析
reqwest = { version = "0.11", features = ["json", "multipart", "stream"] }  # HTTP 客户端
serde = { version = "1", features = ["derive"] }  # 序列化
serde_json = "1"                                   # JSON 处理
base64 = "0.21"                                    # Base64 编解码
aes = "0.8"                                        # AES 加密
sha1 = "0.10"                                      # SHA1 哈希
md5 = "0.7"                                        # MD5 哈希
tokio = { version = "1", features = ["full"] }    # 异步运行时
directories = "5"                                  # 配置目录路径
chrono = { version = "0.4", features = ["serde"] } # 时间处理
log = "0.4"                                        # 日志
env_logger = "0.10"                                # 日志实现
thiserror = "1"                                    # 错误处理
```

## 7. CLI 命令设计

```bash
# 登录
139yun login -u <手机号> -p <密码> -c <邮箱cookies> [-t personal_new|family|group|personal]

# 列出文件
139yun ls [路径]
139yun ls /

# 上传文件
139yun upload <本地路径> [远程目录]
139yun upload ./test.txt /

# 下载文件
139yun download <远程路径> [本地路径]
139yun download /test.txt ./

# 删除文件
139yun rm <远程路径>
139yun rm /test.txt

# 查看帮助
139yun --help
139yun login --help
```

## 8. 配置存储

- 配置文件路径: `{config_dir}/mobile-cloud-cli/config.json`
- 存储内容:
  - 授权令牌 (authorization)
  - 用户名 (username)
  - 存储类型 (type)
  - 云 ID (cloud_id)
  - 用户域 ID (user_domain_id)

## 9. 实现步骤

1. **Phase 1: 基础框架**
   - 创建项目结构
   - 配置 Cargo.toml
   - 实现 CLI 框架 (clap)

2. **Phase 2: 认证模块**
   - 实现加密工具函数 (AES, SHA1, Base64)
   - 实现三步登录流程
   - 实现令牌刷新
   - 实现配置持久化

3. **Phase 3: 核心功能**
   - 实现文件列表
   - 实现文件上传（含分片）
   - 实现文件下载
   - 实现文件删除

4. **Phase 4: 完善**
   - 添加进度显示
   - 添加日志
   - 错误处理优化

## 10. 关键数据类型（来自 Go 翻译）

```rust
// 响应类型
struct BaseResp {
    success: bool,
    code: String,
    message: String,
}

struct PersonalListResp {
    items: Vec<PersonalFileItem>,
    next_page_cursor: String,
}

struct PersonalFileItem {
    file_id: String,
    name: String,
    size: i64,
    file_type: String,  // "file" or "folder"
    created_at: String,
    updated_at: String,
}

struct UploadResp {
    // 上传响应
}

struct DownloadUrlResp {
    url: String,
    cdn_url: String,
}
```
