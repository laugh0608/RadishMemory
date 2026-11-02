# RadishMemory 当前状态

导航整理：2026-11-01；最近新增证据：2026-11-02 的 Q01 标签静态复核、Q04 局部编码优化及 100 来源 / 多版本有限 release 基线，未新增原生或跨平台证据。

从[专题地图](../topics.md#当前五项后续的任务与验收链)进入当前任务的主要归属、原实施切片与验收来源。近期动作以“后续事项”五项顺序为准；“当前顺位”保留既有接入进展与剩余范围，不另定一套任务顺序。

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

原逐批命令、数量、提交、平台准备与运行 / 清理事实完整保留于[整理前验证流水](planning-before-alignment-2026-11-01.md#当前验证入口)，按其链接进入原证据。旧批次“未执行”与后续新增结果按日期区分，不将历史记录当今天的运行状态；本批文档验证另见[整理记录](2026-11-01-planning-alignment.md)。

| 范围 | 标准与实际结果入口 | 结论限制 |
| --- | --- | --- |
| M0 / 文件 / 宿主 | 原 ADR、[fixture](../evaluation/m0-fixture-contract.md)、[能力地图](../topics.md#子专题主要归属)与原平台记录 | 固定场景、runner 与真实产品入口分列，不扩大既有验收 |
| Source Vault 与 application / host | [宿主记录](../implementation/phase1-source-vault-host.md)、[原生验收](../evaluation/phase1-source-vault-native-acceptance.md) | 默认 v6、维护 v7 与已迁移 v8–v12 分开；FTS 明文、evidence_digest、原请求关闭及其它错误窗口仍按原边界 |
| Windows / Linux 正常链路 | [集中平台记录](../evaluation/phase1-windows-linux-provider-gui-plan.md)及其原输入 / 构建记录 | 10 月 24 日两轮正常链路不代替新字体、中文格式切换或全部失败场景；不把历史 VM 全停当实时状态 |
| 中文检索、显示与质量 | [检索修复](../implementation/phase1-chinese-recall.md)、[中文显示](../implementation/phase1-desktop-chinese-display.md)、[质量标准](../evaluation/phase1-local-library-quality.md) | 标签、性能、原生窗口和跨平台分别验收；Q01 / Q04 / P1-S05 继续开放 |
| Keychain 有限诊断 | [诊断计划](../evaluation/phase1-keychain-prompt-diagnostic-plan.md)与原日期记录 | D01 / D02 观测不解释系统内部原因，M01 保持暂缓，旧冻结件不改写 |

当前维护与读取入口接受已完成迁移的 v8 至 v12，正文迁移入口仍仅接受 v7 / v8，普通产品入口仍为 v6。删除中的对象与历史正文缺失凭据须复验，verify / rebuild / reconciliation 不自动推进删除；详见[历史请求兼容](../implementation/phase1-source-vault-legacy-deletion.md)。数据库身份检查已修复 Unix 额外文件描述符关闭导致的 POSIX 锁释放，并分离单对象与数据库大小限制；[读取记录](../implementation/phase1-source-vault-reader.md)保存跨进程证据。每次读取的全库认证成本、持锁期间的操作串行化及产品 R01 至 R06 缺口尚需后续处理。

macOS / Linux：

```bash
./scripts/check-repo.sh
./scripts/check-m0-fixtures.py
```

Windows：

```powershell
pwsh ./scripts/check-repo.ps1
```

这些是既有检查入口，不授权安装、联网、GUI、VM、真实 key store 或重跑旧操作包；按当前任务的明确范围执行。

## 后续事项

2026-11-02 100 来源切片：在 `e0b9926` 之上新增固定 100 个 lineage / 160 个正文版本、25 长 / 75 短来源的合成 release 基线。默认与显式加密入口的导入 / 更新、重开、目录 / 版本、7 类查询、Verify / Rebuild 及三阶段版本 / 正文断言均通过；两轮加密中文 5 命中 p50 为 1,201.358 / 1,201.121 ms，仍为每次 13 轮全库对象认证。该档位不能外推生产容量，1000 / 10000、物理冷缓存、真实 provider 和 GUI 等仍未验。输入、完整数据和口径见[100 来源基线](../evaluation/phase1-local-library-scale.md)，验证与交接范围见[本批记录](2026-11-02-q04-hundred.md)。本批只新增 / 复用 test-only 验收代码，没有修改生产逻辑、schema、依赖或 lockfile。

2026-11-02 后续切片：基线 20 个文件已按项目所有者要求提交为 `2156759`，随后新增字段重算 / FTS 内部核对子计时，并完成字节等价的局部编码优化。两组同条件 release 合成对照中，新格式加密中文 5 命中 p50 为 509.168 → 468.397 ms、502.620 → 454.724 ms，约改善 8.0% / 9.5%；字段重算约 52 → 10 ms，认证 / 完整性检查次数和索引空间保持。此为固定 32 份单版本资料的局部改进，不能外推生产容量；详细实现与完整样本见[局部优化](../implementation/phase1-chinese-recall.md#2026-11-02-派生字段编码局部优化)，验证与交接范围见[后续记录](2026-11-02-derived-encoding.md)。

2026-11-02：已逐项静态复核 12 份资料 / 24 个查询，未改变候选标签；独立人工审阅仍待完成。沿既有 `q01_cost` 加入 opt-in 分阶段计量，两轮独立 release 进程中，新格式加密中文 5 命中中位分别为 504.053 / 500.386 ms，每次有 13 轮全库对象认证，FTS MATCH 约 0.4 ms；默认构建不采集计时。此为基线，不是性能优化，也不是生产容量或完整 Q04 验收；不能与旧 debug 约 4.68 秒作直接改进对比。详细输入、阶段归属、资源范围与下一切片见[读取成本](../implementation/phase1-chinese-recall.md#2026-11-02-分阶段读取成本)，实际验证与交接见[本批记录](2026-11-02-read-cost.md)。

2026-10-28 日终：当天四个实现 / 证据提交、相关文档复核与保留范围见[今日日终记录](2026-10-28-chinese-recall.md)。`eab2b0f` / `3152f2c` 已接入固定随包中文字体，并通过 macOS 已观察窗口的中文可读显示与有资料 Refresh 认证失败撤回 / 显式恢复；760×520 最小窗口和 Windows / Linux 新字体仍待验。`f2f8e10` 保存旧检索 15/30 的历史基线；`4c9b0be` 的[生产修复](../implementation/phase1-chinese-recall.md)在相同标签下双入口达到 30/30、24/24 集合匹配、0 误召回，并提供显式升级 / 降级与旧客户端拒绝。完整检查 427 项测试通过；新切换 UI 的原生与跨平台证据尚未取得，Q01 / Q04 / P1-S05 均保持开放。

1. **Q01 独立人工复核与 Q04 后续规模评审**：标签[逐项静态审阅表](../evaluation/phase1-chinese-retrieval-baseline.md#2026-11-02-标签静态审阅)已就绪，不能把 AI 复核当独立人工标注，也不能为迁就算法改写标签。100 来源 / 部分多版本的固定档位已有有限证据，1000 / 10000、物理冷缓存、其它分布和平台仍待验；下一档先明确数据、操作与总运行预算，不把小样本外推生产容量。主要剩余成本包含对象认证 / IO 与 canonical 复验，多结果回源的重复认证组织另做边界评审，不省略权限、完整性、时间或删除校验。生产延迟 / 容量和 CI 时间预算尚未冻结；第 2 项可见验收可继续按独立具体范围准备。
2. **补新格式的有限可见验收**：围绕当前提交准备独立合成旧格式库，覆盖升级确认 Cancel / Confirm、真实中文命中、关闭重开、显式降级与提示、精确导出和 760×520 最小窗口。新库现在默认使用新格式，不能把旧操作单的“新空库”当成旧格式 fixture；不得改写或复用旧冻结摘要。真实 GUI / key store 执行须另行说明目标、时长、停止与保留范围并取得授权。Windows / Linux 先按最终输入做构建 / 合成，再补字体和切换 UI 可见证据；VM 启动前核对完整清单，始终串行。
3. **继续文本资料库质量闭环**：随后推进 Q02 第 201 条及完整目录访问、Q06 正文后段命中定位与更新 / 删除后的结果刷新；默认 v6 failed-open 派生维护入口保持 Q03 独立事项。已经通过的 macOS 有资料 Refresh 认证失败、空库自然 busy 关闭、双 GUI 交替和 Windows / Linux 正常链路不重复列为未验；独立标签、性能、原生窗口分别验收。
4. **其余失败窗口与契约保持独立**：真实 key 首尾取消、其它有资料操作失败、pending 原请求关闭 Cancel / Confirm、跨平台错误窗口仍按[有限检查点](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)另定范围。精确同帧关闭只引用 headless 回归，不能由正常原生退出替代。`evidence_digest` 复算及历史兼容先做契约评审，完整 P1-S05 与 PDF / 图片停止线保持。
5. **M01 继续暂缓**：10 月 24 日确认的源码摘要漂移与旧 v2 冻结件保留，详见[当日日终记录](2026-10-24-source-vault.md)及[副本范围 v2](../evaluation/phase1-keychain-prompt-diagnostic-plan.md#2026-10-18-m01-副本范围-v2-实现与重新冻结)。未来先审阅绑定和独立冻结范围，再按新版操作单精确授权；不重签、不跳过校验、不改 key / ACL、不把副本匹配当作原 app 授权。D01 / D02 观测与未解释提示仍见[10 月 18 日记录](2026-10-18-source-vault.md)，四读及全部认证检查保持。

2026-10-28 历史收尾：当时最终文档提交后停止，未创建提醒、自动化、新 chat 或后台开发，未 push。当天 macOS 验收 GUI 已正常退出；当时收尾没有新启动 GUI / VM 或访问系统 key store，不以 10 月 24 日“九台 VM 全停”的历史结果冒充重新核对。历史忽略目录中的合成库、输入、导出、密文备份、app 包装、日志、字体探针、兼容探针、历史 M01 冻结件及各自测试 key 均保留；应用层来源删除不等于这些副本或凭据已删除。本次 11 月 2 日工作也未启动 GUI / VM、访问真实 key store 或执行远程动作；本批临时 fixture 由各测试清理，计量日志保留；基线与局部优化分别提交为 2156759 / e0b9926；100 来源切片随本次提交收口，均未 push。下次先读 Git 与当前状态，再按必要文档链推进，新增系统操作、清理和远程动作重新说明精确范围。
