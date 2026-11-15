# RadishMemory 当前状态

验收更新：2026-11-15，macOS 启动引导、加密库 201 条分页、明文 / 加密 760×520 组合筛选与版本对照、加密手输已通过选入的原生验收。固定中文语料的独立人工标签复核与双入口复验已完成。默认 v6 派生修复遇读锁失败和显式恢复已通过 macOS 原生验收。本批继续核对加密失败与待处理原请求，并确定性能预算。所有者于 2026-11-10 确认 macOS 文本资料库有限收口方案。

[专题地图](../topics.md#当前五项后续的任务与验收链)只路由职责与证据；实际执行顺序只见[后续事项](#后续事项)。历史接续与验证流水已移至[11 月 10 日归档](2026-11-10-planning-history.md)，11 月 8 日的提交与原生结果见[日终记录](2026-11-08-desktop-library.md)，不能将旧授权或旧环境状态当作当前事实。

## 当前阶段

`Phase 1 encrypted desktop integration; macOS minimal native loop verified; extended acceptance pending`

当前交付批次为 **macOS 文本资料库有限验收**，覆盖既有默认明文与显式加密对象两种入口的所选用户路径。技术批次、产品价值、完整 P1-S05、完整阶段 1 和发布分别判断。有限范围与退出条件统一见[质量验收](../evaluation/phase1-local-library-quality.md#finite-desktop-closeout)，原生执行结论按实际构建及场景记录。

M0、文本 / Markdown 文件入口和本地桌面宿主已建立；原始对象加密已完成独立 portable crypto 与 filesystem adapter 实现，并具备 macOS 本机、Windows ARM64 / NTFS、Linux ARM64 / ext4 的合成运行证据，现已接入显式 application 加密用例与桌面 worker / 恢复界面。启动引导按格式和迁移状态分流。默认桌面产品入口仍是 SQLite v6 inline plaintext body，对应普通产品正文路径。P1-S04a 的 v7 仅用于显式维护入口的密钥准备 checkpoint；P1-S04b 的 v8 承载正文迁移状态与已迁移对象库，后续 v9 供显式加密 capture 协调，v10 增加显式放弃与终态记录，v11 增加既有 DeleteRequest 下的对象删除执行 checkpoint，v12 保存历史正文缺失的原执行凭据引用；默认 `LocalLibrary` 入口拒绝 v7 / v8 / v9 / v10 / v11 / v12，显式加密入口只接受迁移完成的 v8 至 v12。项目具备受约束的工程原型。固定中文语料已完成独立人工复核和双入口复验。明文和显式加密库的分页与正文后段回源已有 macOS 原生证据。默认 v6 派生修复的正常路径和读锁失败恢复已有 macOS 常规窗口证据，跨平台仍待验。

P1-S03c-2 的独立 provider、macOS 构建与合成验证见[落地记录](../implementation/phase1-source-vault-key-provider.md)，三平台 locked CI 由下述 PR #4 复验；macOS 最小真实密钥库 / GUI 正常链路、受损库边界、权限故障下的三条原请求恢复路径及外部 SQLite 持锁下 GUI 失败恢复已验收；双 GUI 交替与精确导出也已通过。新签名副本首次误加的 Prompt 常许许可已按追加授权精确撤销；重新交接拒绝后，真实 `KeyStoreCancelled` 显式打开失败关闭通过。其它系统错误 / 操作阶段、失败窗口与平台仍待验收。P1-S03b 实现与证据见[filesystem adapter 落地记录](../implementation/phase1-source-vault-filesystem.md)；实现范围为独立 Source Vault package 与 Windows 文件身份 adapter，没有修复既有文本产品质量缺口。已确认问题、静态发现和待测风险见[2026-09-05 项目审阅](../implementation/2026-09-05-project-review.md)；截至 2026-09-03 的详细提交、三平台 CI、依赖数量与 M0 完成流水见[阶段基线归档](2026-09-03-baseline.md)。

## 能力与证据

| 能力 | 已成立 | 当前限制与真相源 |
| --- | --- | --- |
| M0 领域与存储 | canonical core、SQLite v6、来源 / 记忆 / 事件、FTS5、本地删除与真实 M0 runner 已经建立 | [ADR 0002](../adr/0002-m0-local-memory-loop.md)；runner 编排不等于 production 历史查询与上下文编译接口 |
| 字段与 fixture | M0 字段级 canonical schema 定义九种顶层对象；fixture 固定 12 个场景的 86 个有序操作和 12 个指标 gate | [M0 Canonical Schema](../schema/m0-canonical-schema.md)不绑定数据库、生产 ID 编码或语言类型；[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md)说明实际证据限制 |
| 文件入口 | `radishmemory-file-entry` 的 P1-I01 file snapshot contract、P1-I02 atomic source capture、P1-I03 exact export、P1-I04 lineage deletion 已落地 | [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md)；`SourceCaptureStore` 原子提交，`P1-F01` 至 `P1-F18` 已有三平台证据，故障 seam 仅 opt-in `acceptance-test-support`；不代表完整 importer / exporter 已实现 |
| 本地宿主 | P1-H02 application service、P1-H03 source catalog、P1-H04 desktop UI、P1-H05 host acceptance 已有实现及合成宿主证据 | [ADR 0007](../adr/0007-phase1-local-library-host.md)的 `P1-HF01` 至 `P1-HF12` 保留历史记录；[Q02 第 201 条](../evaluation/phase1-local-library-quality.md#2026-11-08-q02-桌面目录分页与页外选择)双入口合成回归已通过，macOS 明文库原生窗口已验证第二页与页外回源，并通过[系统文件选择器验收](../evaluation/phase1-local-library-quality.md#macos-原生页外文件更新与精确导出)和[页外文件删除重开](../evaluation/phase1-local-library-quality.md#macos-原生页外文件删除与重开)；[Q03 默认 v6 派生修复](../evaluation/phase1-local-library-quality.md#2026-11-08-q03-默认-v6-启动失败后的派生修复)与显式加密入口的正常修复均有 macOS 真实 GUI 证据。默认 v6 已补[读锁失败与恢复](../evaluation/phase1-local-library-quality.md#q03-plaintext-busy-native)，其它原生故障及跨平台仍待验 |
| 加密 Source Vault | P1-S01 storage contract、P1-S02 dependency and cipher review、P1-S03a portable crypto dependency landing 已完成对应范围；P1-S03b immutable object filesystem adapter 已通过 macOS 合成验证、Windows ARM64 / NTFS 提升权限与普通用户验收、Linux ARM64 / ext4 普通用户验收 | [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)的 `P1-SF01` 至 `P1-SF18` 尚未全部实现；object adapter 的 Windows 文件身份替换缺陷已修复并通过普通用户 / ACL 回归；P1-S03c-2 独立 key provider 已实现并有 macOS 构建与合成测试证据；P1-S04a 已建立密钥初始化 checkpoint；正文迁移 / 恢复、加密 capture、inventory reconciliation 及显式放弃 / 精确 orphan retirement 合成切片已落地；独立 object-backed verify / rebuild、删除执行及历史请求接管已实现；认证读取、目录 / 检索及精确导出组合验收已通过；显式 application 打开 / 恢复、读取、原请求 capture / 删除及派生维护已接入；桌面后台接线与原请求恢复已实现并通过合成测试；macOS 最小正常链路、受损库边界及权限故障下的原请求重试 / 精确放弃 / 原删除授权恢复已完成真实 Keychain / GUI 验收；外部 SQLite 持锁下 GUI 失败恢复、双 GUI 交替及精确导出已通过；误加的 Prompt 常许许可已精确撤销，真实系统交互返回 `KeyStoreCancelled` 的空库打开失败关闭通过；当前 Linux / Windows ARM64 的默认构建与常规合成测试已通过，Windows 另通过显式 reparse 与普通用户 ACL 验收；Windows / Linux ARM64 两轮真实 provider / GUI 正常链路、重登后五份精确导出与应用层删除均已通过；Linux 新账号失效 default 已按追加授权修复，重登后本批 collection 由用户解锁；其它系统错误 / 操作阶段继续待验 |
| 用户价值 | 桌面可直接保存纯文本和 Markdown，也可导入文件。已保存资料可搜索、查看原文、更正、查阅历史、精确导出及删除 | [手输资料规则](../adr/0010-phase1-desktop-text-entry.md)已有实现。明文手输有 macOS 常规窗口证据，[加密手输与双入口筛选 / 对照](../evaluation/phase1-local-library-quality.md#macos-text-workflows-760)有 macOS 760×520 原生证据。跨平台手输及明文 760×520 原生手输待验。尚无完整记忆控制台、模型问答、PDF / 图片解析、向量、同步、恢复或签名发行包 |

## 已确认待实施的客户端方向

2026-10-12 已确认[客户端连接规划](../client-server-connection-plan.md)：同一个官方安装包运行时支持官方零知识同步端点与用户自部署后端，无需重新编译。该目标归入阶段 4；本轮只更新产品和客户端文档，同步服务与服务器选择尚未交付，当前本地加密桌面集成顺位不变。

## 当前顺位

本节保留旧链接入口；唯一顺序见[后续事项](#后续事项)。已实现的桌面手输、筛选、对照、本地摘录式 mock 问答及跨窗口失效分别沿[质量证据](../evaluation/phase1-local-library-quality.md)核对；原接入进展与平台历史见[归档](2026-11-10-planning-history.md#当前顺位)。

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

以下是唯一执行顺序，实施与系统操作按当前任务范围授权，不沿用历史冻结包。原五项内容已按交付依赖重排；专题地图只引用本节。

1. **启动引导限定原生验收已通过。** 2026-11-15 的六个隔离窗口覆盖四类启动状态。LLDB 在唯一一次显式打开前的密钥观测点零命中，正对照只查询指定合成条目属性并返回 `KeyMissing`。详见[启动引导原生记录](../evaluation/phase1-local-library-quality.md#desktop-startup-native)。
2. **收口所选用户路径与质量。** [加密库 201 条分页](../evaluation/phase1-local-library-quality.md#macos-catalog-and-min-viewport)、[加密手输和双入口 760×520 筛选 / 对照](../evaluation/phase1-local-library-quality.md#macos-text-workflows-760)、[固定中文语料的独立人工复核与双入口对账](../evaluation/phase1-chinese-retrieval-baseline.md#q01-human-recheck)已通过所选范围。按[有限批次清单](../evaluation/phase1-local-library-quality.md#finite-desktop-closeout)继续适用的失败与恢复分支。[Q03 明文修复遇读锁失败与恢复](../evaluation/phase1-local-library-quality.md#q03-plaintext-busy-native)已通过。有资料的显式加密打开和刷新本轮均正常返回，[未观察到可安全交接的系统提示](../evaluation/phase1-local-library-quality.md#macos-populated-key-failure-attempt)。真实凭据失败后的关闭状态与正文撤回保留待验。待处理原请求的取消关闭、确认关闭和重开继续待验。影响本批的安全 / 一致性缺口仍阻塞对应能力，缺口归属见[Source Vault 整合清单](../evaluation/phase1-local-library-quality.md#source-vault-closeout)。
3. **确定本批性能边界。** 先固定代表资料分布、版本数、操作与可接受延迟 / 资源预算，再测量并决定是否需优化。100 来源成本与 201 条功能证据分开解释；不自动扩大 1000 / 10000、物理冷缓存或全平台矩阵。只针对影响本批的成本评审读取会话和重复认证组织，权限、完整性、时间和删除校验保持；详细规则见[规模基线](../evaluation/phase1-local-library-scale.md#finite-scale-review)。第 2 / 3 项可独立准备，性能预算未定不能宣称容量达标。
4. **技术出口后作价值复核。** 围绕一个长期项目，用合成 / 已明确授权的材料观察是否找得到、引用是否可回源、更正 / 删除后是否仍误用旧内容及维护负担；未经授权不使用真实个人资料。摘录式 mock 不代表真实模型问答。根据结果选择一个下一能力或修正已有路径，不自动扩展界面、模型或故障实验。平台扩展与完整 P1-S05 保持各自出口，不由本次价值复核代签。
5. **后置与暂停分别维护。** Windows / Linux 的新字体、中文格式和新增桌面流程按目标平台单独补验；支持范围不能从旧正常链路外推。M01 继续暂缓：先复核身份绑定、源码漂移与独立冻结范围，再按新精确操作单授权；不重签、不改 key / ACL、不跳过校验。无法安全构造的原生时序保留合成证据与未覆盖边界；若相关缺陷影响当前用户路径，转为对应阻塞。完整 P1-S05、PDF / 图片、模型外发、同步、恢复与发行沿原合同进入，不随有限批次放行。

本批完成只表示已选 macOS 范围的技术路径与质量标准成立；未通过项、真实用户价值、全平台、完整阶段和发布结果继续分别记录。没有用户价值结论时不以新增功能替代复核。
