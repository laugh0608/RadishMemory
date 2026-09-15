# Source Vault 独立 platform key provider 落地

日期：2026-09-15

状态：`P1-S03c-2 isolated provider implementation complete — native acceptance pending`

范围：项目所有者确认 [P1-S03c-1 预检](2026-09-15-source-vault-provider-preflight.md#建议决策与下一批精确范围)后的六项精确依赖、平台映射和独立实现范围。本批只修改 Source Vault、相应依赖 / notices、检查器及正式文档；没有接入 SQLite、application 或 UI，没有访问系统密钥库。Windows / Linux provider 尚无 locked compile 证据，不能以本机通过代替三平台完成。

## 实际 API 与资格边界

- `KeySlot::new` 只接受 `namespace-` / `device-` 各加 32 位 lowercase hex，构成冻结的 account 和 Windows target；它只校验语法，不证明用户权限、library 所有权或 bootstrap 资格。`Debug` 不输出 identity。
- `PlatformKeyProvider::new` 没有系统副作用；公开的 `load_existing` 才连接明确平台 store，返回 zeroizing `KeyEncryptionKey`。读取不调用 setter、标签修复或替代 key 生成；missing / corrupt / ambiguous / metadata mismatch 均失败关闭。
- value 使用三平台相同的 70 个 ASCII bytes：`rmkek1:` 加 64 个小写 hex；使用 secret bytes API，避免 Windows password API 的 UTF-16LE 转码。解码拒绝非 UTF-8、空白、尾随、错误长度和大小写替代；读回值必须一致。
- 唯一 production 写入口 `create_if_absent_for_bootstrap` 保持 **private**，没有 public setter、删除、rotation、恢复或 boolean eligibility 参数。它的窄 `expect(dead_code)` 明确记录等待 P1-S04；compile-fail doctest 保护当前不可从 package 外部调用的边界，不为消除 lint 而公开未授权写入。
- 私有 bootstrap 对合法既有 key 复用，对 corrupt / provider failure 停止；只有 missing 才生成，写前再读一次，写后复验 secret，必要时补齐固定 label，再验证 label / value。label 写入失败保留已写入的同 slot KEK，合成重试证明不生成 replacement。已正确的 label 不重写，避免 Windows attribute updater 再次保存 secret。
- 上述写前重查缩小竞态窗口，**不提供跨进程 create-only 保证**。P1-S04 仍必须以 SQLite `IMMEDIATE` transaction 重验真实 bootstrap 资格和已有事实、串行化同 library writer，并验证 key-store write 成功而 database commit 失败后的复用。P1-S03c-2 没有创建资格的公共签发入口，也没有修改 Source Vault 公共 profile。

## 平台实现

| 平台 | 实现 | 当前证据 / 限制 |
| --- | --- | --- |
| macOS | 明确构造 Apple legacy `Cred` / User domain；通过 `security-framework` 安全 API 在同一 keychain、generic password、service / account 上查 attributes；检查唯一性、identity、固定 `labl`，读取前后复验 User keychain 未换；仅内部 bootstrap 可更新 label | macOS ARM64 实际编译、Clippy 与纯合成测试通过；没有调用 Keychain，真实 attribute 字典、更新查询、prompt / cancel / deny 尚未实测。没有第一方 unsafe、protected feature、iCloud 或系统配置修改 |
| Windows | 明确构造 `Cred` 的 exact target、account 和 Local persistence；不能使用上游 explicit-target builder 丢失 account 的默认结果。secret 读取前后检查实际 `persistence=Local`、target 和 username；固定 label 映射 `comment` | 代码已落地，仅源码与目标图复核；未编译、未调用 Credential Manager。属性与 secret 是分开读取的，前后复验不等于 OS 原子快照；上游 setter 仍是 upsert，必须保留 P1-S04 串行化 |
| Linux | `secret-service` 只读检查 default collection 存在且未锁定，并复验会话内 alias 身份；`zbus-secret-service-keyring-store` 不带 target，保留跨 collections 的 exact service / username 搜索和 ambiguity；固定 label 通过 concrete `Specifier` 读取 / 设置 | 代码已落地，仅源码与目标图复核；未编译、未连接 D-Bus。guard 会独立建立一个 DH session，不调用 unlock / create / get-any fallback；provider 另建会话，运行成本与服务重启待实测。暂存 path 只用于当前操作比对，不持久化、不用于绕过搜索 |

具体 store 不使用全局 default store，也不提供 caller 注入的 production provider。macOS / Windows 使用上游 concrete credential，避免 `Entry` 的 identity debug 日志；Linux provider 内部仍可构造 `Entry` 并输出 identity。**真实调用前必须建立 upstream logger target 过滤，并验证 debug / trace 不会绕过**；本批没有引入 host logger 或把这一待验项描述为已解决。

默认 collection 被锁定时 Linux guard 直接返回 locked，用户需在系统环境解除后重试；其它 collection 中的 locked matches 仍可能使上游尝试 unlock / prompt。多项查找可能先遇到取消或 access failure，因此不保证所有锁定重复项都先返回 ambiguous。真实 GNOME 与 KDE / KWallet-compatible 验收仍不可省略。

## 错误与敏感缓冲

`SourceVaultErrorCode` 增加 slot、missing、ambiguous、corrupt、metadata / read-back mismatch、locked、denied、cancelled、unavailable、failure。`reason` 只保存固定 operation，`io_kind` 不伪装成 OS credential 错误；可可靠取得的 macOS OSStatus 保存在数值 `os_code`。未知错误保持 provider failure，不解析上游 Display 文本猜测 Windows code，也不把 `NoStorageAccess` 全部归为 locked。

`BadEncoding` / `BadDataFormat` 的字节在错误映射时主动零化；不输出或附带原始 source chain。secret read、encoded value、生成的 32-byte KEK 使用 `Zeroizing`，返回 KEK 复用既有类型。合成测试检查 payload 清空、诊断和原始 source 不外露；这不证明 OS、D-Bus 或第三方库内部全部副本已清零，也不改变进程内存、交换区等既有隐私限制。

## 依赖与 notices

六项直接依赖保持预检批准的版本 / target / feature：共同 `keyring-core =1.0.0`；macOS `apple-native-keyring-store =1.0.2` / `keychain`、`security-framework =3.7.0`；Windows `windows-native-keyring-store =1.1.0` / no defaults；Linux `zbus-secret-service-keyring-store =1.0.1`、`secret-service =5.2.0` / `crypto-rust`。均关闭自身 defaults，保留必要的上游 feature 传播。

先在独立 `CARGO_HOME` 副本补齐两条直接依赖，再复验 registry identity / version / checksum、三个 locked metadata feature 集合和 target trees：与 P1-S03c-1 一致，没有额外 package 或 feature 增量。正式 lockfile 由 Cargo 生成，包含 **453** 个 package（8 个第一方、445 个第三方），比原正式基线新增 22 个第三方，旧版本未升级。

22 项的 MIT distribution basis、source / checksum、MSRV、已有 build script / proc macro 的可达性、Security framework / Win32 Credential / D-Bus native 面，以及 RustSec 快照 `e2e640471715167f73e22eaf761f2e547adafeec` 的复核见预检记录；两个补充直接依赖不扩大该集合。没有安装新全局工具或更换 Rust `1.96.0`。

notices 生成器未放宽规则：当前 **366** 项，metadata 保守图分别为 macOS 222、Linux 301、Windows 214；对应正式 `Cargo.lock` SHA-256 `f009a52e68e78a5dc125fe329f6a976b028f84dce85a91e2e25ae028d7b5dd5d`，inventory SHA-256 `fc17c7a1f4f93e93761c8668beb988fa83290fbbc81ef592f0ab0efe60692bf3`。实际 target compile feature 范围另由 Cargo tree 复验；metadata 的保守归属不当成实际 OS 构建证据。

检查器继续精确比较两个 manifest 和完整 lock identity digest；原“尚未授权 platform dependency”阶段禁令由已授权精确集合替代，未允许未知 provider、版本漂移、sample feature 或 fallback。

## 验证与下一步

- 本机 `cargo clippy -p radishmemory-source-vault --all-targets --locked --offline -- -D warnings` 通过。
- 本机 Source Vault 47 个 unit tests 通过，其中新增 12 个共同 provider 测试与 1 个纯 OSStatus 分类测试；既有 macOS filesystem / crypto 测试继续通过。Linux / Windows integration crates 在本机各为 0 个 target-specific tests，不将其计为跨平台验收。
- 新增合成覆盖：slot 与字节格式、固定编码向量与既有 cipher 兼容、load 无写入、读取期间 key 替换、bootstrap 旧 key 复用 / 标签修复、label 失败后重试、竞争 key 复用、非 missing 拒绝、random / write / read-back 失败、payload 零化与诊断。没有真实 credential fixture。
- 公共写入口不可调用的 compile-fail doctest 通过；完整 `./scripts/check-repo.sh` 检查 174 个文件，workspace fmt / Clippy 与 175 个 Rust tests 通过；35 个 Python 检查器回归测试通过。`git diff --check` 通过。
- 22 个新增 archive 与 467 个发布文件已再次核验，正式 lockfile 与隔离 Cargo 生成结果一致。候选源码与独立 Cargo cache 已精确清理并复验不存在，隔离 target 从未创建；保留本地依赖图、hash、检查日志与清理 receipt。正式 Cargo cache 和 workspace target 作为本批依赖 / 构建产物保留。
- 未启动后台服务、VM 或 GUI，未调用系统 key-store API，未改权限、凭据、全局工具或远程状态。工作区改动尚未提交，`dev` 仍有此前 2 个未推送提交。
- 下一步先补齐 Windows / Linux locked compile，再按测试账户、专用 slot、logger 过滤、prompt / 锁定 / 拒绝 / duplicate、重开和精确清理范围做 P1-S03c-3。真实 store 授权不由本批依赖 / 构建授权推导。
- P1-S04 / P1-S05、SQLite v6 明文迁移和宿主接入仍未实现。PDF / 图片解析继续等待完整 encrypted Source Vault 链路。
