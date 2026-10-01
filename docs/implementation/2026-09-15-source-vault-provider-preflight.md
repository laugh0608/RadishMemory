# Source Vault platform provider 隔离预检

日期：2026-09-15

状态：`P1-S03c-1 preflight complete — supplemental API decisions pending`

时间口径：本文保存 P1-S03c-1 结束时的预检与待决策状态；下文补充方案随后已获批准并进入 [P1-S03c-2 落地](phase1-source-vault-key-provider.md)，不是当前尚未授权事项。

基线：`5acdf74`。本批经项目所有者确认，完成 [P1-S02 实施前核对范围](phase1-encrypted-source-vault-dependency-review.md#platform-provider-实施前核对与批次范围2026-09-15)中的隔离解析、源码和供应链复核。结论是：四项固定依赖可以解析，但直接使用它们仍不足以满足全部契约；正式落地前需要确认下文的补充接口方案。没有修改正式 manifest、lockfile、notices 或 Rust 实现，没有编译候选 provider 或访问系统密钥库。

## 输入与方法

- 本机 Rust / Cargo `1.96.0`；将受控 workspace 源码、manifest、lockfile 复制到唯一任务临时目录，使用独立 `CARGO_HOME` / target 路径。未复制 Cargo 凭据或全局配置。
- 只在副本的 Source Vault manifest 加入 `keyring-core =1.0.0`、macOS `apple-native-keyring-store =1.0.2` / `keychain`、Windows `windows-native-keyring-store =1.1.0` / no defaults、Linux `zbus-secret-service-keyring-store =1.0.1` / `crypto-rust`，均关闭自身 default features。未执行 `cargo update`。
- 从 crates.io 取得依赖后执行 `cargo metadata --format-version 1`，并分别对 `aarch64-apple-darwin`、`aarch64-pc-windows-msvc`、`aarch64-unknown-linux-gnu` 执行 `--locked --offline --filter-platform`；三份 metadata 均成功。
- 对原基线和候选副本执行各目标的 `cargo tree --locked --offline --target <target> -e normal,build`，分别以 Source Vault 和 desktop + Source Vault 为根；另导出 Source Vault 的 `normal,build,features` 树。metadata 的 feature 合并结果是保守图，可能包含其它目标激活的共享 feature；不能据此声称 macOS 实际启用了 Linux crypto 依赖。下文目标增量采用 target-specific Cargo tree。
- 未执行候选代码、第三方 build script、proc macro、provider tests、VM、GUI 或远程 CI；metadata / tree 成功不等于三平台编译或系统验收通过。

## 解析增量与许可证

临时 lockfile 从 **431** 个 package 增至 **453** 个：8 个第一方 package、445 个 crates.io package。新增 22 个第三方 package，原有 package 的版本、来源和 checksum 没有变动，没有删除项或 Git dependency。

| 来源 | 新增 package / 精确版本 |
| --- | --- |
| 共同 | `keyring-core 1.0.0` |
| macOS | `apple-native-keyring-store 1.0.2`、`core-foundation 0.10.1`、`security-framework 3.7.0`、`security-framework-sys 2.17.0` |
| Windows | `windows-native-keyring-store 1.1.0`、`byteorder 1.5.0` |
| Linux | `zbus-secret-service-keyring-store 1.0.1`、`secret-service 5.2.0`、`aes 0.9.3`、`block-padding 0.4.2`、`cbc 0.2.1`、`const-oid 0.10.2`、`cpubits 0.1.1`、`hkdf 0.13.0`、`hmac 0.13.0`、`num 0.4.3`、`num-bigint 0.4.8`、`num-complex 0.4.6`、`num-integer 0.1.47`、`num-iter 0.1.46`、`num-rational 0.4.2` |

全部 445 个 `.crate` archive 的 SHA-256 均匹配 Cargo 解析所得 checksum；其中新增 22 个 package 解包后的 **467** 个发布文件逐一与 archive 比较一致。22 个 package 都有 MIT 文本：`byteorder` 声明 `Unlicense OR MIT`，其它声明 `MIT OR Apache-2.0` 或等价顺序；拟沿用 MIT distribution basis，正式落地时必须纳入 notices。未发现这 22 项新增 NOTICE 文件、build script、proc macro 或 Cargo `links` 字段。声明的最高 MSRV 为 `aes` 的 Rust `1.89`，低于当前工具链；这不是 compile 证据。

| 目标 | desktop + Source Vault 分发根的 package 增量 | 主要 feature / native 影响 |
| --- | --- | --- |
| macOS ARM64 | 5 项：共同 1 + macOS 4 | Apple provider 仅 `keychain`；底层 `security-framework` 仍启用其上游默认 `OSX_10_14`、`alpn`、`session-tickets`。`core-foundation-sys` 增加 default；Security / CoreFoundation framework FFI 真实存在，不能因无 `links` 字段说成无 native surface |
| Windows ARM64 | 3 项：共同 1 + Windows 2 | provider 无 `search`；既有 `windows-sys 0.61.2` 增加 `Win32_Security` / `Win32_Security_Credentials`，`zeroize 1.9.0` 增加 default。实际读取 / 写入使用 `CredReadW` / `CredWriteW` |
| Linux ARM64 | 16 项：共同 1 + Linux 15 | `crypto-rust` 向 `secret-service 5.2.0` 传播；provider 直接依赖的 `zbus 5.19.0` 默认启用既有 async-io runtime，并增加 `blocking-api` / default；`zbus_macros` 增加 `blocking-api`。没有引入 Tokio / OpenSSL |

Linux 还使既有 `cipher` / `inout` 激活 block-padding、`digest` 激活 alloc / mac / oid、`sha2` 激活 alloc / default / oid、`num-traits` 激活 i128。这些 feature 不改变本项目对象 cipher profile。Linux Source Vault 树复用 10 个已有 build-script package 和 9 个已有 proc-macro package；其 native / runtime 面扩大到本地 D-Bus 会话、Secret Service 和系统随机源，不能描述为纯 portable crypto。macOS / Windows Source Vault 树分别复用 2 / 1 个已有 build-script package。上述代码均未在预检中运行。

## 当前 RustSec 复核

读取 [RustSec advisory-db](https://github.com/RustSec/advisory-db/tree/e2e640471715167f73e22eaf761f2e547adafeec) commit `e2e640471715167f73e22eaf761f2e547adafeec` 的 1,226 份 crate advisory，TOML 全部解析成功。先匹配新增 package 名，再扩大到三目标 Source Vault 保守依赖闭包；命中 15 份历史记录，当前版本均满足相应 patched constraint：

| Package / 当前版本 | 命中记录 | patched 下界 |
| --- | --- | --- |
| `security-framework 3.7.0`（新增） | `RUSTSEC-2017-0003` | `0.1.12` |
| `chacha20 0.10.2` | `RUSTSEC-2019-0029` | `0.2.3` |
| `cmov 0.5.4` | `RUSTSEC-2026-0003` | `0.4.4` |
| `crossbeam-utils 0.8.22` | `RUSTSEC-2022-0041` | `0.8.7` |
| `enumflags2 0.7.12` | `RUSTSEC-2023-0035` | `0.7.7` |
| `event-listener 5.4.2` | `RUSTSEC-2026-0221` | `5.4.2` |
| `futures-task 0.3.34` | `RUSTSEC-2020-0060`、`RUSTSEC-2020-0061` | `0.3.6` / `0.3.5` |
| `futures-util 0.3.34` | `RUSTSEC-2020-0059`、`RUSTSEC-2020-0062` | `0.3.7` / `0.3.2` |
| `hashbrown 0.16.1 / 0.17.1` | `RUSTSEC-2024-0402` | `0.15.1` |
| `once_cell 1.21.4` | `RUSTSEC-2019-0017` | `1.0.1` |
| `sha2 0.11.0` | `RUSTSEC-2021-0100` | `0.9.8` |
| `slab 0.4.12` | `RUSTSEC-2025-0047` | `0.4.11` |
| `tracing 0.1.44` | `RUSTSEC-2023-0078` | `0.1.40` |

除 `security-framework` 外，其余 21 个新增 package 名无数据库记录。以上为固定数据库、版本条件和可达性的静态复核，没有安装或运行 `cargo-audit` / `cargo-deny`，不代表完整 workspace 安全审计或未来零漏洞承诺。

## 发布源码确认的行为与缺口

源码路径均相对于相应精确版本的 crates.io 发布 archive；这些是静态确认，不是实际 OS 行为验收。

| 项目 | 源码证据与结论 | 后续实现约束 |
| --- | --- | --- |
| store 选择 | `keyring-core/src/lib.rs` 的 `Entry::new` / `new_with_modifiers` / `search` 访问全局 default；三个 provider 的 `CredentialStoreApi::build` 可直接使用 | 持有明确的 platform store，不设置全局 default 或 fallback；Linux `Store::new` 就会连接 D-Bus / 建立 DH session，不能在默认测试或纯 slot 校验阶段调用 |
| 写入 | 三个平台 setter 均可覆盖已有项；Linux 创建传 `replace=true`；Windows setter 内部还会忽略一次 attribute 读取失败后继续写入 | 不能把 setter 当成 create-only；P1-S04 事务内重验资格、同 slot 串行化、读取合法旧值并复用、写后复验仍不可省略 |
| value 编码 | Windows `cred.rs` 的 `set_password` 把字符串转 UTF-16LE，`get_password` 对称解码；macOS / Linux 默认 password API 使用 UTF-8 | 为落实已冻结的 ASCII bytes，三平台均应使用 `set_secret(encoded.as_bytes())` / `get_secret`，自行严格解码 `rmkek1:` + 64 lowercase hex；不能用 Windows password API 写出另一种字节格式 |
| 错误与日志 | `keyring-core/error.rs` 的 `BadEncoding` / `BadDataFormat` 持有原始字节；`lib.rs` 的 debug 日志会输出 credential identity，即使直接 store build 也可经 `Entry::new_with_credential` 输出 | 对错误 payload 主动零化；公共错误不附原始 source chain。仅覆盖第一方 Debug 不足以证明日志脱敏，真实运行前必须保证 logger 对 `keyring_core` 等敏感 upstream target 禁止输出，并验证 debug / trace 配置不能绕过 |
| Windows Local | `utils.rs::extract_attributes` 从实际 `CREDENTIALW.Persist` 返回 `Local`；Session / 未知值不会映射成 Local | 读取合法值还须核验实际 persistence、target、username；失败不调用 setter 修复；attribute / secret 分开读取有竞态，须在串行化编排中前后复验并由真实平台验收限定保证 |
| Windows label | provider 没有通用 `label` modifier，只有 `target_alias` / `comment` 等 attributes；`update_attributes` 会重写 secret，不能当成无副作用读操作 | 建议将固定 human-readable label 映射到 `comment`，只在受控 bootstrap 写入并复验；需确认该平台映射，不改变 exact target |
| macOS label | `apple-native-keyring-store/keychain.rs` 不实现 attribute 方法；默认 `update_attributes` 返回 `NotSupportedByStore`。`security-framework 3.7.0/item.rs` 则提供安全的 `ItemUpdateOptions::set_label`、`update_item`、`ItemSearchOptions::keychains` / `load_attributes` / `limit` / `search` 与 `SearchResult::simplify_dict` | 四项直接依赖不足；可直接声明已解析的 `security-framework =3.7.0`，在同一 User-domain file keychain、generic-password、精确 service / account 上做窄 label 更新和只读复验；不需要第一方 unsafe 或切换 protected provider。未编译、未证明运行可用 |
| Linux 默认 collection | `service.rs` 对字符串 `default` 只调用 `get_default_collection`；`NoResult` 被映射为 storage-access failure，不会创建替代 collection。但已有项查找只调用跨 collections 的 `search_items`，不检查 default 是否存在 | “创建不 fallback”已静态确认；“default 缺失时所有操作失败”尚不能由该 provider 单独保证。建议直接声明已解析 `secret-service =5.2.0`，以 `get_default_collection` 增加前后只读检查；禁止使用会回退 session 的 `get_any_collection` |
| Linux ambiguity / prompt | `cred.rs::get_unique_item` 对多个 paths 返回 `Ambiguous`；但 `find_matching_items` 在计数前会尝试解锁所有 locked matches | 保留跨 collection 搜索和歧义拒绝，不以 target / 默认 collection 过滤隐藏重复项。解锁或取消会先失败，因此不能保证锁定的重复项一定先返回 ambiguous；须真实验证提示、取消及错误分类，不能声称预检已覆盖 |

`NoStorageAccess` 在三平台各含多种含义，不能统一标记为 locked。Windows 的数值错误包装类型位于上游私有模块，不应解析 Display 文本伪造 OS code；无法可靠取得类型 / 分类时保留脱敏 provider failure。macOS / Linux 中可下转型的受审阅错误也只能用于稳定分类，不能输出路径、slot、原始 bytes 或 D-Bus 消息正文。

## 建议决策与下一批精确范围

**建议保持现有 service / account / ASCII value / provider profile / bootstrap 契约，增加两个已在候选图中的直接依赖，补齐平台检查。此建议尚未批准或解析。**

```toml
# 追加到现有 macOS target dependencies
security-framework = { version = "=3.7.0", default-features = false }

# 追加到现有 Linux target dependencies
secret-service = { version = "=5.2.0", default-features = false, features = ["crypto-rust"] }
```

这两项不是换 provider，也不新增本批之外的 package version；依赖边由传递变为直接，用于 macOS 固定 label 和 Linux default collection guard。静态上预计不增加当前候选 feature / package 集合，仍须正式 locked graph 复验。另需明确 Windows 使用 `comment` 承载固定 label。拒绝此补充时应重新选择或修订 provider 方案，不能忽略 label 错误或放宽 collection 边界。

label 写入失败必须阻止 bootstrap 成功；已写入的 KEK 保留为同 slot 可复用项，只能在重新验真的 bootstrap 中补齐 label 并复验，不能因 label 失败覆盖或删除 KEK。

获准后的 `P1-S03c-2` 范围：

1. 先在隔离副本落实上述直接依赖差异、精确 pin 和三目标图，确认仍为当前 22-package 增量；如再有 package / feature / native 增量，停在具体差异处重新评审。
2. 正式修改 workspace / Source Vault manifest、Cargo lockfile、依赖 notices；在现有 Source Vault 中增加窄 provider 模块与 slot / value / error 类型，复用已有 KEK 和 zeroization。沿用仓库禁止第一方 unsafe 的边界。
3. 增加纯合成的 slot、ASCII encoding、missing / ambiguous / corrupt / denied、错误 payload 和诊断测试；默认测试不构造 Linux store、不访问真实 OS store。macOS 本机 locked check / Clippy / tests 和仓库门禁可运行；不自动把未安装目标或未运行平台记为通过。
4. 记录 logger 过滤与平台副作用验收前置条件。完整 bootstrap 的资格证明、SQLite `IMMEDIATE` 锁、两实例行为仍归 P1-S04；独立 provider 不提供调用方自报 `empty=true` 或公开任意 setter。
5. 更新本评审、当前状态和实现记录，准确标记正式实现与实际平台证据。预计 60–120 分钟，主要副作用为本地 dependency cache、编译产物与可审阅仓库差异；不改数据库、host profile 或真实凭据。回滚为撤销本批精确文件差异和清理任务目录，不重写历史。

此下一批不包含真实 Keychain / Credential Manager / Secret Service 操作、VM / GUI、安装全局工具、SQLite migration、application / UI 接入、push 或远程 CI；P1-S03c-3 仍按测试账户、slot、交互和精确清理范围单独执行。

## 证据与收尾

- 原正式 lockfile SHA-256：`161f1d2a4539abab952293c6d708d7ae424ca983f532730aedb2b0ee126a398d`。
- 隔离候选 lockfile SHA-256：`c1cdf98015606b4cbb9c318b83699f114ff78149d9fbd4f862b49c9787b48d11`。
- 22 项 package / license / source inventory SHA-256：`066bf3c21310552cce3c1e5c0c0082aa7f7ed016b3e7d97fc5e236c0f97c38cf`。
- 本地保留输入文件 hash、候选 manifest / lockfile、解析日志、metadata、目标 feature trees、advisory snapshot 身份和命中明细；不把第三方源码或缓存复制进仓库。
- 本机离线 `./scripts/check-repo.sh` 通过，162 个既有 Rust tests 通过；`git diff --check` 通过。输入 hash 复验确认正式源码 / manifest / lockfile 未变。
- 任务专用候选 / 基线源码副本、独立 Cargo cache 与 RustSec 解包目录已精确清理并复验不存在；候选 target 目录从未创建。仅保留本地预检证据和本任务脚本，没有后台服务、VM 或系统凭据变更，也未 push。
- 本批仅修改三份文档，尚未提交；真实 provider 编译、OS 交互、平台验收和产品加密均未完成。
