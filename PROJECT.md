# 移动云盘 CLI (139 Yun) 项目规划

> 本项目是对 OpenList 的 139 网盘驱动（drivers/139）的 Rust 重构，实现 CLI 工具。
> **重构过程中可参考 OpenList-main 中的 Go 实现。**

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
│   │   ├── delete.rs          # rm/delete 子命令
│   │   ├── mkdir.rs           # mkdir 子命令
│   │   ├── mv.rs              # mv/rename 子命令
│   │   └── cp.rs              # cp 子命令
│   ├── models/
│   │   ├── mod.rs
│   │   └── types.rs           # 响应类型定义
│   ├── config/
│   │   ├── mod.rs
│   │   └── store.rs           # 配置持久化
│   └── utils/
│       ├── mod.rs
│       ├── crypto.rs           # AES/SHA1/MD5 加密
│       └── http.rs             # HTTP 请求封装
```

## 3. 功能规划

| 功能 | 说明 | 优先级 |
|------|------|--------|
| **登录** | 支持用户名密码+邮箱cookies登录，支持3种类型（personal_new, family, group） | P0 |
| **令牌刷新** | 令牌有效期小于15天时自动刷新 | P0 |
| **列出文件** | 分页获取文件列表，区分文件夹和文件 | P0 |
| **文件上传** | 分片上传+秒传支持，显示进度 | P0 |
| **文件下载** | 获取下载链接，流式下载到本地 | P0 |
| **文件删除** | 移动到回收站 | P0 |
| **创建目录** | 在指定路径创建新文件夹 | P1 |
| **移动/重命名** | 移动文件或重命名 | P1 |
| **复制文件** | 复制文件到目标目录 | P1 |
| **获取存储信息** | 查询云盘容量使用情况 | P2 |

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
| 个人云 | `personal_new` | 推荐使用新 API |
| 家庭云 | `family` | 家庭共享存储 |
| 群组云 | `group` | 企业/团队存储 |

## 5. 核心 API 端点

### 5.1 认证相关

| 操作 | 端点 | 方法 |
|------|------|------|
| 用户名密码登录 | `https://mail.10086.cn/Login/Login.ashx` | POST |
| 获取Artifact | `https://smsrebuild1.mail.10086.cn/setting/s?func=umc:getArtifact&sid={sid}` | GET |
| 第三方登录 | `https://user-njs.yun.139.com/user/thirdlogin` | POST |
| 令牌刷新 | `https://aas.caiyun.feixin.10086.cn/tellin/authTokenRefresh.do` | POST |
| 查询路由策略 | `https://user-njs.yun.139.com/user/route/qryRoutePolicy` | POST |

### 5.2 个人云

| 操作 | 端点 | 说明 |
|------|------|------|
| 文件列表 | `{PersonalCloudHost}/file/list` | |
| 文件上传 | `{PersonalCloudHost}/file/create` | 支持分片 |
| 获取上传地址 | `{PersonalCloudHost}/file/getUploadUrl` | 分片上传用 |
| 完成上传 | `{PersonalCloudHost}/file/complete` | |
| 下载链接 | `{PersonalCloudHost}/file/getDownloadUrl` | |
| 重命名 | `{PersonalCloudHost}/file/update` | |
| 移动 | `{PersonalCloudHost}/file/batchMove` | |
| 复制 | `{PersonalCloudHost}/file/batchCopy` | |
| 删除 | `{PersonalCloudHost}/recyclebin/batchTrash` | 移至回收站 |
| 创建目录 | `{PersonalCloudHost}/file/create` | type=folder |
| 视频预览 | `{PersonalCloudHost}/videoPreview/getPreviewInfo` | |
| 存储信息 | `{PersonalCloudHost}/user/getDiskInfo` | |

### 5.3 家庭云

| 操作 | 端点 | 方法 |
|------|------|------|
| 文件列表 | `https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.2/queryContentList` | POST |
| 创建目录 | `https://yun.139.com/orchestration/familyCloud-rebuild/cloudCatalog/v1.0/createCloudDoc` | POST |
| 上传文件 | `https://yun.139.com/orchestration/familyCloud-rebuild/content/v1.0/getFileUploadURL` | POST |
| 批量任务 | `https://yun.139.com/orchestration/familyCloud-rebuild/batchOprTask/v1.0/createBatchOprTask` | POST |
| 重命名 | `https://yun.139.com/orchestration/familyCloud-rebuild/photoContent/v1.0/modifyContentInfo` | POST |
| 复制 | `/copyContentCatalog` (andAlbum) | POST |
| 存储信息 | `/getFamilyDiskInfo` | POST |

### 5.4 群组云

| 操作 | 端点 | 方法 |
|------|------|------|
| 文件列表 | `https://yun.139.com/orchestration/group-rebuild/content/v1.0/queryGroupContentList` | POST |
| 创建目录 | `https://yun.139.com/orchestration/group-rebuild/catalog/v1.0/createGroupCatalog` | POST |
| 批量任务 | `https://yun.139.com/orchestration/group-rebuild/task/v1.0/createBatchOprTask` | POST |

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
tokio-stream = "0.1"                              # 流处理 (用于进度)
futures-util = "0.3"                              # 异步工具
hex = "0.4"                                        # 十六进制编解码
regex = "1"                                        # 正则表达式
url = "2"                                          # URL 处理
```

[dev-dependencies]
tempfile = "3"                                      # 临时文件测试

## 7. CLI 命令设计

```bash
# 登录
139yun login -u <手机号> -p <密码> -c <邮箱cookies> [-t personal_new|family|group]

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

# 创建目录
139yun mkdir <目录名> [父目录]
139yun mkdir new_folder /

# 移动/重命名文件
139yun mv <源路径> <目标路径>
139yun mv /old.txt /new.txt

# 复制文件
139yun cp <源路径> <目标目录>
139yun cp /file.txt /backup/

# 查看存储信息
139yun info

# 查看帮助
139yun --help
139yun login --help
```

## 8. 配置存储

- 配置文件路径: `{config_dir}/mobile-cloud-cli/config.json`
- 存储内容:
  - 授权令牌 (authorization) - Base64编码
  - 用户名 (username)
  - 存储类型 (type)
  - 云 ID (cloud_id)
  - 用户域 ID (user_domain_id)
  - 自定义分片大小 (custom_upload_part_size)
  - 上报真实文件大小 (report_real_size)
  - 使用大缩略图 (use_large_thumbnail)

### 8.1 配置文件JSON结构
```json
{
  "authorization": "Base64(pc:手机号:authToken|时间戳|...|过期时间)",
  "username": "13800138000",
  "storage_type": "personal_new",
  "cloud_id": "",
  "user_domain_id": "",
  "custom_upload_part_size": 0,
  "report_real_size": true,
  "use_large_thumbnail": false
}
```

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
// ========== 基础类型 ==========

#[derive(Debug, Deserialize)]
pub struct BaseResp {
    pub success: bool,
    pub code: String,
    pub message: String,
}

// ========== 存储类型常量 ==========
// personal_new: 个人云
// family: 家庭云
// group: 群组云

// ========== 个人云 类型 ==========

#[derive(Debug, Deserialize)]
pub struct PersonalListResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: PersonalListData,
}

#[derive(Debug, Deserialize)]
pub struct PersonalListData {
    pub items: Vec<PersonalFileItem>,
    pub next_page_cursor: String,
}

#[derive(Debug, Deserialize)]
pub struct PersonalFileItem {
    pub file_id: String,
    pub name: String,
    pub size: i64,
    #[serde(rename = "type")]
    pub file_type: String,  // "file" or "folder"
    pub created_at: String,
    pub updated_at: String,
    pub thumbnail_urls: Option<Vec<PersonalThumbnail>>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalThumbnail {
    pub style: String,
    pub url: String,
}

// 个人云上传响应
#[derive(Debug, Deserialize)]
pub struct PersonalUploadResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: PersonalUploadData,
}

#[derive(Debug, Deserialize)]
pub struct PersonalUploadData {
    pub file_id: String,
    pub file_name: String,
    pub part_infos: Option<Vec<PersonalPartInfo>>,
    pub exist: bool,
    pub rapid_upload: bool,
    pub upload_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalPartInfo {
    pub part_number: i32,
    pub upload_url: String,
}

// 个人云下载链接响应
#[derive(Debug, Deserialize)]
pub struct DownloadUrlResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: DownloadUrlData,
}

#[derive(Debug, Deserialize)]
pub struct DownloadUrlData {
    pub url: String,
    pub cdn_url: Option<String>,
}

// 个人云存储信息
#[derive(Debug, Deserialize)]
pub struct PersonalDiskInfoResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: PersonalDiskInfoData,
}

#[derive(Debug, Deserialize)]
pub struct PersonalDiskInfoData {
    #[serde(rename = "freeDiskSize")]
    pub free_disk_size: String,
    #[serde(rename = "diskSize")]
    pub disk_size: String,
}

// ========== 家庭云 类型 ==========

#[derive(Debug, Deserialize)]
pub struct QueryContentListResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: QueryContentListData,
}

#[derive(Debug, Deserialize)]
pub struct QueryContentListData {
    pub result: ApiResult,
    pub path: String,
    #[serde(rename = "cloudContentList")]
    pub cloud_content_list: Vec<CloudContent>,
    #[serde(rename = "cloudCatalogList")]
    pub cloud_catalog_list: Vec<CloudCatalog>,
    pub total_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct CloudContent {
    #[serde(rename = "contentID")]
    pub content_id: String,
    #[serde(rename = "contentName")]
    pub content_name: String,
    #[serde(rename = "contentSize")]
    pub content_size: i64,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: String,
    #[serde(rename = "thumbnailURL")]
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloudCatalog {
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "catalogName")]
    pub catalog_name: String,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: String,
}

// 家庭云存储信息
#[derive(Debug, Deserialize)]
pub struct FamilyDiskInfoResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: FamilyDiskInfoData,
}

#[derive(Debug, Deserialize)]
pub struct FamilyDiskInfoData {
    #[serde(rename = "usedSize")]
    pub used_size: String,
    #[serde(rename = "diskSize")]
    pub disk_size: String,
}

// ========== 群组云 类型 ==========

#[derive(Debug, Deserialize)]
pub struct QueryGroupContentListResp {
    #[serde(flatten)]
    pub base: BaseResp,
    pub data: QueryGroupContentListData,
}

#[derive(Debug, Deserialize)]
pub struct QueryGroupContentListData {
    pub result: ApiResult,
    #[serde(rename = "getGroupContentResult")]
    pub get_group_content_result: GetGroupContentResult,
}

#[derive(Debug, Deserialize)]
pub struct GetGroupContentResult {
    #[serde(rename = "parentCatalogID")]
    pub parent_catalog_id: String,
    pub catalog_list: Vec<GroupCatalog>,
    pub content_list: Vec<GroupContent>,
    pub node_count: i32,
    pub ctlg_cnt: i32,
    pub cont_cnt: i32,
}

#[derive(Debug, Deserialize)]
pub struct GroupCatalog {
    #[serde(rename = "catalogID")]
    pub catalog_id: String,
    #[serde(rename = "catalogName")]
    pub catalog_name: String,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "updateTime")]
    pub update_time: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct GroupContent {
    #[serde(rename = "contentID")]
    pub content_id: String,
    #[serde(rename = "contentName")]
    pub content_name: String,
    #[serde(rename = "contentSize")]
    pub content_size: i64,
    #[serde(rename = "createTime")]
    pub create_time: String,
    #[serde(rename = "updateTime")]
    pub update_time: String,
    #[serde(rename = "thumbnailURL")]
    pub thumbnail_url: Option<String>,
    pub digest: Option<String>,
}

// ========== 路由和令牌 ==========

#[derive(Debug, Deserialize)]
pub struct QueryRoutePolicyResp {
    pub success: bool,
    pub code: String,
    pub message: String,
    pub data: RoutePolicyData,
}

#[derive(Debug, Deserialize)]
pub struct RoutePolicyData {
    #[serde(rename = "routePolicyList")]
    pub route_policy_list: Vec<RoutePolicy>,
}

#[derive(Debug, Deserialize)]
pub struct RoutePolicy {
    #[serde(rename = "siteID")]
    pub site_id: String,
    #[serde(rename = "siteCode")]
    pub site_code: String,
    #[serde(rename = "modName")]
    pub mod_name: String,
    #[serde(rename = "httpUrl")]
    pub http_url: String,
    #[serde(rename = "httpsUrl")]
    pub https_url: String,
}

// 令牌刷新响应 (XML格式)
#[derive(Debug, Deserialize)]
#[serde(rename = "root")]
pub struct RefreshTokenResp {
    #[serde(rename = "return")]
    pub return_code: String,
    pub token: String,
    pub expiretime: i32,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    pub desc: String,
}

// ========== 通用类型 ==========

#[derive(Debug, Deserialize)]
pub struct ApiResult {
    #[serde(rename = "resultCode")]
    pub result_code: String,
    #[serde(rename = "resultDesc")]
    pub result_desc: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommonAccountInfo {
    pub account: String,
    #[serde(rename = "accountType")]
    pub account_type: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateBatchOprTaskResp {
    pub result: ApiResult,
    #[serde(rename = "taskID")]
    pub task_id: String,
}

// 分片上传信息
#[derive(Debug, Deserialize)]
pub struct PartInfo {
    #[serde(rename = "partNumber")]
    pub part_number: i64,
    #[serde(rename = "partSize")]
    pub part_size: i64,
    #[serde(rename = "parallelHashCtx")]
    pub parallel_hash_ctx: ParallelHashCtx,
}

#[derive(Debug, Deserialize)]
pub struct ParallelHashCtx {
    #[serde(rename = "partOffset")]
    pub part_offset: i64,
}
```

## 11. 配置字段详解

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 授权令牌 (Base64(pc:{account}:{authToken}))
    pub authorization: String,
    /// 手机号码
    pub username: String,
    /// 密码
    pub password: String,
    /// 邮箱Cookies (从 mail.139.com 获取，需包含 RMKEY)
    pub mail_cookies: String,
    /// 存储类型: personal_new | family | group
    #[serde(default = "default_type")]
    pub storage_type: String,
    /// 云ID (家庭云/群组云需要)
    pub cloud_id: Option<String>,
    /// 用户域ID (用于显示存储空间)
    pub user_domain_id: Option<String>,
    /// 自定义分片大小 (字节，0表示自动)
    #[serde(default)]
    pub custom_upload_part_size: i64,
    /// 上传时上报真实文件大小
    #[serde(default = "default_true")]
    pub report_real_size: bool,
    /// 使用大缩略图
    #[serde(default)]
    pub use_large_thumbnail: bool,
}

fn default_type() -> String { "personal_new".to_string() }
fn default_true() -> bool { true }
```

## 12. 加密工具函数

### 12.1 SHA1 哈希
```rust
// 格式: sha1("fetion.com.cn:{password}")
pub fn sha1_hash(data: &str) -> String;
```

### 12.2 MD5 哈希
```rust
pub fn md5_hash(data: &str) -> String;
```

### 12.3 AES-ECB 加密/解密
```rust
// 两层AES加密密钥 (十六进制字符串)
const KEY_HEX_1: &str = "73634235495062495331515373756c734e7253306c673d3d";
const KEY_HEX_2: &str = "7150714477323633586746674c337538";

// AES-ECB PKCS7 解密
pub fn aes_ecb_decrypt(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, Error>;

// 两层加密请求 (用于step3第三方登录)
pub fn yun139_encrypted_request(
    url: &str,
    body: Value,
    headers: Map<String, String>,
    key_hex: &str,
) -> Result<Vec<u8>, Error>;
```

### 12.4 签名计算
```rust
// 计算请求签名
// 1. URL编码body
// 2. 按字母排序
// 3. Base64编码
// 4. MD5(body) + MD5(ts:randStr) 后转大写
pub fn calc_sign(body: &str, ts: &str, rand_str: &str) -> String;
```

### 12.5 PKCS7 填充
```rust
pub fn pkcs7_pad(data: &[u8], block_size: usize) -> Vec<u8>;
pub fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>, Error>;
```

## 13. HTTP 请求封装

所有API请求需要携带以下Header:
```rust
headers: {
    "Accept": "application/json, text/plain, */*",
    "CMS-DEVICE": "default",
    "Authorization": "Basic {authorization}",
    "mcloud-channel": "1000101",
    "mcloud-client": "10701",
    "mcloud-sign": "{ts},{randStr},{sign}",
    "mcloud-version": "7.14.0",
    "Origin": "https://yun.139.com",
    "Referer": "https://yun.139.com/w/",
    "x-DeviceInfo": "||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||",
    "x-huawei-channelSrc": "10000034",
    "x-inner-ntwk": "2",
    "x-m4c-caller": "PC",
    "x-m4c-src": "10002",
    "x-SvcType": "1",  // 1: 个人云, 2: 家庭云
    "Inner-Hcy-Router-Https": "1",
}
```

## 14. 实现步骤 (更新)

1. **Phase 1: 基础框架**
   - 创建项目结构
   - 配置 Cargo.toml
   - 实现 CLI 框架 (clap)
   - 实现配置持久化

2. **Phase 2: 认证模块**
   - 实现加密工具函数 (AES, SHA1, MD5, Base64)
   - 实现三步登录流程 (step1/step2/step3)
   - 实现令牌刷新机制
   - 实现查询路由策略

3. **Phase 3: 个人云核心功能**
   - 实现文件列表
   - 实现文件上传（含分片、秒传）
   - 实现文件下载
   - 实现文件删除

4. **Phase 4: 其他存储类型**
   - 实现家庭云(family)文件操作
   - 实现群组云(group)文件操作

5. **Phase 5: 扩展命令**
   - 实现 mkdir 创建目录
   - 实现 mv 移动/重命名
   - 实现 cp 复制文件
   - 实现 info 查看存储信息

6. **Phase 6: 完善**
   - 添加进度显示
   - 添加日志
   - 错误处理优化

## 15. 参考资料

- **OpenList 139驱动源码**: `OpenList-main/drivers/139/`
  - `driver.go` - 驱动主实现
  - `util.go` - 工具函数（登录、API请求等）
  - `types.go` - 类型定义
  - `meta.go` - 配置定义
