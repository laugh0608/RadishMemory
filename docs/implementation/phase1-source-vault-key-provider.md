# Source Vault 独立 platform key provider 落地

日期：2026-09-15

当批状态（2026-09-15）：`P1-S03c-2 isolated provider implementation complete — native acceptance pending`

后续：本文保留 P1-S03c-2 的实现范围与证据；[P1-S04a](phase1-source-vault-key-bootstrap.md) 已新增受 SQLite 事务约束的 library initializer，取代当时等待协调器的 dead-code 写入口。2026-10-01 PR #4 的 Linux locked CI 已通过，Windows 暴露并修复上游 `Cred` 私有可见性阻断，详见本文末节；2026-10-06 已完成 macOS 真实 Keychain 最小正常链路及受损库恢复扩展，见[宿主记录](phase1-source-vault-host.md#2026-10-06-受损库重启与恢复扩展)。2026-10-09 又在既有空合成库显式打开时观察到真实 `KeyStoreCancelled` 并失败关闭；误加的 Prompt 常许许可已按追加授权精确撤销，原四项和 key 保留，见[重验记录](phase1-source-vault-host.md#2026-10-09-prompt-许可精确撤销与拒绝重验)。该结果不覆盖有正文的旧视图撤回、capture / export 中途、`KeyStoreDenied` / locked / ambiguity 或其它平台，剩余范围按[当前状态](../status/current.md)推进。

范围：项目所有者确认 [P1-S03c-1 预检](2026-09-15-source-vault-provider-preflight.md#建议决策与下一批精确范围)后的六项精确依赖、平台映射和独立实现范围。本批只修改 Source Vault、相应依赖 / notices、检查器及正式文档；没有接入 SQLite、application 或 UI，没有访问系统密钥库。Windows / Linux provider 尚无 locked compile 证据，不能以本机通过代替三平台完成。

## 实际 API 与资格边界

- `KeySlot::new` 只接受 `namespace-` / `device-` 各加 32 位 lowercase hex，构成冻结的 account 和 Windows target；它只校验语法，不证明用户权限、library 所有权或 bootstrap 资格。`Debug` 不输出 identity。
- `PlatformKeyProvider::new` 没有系统副作用；公开的 `load_existing` 才连接明确平台 store，返回 zeroizing `KeyEncryptionKey`。读取不调用 setter、标签修复或替代 key 生成；missing / corrupt / ambiguous / metadata mismatch 均失败关闭。
- 2026-10-13 的固定 `read_library_snapshot` 在数据库资格检查后延迟连接 store，同一连接身份贯穿首尾两轮 `load_existing` 双读；第二轮失败或 key 不同拒绝整个快照。普通 `load_existing` 和 bootstrap 算法未改动；具体作用域、取消及不可原子撤销的限制见[动作会话契约](phase1-source-vault-application.md#2026-10-13-只读动作认证会话)。
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

具体 store 不使用全局 default store，也不提供 caller 注入的 production provider。macOS / Windows 使用上游 concrete credential，避免 `Entry` 的 identity debug 日志；Linux provider 内部仍可构造 `Entry` 并输出 identity。**真实调用前必须抑制 upstream 敏感日志，并验证 debug / trace 不会绕过**；P1-S03c-2 初始批次未引入 host logger。2026-10-01 [桌面接线](phase1-source-vault-host.md) 已安装全局无输出 logger 与 panic payload 抑制，并有隔离子进程正反例；后续 macOS 正常调用已观察原生输出和用户授权后的成功访问，保留 TSM CapsLock 诊断；后续真实 `KeyStoreCancelled` 空库打开失败及原 A 对照两轮 stdout / stderr 为空；其它平台与其余异常路径继续待验，不把无输出 logger 或有限运行日志当作系统诊断已全部覆盖。

上表保留 9 月 15 日的证据范围。Windows 对 `Cred` 的直接构造需下述同版本可见性补丁；原记录的源码复核遗漏了其 `pub(crate)` 限制，不能将原实现视为当时已具备 Windows 编译证据。

默认 collection 被锁定时 Linux guard 直接返回 locked，用户需在系统环境解除后重试；其它 collection 中的 locked matches 仍可能使上游尝试 unlock / prompt。多项查找可能先遇到取消或 access failure，因此不保证所有锁定重复项都先返回 ambiguous。真实 GNOME 与 KDE / KWallet-compatible 验收仍不可省略。

## 错误与敏感缓冲

`SourceVaultErrorCode` 增加 slot、missing、ambiguous、corrupt、metadata / read-back mismatch、locked、denied、cancelled、unavailable、failure。`reason` 只保存固定 operation，`io_kind` 不伪装成 OS credential 错误；可可靠取得的 macOS OSStatus 保存在数值 `os_code`。未知错误保持 provider failure，不解析上游 Display 文本猜测 Windows code，也不把 `NoStorageAccess` 全部归为 locked。

`BadEncoding` / `BadDataFormat` 的字节在错误映射时主动零化；不输出或附带原始 source chain。secret read、encoded value、生成的 32-byte KEK 使用 `Zeroizing`，返回 KEK 复用既有类型。合成测试检查 payload 清空、诊断和原始 source 不外露；这不证明 OS、D-Bus 或第三方库内部全部副本已清零，也不改变进程内存、交换区等既有隐私限制。

## 依赖与 notices

六项直接依赖保持预检批准的版本 / target / feature：共同 `keyring-core =1.0.0`；macOS `apple-native-keyring-store =1.0.2` / `keychain`、`security-framework =3.7.0`；Windows `windows-native-keyring-store =1.1.0` / no defaults；Linux `zbus-secret-service-keyring-store =1.0.1`、`secret-service =5.2.0` / `crypto-rust`。均关闭自身 defaults，保留必要的上游 feature 传播。

先在独立 `CARGO_HOME` 副本补齐两条直接依赖，再复验 registry identity / version / checksum、三个 locked metadata feature 集合和 target trees：与 P1-S03c-1 一致，没有额外 package 或 feature 增量。正式 lockfile 由 Cargo 生成，包含 **453** 个 package（8 个第一方、445 个第三方），比原正式基线新增 22 个第三方，旧版本未升级。

22 项的 MIT distribution basis、source / checksum、MSRV、已有 build script / proc macro 的可达性、Security framework / Win32 Credential / D-Bus native 面，以及 RustSec 快照 `e2e640471715167f73e22eaf761f2e547adafeec` 的复核见预检记录；两个补充直接依赖不扩大该集合。没有安装新全局工具或更换 Rust `1.96.0`。

P1-S03c-2 初始批次的 notices 生成器未放宽规则：当时 **366** 项，metadata 保守图分别为 macOS 222、Linux 301、Windows 214；对应正式 `Cargo.lock` SHA-256 `f009a52e68e78a5dc125fe329f6a976b028f84dce85a91e2e25ae028d7b5dd5d`，inventory SHA-256 `fc17c7a1f4f93e93761c8668beb988fa83290fbbc81ef592f0ab0efe60692bf3`。实际 target compile feature 范围另由 Cargo tree 复验；metadata 的保守归属不当成实际 OS 构建证据。

检查器继续精确比较两个 manifest 和完整 lock identity digest；原“尚未授权 platform dependency”阶段禁令由已授权精确集合替代，未允许未知 provider、版本漂移、sample feature 或 fallback。

## 验证与下一步

- 本机 `cargo clippy -p radishmemory-source-vault --all-targets --locked --offline -- -D warnings` 通过。
- 本机 Source Vault 47 个 unit tests 通过，其中新增 12 个共同 provider 测试与 1 个纯 OSStatus 分类测试；既有 macOS filesystem / crypto 测试继续通过。Linux / Windows integration crates 在本机各为 0 个 target-specific tests，不将其计为跨平台验收。
- 新增合成覆盖：slot 与字节格式、固定编码向量与既有 cipher 兼容、load 无写入、读取期间 key 替换、bootstrap 旧 key 复用 / 标签修复、label 失败后重试、竞争 key 复用、非 missing 拒绝、random / write / read-back 失败、payload 零化与诊断。没有真实 credential fixture。
- 公共写入口不可调用的 compile-fail doctest 通过；完整 `./scripts/check-repo.sh` 检查 174 个文件，workspace fmt / Clippy 与 175 个 Rust tests 通过；35 个 Python 检查器回归测试通过。`git diff --check` 通过。
- 22 个新增 archive 与 467 个发布文件已再次核验，正式 lockfile 与隔离 Cargo 生成结果一致。候选源码与独立 Cargo cache 已精确清理并复验不存在，隔离 target 从未创建；保留本地依赖图、hash、检查日志与清理 receipt。正式 Cargo cache 和 workspace target 作为本批依赖 / 构建产物保留。
- 未启动后台服务、VM 或 GUI，未调用系统 key-store API，未改权限、凭据、全局工具或远程状态。P1-S03c-2 结束时改动尚未提交，随后已提交为 `146ea8f`；日终提交状态见[9 月 15 日记录](../status/2026-09-15-source-vault.md)。
- P1-S03c-2 结束时原定先补齐 Windows / Linux locked compile；当前顺位以上述后续说明为准。真实凭据仍按测试账户、专用 slot、logger 过滤、prompt / 锁定 / 拒绝 / duplicate、重开和精确清理范围做 P1-S03c-3。真实 store 授权不由本批依赖 / 构建授权推导。
- 后续 P1-S04 已完成密钥初始化、[正文迁移 / 恢复](phase1-source-vault-body-migration.md)、capture、放弃、维护、删除与认证读取；2026-10-01 又完成 [application 加密用例](phase1-source-vault-application.md) 与[桌面接线](phase1-source-vault-host.md)。P1-S05 的真实平台凭据和 GUI 验收仍待完成。PDF / 图片解析继续等待完整 encrypted Source Vault 链路。

## Windows 编译阻断与最小补丁（2026-10-01）

[PR #4 首轮 CI](https://github.com/laugh0608/RadishMemory/actions/runs/36833340834) 在 candidate `43cd69c` 通过 Repo Hygiene、Linux / macOS Rust Quality；Windows 在 Clippy 阶段因 `cred::Cred` 为 `pub(crate)` 报 E0603，测试未执行。上游公共 explicit-target builder 会把 specifiers 丢弃，新写入 username 为空；直接改用该 builder 将破坏冻结 account 校验与 bootstrap 重试边界，因此不能只替换 import 或放宽校验。

项目所有者确认修复后，保留 exact target/account 与 Local persistence，应用 [上游最小补丁](../../third_party/vendor/README.md)：`windows-native-keyring-store 1.1.0` 的 `Cred` 改为 public。补丁不改变任何 secret、attributes、Win32 操作或 setter 算法；仍承认 setter 是 upsert，真实使用前仍需要 SQLite writer 串行化与 logger 过滤。原 archive checksum、原 / 改后逐文件摘要、原许可证、作者与 source 归属可复验；补丁从 workspace 排除，不添加版本 / feature。

新增 Windows 专用、无系统访问的 construction 回归，直接检查公开构造的 concrete credential 保留冻结 target、service/account specifiers、内部 account 与 Local persistence。仓库门禁同时验证这份 patch 源码完整性与 notices，不把 `source=None` 当作第一方代码而漏掉归属。当前候选三平台编译与合成运行结果以 PR #4 的 head checks 为准；真实 Credential Manager、logger、提示 / 取消 / 拒绝和生产宿主仍不在本批验收内。撤回补丁会恢复该依赖版本的 Windows 编译阻断，不能以改映射或临时成功替代。

本地 macOS `./scripts/check-repo.sh --base-ref origin/master` 通过：235 个仓库文件、notices、fmt、all-targets / all-features locked Clippy、309 项 Rust tests；42 项 Python 检查器回归和 `cargo test --workspace --doc --all-features --locked` 的 compile-fail doctest 通过。独立从原 archive 重比 11 个文件的原 / 改后摘要，确认只有一处可见性变化。没有访问真实系统凭据，未安装新工具或新增 / 升级第三方版本。

## 2026-10-13 合并会话后九次提示的静态诊断与修复提案

状态：诊断 / 设计，未实施 provider 修改，也未启动 GUI 或访问真实 key。三份既有原生记录已先复核并提交为 `1cf3931`；现场事实仍以[局部原生结果](phase1-source-vault-host.md#2026-10-13-新构建原生入口取消与提示预算停止)为准。本节依据 `89d53b0` 实现、锁定的 `apple-native-keyring-store 1.0.2` / `security-framework 3.7.0` 本地源码、已保存的 A01 栈及公开第一方文档；未重新查询凭据、ACL 或系统日志。

### 已证实的调用预算与证据边界

`worker::perform(OpenEncrypted)` 只调用 `open_with_snapshot`，安装 controller 时只复制完整快照；`execute` 取 view 也是内存 clone，没有追加 Inspect 或 reader。`Worker::send` 在 busy 时拒绝派发，完成后没有自动重试。既有 worker 合成测试分别验证空库 / 有资料的 OpenEncrypted 和显式 Refresh 各两次 loader；provider snapshot 测试验证每动作四次 secret 读取。静态控制流和现有测试支持会话合并已生效，不支持“仍走旧的四次 loader”这个解释；A02 没有逐调用遥测，不能宣称已经测得它的真实 API 总数。

正常成功路径展开如下；计数是调用预算，均不是系统提示计数：

| 层级 / 操作 | 每动作次数 | 依据 |
| --- | --- | --- |
| 固定 read snapshot | 1 | `worker.rs` 的 OpenEncrypted 分支；Refresh 使用同入口 |
| `load_existing` | 2 | `reader/snapshot.rs:43` 的 open 与 `:60` 的交付前检查 |
| `KeyStore::read_secret` | 4 | 每 loader 在 `provider.rs:546` / `:549` 双读 |
| `attributes → SecItemCopyMatching` | 12 | 四次 read 的前后检查共 8 次，加四次独立 label 检查 |
| `SecKeychainFindGenericPassword` | 4 | `macos.rs:102 → Cred::get_secret → security-framework`，上游没有 Rust 重试循环 |
| User domain 默认 keychain 获取 | 29 | connect 1 次，12 次 attributes 的前后检查 24 次，四次上游 secret 获取前重新解析 domain 4 次 |

`Store` 只保存 `Cred` 和 `SecKeychain`，没有 `LAContext`、授权票据或系统会话令牌。合并的是应用读取 / 锁 / key 生命周期，不是 OS 的许可生命周期。Apple 说明 Allow Once 仅授权本次获取，下一次需要密码时仍可能询问，见 [Keychain Access 指南](https://support.apple.com/guide/keychain-access/if-youre-asked-for-access-to-your-keychain-kyca1243/mac)。因此重复提示的结构性原因仍存在；保留四次独立 secret 读取的当前契约本身不能承诺一次提示。

metadata 查询只设置 `load_attributes(true)`，没有 data / ref；锁定版本源码默认布尔值为 false，`search` 直接调用 `SecItemCopyMatching`，`simplify_dict` 只转换返回值，没有第二次系统查询。secret 路径使用另一个 legacy API；`Cred::get_secret` 每次重新解析 User keychain，调用 `SecKeychainFindGenericPassword` 后复制字节，未复用一个 OS 授权上下文。不能把十二次 metadata 查询叫作十二次 secret 读取，也不能仅因未请求 data 就断言系统永不提示。

现有 A01 唯一采样正向确认首个提示卡在入口的第一个 `SecKeychainFindGenericPassword`；A02 只有用户汇总九次、最终成功和前后状态相同，没有每个提示的文案、调用点或 begin/end。故九次的精确分解仍未知。候选是 metadata 查询的系统交互、单次 secret API 内多重系统授权 / 挑战，或两者组合；不把它写成“4 + 4 + 1”、label 触发已证实、权限损坏或用户误操作。Mozilla 的[原始故障记录 comment 4](https://bugzilla.mozilla.org/show_bug.cgi?id=1582486#c4)曾直接观察到单次 `SecItemCopyMatching` 产生两个不同提示，这只证明 API 与弹窗不必一一对应，不证明本机也是同因。公开 Apple [Security 源码](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_keychain/lib/SecKeychain.cpp)显示 find-password 进入 item data 获取；公开 main 不是本机系统二进制的精确源码，不据此推算九次。

### 建议顺序与候选修复

首选先执行另行批准的、保持原默认二进制不变的逐提示观测单，仅一次空库打开。逐次结合系统提示类型与正向源码调用点，区分入口 / 末轮、第一次 / 第二次 read、pre / post metadata 和 label。保留最多八次 Allow Once，第九次只采样后拒绝，不再要求用户连续点击再回忆总数；同一提示采样一次，未知调用点即停止。详见[观测单边界](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-13-单动作逐提示观测提案)。

若采样不能区分精确调用实例，下一步应先单独批准最小可观测性补丁：只在现有隔离 acceptance 入口显式启用，记录动作序号、entry / release、read 1 / 2、metadata pre / post / label、domain / secret、call begin / end、单调时间及有界结果。默认关闭，不记录 slot、路径、对象 ID、key 长度 / 字节 / 摘要、metadata 值、原始 OS 错误文本或用户输入。观测失败应中止该次诊断而非静默丢记录；不得添加等待放行、重试、跳过校验或更长 key 生命周期。不是新增业务 schema 或生产审计协议。该补丁尚未实现 / 授权，不能在本次计划中当作已经存在的能力。

| 观测结论 | 候选改动 | 安全及验证条件 |
| --- | --- | --- |
| 独立 label metadata 查询确实触发额外提示 | 将已完成的 read 后 metadata 校验与 label 校验设计为一个可验证读结果，减少独立重复查询 | 不从早先结果盲目缓存 label；需重审检查时点及删除 / 替换竞争，保留 secret 前后唯一性、domain、身份与 label 校验，首尾双读保持；当前 shared trait / bootstrap 也使用 label 校验，不能直接删调用或改成无条件成功 |
| 多个提示落在同一 legacy secret 获取内 | 评估仅 macOS `read_secret` 改用现有 `security-framework` 的精确查询，固定 `self.keychain`，`Limit::Max(1)` 只取 data | 四次读取和独立全量唯一性查询保持，单条 data 查询不替代 ambiguity 检查；不设 label filter 隐藏错误项；不将 data 与 Limit::All 混用。全部错误保留 OSStatus、有界分类，返回类型 / 数量不符失败关闭；secret 立即置于 Zeroizing，禁止 stringify / Debug data 结果，不新增 fallback。此候选也可能多次提示，不宣称换 API 即修复 |
| 单次读取无多余 provider 访问，系统确实逐次要求许可 | 保留当前语义并明确交互成本；若产品要求一次提示，单独评审 OS 授权模型与密钥复验契约 | 不把合并读会话包装成一次授权，不减少首尾双读、不跨动作缓存、不要求永久许可。迁移凭据、稳定签名 / ACL 或另一 keychain 信任模式须单独决策，均不在本提案的最小修复范围 |

第二项候选不需要引入新 crate：当前 `security-framework 3.7.0` 已暴露 query、data 返回及 Limit；现有 metadata 前后查询和 `Store::check_domain` 继续存在，避免混合 domain 重新解析导致的数据路径漂移。它会影响使用同一 macOS provider 的其它读取 / bootstrap 复验，必须列入影响面，不能只测试桌面 OpenEncrypted。是否实施必须由新观测证据决定，不能把未确认的性能 / 提示收益当作理由。

Apple [TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)明确区分 file-based 与 data protection keychain 的访问控制；当前 User domain legacy 项不能凭一个 `LAContext` 字段就假定拥有动作级可复用授权。不得通过 authentication UI skip 排除受保护项、禁止系统交互后重试、改默认 keychain 或删除重建唯一测试 key 来降低计数。

实施前冻结选定改动范围。合成验证须覆盖四次 secret 的每个错误位置、首尾 key 不同、各 metadata 检查的缺失 / 多项 / label / domain 变化、错误类型与零化、日志无内容、无 setter / fallback / 自动重试，以及 bootstrap、写入、导出、恢复等共用 provider 路径；沿用 worker 和 reader 的失败关闭回归，追加 fmt / Clippy、完整仓库检查和默认无 acceptance feature 的构建。合成调用次数不能替代原生提示证据，原生再按新二进制 / dSYM 摘要单独批准。

本次诊断记录的 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：263 个文件，notices、fmt、locked Clippy、25 个 suites 共 379 项 Rust tests 通过，11 个 ignored helper 由父测试执行；沿用合成本地端口权限在沙箱外离线运行。补记后文本 / 链接检查与 `git diff --check` 再次通过；未重跑未受影响的默认构建或其它平台。本次无实现、依赖或 lockfile 改动，未访问真实 key / ACL、启动 GUI、修改权限或执行远程写入；诊断新增的三份文档未提交，`dev` 领先本地 `origin/dev` 五个提交，未 push。新观测单仅准备，检查进程已退出。

### 2026-10-13 逐提示实测后的定位收敛

上述提案提交为 `dffb40d` 后，项目所有者另行批准同构建的一次空库打开观测。实际八个提示各有独立 sample / atos 和用户单次允许确认：1/2 在入口第一读，3/4 在入口第二读，5/6 在末轮第一读，7/8 在末轮第二读，全部定位 `macos.rs:102 → SecKeychainFindGenericPassword`；第八次后 GUI 成功，未出现第九次。具体链路及未覆盖项见[宿主证据](phase1-source-vault-host.md#2026-10-13-单次空库打开的八提示逐次定位)。

当前优先评审 legacy secret 获取路径候选，不先合并 / 删除 metadata 校验：本次没有提示停留于 metadata 查询，而四个 secret 静态位置均成对出现提示。此结论是当前运行的调用位置定位，未取得 OS 调用实例 begin/end 或系统内部授权原因；上一批九次的额外一次仍未解释。`SecItemCopyMatching` 候选仍需验证，不能因改用另一个 API 就承诺一次提示或四次提示；保留四次 secret 读取、domain / identity / ambiguity / label 复验、Zeroizing、真实 OSStatus 和失败关闭，无 fallback / ACL 改动。没有实施代码修改或扩张其它原生验收。


## 2026-10-13 macOS SecItemCopyMatching secret 读取候选

状态：项目所有者在 `b07f861` 观测记录与最小方案评审后明确授权实现、合成验证及文档；本批禁止 GUI 和真实 key 访问。旧构建的八次提示证据继续保留，不改写成新候选的运行结果。

### 实现与安全边界

只替换 `provider/macos.rs` 的 secret 获取：从 `Cred::get_secret → SecKeychainFindGenericPassword` 改为现有锁定版本 `security-framework 3.7.0` 的 `ItemSearchOptions::search → SecItemCopyMatching`。新 data-only 查询显式限定连接时固定的 `self.keychain`、generic password、冻结 service / account、`Limit::Max(1)`，只设置 `load_data(true)`；不请求 attributes / ref，不增加 label 筛选，不设置 Data Protection、同步、认证跳过或授权上下文。旧 attributes 查询保持 `Limit::All` 与 attributes-only，不能在它上面追加 data：Apple 的 [SecItemCopyMatching 约束](https://developer.apple.com/cn/documentation/security/secitemcopymatching%28_%3A_%3A%29)禁止密码查询组合 ReturnData 与 MatchLimitAll。固定 search list 保持文件型 User Keychain，未迁移信任模式或凭据，见 [Apple TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)。

`read_checked_secret` 是模块私有的顺序边界：前置 attributes 成功 → 一次 data search → 严格提取结果 → 后置 attributes 成功才交付。没有生产 provider 注入、自动重试、旧 API fallback 或跨动作缓存。`attributes` 内的前后 User domain 检查、唯一性 / identity 校验，以及 `load_existing` 每次 secret 后的独立 label 校验均保留。单结果 data 查询不证明唯一性；前后独立检查仍不等于 OS 原子快照。

OpenEncrypted / 显式 Refresh 正常路径仍有两轮 loader、四次 secret 获取和十二次 attributes 查询；现在四次 secret 也使用 SecItemCopyMatching，因此成功路径共十六次 SecItemCopyMatching。四次旧 `Cred::get_secret` 内部重新解析默认 domain 被固定 keychain 查询取代，其余 connect 与 attributes 内 domain 检查不变，静态预算从 29 次降为 25 次。这些都是代码预算，不是已测提示次数或运行遥测。

仅接受单个 `SearchResult::Data`，将其 Vec 原分配移动进 `Zeroizing`，仍由现有 decoder 检查 canonical 编码。成功却返回空结果或非 Data 时报告 `KeyStoreFailure`，多结果报告 `KeyAmbiguous`；它们没有 OSStatus，也绝不能当作 `KeyMissing` 授予 bootstrap 创建资格。真实 API 错误沿用 `os_error` 的固定操作 / 数值 OSStatus 分类；不复制上游错误链、不解析文本猜测 locked。

私有 `SecretResults` guard 在返回错误及 unwind 时零化结果中每个 Rust Data 缓冲，包括异常多结果；成功提取后的值由 `Zeroizing` 覆盖后置校验失败 / unwind。不得 Debug、格式化或 simplify_dict secret 结果；上游 SearchResult 的 Debug 会输出 Data 字节。CF / OS 内部副本只按上游所有权释放，不能承诺可清零；上游 search 内尚未返回的缓冲与异常类型处理也不在第一方 guard 的控制内。不新增 unsafe 或依赖补丁来扩大保证。

影响所有 macOS 共用读取路径，包含普通读取、导入 / 导出、恢复 / 维护、删除和 bootstrap 读回；写入与 label 更新算法、SQLite bootstrap 资格及双读 / 首尾 key 一致性保持原样。Windows / Linux、公共 schema、密钥编码、dependencies / lockfile、ACL、签名与系统设置均未改动。`Cred` 仍用于原有写入，不删除现有依赖。

### 合成验证与原生停止线

新增七项 macOS 纯合成回归，覆盖单结果移动 / decoder、空结果和错误类型 / 多结果拒绝与缓冲清理、unwind 清理、前后检查交付顺序、任一 attributes 失败、search 原始错误与异常成功结果，以及真实 bootstrap 算法全部五个既有-key 读位置和 missing 后写前重查不被异常成功结果误导。原 OSStatus 测试扩展覆盖全部现有映射。结果清理测试只检查合成 Rust 缓冲，不宣称测得 OS 内存擦除；查询参数经锁定源码静态复核，合成测试不执行 search 或真实 Keychain API。

现有 snapshot 回归扩展 `KeyStoreFailure` / `KeyCorrupt` 的四个读取位置；桌面回归扩展这两种错误与 unavailable 在入口 / 末轮的视图撤回、恢复状态失效和无自动重试。原四读计数、key 替换、无跨动作缓存、bootstrap、正文与原请求恢复测试继续执行。

原生效果尚未验证：不能承诺一次或四次提示，不以合成通过关闭 P1-S05。下一步先由项目所有者复核候选；之后另行确认新构建摘要、匹配 dSYM、既有空合成库 / 唯一 slot 和逐提示操作单。首轮只做一次空库打开，区分新 data search 与原 metadata 的正向调用点；最多八次允许，第九次交接拒绝，阶段不明即停止，实际零提示也不算取消覆盖。随后另批覆盖首尾取消、有资料 OpenEncrypted / Refresh、busy 与 pending 关闭。不得套用旧构建行号 / dSYM、沿用旧 GUI 授权或提高提示预算。

### 本批实际验证与交接

定向 `cargo test -p radishmemory-source-vault --lib --locked --offline provider::` 的 23 项测试通过。文档与实现合并后的 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整退出 0：264 个文件、notices、fmt、all-targets / all-features locked Clippy，以及 25 个 suites 共 386 项 Rust tests 通过，比 379 项基线新增七项；11 个 ignored 子进程 helper 由父测试执行。沿用本会话已确认的合成端口权限需求在沙箱外离线运行，没有跳过检查。

默认 `cargo build -p radishmemory-desktop --locked --offline` 退出 0；默认 normal / build / features 依赖图未包含 `acceptance-test-support`。补记后仓库文本 / 链接检查和 `git diff --check` 通过。未额外运行 Python 检查器回归、M0 fixture runner、compile-fail doctest 或其它平台构建；本次未改动这些入口，完整检查中的 macOS 结果不扩张为跨平台证据。

未启动 GUI、访问真实 key、采样、改 ACL / 权限、安装 / 升级依赖、修改签名或执行远程动作；没有持锁 helper 或遗留后台进程，历史合成库、原 key、包装 / dSYM 和观测证据保留。检查、构建与默认依赖图日志在本地临时目录保留；本批七个文件未暂存、未提交，等待项目所有者复核。`dev` 仍领先本地 `origin/dev` 七个既有提交，未 push。


### 2026-10-13 SecItem 候选的单次原生结果

候选提交 `bf8dddf` 后按新授权操作单执行一次空库打开，实际八个提示全部逐次确认并采样：四个 secret 静态位置各两个，均定位 `macos.rs:117 → SecItemCopyMatching`；第八次后打开成功、零活动来源，无第九次。新 API 已真实执行，但本次未减少提示。系统内部经 `SecItemCopyMatching_osx / AddItemResults / SecKeychainItemCopyContent` 进入 decrypt 等待，不是应用 fallback；内部授权因果仍未确定，不把静态调用槽当作 OS 调用实例遥测。完整范围与收尾见[宿主记录](phase1-source-vault-host.md#2026-10-13-secitem-secret-新构建逐提示观测)。

GUI 正常退出 0，数据 / profile / 权限与基线一致，无其它验收或 key / ACL 修改。本轮不能证明取消、拒绝、有资料 / Refresh 或写入路径通过，也不自动授权继续诊断。保留四读与全部校验；下一步先复核未改善证据，再限定后续诊断，不直接提高预算、删复验或增加跨动作缓存。

### 2026-10-18 离线复核与下一诊断判据

两轮共 63 份索引文件摘要与长度一致，十六份保存栈 / 符号的 entry / release、read 1 / 2 与 API 定位同原记录吻合；SecItem 冻结构建 / dSYM 与补丁实施前相关源码匹配，既有空合成库文件 / SQLite 基线保持。没有启动 GUI、访问 Keychain 或重验 ACL。下一项诊断需要应用 search 的 call ID / begin-end，才能区分相同静态位置的提示是否属于同一调用区间；不再重复没有新增判别能力的采样。具体假设、观测补丁边界、停止条件和独立原生操作单见[有限诊断方案](../evaluation/phase1-keychain-prompt-diagnostic-plan.md)。该离线复核阶段未修改实现；随后经明确授权完成下述受限补丁，系统内部授权原因及更早九提示仍未知。


### 2026-10-18 受限调用区间观测补丁

macOS 隔离验收入口新增显式 `--acceptance-key-read-trace`；默认无观测作用域时沿原读取路径执行。entry / release、read 1 / 2 和实际 domain / attributes / secret 调用使用固定枚举事件，不更换 query、API、错误映射或认证顺序。成功路径仍是四次 secret、十二次 attributes 与 25 次 User domain；输出失败阻止后续调用，返回的 Data 在写 end 前取得零化 guard，失败不交付快照。业务错误与 end 故障同时发生时保留业务错误，trace 不完整即无诊断结论。

允许文件、接口边界、审阅修正、独立验证及未覆盖项集中见[补丁记录](../evaluation/phase1-keychain-prompt-diagnostic-plan.md#受限观测补丁实现与独立审阅)。本次只运行合成测试和默认构建，没有启动 GUI、访问真实 Keychain 或改动依赖 / lockfile。八提示历史不因新增观测能力而改变；新构建原生验证仍须单独冻结与授权。
