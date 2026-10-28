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

2026-10-28 中文显示切片已提交为 `eab2b0f`：固定随包 Noto Sans SC Regular 2.004（7.95 MiB、OFL-1.1）、notices 摘要门禁和有资料失败撤回回归已接入，无 Cargo 依赖或 lockfile 变化；完整本机检查 403 项 Rust tests、fmt / Clippy 与默认构建通过，见[显示切片](../implementation/phase1-desktop-chinese-display.md)。独立授权后，macOS 单窗口已验证中文标题 / 混排输入 / 简繁中文结果可读，以及[有资料 Refresh 认证失败撤回与显式恢复](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-28-macos-有资料-refresh-原生结果)；两份精确导出、两次 Verify、本地 lineage 删除与正常退出 0 通过，测试 key 和复验材料保留。760×520 最小窗口未取得有效尺寸调整证据，Windows / Linux 新字体、Q01 中文检索、真实 key 首尾取消及完整 P1-S05 继续开放。

下一步先按 Q01 建立中文找回的 production 样本与命中 / 误召回基线，区分默认 v6 与显式加密入口；显示补验覆盖最小窗口及 Windows / Linux 当前字体构建。随后按 Q02 完整目录、Q06 回源与结果刷新推进；剩余原生失败窗口继续按精确操作单授权，不重启已完成的本批窗口。

2026-10-24 日终（历史）：当天提交回顾、代码与文档核对及环境交接见[日终记录](2026-10-24-source-vault.md)。Windows / Linux 正常链路已完成；当时安排下一次先核实中文字体方案并选一个失败窗口，现由上述切片推进。当天停止安排不产生自动化或后台授权。

2026-10-24 继续复核：工作区从干净的 `dev` / `5365e27` 开始，本地历史已包含 PR #5 合并，未重新读取远程检查。M01 v2 的普通文件门禁退出 3；逐项核对确认 v2 的 24 份冻结文件、旧 helper / 单次回执和 D01 产物保持，但 D01 清单绑定的 `key_read_observation.rs` 已由后续 `ab0da97` 增加平台条件编译，当前源码不再匹配旧摘要。没有改写冻结件或跳过校验，新 native 输出仍不存在；M01 暂缓，未来恢复须先解决源码证据绑定并重新审阅。既有空合成库只读核对通过；当前默认 desktop 已构建，持锁 helper 的 17 项合成回归通过，已准备绑定当前构建的[有限关闭验收](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-24-当前构建的关闭验收准备)。准备记录随后提交为 `b13c3b6`，项目所有者精确授权执行；空闲关闭、自然 busy 时拒绝关闭、锁错误交付保留和结果后正常退出通过。首轮在工具前置错误后无业务动作释放，第二轮取得实际证据，第三轮未使用；两个 GUI / 两个 helper 均退出 0，库状态和冻结输入保持，详见[原生结果](../implementation/phase1-source-vault-host.md#2026-10-24-空库锁等待下的原生关闭验收)。不关闭 P1-S05。

[10 月 18 日日终记录](2026-10-18-source-vault.md)汇总当天全部提交、受限观测代码 / 文档复核及各批证据。当前已确认：D01 成功路径保持 4 次 secret、12 次 attributes、25 次 User domain；D02 证明入口第一读同一 SecretSearch 区间双提示，第二提示拒绝后真实失败关闭。M01 首次执行在身份前置返回 `-67056`、目标查询全部为零；离线定位后，[副本范围 v2](../evaluation/phase1-keychain-prompt-diagnostic-plan.md#2026-10-18-m01-副本范围-v2-实现与重新冻结)已验证并冻结，新 native helper 未执行。原 app 授权、当前 partition / 已存要求和系统内部提示原因仍未知，历史第九提示未解释。

1. **M01 暂缓，保留失败门禁**：10 月 24 日已完成冻结件复核并定位源码摘要漂移；不直接执行原生 v2，也不通过改写历史冻结件恢复门禁。未来决定继续时，先审阅源码证据绑定与独立冻结范围，再按新版副本范围操作单另行精确授权；只允许既定唯一目标、两轮有界元数据观察、零 secret / 提示允许 / key 或 ACL 写入，不启动 GUI、不重签、不跳过校验、不重试。意外提示交接取消，拒绝 / 未知结构如实记未完成。副本匹配或不匹配均不能代表原 app 授权，不恢复旧因果判据；四读和全部认证检查保持。
2. **补剩余只读会话与宿主窗口**：当前构建的空库自然 busy 关闭、真实锁错误交付保留和结果后退出已通过，不重复列为未验。末轮系统取消、有资料旧视图撤回、pending 原请求 Cancel / Confirm 仍待原生验收；Windows / Linux 的正常显式 Refresh 已通过，取消后的会话复用窗口仍待验；精确同帧关闭竞争继续只引用 headless 回归。新增原生切片需单独确认目标、预计时长、停止和释放方式。按[有限验收检查点](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)选取切片，不把 headless / worker 回归、入口拒绝或提交 / evidence 回调错误当作其它原生窗口通过。
3. **两平台真实 provider / GUI 正常链路已通过，扩展错误窗口待定**：Linux / Windows ARM64 的 10 月 24 日构建、合成检查及各两轮普通用户 GUI 已完成；重登、五份精确导出、两条 lineage 删除与零 pending 有证据，完整清单确认当时 VM 全停，见[执行记录](../evaluation/phase1-windows-linux-provider-gui-plan.md)。不重复执行正常链路；新字体的两平台构建 / 可见验收及平台系统错误 / 操作阶段仍另定范围。Linux 验收门禁拒绝 locked 状态不替代产品 locked 失败窗口；无法安全构造的错误保留待验，不删除唯一 key、锁定整个用户 keychain 或不受控结束进程。
4. **保留独立质量和契约事项**：R01 至 R06、中文显示、默认 v6 failed-open 继续按质量计划推进；`evidence_digest` 复算及历史兼容先做契约评审。已通过的权限故障原请求恢复、外部锁失败恢复、双 GUI 交替与空库取消不重复列为未完成任务。其它系统错误 / 写操作阶段另定范围，P1-S05 不整体关闭。

10 月 18 日日终停止安排保留为历史记录；10 月 24 日已按项目所有者要求完成有限验收准备和授权执行，不安排提醒、自动化或后台开发，不 push。忽略目录内的合成库、导出、备份、包装 / dSYM、原始证据、唯一既有测试 key、旧 M01 与 v2 冻结件继续保留，不纳入 Git，也不自动清理。历史 GUI / sample / helper 均已退出；本日 macOS 两个 GUI 和两个持锁 helper 均正常退出，第三轮锁未使用；Windows / Linux 后续四个 GUI 也已退出，测试账号已注销，VM 全停；v2 原生进程仍未启动。新增系统操作、清理或远程动作仍须精确授权；[10 月 13 日](2026-10-13-source-vault.md)及更早日终安排保留为历史记录。
