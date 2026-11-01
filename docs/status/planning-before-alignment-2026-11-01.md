# 2026-11-01 规划整理前来源快照

保留原 current 和路线图全文，仅重定位相对链接。批次日期、旧运行状态与历史授权不作为当前事实或新操作权限；正式顺位见[current](current.md)。

# RadishMemory 当前状态

更新时间：2026-10-28

## 当前阶段

`Phase 1 encrypted desktop integration; macOS minimal native loop verified; extended acceptance pending`

M0、文本 / Markdown 文件入口和本地桌面宿主已建立；原始对象加密已完成独立 portable crypto 与 filesystem adapter 实现，并具备 macOS 本机、Windows ARM64 / NTFS、Linux ARM64 / ext4 的合成运行证据，现已接入显式 application 加密用例与桌面 worker / 恢复界面，默认启动仍保留旧入口。默认桌面产品入口仍是 SQLite v6 inline plaintext body；这里指普通产品正文路径，P1-S04a 的 v7 仅用于显式维护入口的密钥准备 checkpoint；P1-S04b 的 v8 承载正文迁移状态与已迁移对象库，后续 v9 供显式加密 capture 协调，v10 增加显式放弃与终态记录，v11 增加既有 DeleteRequest 下的对象删除执行 checkpoint，v12 保存历史正文缺失的原执行凭据引用；默认 `LocalLibrary` 入口拒绝 v7 / v8 / v9 / v10 / v11 / v12，显式加密入口只接受迁移完成的 v8 至 v12。项目具备受约束的工程原型，但中文找回、完整目录访问、默认 v6 启动失败后的派生修复和生产验收仍有缺口，不能据历史合成验收宣称日常资料库已完整可用。

P1-S03c-2 的独立 provider、macOS 构建与合成验证见[落地记录](../implementation/phase1-source-vault-key-provider.md)，三平台 locked CI 由下述 PR #4 复验；macOS 最小真实密钥库 / GUI 正常链路、受损库边界、权限故障下的三条原请求恢复路径及外部 SQLite 持锁下 GUI 失败恢复已验收；双 GUI 交替与精确导出也已通过。新签名副本首次误加的 Prompt 常许许可已按追加授权精确撤销；重新交接拒绝后，真实 `KeyStoreCancelled` 显式打开失败关闭通过。其它系统错误 / 操作阶段、失败窗口与平台仍待验收。P1-S03b 实现与证据见[filesystem adapter 落地记录](../implementation/phase1-source-vault-filesystem.md)；实现范围为独立 Source Vault package 与 Windows 文件身份 adapter，没有修复既有文本产品质量缺口。已确认问题、静态发现和待测风险见[2026-09-05 项目审阅](../implementation/2026-09-05-project-review.md)；截至 2026-09-03 的详细提交、三平台 CI、依赖数量与 M0 完成流水见[阶段基线归档](2026-09-03-baseline.md)。

## 能力与证据

| 能力 | 已成立 | 当前限制与真相源 |
| --- | --- | --- |
| M0 领域与存储 | canonical core、SQLite v6、来源 / 记忆 / 事件、FTS5、本地删除与真实 M0 runner 已经建立 | [ADR 0002](../adr/0002-m0-local-memory-loop.md)；runner 编排不等于 production 历史查询与上下文编译接口 |
| 字段与 fixture | M0 字段级 canonical schema 定义九种顶层对象；fixture 固定 12 个场景的 86 个有序操作和 12 个指标 gate | [M0 Canonical Schema](../schema/m0-canonical-schema.md)不绑定数据库、生产 ID 编码或语言类型；[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md)说明实际证据限制 |
| 文件入口 | `radishmemory-file-entry` 的 P1-I01 file snapshot contract、P1-I02 atomic source capture、P1-I03 exact export、P1-I04 lineage deletion 已落地 | [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md)；`SourceCaptureStore` 原子提交，`P1-F01` 至 `P1-F18` 已有三平台证据，故障 seam 仅 opt-in `acceptance-test-support`；不代表完整 importer / exporter 已实现 |
| 本地宿主 | P1-H02 application service、P1-H03 source catalog、P1-H04 desktop UI、P1-H05 host acceptance 已有实现及合成宿主证据 | [ADR 0007](../adr/0007-phase1-local-library-host.md)的 `P1-HF01` 至 `P1-HF12` 保留历史记录；目录第 201 条和默认 v6 重启后损坏修复等缺口尚未关闭；显式加密入口的派生损坏重启维护已通过真实 GUI 验收，默认 v6 和其余失败路径仍待覆盖 |
| 加密 Source Vault | P1-S01 storage contract、P1-S02 dependency and cipher review、P1-S03a portable crypto dependency landing 已完成对应范围；P1-S03b immutable object filesystem adapter 已通过 macOS 合成验证、Windows ARM64 / NTFS 提升权限与普通用户验收、Linux ARM64 / ext4 普通用户验收 | [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)的 `P1-SF01` 至 `P1-SF18` 尚未全部实现；object adapter 的 Windows 文件身份替换缺陷已修复并通过普通用户 / ACL 回归；P1-S03c-2 独立 key provider 已实现并有 macOS 构建与合成测试证据；P1-S04a 已建立密钥初始化 checkpoint；正文迁移 / 恢复、加密 capture、inventory reconciliation 及显式放弃 / 精确 orphan retirement 合成切片已落地；独立 object-backed verify / rebuild、删除执行及历史请求接管已实现；认证读取、目录 / 检索及精确导出组合验收已通过；显式 application 打开 / 恢复、读取、原请求 capture / 删除及派生维护已接入；桌面后台接线与原请求恢复已实现并通过合成测试；macOS 最小正常链路、受损库边界及权限故障下的原请求重试 / 精确放弃 / 原删除授权恢复已完成真实 Keychain / GUI 验收；外部 SQLite 持锁下 GUI 失败恢复、双 GUI 交替及精确导出已通过；误加的 Prompt 常许许可已精确撤销，真实系统交互返回 `KeyStoreCancelled` 的空库打开失败关闭通过；当前 Linux / Windows ARM64 的默认构建与常规合成测试已通过，Windows 另通过显式 reparse 与普通用户 ACL 验收；Windows / Linux ARM64 两轮真实 provider / GUI 正常链路、重登后五份精确导出与应用层删除均已通过；Linux 新账号失效 default 已按追加授权修复，重登后本批 collection 由用户解锁；其它系统错误 / 操作阶段继续待验 |
| 用户价值 | 已能通过本地入口导入、版本化、搜索、精确导出和删除合成文本 | 尚无完整记忆控制台、模型问答、PDF / 图片解析、向量、同步、恢复或签名发行包 |

## 已确认待实施的客户端方向

2026-10-12 已确认[客户端连接规划](../client-server-connection-plan.md)：同一个官方安装包运行时支持官方零知识同步端点与用户自部署后端，无需重新编译。该目标归入阶段 4；本轮只更新产品和客户端文档，同步服务与服务器选择尚未交付，当前本地加密桌面集成顺位不变。

## 当前顺位

1. `P1-S04a SQLite key bootstrap coordination` 已实现：在真实 SQLite `IMMEDIATE` transaction 内验真初始化资格，复验对象目录、创建 / 复用密钥并提交 maintenance-only v7 `key_ready` checkpoint。合成测试覆盖并发连接、独立子进程、真实 commit 失败、缺 key、历史正文损坏和 schema 漂移，见[落地记录](../implementation/phase1-source-vault-key-bootstrap.md)。普通产品入口继续使用 v6；该初始化步骤不迁移正文；后续迁移由独立 v8 维护入口承担。
2. `P1-S04` 已完成独立的正文迁移 / 恢复、加密 capture、inventory reconciliation、显式放弃、verify / rebuild、对象删除与历史请求接管，以及认证读取 / 目录 / 检索和精确导出组合验收；各批实现与代码复核见[9 月 26 日日终记录](2026-09-26-source-vault.md)。[application 加密用例](../implementation/phase1-source-vault-application.md) 已完成显式旧连接关闭、密钥准备 / 迁移恢复、已迁移库打开及完整读取 / 导出会话，具备合成验收。本批已接入 application 的 import / update、lineage 删除及 verify / rebuild，保留原请求精确恢复。[桌面宿主接入](../implementation/phase1-source-vault-host.md) 已完成串行 worker、显式初始化 / 迁移 / 打开、内存原请求重试、重启发现原删除授权、精确放弃及 failed-open 维护入口。依赖连线已获批准；诊断采用无输出 logger 与 panic payload 抑制，并通过隔离子进程验证。2026-10-06 已按精确授权完成 macOS 真实 Keychain / GUI 的最小正常闭环；随后完成派生损坏重启恢复与密文损坏拒绝的真实 GUI 扩展；2026-10-09 又完成权限故障下的原请求重试、重启精确放弃及原删除授权恢复；随后外部 SQLite 持锁下的 GUI 失败恢复通过，并只读核对测试 key 访问控制。随后不同 bundle ID 的 A/B 已完成独立定位、跨窗口版本刷新与精确导出；Prompt 首次“始终允许”新增许可已按追加授权精确撤销；在触发前交接拒绝后，空合成库打开返回真实 `KeyStoreCancelled` 并失败关闭，原 A 读取对照通过。2026-10-13 已修复关闭事件与 worker 结果同帧到达、接受关闭后仍派发动作的 UI 竞争，并补齐 headless UI 和真实 worker 退出回归；2026-10-24 当前构建已通过空库锁等待下的自然 busy 关闭拒绝、错误交付保留与再次正常关闭；精确同帧竞争及原请求关闭确认不在该原生证据范围。其余提交 / evidence 中断窗口、其它系统密钥错误 / 操作阶段、重叠 GUI 事务、中文最小窗口 / 跨平台显示与跨平台扩展验收仍待收口。详见[宿主证据](../implementation/phase1-source-vault-host.md#2026-10-06-macos-最小真实闭环)，新增系统操作继续按具体范围授权。
3. 已进入 Windows / Linux 集中检查点：2026-10-24 Linux ARM64 普通用户与 Windows ARM64 的离线默认构建、常规合成测试均通过，Windows 另完成显式 reparse 与普通用户 ACL 验收；两平台随后各完成两轮真实系统 key store / GUI 正常链路、重登、精确导出与应用层删除，详见[执行记录](../evaluation/phase1-windows-linux-provider-gui-plan.md)。其它失败窗口、中文质量和完整 P1-S05 尚未收口；PDF / 图片解析继续等待完整加密链路通过。
4. R01 至 R06 的产品质量缺口继续跟踪：[本地资料库质量验收计划](../evaluation/phase1-local-library-quality.md)中的中文检索、目录分页、派生损坏维护入口、读取性能、runner 证据和回源 / 结果刷新仍未收口。本批基础存储实现不等于这些产品问题已经修复。
5. 产品验证继续聚焦“同一个长期项目的资料、关键事实、更正与受控上下文”，完整阶段依赖以[MVP 路线图](../mvp-roadmap.md)为准。

## 已接受的边界

- [ADR 0005](../adr/0005-m0-implementation-stack.md)冻结 Rust 2024 模块化单体，首个工具链固定为 Rust `1.96.0`。当前依赖、manifest / lockfile 和 notices 以[Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)及[第三方 notices 记录](../implementation/phase1-third-party-notices.md)为准。
- P1-S02 已选择 `radishmemory.xchacha20poly1305-stream-be32/1` 与 `radishmemory.xchacha20poly1305-dek-wrap/1`；P1-S03a 已使 portable manifest / `Cargo.lock` / notices 和 cipher 实现落地；P1-S03b 复用该依赖与 AAD，新增严格 envelope、不可覆盖发布及精确 attempt 检查；Windows 使用单独审阅的最小文件身份 adapter，P1-S03c-2 另落地独立 provider；后续已完成 macOS 最小真实系统 key store 正常链路验收。
- 原始对象按 source version 独立认证加密，不跨 provenance 物理去重；publish → SQLite commit → read-back 必须完整成立。未知 profile、缺 key、认证失败或 ambiguous state 失败关闭，不回退旧 BLOB 或外部原件。
- 首批对象加密不覆盖 SQLite metadata、FTS、派生数据；当前整文件片段使 FTS 保存完整可读正文，不能简化为“只有少量元数据未加密”。历史明文、进程内明文、交换区、快照、备份和用户导出仍在该保证之外。
- [ADR 0003](../adr/0003-zero-knowledge-sync-first.md)选择零知识同步服务，可信计算节点后置为显式可选能力；不代表零知识同步已经实现。
- [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md)将首次接入放在完整 MVP 阶段 3，以显式可关闭的 Model Gateway 接入；首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回。本地能力不依赖 RadishMind。

## 当前停止线

- 不将独立 adapter 或文档更新视为产品缺陷已修复，不扩大 P1-H05、M0 fixture 或 portable crypto 的证据范围；P1-S03b 已有平台证据不替代其它 Linux / Windows 文件系统、架构、真实断电、SQLite / host 验收。
- 不在 `P1-S03b` 至 `P1-S05` 完成前声明加密 Source Vault 可用，不进入 PDF / 图片解析；真实 key store、migration 与宿主接入须分别授权和验证。
- 不将原始对象加密表述为整个资料库静态加密；不声明零知识同步、取证级永久删除、备份可恢复或生产可用。
- 不使用真实个人资料进行仓库 / CI 验收；不自动接入模型、网络、同步或兄弟项目，不增加虚拟形象、图数据库或微服务。
- 不改变数据所有权、canonical schema、记忆确认、权限失败关闭、许可证或远程治理；待决策建议须先明确影响再取得范围确认。

## 当前验证入口

2026-10-24 Windows / Linux ARM64 真实 provider / GUI 正常链路均通过：各新建普通账户、唯一 slot、一次 Prepare、两轮 GUI，完成迁移、加密导入 / 更新、真实注销重登、五份逐字节导出与两条 lineage 删除。四个 GUI 均退出 0，最终活动来源 / 对象引用 / FTS 均为 0，账号已注销，九台 VM 全停；两平台账户、key 与复验材料保留。Linux 失效 default 修复取得追加授权，GNOME schema 属性的观测脚本修正、重登后锁定门禁及用户解锁均有独立记录。实际结果、交接异常与保留边界见[执行记录](../evaluation/phase1-windows-linux-provider-gui-plan.md#2026-10-24-windows-真实执行结果)。中文显示、其它失败窗口与完整 P1-S05 仍未关闭。

2026-10-24 Windows ARM64 离线执行及普通用户追加验收通过：Linux 结果提交为 `da4cd4f` 后，当前输入完成 fmt、locked Clippy、两组常规测试（368 passed）与默认 ARM64 PE 构建；另按授权完成 reparse 和临时普通用户 ACL 两项测试，合计 370 项实际通过、0 failed。两处验收脚本问题已修复并复验，产品源码与 lockfile 保持。临时账户及目录授权已回收，最终正常关机并确认九台 VM 全停；约 5.08 GiB 本批输入 / 构建材料保留。真实 provider / GUI 未执行，详见[Windows 结果](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-24-windows-arm64-离线构建与合成测试结果)。

2026-10-24 Linux ARM64 离线执行通过：准备文档提交为 `4eb5f32` 后，按授权仅启动日常 Debian，以普通用户验证 `c30b310` 离线输入并完成目标解析、fmt、locked Clippy、workspace 合成测试及默认 desktop 构建。实际为 25 suites、387 passed、0 failed、11 个父测试调用的 ignored helper；产物确认 AArch64，源码清单保持。证据回收后正常关机，最终九台 VM 全停；约 5.03 GiB 本批目录按方案保留。未执行真实 provider / GUI 或 Windows 批次，详见[实际结果](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-24-linux-arm64-离线构建与合成测试结果)。

2026-10-24 集中跨平台准备及串行环境核对完成：同实现的三平台 CI、源码归档与本机目标缓存已复核；随后按授权先 Linux、确认全停后 Windows，核对已有 Rust 1.96.0 与原生工具，最终九台 VM 全部停止。该环境核对批次未执行客体构建 / 测试、安装或真实 provider 调用。Linux 普通账户可执行 rustc，但无活动桌面会话；Windows agent 为 SYSTEM、UAC 关闭，不能视为普通用户或用户凭据证据。全机最多一台 VM 运行已纳入协作约束；实际环境、异常与收尾见[串行核对结果](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-24-串行环境核对结果)，原准备与 CI 范围见[准备记录](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-24-集中跨平台准备)。

2026-10-24 当前构建的空库原生关闭验收通过：自然 busy 时关闭被拒，真实 SQLite 锁错误交付后窗口保留，释放锁后再次关闭正常退出；两个 GUI / 两个持锁 helper 均退出 0，库状态与冻结输入保持。准备和结果文档更新后的完整离线仓库检查均通过，Rust 基线仍为 398 项；默认 desktop 构建与 17 项 helper 合成回归通过。M01 v2 普通文件门禁因旧源码摘要漂移失败并暂缓，未执行原生查询；其它平台与远程检查未重验，详见[本次宿主记录](../implementation/phase1-source-vault-host.md#2026-10-24-空库锁等待下的原生关闭验收)。

2026-10-18 `dev -> master` PR 预检：独立模块探针在已安装的非 macOS target 上复现观测枚举六个专用事件的 `dead_code`；已按 macOS / 合成测试的实际使用范围补齐条件编译，同步受审阅源码摘要，未放宽门禁或改变 macOS 事件与认证流程。修复后普通 / 测试模块探针均通过 `-D warnings` metadata 编译；它不等于 Linux / Windows 完整构建。`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh --base-ref origin/master` 通过仓库基线、PR 提交 / diff、notices、检查器回归、fmt、locked Clippy 及 398 项 Rust tests（25 suites，11 个 ignored 子进程 helper 由父测试调用）；默认 desktop 的 locked / offline 构建通过。三平台远程结果以本次 PR 的 `Candidate Quality` 为准，本轮没有启动 GUI、访问真实 key store 或执行 M01 v2，不改变 P1-S05 待验边界。

[PR #5](https://github.com/laugh0608/RadishMemory/pull/5) 首轮 CI 的仓库卫生及 Linux / macOS Rust Quality 通过，Windows 在隔离入口路径拒绝测试失败：canonical verbatim 路径上的 `PathBuf::join` 已提前消去测试中的 `..`，传入了合法路径。已改用 `OsString` 保留原始父目录片段并断言输入有效，生产校验与拒绝边界不变；定点回归、fmt 及同一完整离线仓库检查再次通过（398 项 Rust tests）。后续三平台结果以修复提交的 PR 检查为准，首轮失败不记作通过。

PR #5 第二轮的路径拒绝回归通过，仓库卫生与 Linux / macOS 继续成功；Windows 在 10 分钟整体上限取消，Source Vault 180 项中已有 179 项报告结束，201 条资料分页迁移测试尚未结束，取消不记作通过。已按[CI 契约](../governance/repository-governance.md#ci-契约)将 Windows 测试分为 Source Vault / 其余 workspace 两个必需矩阵项，保留所有用例、断言、逐 job 10 分钟限制及聚合失败关闭；同步 ADR、Ruleset 说明与检查器，并新增遗漏分组 / 命令的负向回归。本机完整离线仓库检查再次通过（398 项 Rust tests），测试路由 shell 语法与未知 scope 拒绝通过；最终 Windows 覆盖仍须以新一轮两个矩阵项均成功为准。

2026-10-18 日终基线：受限观测补丁及 D01 / D02 记录已提交；M01 首次执行在静态身份前置失败，目标查询为零，副本范围 v2 已完成合成验证和重新冻结、新 native helper 未执行。完整离线仓库检查通过，Rust 基线为 398 项；当天另有 45 项检查器回归、默认 desktop 构建及 v2 的 47 项合成测试证据。各批验证、真实访问与未覆盖项分开记录于[日终回顾](2026-10-18-source-vault.md)；不以合成或副本静态验证关闭 P1-S05。

macOS / Linux：

```bash
./scripts/check-repo.sh
./scripts/check-m0-fixtures.py
```

Windows：

```powershell
pwsh ./scripts/check-repo.ps1
```

2026-10-13 日终基线：busy 关闭竞争、只读动作会话和 macOS SecItem secret 读取已提交；完整离线检查、locked Clippy 与 386 项 Rust tests、默认 desktop 构建通过。新 API 单次原生空库打开仍八次提示，未减少；GUI 正常退出，基线保持。代码 / 文档复核与逐批证据见[今日日终记录](2026-10-13-source-vault.md)，取消、关闭竞争及平台边界按记录区分。

2026-10-13 关闭竞争切片：修复同帧完成 / 关闭与退出前派发动作问题，新增六项 headless UI 和一项真实 worker Drop / join 回归。完整离线检查、locked Clippy 与 366 项 Rust tests、默认 desktop 构建通过；首轮 P1-F17 沙箱端口限制经权限流程完整重跑解决。没有启动 GUI 或访问系统 key store；新构建的原生关闭竞争仍待验，见[本批证据](../implementation/phase1-source-vault-host.md#2026-10-13-busy-关闭竞争修复与剩余窗口核对)。

2026-10-09 双实例原生验收：A/B 独立定位、跨窗口版本刷新与四份精确导出通过，三个进程正常退出，来源删除 Completed 十组件、零活动资料。Prompt 新签名验证通过；首次误加的常许许可已精确撤销，重新交接后空库打开返回 `KeyStoreCancelled` 并失败关闭，原 A 对照成功；后续两个进程也已退出，见[最终结果](../implementation/phase1-source-vault-host.md#2026-10-09-prompt-许可精确撤销与拒绝重验)。

2026-10-09 原生锁 / 凭据核对：已授权范围中，外部 SQLite 锁下 Export / Refresh 失败关闭、释放后显式恢复与两份精确导出通过；Keychain Access 只读核对完成，未改变权限或 secret。该子批次尚未覆盖双 GUI 交替和实际系统取消 / 拒绝，后续覆盖范围见上段最终结果。两个 GUI、helper 与 Keychain Access 均退出，本轮来源删除 Completed、十组件 succeeded、零活动资料，见[原生记录](../implementation/phase1-source-vault-host.md#2026-10-09-原生锁竞争与-keychain-只读核对)。

2026-10-09 多实例准备：新增桌面真实 SQLite 锁竞争 / 合成 key 回归通过，完整离线检查、locked Clippy 与 359 项 Rust tests 通过；临时持锁工具的独立子进程释放 / 超时预演通过。P1-F17 沙箱端口限制经权限流程完整离线重跑解决；该准备批次未启动 GUI 或访问系统凭据，见[准备证据](../implementation/phase1-source-vault-host.md#2026-10-09-keychain-与多实例准备)。

2026-10-09 原请求恢复预演：三项真实 Unix 文件系统故障 / 合成 key 回归通过；完整离线仓库检查、locked Clippy 与 358 项 Rust tests、默认 desktop 构建通过。首轮 P1-F17 合成本地端口受沙箱限制，按权限流程完整重跑通过；随后经当前任务精确授权完成三条 macOS GUI 恢复路径、关闭 / 放弃确认与 Cancel；三轮进程正常退出，权限恢复，两个 Completed 回执与零活动资料已复核，见[原生记录](../implementation/phase1-source-vault-host.md#2026-10-09-原请求恢复的-macos-原生验收)。

2026-10-06 受损库原生扩展：显式派生重建恢复同一来源，密文认证失败保持关闭；三次精确导出一致，canonical / 对象摘要按场景复核。本轮来源删除回执 Completed，四轮正常退出；完整检查与 355 项 Rust tests 通过。范围、保留备份及尚未关闭的 Q03 / Q07 见[恢复扩展证据](../implementation/phase1-source-vault-host.md#2026-10-06-受损库重启与恢复扩展)。

2026-10-06 macOS 原生最小闭环与布局复验：真实 Keychain 准备 / 读取、迁移、加密导入 / 更新、历史导出、关闭重开、Verify 与删除通过；布局修复后完整仓库检查、354 项 Rust tests、43 项检查器回归、M0 fixture 和默认构建通过。额外默认空库窗口的工具启动副作用及保留产物已如实记录，详见[宿主收尾](../implementation/phase1-source-vault-host.md#原生复验收尾与异常)。恢复 / 失败扩展、中文显示及其它平台继续待验。

2026-10-06 隔离原生入口准备：完整仓库检查、354 项 Rust tests（新增 9 项隔离入口 / worker 回归）、43 项检查器回归、M0 fixture、compile-fail doctest 和默认 desktop 构建通过。首轮 P1-F17 合成端口受沙箱拒绝后，按权限流程完整重跑通过；具体证据见[宿主记录](../implementation/phase1-source-vault-host.md#2026-10-06-隔离入口准备证据)。准备切片结束时合成目录 / profile 与精确操作单已就绪；本任务随后经授权完成 macOS 最小真实闭环，Windows / Linux 保留待验。

2026-10-01 桌面宿主接入：恢复前置接口已提交；新 worker、原请求恢复、显式加密 UI 和日志策略已完成本机合成验证；完整仓库检查、345 项 Rust tests、43 项检查器回归、M0 fixture、compile-fail doctest 与默认 desktop 构建通过。完整范围及尚未执行的真实 Keychain / GUI、Windows / Linux 集中验收见[宿主记录](../implementation/phase1-source-vault-host.md)。默认启动仍为 v6，显式加密模式的 FTS 也保留完整明文；既有删除证据摘要复算缺口单独记录，不扩大保证。

2026-10-01 application 加密用例切片：本机完整仓库检查、notices、fmt、locked Clippy 与 330 项 Rust tests（写用例批次新增 9 项、此前读取批次新增 12 项）通过；42 项检查器回归、M0 fixture 和 compile-fail doctest 通过。P1-F17 合成本地端口的沙箱限制经授权在沙箱外完整重跑解决，全部使用合成资料和测试 key；范围、原请求恢复、派生修复与未验收边界见[application 记录](../implementation/phase1-source-vault-application.md)。默认桌面及真实系统密钥库仍未切换。

2026-09-26 实现基线 `8d2c651`：本机完整仓库检查、notices、fmt、locked Clippy / Rust tests、检查器回归、M0 fixture 和 compile-fail doctest 通过；批次数量、独立子进程锁回归、沙箱限制与验证日志范围见[日终记录](2026-09-26-source-vault.md#验证与环境收尾)。全部使用合成库与测试 key，未调用真实系统密钥库。

早期 Linux ARM64 / ext4 filesystem 证据来自 `9d88319`；Windows ARM64 / NTFS 的历史证据仍引用 9 月 10 日基线，不覆盖其后续 provider / SQLite 协调和读取代码。当前 Linux / Windows ARM64 的构建和合成覆盖已由上述 10 月 24 日批次补齐；两平台真实 key store / GUI 正常链路已由本日后续批次补齐，其它系统错误与宿主失败窗口仍待验收。历史范围与清理见[filesystem 记录](../implementation/phase1-source-vault-filesystem.md#linux-arm64--ext4-普通用户验收2026-09-15)、[9 月 10 日记录](2026-09-10-source-vault.md)和[9 月 15 日记录](2026-09-15-source-vault.md)。

当前维护与读取入口接受已完成迁移的 v8 至 v12，正文迁移入口仍仅接受 v7 / v8，普通产品入口仍为 v6。删除中的对象与历史正文缺失凭据须复验，verify / rebuild / reconciliation 不自动推进删除；详见[历史请求兼容](../implementation/phase1-source-vault-legacy-deletion.md)。数据库身份检查已修复 Unix 额外文件描述符关闭导致的 POSIX 锁释放，并分离单对象与数据库大小限制；[读取记录](../implementation/phase1-source-vault-reader.md)保存跨进程证据。每次读取的全库认证成本、持锁期间的操作串行化及产品 R01 至 R06 缺口尚需后续处理。

2026-10-01 阶段晋级 [PR #4](https://github.com/laugh0608/RadishMemory/pull/4) 的首轮 CI 已通过仓库卫生和 Linux / macOS locked Rust Quality；Windows 暴露 provider 引用私有 `Cred` 的编译阻断。经项目所有者批准，以 [同版本最小补丁](../../third_party/vendor/README.md)保留冻结 target/account 和既有依赖图，修复源码可见性并增加无系统访问的构造回归。本地历史已包含该 PR 的合并提交 `689c166`；本批未重新读取远程 head checks，不以补丁、合并或历史 filesystem 记录代替各平台通过证据；macOS 最小真实闭环的后续证据见上述 10 月 6 日记录；Windows / Linux 正常 key store / GUI 的当前证据见本日执行记录，logger 其它失败路径和宿主扩展交互仍待验收。修复范围与来源见 [provider 记录](../implementation/phase1-source-vault-key-provider.md#windows-编译阻断与最小补丁2026-10-01)。

## 后续事项

2026-10-28 日终：当天四个实现 / 证据提交、相关文档复核与保留范围见[今日日终记录](2026-10-28-chinese-recall.md)。`eab2b0f` / `3152f2c` 已接入固定随包中文字体，并通过 macOS 已观察窗口的中文可读显示与有资料 Refresh 认证失败撤回 / 显式恢复；760×520 最小窗口和 Windows / Linux 新字体仍待验。`f2f8e10` 保存旧检索 15/30 的历史基线；`4c9b0be` 的[生产修复](../implementation/phase1-chinese-recall.md)在相同标签下双入口达到 30/30、24/24 集合匹配、0 误召回，并提供显式升级 / 降级与旧客户端拒绝。完整检查 427 项测试通过；新切换 UI 的原生与跨平台证据尚未取得，Q01 / Q04 / P1-S05 均保持开放。

1. **先审阅 Q01 标签与 Q04 成本**：独立复核 12 份固定合成资料 / 24 个查询的相关来源及短语边界；标签不能为迁就算法而改写。32 份约 0.7 MB 合成资料的 debug 测量中，加密中文查询中位约 4.68 秒，新索引空间也增加。下一切片先用可重复的 release / 分阶段计时区分全库认证、canonical / 派生复验、FTS、结果回源与刷新成本，并记录资料量、版本、缓存状态与运行环境，再确定优化范围和预算；不省略权限、完整性、时间或删除校验，也不据本次数字承诺生产容量。
2. **补新格式的有限可见验收**：围绕当前提交准备独立合成旧格式库，覆盖升级确认 Cancel / Confirm、真实中文命中、关闭重开、显式降级与提示、精确导出和 760×520 最小窗口。新库现在默认使用新格式，不能把旧操作单的“新空库”当成旧格式 fixture；不得改写或复用旧冻结摘要。真实 GUI / key store 执行须另行说明目标、时长、停止与保留范围并取得授权。Windows / Linux 先按最终输入做构建 / 合成，再补字体和切换 UI 可见证据；VM 启动前核对完整清单，始终串行。
3. **继续文本资料库质量闭环**：随后推进 Q02 第 201 条及完整目录访问、Q06 正文后段命中定位与更新 / 删除后的结果刷新；默认 v6 failed-open 派生维护入口保持 Q03 独立事项。已经通过的 macOS 有资料 Refresh 认证失败、空库自然 busy 关闭、双 GUI 交替和 Windows / Linux 正常链路不重复列为未验；独立标签、性能、原生窗口分别验收。
4. **其余失败窗口与契约保持独立**：真实 key 首尾取消、其它有资料操作失败、pending 原请求关闭 Cancel / Confirm、跨平台错误窗口仍按[有限检查点](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)另定范围。精确同帧关闭只引用 headless 回归，不能由正常原生退出替代。`evidence_digest` 复算及历史兼容先做契约评审，完整 P1-S05 与 PDF / 图片停止线保持。
5. **M01 继续暂缓**：10 月 24 日确认的源码摘要漂移与旧 v2 冻结件保留，详见[当日日终记录](2026-10-24-source-vault.md)及[副本范围 v2](../evaluation/phase1-keychain-prompt-diagnostic-plan.md#2026-10-18-m01-副本范围-v2-实现与重新冻结)。未来先审阅绑定和独立冻结范围，再按新版操作单精确授权；不重签、不跳过校验、不改 key / ACL、不把副本匹配当作原 app 授权。D01 / D02 观测与未解释提示仍见[10 月 18 日记录](2026-10-18-source-vault.md)，四读及全部认证检查保持。

以上为下次继续时的建议，今天最终文档提交后停止，不创建提醒、自动化、新 chat 或后台开发，不 push。今日 macOS 验收 GUI 已正常退出；本轮收尾没有新启动 GUI / VM 或访问系统 key store，不以 10 月 24 日“九台 VM 全停”的历史结果冒充今日重新核对。忽略目录中的合成库、输入、导出、密文备份、app 包装、日志、字体探针、兼容探针、历史 M01 冻结件及各自测试 key 均保留；应用层来源删除不等于这些副本或凭据已删除。下次先读 Git 与当前状态，再按必要文档链推进，新增系统操作、清理和远程动作重新说明精确范围。

# RadishMemory MVP 路线图

## MVP 要证明的事情

MVP 不以功能数量为目标，而要证明一个可信闭环：

> 用户随手保存资料，经过大量历史后仍能可靠找回并引用来源；用户能纠正或忘记记忆；未授权资料不会被发送给云端；资料能通过用户自部署服务安全地到达第二台设备。

## 阶段 0：边界与评测基线

目标：实现前先确定能否客观判断记忆系统变好或变坏。

交付：

- 产品范围、架构、记忆模型和威胁模型评审；
- [M0 本地记忆闭环](../adr/0002-m0-local-memory-loop.md)及其[合成验收边界](../evaluation/m0-local-memory-loop.md)；
- 已冻结的 [M0 Canonical Schema](../schema/m0-canonical-schema.md)；
- 已冻结的 [M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md)；
- 至少覆盖事实、偏好、时间变化、冲突、敏感度和删除的本地评测集；
- 全历史输入、全文检索、向量检索和混合检索的基线方案；
- 已通过 [ADR 0003](../adr/0003-zero-knowledge-sync-first.md) 冻结的首个零知识同步信任模式；
- 已通过 [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md) 冻结的 RadishMind 可选 Gateway 首次接入阶段；
- 已通过 [ADR 0005](../adr/0005-m0-implementation-stack.md) 冻结的 Rust 模块化单体、SQLite / FTS5、依赖和验证基线。

停止线：不先选择复杂图数据库、不先制作虚拟形象、不声明新记忆算法。

阶段 0 的首个可执行验证是 M0：只用合成文本与 Markdown、本地全文基线和确定性 proposal / decision 流程证明来源、引用、时间更正、失败关闭和删除证据。它不要求向量、模型、RadishMind 或同步先存在。

## 阶段 1：单机资料库与可追溯问答

目标：完成单用户、单设备、本地优先的资料采集、检索、引用、导出与删除能力。

范围：

- 快速保存短语和 Markdown；
- 导入文本、PDF 和常见图片；
- 原始对象与派生文本分离保存；
- 全文检索和一个可替换向量索引；
- 按来源、时间、类型和敏感度过滤；
- 使用 mock 或一个直接模型 adapter 生成带引用回答，不以 RadishMind 为启动依赖；
- 显示每个回答使用的来源；
- 基础导出和本地删除。

验收：

- 原件可恢复、派生索引可删除并重建；
- 问答引用能定位到具体片段、页码或时间码；
- `local_only` 内容在云端调用前失败关闭；
- 无模型时仍能采集、浏览和搜索。

阶段 1 不把上述范围一次性展开为大批次。首个评审单元已通过 [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md) 冻结用户显式选择的 UTF-8 文本 / Markdown 文件入口、来源身份 / 版本、幂等导入、精确导出、派生重建、删除边界和 18 个合成验收场景；该入口已经通过 Linux、macOS、Windows locked CI 并合入稳定主线。下一评审单元由 [ADR 0007](../adr/0007-phase1-local-library-host.md) 冻结本地桌面宿主、一次性文件授权、application service、来源目录、基础 UI 和十二项宿主验收；P1-H02 至 P1-H05 已完成本机实现、三平台真实 picker、当前 `wgpu` locked CI 和 [third-party notices 与条件平台依赖](../implementation/phase1-third-party-notices.md)。阶段评审随后通过 [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md) 冻结受管原始对象认证加密、一 source version 一密文对象、设备本地 KEK 包装、文件系统 / SQLite 提交协调、v6 inline body migration、删除边界和十八项合成验收；`P1-S02 dependency and cipher review` 又由[专项评审](../implementation/phase1-encrypted-source-vault-dependency-review.md)选定 XChaCha20-Poly1305 + STREAM-BE32、AEAD DEK wrap、系统随机、secret zeroization 和 macOS Keychain / Windows Credential Manager / Linux Secret Service 精确 provider。[P1-S03a portable crypto dependency landing](../implementation/phase1-source-vault-portable-crypto.md)现已完成 manifest / lockfile / notices 变化、portable dependency graph、AAD codec、known-answer / tamper tests 和合成 random / key seam；[P1-S03b immutable object filesystem adapter](../implementation/phase1-source-vault-filesystem.md)现已实现 versioned envelope、应用专用 object / staging capability、durable no-overwrite publish、认证 read-back 与精确 attempt 检查，通过 macOS 本机合成验证及 Windows ARM64 / NTFS 提升权限、普通用户与 ACL 验收，文件身份替换缺陷已修复；Linux ARM64 / ext4 普通用户验收也已通过，对象 filesystem 范围收口；这些证据不代表所有文件系统、真实断电或产品加密链路已通过。[P1-S03c-2 独立 key provider](../implementation/phase1-source-vault-key-provider.md) 已完成依赖与实现落地，已有 macOS 构建、合成测试及最小真实 Keychain / GUI 正常链路证据；Windows / Linux ARM64 的真实 key store 与宿主正常链路已于 2026-10-24 通过[两轮验收](../evaluation/phase1-windows-linux-provider-gui-plan.md)，其余恢复 / 失败扩展仍须按具体范围批准；SQLite 正文迁移已按下述 P1-S04b 切片推进。PDF / 图片解析只能在 encrypted Source Vault 的 adapter、migration 和 host acceptance 完成后另行评审；向量实现和模型 adapter 继续分别审查隐私失败关闭和质量指标。

[P1-S04a 密钥初始化协调](../implementation/phase1-source-vault-key-bootstrap.md)已建立 SQLite 事务资格证明和 v7 维护 checkpoint；[P1-S04b](../implementation/phase1-source-vault-body-migration.md) 已完成维护专用正文迁移 / 恢复首个合成切片；[加密 capture 切片](../implementation/phase1-source-vault-capture.md) 又完成维护 v9 的新来源 / 版本提交、认证回读与恢复；[inventory reconciliation](../implementation/phase1-source-vault-reconciliation.md) 已完成独立核对、pending 保留和 committed staging link 清理；[显式放弃切片](../implementation/phase1-source-vault-abandonment.md) 已完成维护 v10 的放弃决定、精确 orphan retirement 与终态防重放；[verify / rebuild 切片](../implementation/phase1-source-vault-maintenance.md) 已完成独立对象认证与派生行修复；[删除执行切片](../implementation/phase1-source-vault-deletion.md) 已完成维护 v11 的请求冻结、精确清理及十组件结果；[历史请求兼容](../implementation/phase1-source-vault-legacy-deletion.md) 已完成维护 v12 的原请求接管与旧执行凭据验证；[读取切片](../implementation/phase1-source-vault-reader.md) 已完成独立认证读取、目录与检索适配；精确导出组合验收已通过；[application 加密用例](../implementation/phase1-source-vault-application.md) 已接入显式打开、准备 / 恢复及完整读取 / 导出会话，并已接入保留原请求的 import / update、lineage 删除及 verify / rebuild；[桌面宿主接线](../implementation/phase1-source-vault-host.md) 已实现 worker、恢复 UI 与日志抑制并具备合成测试；macOS 最小真实 Keychain / GUI 正常链路已在隔离合成库通过；搜索布局缺陷已修复，派生损坏重启恢复及密文损坏拒绝也已通过；2026-10-09 又完成权限故障下的原请求重试、重启精确放弃与原删除授权恢复，并完成外部 SQLite 持锁下的 GUI 失败恢复及测试 Keychain 项的只读核对。随后不同 bundle ID 的双 GUI 交替和精确导出通过；Prompt 首次误加的常许许可已按追加授权精确撤销，重新交接后真实 `KeyStoreCancelled` 空库打开失败关闭与原 A 读取对照通过。其它系统错误 / 操作阶段、提交 / evidence 中断窗口和集中跨平台检查按[当前状态](current.md)推进。该切片不改变默认桌面产品仍以 SQLite v6 inline plaintext body 运行的事实。

2026-10-13 已修复 busy 关闭竞争并合并显式加密打开 / 刷新的只读认证会话；macOS secret 查询切换 `SecItemCopyMatching` 后仍有八次提示。2026-10-18 的受限观测保留四读及全部认证检查，D02 确认入口第一读同一调用区间双提示并在第二提示拒绝后失败关闭，系统内部原因仍未知。M01 首次尝试在静态身份前置失败、目标查询为零；后续副本范围 v2 已合成验证并冻结，尚未执行真实 Keychain 访问，不能代表原 app 授权。2026-10-24 复核发现旧源码摘要绑定漂移，M01 暂缓；当前默认构建已通过[空库自然 busy 关闭与结果后退出](../implementation/phase1-source-vault-host.md#2026-10-24-空库锁等待下的原生关闭验收)。下一步按[当前后续事项](current.md#后续事项)补剩余只读会话窗口与跨平台错误场景。2026-10-28 已接入受审阅的[固定随包中文字体](../implementation/phase1-desktop-chinese-display.md)，本机 headless 字形 / UI 回归与默认构建通过，后续 macOS 可见中文显示及[有资料 Refresh 认证失败撤回 / 显式恢复](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-28-macos-有资料-refresh-原生结果)通过；最小窗口、Windows / Linux 新字体及中文检索的原生交互仍分别待验；Windows / Linux ARM64 的 10 月 24 日构建、合成及真实正常链路已完成，见[日终回顾](2026-10-24-source-vault.md)；精确同帧、末轮取消与其它有资料失败窗口不由本次通过替代，P1-S05 未关闭。10 月 18 日历史回顾见[日终记录](2026-10-18-source-vault.md)。

### 阶段 1 的用户路径与质量补齐

阶段 1 不以格式数量或模型数量证明完成。当前文本能力应先能通过 production 入口完成保存、中文找回、完整目录访问、回源、版本更新、导出与删除，并能在派生损坏后进入受限修复。具体计划见[本地资料库质量验收](../evaluation/phase1-local-library-quality.md)，这些场景尚未因文档加入而通过。

[ADR 0009](../adr/0009-versioned-chinese-recall.md)的版本化中文短语检索已接入默认与显式加密 production 入口，固定合成样本由 15/30 提升到 30/30，旧库显式升级与降级具备合成及真实旧 adapter 证据。有限长文测量显示空间与延迟增加；人工标签审阅、原生切换、跨平台与规模成本仍待完成，详见[落地记录](../implementation/phase1-chinese-recall.md)，不据此关闭完整 Q01。

扩大资料规模前先建立启动、搜索、导入、重建和内存基线；原始对象加密接入后分别报告对象文件、SQLite / FTS 正文与恢复能力。既有文件 / 宿主合成证据仍按各自实际场景解释，不替代新增质量验收。

第一次真实云端模型调用就必须满足[隐私模型](../privacy-threat-model.md#模型外发控制)要求的权限与外发过滤、最小 ContextPack、OutboundContextManifest、接收方 / 用途和实际 attempts 记录。阶段 3 扩展多模型及完整 Context Compiler；不能把最小外发清单推迟到阶段 3。此处澄清既有安全依赖，不授权当前接入模型。

围绕长期项目的窄文本价值路径可以作为阶段 1 至 3 的连续验证主线。提前实现阶段 2 / 3 能力或改变 PDF / 图片的既有依赖属于待决策事项，需在修改 ADR 0004、ADR 0008 或阶段顺位前确认范围。

## 阶段 2：长期记忆生命周期

目标：从“资料搜索”进入“长期记忆”。

范围：

- Observation、Claim、Episode、Preference 和 Procedure；
- MemoryProposal 审查收件箱；
- confirmed、rejected、superseded、contradicted、retracted、expired 状态；
- 时间有效性和 point-in-time 查询；
- 用户显式“记住”“更正”“忘记”；
- 背景去重与整理候选；
- 记忆污染和错误个人化评测。

验收：

- “以前喜欢 A、现在喜欢 B”不会被静默覆盖；
- 被拒绝候选不会用相同来源反复出现；
- 模型不能绕过 MemoryProposal 直接修改确认记忆；
- 当前回答能说明使用了事实、偏好还是推断。

## 阶段 3：多模型与 Context Compiler

目标：证明记忆与具体模型解耦。

范围：

- canonical ModelRequest / ModelResponse；
- Memory Compiler 与可解释 ContextPack；
- OutboundContextManifest；
- 至少两个云端 Provider 和一个本地模型；
- Provider 级外发策略、用量和错误记录；
- 在直接 adapter 基线成立后可选接入 RadishMind Gateway；
- 首次 RadishMind 接入不包含 Workflow、Tooling、RAG 数据 owner 或业务写回。

验收：

- 更换模型不修改记忆 canonical schema；
- 同一任务可以比较不同模型使用同一 ContextPack 的结果；
- 每次云端调用能列出实际使用的来源；
- Gateway、允许 Provider 集合和实际 Provider attempts 均进入外发清单；
- Provider 或 Gateway 失败不会隐式回退到不符合隐私策略的模型或另一条 adapter 路径；
- RadishMind 不可用时，本地能力与直接 adapter 基线不受影响。

## 阶段 4：用户自部署同步

目标：完成多端使用和用户数据主权的关键闭环。

2026-10-12 已确认[客户端连接规划](../client-server-connection-plan.md)：同一官方安装包运行时选择官方端点或用户自部署后端，无需重新编译。该目标纳入阶段 4，不提前进入当前本地桌面 / Source Vault 批次，也不改变首个零知识模式。

范围：

- 可自部署的零知识服务端，以及遵守相同兼容协议的官方同步端点交付；
- 设备身份、注册、撤销和恢复；
- 加密对象与操作日志同步；
- 冲突、离线写入和幂等；
- 第二台设备恢复资料、记忆状态和删除状态；
- 零知识同步服务及其可见元数据、客户端密钥和语义计算边界；
- 可信计算节点后置，不作为默认同步服务或首个同步批次的一部分。

验收：

- 同一未改动官方安装包完成官方 / 自部署选择、兼容检查、授权、同步、停止、重启与切换；两类路径分别按实际开放范围取得真实目标证据，官方未开放不额外阻塞兼容自部署验收；
- 实例 / 同步域的凭据、设备授权、密钥封装、cursor、删除与恢复证据不会串用，改地址不自动迁移或上传资料；
- 服务端关闭期间本地仍可采集和搜索，官方端点不可用不阻断兼容自部署路径；
- 两台设备并发修改不会静默丢失确认记忆；
- 被撤销设备不能获取后续数据；
- 删除状态可以跨设备传播并准确显示未完成节点；
- 备份恢复不会造成对象和记忆版本回退；
- 服务端存储、日志和传输载荷不包含内容明文、未包装内容密钥、语义索引或 ContextPack；
- 篡改、重放、缺失和未知协议版本不会被静默接受。

## 阶段 5：个人伴侣体验

目标：让同一记忆核心形成有连续感但不过度自作主张的个人伴侣。

范围：

- 连续对话和会话恢复；
- 可编辑 persona 与表达风格；
- 虚拟形象、语音和状态表现；
- 用户授权的主动提醒和回顾；
- “为什么这样回答”和“为什么记得”的可见解释；
- 所有产品面共用同一个 Memory Engine。

停止线：虚拟形象不能创建独立画像；主动行为不能绕过通知、权限和时间策略。

## 长期研究与连续性验证

[长期记忆研究专题](../long-term-memory-research.md)将跨会话找回、更正与历史解释、跨模型使用和受控删除映射到阶段 1 至 3，并将设备 / 恢复连续性归入阶段 4。参数个性化与推理时神经记忆是后置可选研究，不是 MVP 的新增依赖；只有现有基线与评测揭示明确缺口后，才另行评审实验范围。该专题不修改当前阶段顺位、schema 或权限契约。

## 首批评测指标

| 指标 | 说明 |
| --- | --- |
| Retrieval Recall@k | 正确来源是否进入候选集合 |
| Answer correctness | 答案是否被正确资料支持 |
| Citation accuracy | 引用是否指向真正支持答案的片段 |
| Temporal accuracy | 时间变化和历史状态是否正确 |
| Conflict handling | 冲突证据是否被识别而非静默覆盖 |
| Memory pollution rate | 错误、重复、无来源候选进入长期层的比例 |
| False personalization rate | 未经证据错误推断用户偏好的比例 |
| Context compression ratio | ContextPack 相对全历史的 Token 缩减 |
| Policy violation count | 未授权检索或外发次数，目标为零 |
| Delete completeness | 删除传播到各存储、索引和设备的比例 |
| Retrieval latency | 不同数据量下的本地与服务端延迟 |
| Provider cost | 不同编译策略的模型调用成本 |

## 可持续使用与交付验证

记忆控制台除污染、错误个人化和时间一致性外，记录候选接受 / 拒绝率、重复候选率、确认耗时及持续维护时间，避免用候选数量代替用户收益。检索同时覆盖中文 / 混合语言、高相似干扰和无答案场景；性能阈值须在代表性数据和环境下确定。

原件精确导出、完整资产迁移、备份恢复分别验收。迁移契约应明确来源与记忆关系、版本、决定、策略和必要证据；恢复方案应明确密钥保管、设备 / 系统重装和损坏后的真实结果。具体格式、恢复方式和所在交付批次仍待独立决策，不自动扩展阶段 1 加密范围。

外部试用与发行前须明确可用功能、已知限制、分发 / 自部署授权及维护范围；可公开阅读源码和允许用户部署是不同问题，现行许可证保持不变。

## 完整 MVP 首个可演示场景

以下场景属于完整 MVP，不是 M0 的首批代码范围。建议完整 MVP 的首个真实演示只覆盖一条高价值路径：

1. 用户保存一句灵感和一个相关 PDF；
2. 系统保留原件并生成可审查片段；
3. 在大量无关资料中询问此前想法；
4. 系统找回短语和 PDF，生成带引用回答；
5. 系统提出一个偏好或项目事实候选；
6. 用户确认、随后更正，再查询当前与历史状态；
7. 用户要求忘记，系统展示删除传播证据；
8. 第二台设备同步后不再召回该内容。

这条路径可靠之前，不以插件数量、模型数量、UI 页面数量或虚拟形象精度作为主要进度。
