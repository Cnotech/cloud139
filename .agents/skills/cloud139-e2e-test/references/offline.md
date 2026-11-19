# 离线回归和故障用例

使用虚构凭据、临时配置和只监听回环地址的 mock。运行前审查所选测试的请求目标。需要网络获取依赖时单独处理，缓存就绪后给 Cargo 加 `--locked --offline`。此选项只控制 Cargo 下载，测试的网络访问需另行隔离。若使用代理限制外连，在测试进程中将 HTTP、HTTPS 和 ALL_PROXY 指向不可用的回环端口，只让 localhost、127.0.0.1、::1 直连。

用例表中的测试名可追加到对应 Cargo 命令末尾作为过滤条件。仅报告实际运行并命中相关断言的用例。

```bash
cargo test --locked --offline -p cloud139-cli --test directory_operations_test
cargo test --locked --offline -p cloud139-cli --test storage_support_test
cargo test --locked --offline -p cloud139-cli --test sync_retry_test
cargo test --locked --offline -p cloud139 --test api_path_paging_test --test pagination_cursor_guard_test
cargo test --locked --offline -p cloud139 --test download_dir_paging_test --test personal_pc_test
```

## 命令参数和存储类型

| 用例 | 覆盖与定位 |
| --- | --- |
| 非个人云（1.3） | [storage_support_test](../../../../packages/cli/tests/storage_support_test.rs) 验证 CLI 和库操作拒绝非 `personal_new`，在续期、传输和进度初始化之前失败。 |
| 13.2 | [directory_operations_test](../../../../packages/cli/tests/directory_operations_test.rs) 的 `removed_options_fail_before_config_changes` 覆盖已移除参数，断言退出码 2 且配置不变。 |
| 13.6 | [storage_support_test](../../../../packages/cli/tests/storage_support_test.rs) 核对配置错误和续期前校验。CLI 缺少必需参数为 2。配置隔离、失败登录和并发写回见下节。 |

## 配置和认证

用 `cargo test --locked --offline -p cloud139-cli --bin cloud139 config` 跑配置相关测试。现有 [config_startup_test](../../../../packages/cli/src/tests/config_startup_test.rs) 验证独立配置路径、续期写回原路径、超时和并发续期。[config_atomic_test](../../../../packages/cli/src/tests/config_atomic_test.rs) 验证并发保存和写入失败后的文件完整性。

- **1.4 路径选择**：显式 `--config` 只使用指定路径；缺失或无效时应报错。省略时先用工作目录，缺失才查临时 HOME 的 `.config/cloud139/cloud139rc.toml`。需要核对整套顺序时，在能隔离默认配置目录的平台放置可区分的虚构配置，用本地 mock 验证。若已有测试只覆盖部分分支，据实记录。
- **1.4 失败登录**：用临时用户目录，执行 `login --token '!invalid!'`。分别指定不存在的配置、已存在的虚构配置，退出码均为 1，前者仍不存在，后者字节不变。该用例在 Token 解析时失败，只验证解析失败的落盘行为。服务端拒绝有效格式 Token 的登录落盘，需要对应的 mock 断言。
- CLI 子进程使用独立 HOME、XDG_CONFIG_HOME 和 USERPROFILE。显式指定配置的用例沿用临时路径。平台目录无法可靠隔离时，可在库或配置层注入候选路径验证。

## 路径分页与故障

| 用例 | 覆盖与定位 |
| --- | --- |
| 13.1 | [directory_operations_test](../../../../packages/cli/tests/directory_operations_test.rs) 的 `root_mutations_are_rejected_for_personal_storage_without_network` 检查路径根别名；`server_root_id_alias_is_rejected_before_delete_or_rename` 检查服务端根 ID 与实际变更请求。[api_path_paging_test](../../../../packages/lib/tests/api_path_paging_test.rs) 覆盖公共路径查找入口拒绝根 ID 子项。 |
| 13.3 | [pagination_cursor_guard_test](../../../../packages/lib/tests/pagination_cursor_guard_test.rs) 与目录用例中的 `recursive_listing_collects_all_pages_empty_dirs_and_detects_cycles`、`ordinary_ls_rejects_cursor_cycles` 覆盖分页游标和目录循环。`failed_or_missing_listing_data_is_never_treated_as_empty` 核对失败或缺数据的列表。 |
| 13.4 | 目录用例的 `normalizes_root_aliases_and_checks_names`、`malicious_names_and_local_kind_conflicts_fail_before_writes`、`symlinks_in_upload_and_download_are_rejected` 及大小写、Unicode 冲突用例。检查绝对路径、穿越、Windows 设备名、链接和类型冲突。记录平台条件，核对临时根外及受保护文件未被覆盖。 |
| 13.5 | 目录用例覆盖上传部分失败、批量条目失败、分页重试与断连、下载大小不符、失败列表和上传回执缺失。看 `upload_and_batch_errors_propagate_partial_failure`、`recursive_upload_retries_the_failed_page_without_replaying_writes`、`recursive_upload_finishes_the_paged_fixture_after_a_mid_upload_disconnect` 和 `recursive_download_validates_listed_size_without_content_length`。确认已完成部分、重试范围和错误结果。 |
| 13.8 | `cargo test --locked --offline -p cloud139 --lib services::download::tests` 验证下载断流、错误响应、已有文件及进度重试。[download_dir_paging_test](../../../../packages/lib/tests/download_dir_paging_test.rs) 核对下载链接的业务错误和空 CDN 地址的回退。 |
| 13.9 | [personal_pc_test](../../../../packages/lib/tests/personal_pc_test.rs) 和 `cargo test --locked --offline -p cloud139 --lib client::api::upload::tests` 核对 PC 请求头、上传权益由远端判断、分片和完成失败时保留原同步目标。用 mock 覆盖大文件权益错误。 |

## 同步和清理

| 用例 | 覆盖与定位 |
| --- | --- |
| 13.7 | `cargo test --locked --offline -p cloud139-cli --bin cloud139 sync_args_test` 核对并发、方向和 JSON 参数。再跑 `storage_support_test` 中的 `invalid_command_arguments_are_rejected_before_token_refresh`，覆盖 `--verify` 方向、并发和非法排除规则。 |
| 13.10 | 目录测试的 `verified_sync` 用例覆盖缺哈希回退、同大小修复、空源清理拒绝、发布前校验、重新核对保留/删除对象及远端竞态。`pc_sync`、`ordinary_sync` 用例覆盖保留恢复副本与祖先、目录替换范围、按扫描 ID 清理和先子后父。 |
| 13.11 | 目录测试的 `sync_json` 用例核对计划和实际变更、部分失败跳过清理，以及发布成功但备份回收失败时仍报告已发布文件。`sync_retry_test` 核对暂时性列表错误可以恢复、JSON 不兼容时没有无效重试。 |

阶段 13 可以单独运行。缺少断言时记为待补用例，已有源码行为说明也不能作为测试已通过的证据。
