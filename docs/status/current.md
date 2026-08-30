# RadishMemory 当前状态

更新时间：2026-08-30

## 当前阶段

`M0 merged baseline; Phase 1 P1-F01 through P1-F18 verified locally`

产品、记忆、隐私、评测、同步信任、RadishMind 接入和 M0 实现栈基线已经冻结；最小 Rust workspace、canonical core、SQLite v6 事实存储、FTS5 派生索引、当前状态投影、本地删除闭环与真实 M0 runner 已经建立。本机已由冻结 fixture 实际通过 12 个场景、86 个有序操作和 12 个指标 gate；[PR #1](https://github.com/laugh0608/RadishMemory/pull/1) 的最终 head `6df0891` 已在 [workflow run 32979128488](https://github.com/laugh0608/RadishMemory/actions/runs/32979128488) 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`，随后以 merge commit `fe8186a` 合入 `master` 并 fast-forward 回流到 `dev`。M0 当前作为已合并基线保留；阶段 1 首个真实文本 / Markdown 文件入口的行为与 18 个合成场景已由 ADR 0006 冻结，P1-I01 已建立本地文件快照，P1-I02 已在本机实现 application-level atomic capture，P1-I03 已实现 exact export 与原子不覆盖发布，P1-I04 已复用 canonical 删除语义收口整个来源 lineage、入口状态与派生闭包，`P1-F01` 至 `P1-F18` 已在本机跨 file-entry / SQLite 边界运行通过。下一目标是在独立授权下形成可审阅提交并运行 Linux / macOS / Windows Phase 1 locked CI；远程证据、production host / UI 与平台 bookmark 成立前，不直接扩大到 PDF、向量、模型、UI、网络与同步，也不把本机合成验收表述为可导入真实个人资料的产品入口。

## 已冻结的范围与基线

`M0 Local Memory Loop` 已通过 [ADR 0002](../adr/0002-m0-local-memory-loop.md) 冻结为单用户、单设备、本地、合成文本 / Markdown、无模型和无网络的 M0 代码范围。它按固定场景验证来源、引用、proposal / decision、时间更正、失败关闭和单设备删除证据，不包含 PDF、向量、Provider、RadishMind 或同步实现。

M0 字段级 canonical schema 已在 [M0 Canonical Schema](../schema/m0-canonical-schema.md) 冻结为九种顶层对象及共同逻辑类型。它确定字段、必填性、条件约束、时间、治理标签、事件和删除证据关系，但不绑定数据库、生产 ID 编码或语言类型。

[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 已冻结合成 JSON mapping、fixture ID、摘要 profile、12 个场景的 86 个有序操作和 12 个指标 gate。仓库校验器仍只验证输入与 oracle 自洽；`radishmemory-m0` 现在会把同一 suite 映射到真实 core / SQLite 操作并独立计算 assertion 与 metric。

首个多设备同步信任模式已通过 [ADR 0003](../adr/0003-zero-knowledge-sync-first.md) 冻结为零知识同步服务：默认自部署服务端只中继密文对象、加密操作日志、密钥封装和已枚举最小元数据，不持有内容解密能力，也不运行语义索引、检索或 ContextPack 编译。可信计算节点后置为显式可选能力。该决定不把同步加入 M0，也不代表零知识同步已经实现。

RadishMind 首次运行接入已通过 [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md) 冻结在完整 MVP 阶段 3：只在 mock 或直接 adapter 基线成立后，以显式可关闭的 Model Gateway 接入。M0、阶段 1 单机资料库和阶段 2 记忆生命周期均不依赖它；首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回，也不复制兄弟项目业务 schema。

M0 实现栈已通过 [ADR 0005](../adr/0005-m0-implementation-stack.md) 冻结为 Rust 2024 模块化单体：`radishmemory-core`、`radishmemory-sqlite` 和 `radishmemory-m0` 三个 package，首个工具链固定为 Rust `1.96.0`，本地存储采用 bundled SQLite 与 FTS5。M0 不包含网络、异步运行时、ORM、模型 SDK 或静态加密，也不冻结未来 UI 和同步服务端语言。

阶段 1 首个真实文件入口已通过 [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md) 冻结为用户显式选择、允许根内、最大 8 MiB 的 UTF-8 `.txt` / `.md`。它复用现有 SourceArtifact / SourceFragment、FTS5、citation 与删除语义，冻结路径、symlink / hardlink、来源 lineage / 幂等 / 版本、精确导出、受管副本所有权、删除 / rebuild 和 18 个合成场景；它不把 M0 fixture operation 变为 production API，也不代表完整 importer / exporter 已实现。

`M0-I01` 已建立且仅建立上述三个可编译 package，提交初始第一方 lockfile，并把 fmt、Clippy 和 locked test 接入本地双平台入口与 PR 的 Linux / macOS / Windows matrix。

`M0-I02` 的第一个独立评审单元已实现稳定 core 错误、RFC 3339 / UTC 时间、`ValidTime`、`exact-bytes-v1` / `utf8-nfc-text-v1` / `canonical-json-v1` SHA-256，以及拒绝重复 key 和 `null` 的 `radishmemory-canonical-json-v1` parser / writer。`M0-I02` 的第二个独立评审单元已实现九种 canonical 顶层对象、共同值类型与字段级条件校验。`M0-I02` 的第三个独立评审单元已实现跨对象不变量，包括同 namespace 引用、来源切片、governance 继承、accept 物化闭环、事件链投影、supersede、ContextPack citation / 召回边界与删除计划 / 证据一致性。fixture 外部 JSON mapping 与 application operation dispatch 已由 `M0-I04` 在 runner 层实现，不反向进入 core schema。

`M0-I03 SQLite entry` 已实现版本化 migration、连接安全基线、bundled SQLite 精确版本与 FTS5 双重 capability probe，以及对未知较新版本、现有外来 schema 和 migration metadata 漂移的失败关闭。当前 lockfile 包含五个经审查的直接第三方依赖和 40 个第三方 package，没有 Git dependency 或产品网络能力；`libsqlite3-sys` build script 会通过 `cc` 编译 bundled SQLite `3.53.2` C 源码。feature、许可证与 native build 影响见 [M0 Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)。该入口提交本身没有为未来假设新增空 core port；真实 port 随下述 storage 操作进入。

`M0-I03 SQLite storage` 的首个纵向切片已实现 SourceArtifact / SourceFragment 所需的真实 `SourceVault` core port，并把 adapter schema 单调升级到 v2。来源 metadata 与 exact body BLOB 分表原子写入；片段只保存 byte range、摘要、治理和生成器 metadata，读取时从已验真的 source body 重建 exact content，不建立第二份片段正文真相。写入保持不可变，namespace 读取失败关闭，重复对象不覆盖，批量片段冲突整批回滚，损坏或非 UTF-8 body 不进入领域对象。

`M0-I03 SQLite storage` 的第二个纵向切片已实现 MemoryProposal / MemoryDecision / MemoryRecord / MemoryStateEvent 所需的真实 `MemoryStore` core port，并把 adapter schema 单调升级到 v3。proposal 在写入和读取时解析完整 SourceFragment / SourceArtifact 闭包，相同 namespace、operation、内容摘要、来源集合与目标集合的候选不会重复落库；decision 以不可变、无分叉链追加，accept / reject 终态不能被覆盖。accept materialization 在同一事务创建不可变 record facts、初始 confirmed event，并在 supersede 时同时追加旧记录关闭事件；record 表不保存可变 `current_state` 或 `last_state_event_id`，读取时从已验真的事件链重建。

`M0-I03 SQLite storage` 的第三个纵向切片已实现真实 `LocalSearch` core port，并把 adapter schema 单调升级到 v4。FTS5 只保存 active SourceFragment 与 confirmed MemoryRecord 的派生正文，当前状态表只物化事件链 tip；来源 / 记忆事实、状态事件、索引和投影在同一 `IMMEDIATE` transaction 更新。检索先形成 namespace、敏感度、删除、状态、保留期与 `as_of` 资格集合，再执行本地 FTS5 top-k；普通查询不会返回 proposal、非 active 来源或非 confirmed 记忆。每次打开和搜索都会对事实全量复算并核对派生数据，显式重建也只从已验真的 canonical facts 生成；v1 / v2 / v3 升级会在 migration transaction 内完成首次重建。

`M0-I03 SQLite storage` 的第四个纵向切片已实现真实 `DeletionStore` core port，并把 adapter schema 单调升级到 v5。`DeleteRequest`、十项冻结组件、canonical 目标闭包、adapter 内部物理执行闭包、逐次执行结果和 `DeletionEvidence` 都以不可变或追加式关系持久化；计划入库与语义目标进入 `pending`、FTS / 当前投影移除位于同一 `IMMEDIATE` transaction。执行器真实处理 source body / metadata / fragment、proposal / decision / record / state event、FTS、M0 不持久化的 context cache 与最小审计；失败组件保留真实 error / retryable 结果并把目标收口为 `failed`，不会恢复 active，也不会把部分失败写成 completed。普通读取、检索和派生重建只接受 active facts；完整成功、单组件失败、幂等重试、未展开依赖拒绝、namespace 隔离、证据链和 v4 → v5 升级已由测试覆盖。该能力只证明应用可复验的本地行、索引、投影和 cache 边界，不证明 SQLite 空闲页、临时文件、备份、文件系统快照或介质已被取证级擦除。

`M0-I04 fixture runner` 已实现冻结 suite 摘要与向量复验、每场景独立内存 SQLite、logical key → canonical fixture ID registry、86 个操作的显式 dispatch、确定性本地词项扩展、point-in-time 事件投影、ContextPack citation 解析、删除失败注入、实际 metric 聚合和最小 JSON 证据。内存数据库入口与失败注入只存在于显式启用的第一方 `fixture-runner` feature：前者仍执行 production capability、migration、连接策略和 adapter 操作，但不持久化文件；后者不改变 production `DeletionStore` port。runner 输出不包含 fixture 正文、临时数据库路径或已删除内容；未知 operation、assertion、metric 与摘要漂移均失败关闭并保留稳定 scenario / step 上下文。

`P1-I01 file snapshot contract` 已新增第一方 `radishmemory-file-entry` package，且只依赖 `radishmemory-core`。它实现 `radishmemory.phase1-file-entry/1`、8 MiB 上限、允许根、root 以下 symlink 拒绝、hardlink 普通读取、`.txt` / `.md` 分类、严格 UTF-8 / NUL 拒绝、读取前后可观察文件事实复核、path-free snapshot、稳定脱敏错误和最小 capture receipt。该单元当时不实现 export / deletion；package 至今仍不依赖 SQLite、不持久化 canonical objects，也没有新增第三方 package、native build 或网络能力。

`P1-I02 atomic source capture` 已在 core 增加完整 fragment-set 校验、`SourceCapture` / `SourceCaptureResult` 与最小 `SourceCaptureStore` port；file-entry 用 `FileCapturePlan` 把 snapshot 映射为整文件单片段 canonical candidate，SQLite schema 单调升级到 v6，并在一个 `IMMEDIATE` transaction 内提交 SourceArtifact metadata / exact body、完整 fragment、FTS、opaque origin binding、lineage tip 与最小 audit。重复 exact bytes 返回既有事实，内容变化只允许下一 version / 精确前 tip，旧版本保留但退出普通 FTS；旧 `SourceVault` 两步写入口拒绝显式用户输入。

`P1-I03 exact export` 已在 file-entry 增加显式绝对目标与 export allowed roots、path-free receipt、目标 parent 的 symlink / root 复核、目标不存在检查、任务专用同目录临时文件和无覆盖发布。调用方先以 namespace 与精确 `source_id` 从 `SourceVault` 取得 active 或历史可读的已验真 `SourceArtifact`；file-entry 再复验 deletion state、正文长度、原始字节与 `exact-bytes-v1` 摘要，写入 flush / sync / close 后重新逐字节复验，以同目录 `hard_link` 原子建立目标目录项，复验发布结果后只清理自身临时文件。目标存在、目标 / parent symlink、临时写入或并发发布失败不会覆盖其它目标，也不会修改 Source Vault。

`P1-I04 lineage deletion` 没有新增 schema v7、core object 或平行删除协议，而是继续使用 canonical `DeleteRequest` / `DeletionEvidence` 与 SQLite v6 `DeletionStore`。一个请求包含任一文件 SourceArtifact 时必须精确包含同 namespace、同 lineage 的全部 active 版本，并继续展开引用任一版本的 active memory 依赖；缺一版本或依赖整笔失败且不改变 store。计划提交原子地把全部来源、fragments、proposals 与显式 memories 置为 pending，移除 FTS、当前投影和 lineage tip；执行阶段处理 body、fragment、metadata、origin binding 与 capture audit，并由既有十组件结果和 evidence 报告真实完成 / 失败。rebuild 在修改派生表前复验每个 active 文件来源的 body、完整 fragment 集、capture audit 与 binding，缺失 canonical fragment 失败关闭；pending / failed / deleted lineage 不复活，origin file 与用户导出不进入闭包。

## 当前顺位

1. 复核本轮 `P1-F15` 至 `P1-F18` 的实现、文档与本地聚合证据；获得明确授权后才精确暂存并提交，不把本地未提交工作误写为已合并状态。
2. 获得独立远端授权后，按 `dev -> master` 阶段稳定化拓扑运行 Linux / macOS / Windows locked CI；远程证据成立前不把 M0 三平台结果外推到 Phase 1。
3. 三平台通过并完成稳定化后，再独立评审 production host / UI 与平台 bookmark；合成路径入口不自动升级为可处理真实个人资料的产品授权面。
4. PDF / 图片、加密内容寻址大对象存储、向量索引、模型 adapter 与 UI 分别进入后续独立评审单元，不并入首个文件入口 closure。

## 下一事项（2026-08-30）

主任务已经完成阶段 1 首个真实文件入口的本机跨层 acceptance closure，下一步停在本地证据边界：

1. 由项目所有者审阅当前 diff；明确授权后按主题精确暂存和提交当前 `dev` 工作，不混入范围外文件。
2. 若授权远端动作，先 push `dev`，再从 `dev` 向 `master` 发起阶段稳定化 PR，以真实 `Candidate Quality` 取得 Repo Hygiene 与 Linux / macOS / Windows Phase 1 证据。
3. CI 全部通过后再决定 merge commit 与 `master -> dev` 回流；失败时修复真实跨平台问题，不放宽检查、删除场景或增加 fallback。
4. production host / UI、平台 bookmark、真实个人资料授权面与后续 PDF / 图片单元继续保持独立，不因本机 18 场景通过而默认进入。

下一事项停止线：远程 Phase 1 三平台证据与 production host / UI、平台 bookmark 评审成立前，不导入真实个人资料，不声明产品文件入口完成；不进入 PDF / OCR、Embedding、模型、UI、网络、同步或通用 workflow engine；未经当前任务明确授权，不暂存、不提交、不 push、不创建 PR、不改变远端状态。

## 本轮阶段 1 完成（2026-08-28 至 2026-08-30）

1. [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md) 已把首个真实入口冻结为显式选择、非空允许根、root 以下 symlink 拒绝、hardlink provenance 独立、严格 UTF-8、8 MiB 上限和 `.txt` / `.md` 两种类型；目录、递归、URI、PDF、图片、网络和模型均不进入该切片。
2. 文件路径、inode 和内容摘要均不充当 canonical identity；同 origin binding / 同字节导入幂等，内容变化建立不可变 SourceArtifact 新版本，普通召回只使用 active lineage tip，不为文件系统新增第二套对象。
3. Source Vault 受管 exact bytes 在成功后承担原始真相；精确导出不覆盖目标，删除默认覆盖整个来源 lineage 的本地受管闭包，但不修改或声称删除外部原件、hardlink alias、用户导出、备份或其它设备。
4. `P1-F01` 至 `P1-F18` 已冻结首次导入、字节保真、幂等、版本、hardlink、search / citation、export、rebuild、delete、路径 / symlink / 类型 / 大小拒绝、并发变化、故障原子性、无网络与诊断脱敏验收；ADR 冻结当时只实现其中的本机子集，完整场景后来由本列表第 44 至 50 项收口，但仍不是已通过三平台或 production host 的产品能力。
5. ADR 0006 文档、专题同步与一致性守护已提交为 `6b0e7ab`；未 push、未创建 PR，也未改变其它远程状态。
6. P1-I01 已建立第四个第一方 workspace package `radishmemory-file-entry`，实现 validated file snapshot、最小 receipt 与 14 个冻结错误 reason；文件路径和正文不进入 Debug / Display。
7. 8 个合成临时文件测试覆盖 exact bytes / BOM / CRLF / Unicode、允许根、symlink、hardlink、类型、空文件、UTF-8、NUL、8 MiB 边界、receipt 与错误脱敏；当前只证明 file snapshot contract，不证明 SourceArtifact 持久化或完整 `P1-F01` 至 `P1-F18`。
8. P1-I01 当时只让 `Cargo.lock` 新增第一方 `radishmemory-file-entry 0.1.0`，40 个第三方 package、feature、checksum、native build 和网络能力未变化；该快照单元本身没有数据库 migration 或真实个人资料。
9. core 新增且仅新增 atomic capture 真实需要的完整 fragment-set 校验、`SourceCapture`、path-free result 与 `SourceCaptureStore`；file-entry 增加 snapshot → whole-file fragment candidate mapping，仍只有 core runtime dependency。
10. SQLite schema v6 新增三张 STRICT 表，分别承载可重建 lineage tip、path-free origin binding 和最小 capture audit；SourceArtifact、body、fragment、FTS、binding、tip 与 audit 在单个 `IMMEDIATE` transaction 内完成。
11. 同 binding / exact bytes / governance 幂等返回既有 source / lineage / version，不增加事实或索引；内容变化必须严格下一 version 并精确 supersede 前 tip，历史事实仍可按 source ID 读取，但普通检索只看到当前 tip。
12. 6 个跨 package 合成测试覆盖 `P1-F01`、`P1-F03`、`P1-F04`、`P1-F06` 的 capture / recall 部分、path-like binding 拒绝、派生漂移失败关闭，以及 fragment 冲突发生在 source / tip / FTS 更新后的事务整批回滚；失败后旧 tip、旧 FTS、binding 与 audit 均保持原值。
13. P1-I02 没有新增第三方 package、feature、native build、网络能力或真实个人资料；SQLite 仅增加 first-party file-entry test dependency，production dependency 方向保持隔离。
14. P1-I01 / P1-I02 已提交为 `a79514b`；提交前本机 `./scripts/check-repo.sh` 通过 107 个仓库文件检查、workspace format、Clippy `-D warnings` 与全部 locked test。未 push、未创建 PR，尚未运行 Phase 1 的 Linux / macOS / Windows 远程 CI。
15. P1-I03 已新增 `FileExportRequest`、`FileExportReceipt` 与 `export_managed_source`；请求与 receipt 的 Debug 不携带目标、allowed root 或正文，公共失败继续只暴露已冻结 category / reason 和脱敏 IO source。
16. export 只接受调用方按 namespace 与精确 source ID 读取出的 active / 历史可读 `SourceArtifact`，在文件系统写入前重算受管正文长度与 `exact-bytes-v1` 摘要；pending、failed 或 deleted governance 状态失败关闭。
17. 目标必须是显式 export allowed root 内的绝对未占用路径；root 以下 parent symlink 与最终 symlink 拒绝。实现使用同目录 `create_new` 任务临时文件，write / flush / sync / close 后重新逐字节复验，再以 `hard_link` 原子建立不存在的目标目录项；发布后再次复验字节与文件身份，且只清理身份仍匹配的任务临时文件。
18. `P1-F07` 跨 package 测试已从真实 SQLite Source Vault 分别读取当前 source 与历史 source，逐字节 round-trip BOM、CRLF、分解 / 组合 Unicode 和尾换行差异；目标已存在与 symlink 目标保持原字节，Source Vault facts / body / audit 计数不变。
19. file-entry 内部故障测试覆盖临时写入失败和解析后并发占用目标导致的发布失败；两者均返回真实错误、清理任务临时文件并不覆盖目标。P1-I03 没有新增 schema、第三方 package、feature、native build、网络能力或真实个人资料。
20. P1-I03 完成后本机 `./scripts/check-repo.sh` 再次通过 107 个仓库文件检查、workspace format、Clippy `-D warnings` 与全部 locked test；这是 macOS 本地证据，尚未运行包含该单元的 Linux / macOS / Windows 远程 CI。
21. P1-I03 已提交为 `c686aac`；提交前后工作树边界均只包含 exact export 的 9 个文件，未 push、未创建 PR 或改变其它远程状态。
22. P1-I04 在现有删除 plan validation 中增加完整来源 lineage 门禁：请求包含任一 active 文件来源版本时，必须精确包含同 namespace / lineage 的全部 active SourceArtifact；已有 active memory 依赖展开门禁继续生效，部分版本计划在持久化前失败关闭。
23. P1-I04 继续把 canonical SourceArtifact / MemoryRecord 作为 DeleteRequest 语义目标，把 origin binding 与 capture audit 作为 adapter-private execution closure；计划创建即让全部来源 / fragments / proposals / memories 进入 pending 并移除 FTS / projection / tip，执行的 minimal-audit 阶段清除 binding / capture audit 并保留既有删除请求、结果与 evidence 真相。
24. source capture 验证已扩展到每个 active explicit source 的受管 body、完整 fragment 集、binding 与 capture audit；rebuild 在改写 tip / FTS 前后调用同一验证，原始外部文件缺失不影响重建，但 canonical fragment 缺失会以 stored-integrity failure 拒绝，不能静默把缺失事实当作空集合。
25. 新增 `P1-F08` 至 `P1-F10` 跨层测试：删除 origin file 后仍能从受管 facts 重建；单版本 deletion plan 不改变任何版本；完整两版本 lineage 在 plan、reopen、execute、evidence、rebuild 和再次 reopen 后持续关闭，body / fragments / tip / binding / capture audit 均按闭包处理，先前用户导出与外部原件字节保持不变。
26. P1-I04 没有新增 SQLite migration、core schema、第三方 package、feature、native build、网络能力或真实个人资料。
27. P1-I04 完成后本机 `./scripts/check-repo.sh` 通过 107 个仓库文件检查、workspace format、Clippy `-D warnings` 与全部 locked test；这是 macOS 本地证据，尚未运行包含阶段 1 文件入口的 Linux / macOS / Windows 远程 CI。
28. `P1-F02` 已把 BOM、CRLF、分解 / 组合 Unicode 与尾换行从 `.md` snapshot 贯穿到 capture candidate、SQLite exact body BLOB、whole-file fragment、关闭重开与 rebuild，并逐字节核对正文、长度、`exact-bytes-v1` 摘要和 fragment byte range；过程中没有 Unicode 或换行规范化。
29. `P1-F05` 已用同一外部 inode 的两个 hardlink alias、两个 opaque origin binding 建立内容摘要相同但 source / lineage / tip / audit 均独立的 canonical 来源；只删除第一条 lineage 后，第二条仍可读取、召回、reopen 和 rebuild，两个外部 hardlink 字节均未改变。
30. 本批只新增两个跨 package 合成测试和一个复用现有入口的 path-aware 测试 helper，没有修改 production code、schema、core port、依赖、feature、native build、网络能力或真实个人资料。
31. 定向 `cargo test --locked -p radishmemory-sqlite --test source_capture` 已通过 14 个测试；最终以本轮全仓聚合检查为准。
32. 本批完成后本机 `./scripts/check-repo.sh` 通过 107 个仓库文件检查、workspace format、Clippy `-D warnings` 与全部 locked test；这是 macOS 本地证据，尚未运行包含 `P1-F01` 至 `P1-F10` 的 Linux / macOS / Windows 远程 CI。
33. `P1-F11` 至 `P1-F14` 使用测试专用 `read_file_snapshot → build_source_capture → SourceCaptureStore → FileCaptureReceipt` 流水线，receipt 只在 canonical capture 完整提交后产生；拒绝结果因此不能以单独构造 receipt 或默认成功掩盖。
34. `P1-F11` 已覆盖允许根外文件、带词法 `..` 的逃逸路径和伪装为 `.txt` 的目录；每次稳定拒绝后 source、body、fragment、tip、binding、audit 与 FTS 七类计数均与既有基线完全相等，旧来源仍可召回。
35. `P1-F12` 已在本机 Unix 文件系统覆盖 root 以下 symlink parent 与 symlink leaf；两者均返回 `SymlinkNotAllowed`、不跟随目标、不泄露合成目标路径且不改变 store。该结果尚未替代 Windows symlink 权限与文件系统行为验证。
36. `P1-F13` 已覆盖不支持扩展名、空文件、非法 UTF-8 与 NUL，分别返回冻结 reason，均不进入 canonical 构建、SQLite 事务或 receipt。
37. `P1-F14` 已让恰为 8 MiB 的合法 UTF-8 `.txt` 真实提交 source / body / fragment / tip / binding / audit / FTS 并返回 receipt；8 MiB + 1 byte 返回 `FileTooLarge`，前一成功状态与计数保持不变。
38. 本批只修改跨 package 合成测试、真相源和一致性检查，没有修改 production code、schema、core port、依赖、feature、native build、网络能力或真实个人资料。
39. 定向 `cargo test --locked -p radishmemory-sqlite --test source_capture` 已通过 18 个测试；最终以本轮全仓聚合检查为准。
40. 本批完成后本机 `./scripts/check-repo.sh` 通过 107 个仓库文件检查、workspace format、Clippy `-D warnings` 与全部 locked test；这是 macOS 本地证据，尚未运行覆盖 `P1-F01` 至 `P1-F14` 的 Linux / macOS / Windows 远程 CI。
41. P1-I04、`P1-F02` / `P1-F05` 与前十项验收已提交为 `a945952`；`P1-F11` 至 `P1-F14` 的拒绝 / 大小边界测试已提交为 `a210936`。两次提交都未 push、未创建 PR 或改变其它远程状态。
42. 日终代码—文档审阅确认产品范围、记忆语义、隐私信任模式、同步边界、RadishMind 边界和 MVP 阶段顺序没有扩大；隐私模型与路线图无需语义修改。依赖基线的范围和 file-entry 供应链说明已从 P1-I02 补齐到 P1-I04 / `P1-F14`，ADR 0006 的剩余证据边界已精确收敛为 `P1-F15` 至 `P1-F18`、三平台 CI 与平台 bookmark，`FileCaptureReceipt` 的过期“持久化前”rustdoc 也已改为从成功 atomic capture result 映射。
43. 日终文档审阅完成后，本机 `./scripts/check-repo.sh` 再次通过 107 个仓库文件检查、workspace format、Clippy `-D warnings` 与全部 locked test；`source_capture` 18 个测试保持通过，远程 Phase 1 CI 仍未运行。
44. `P1-F15` 新增仅由 SQLite integration test 启用的第一方 `acceptance-test-support` feature；默认 file-entry build 不包含测试入口，实际读取仍复用一个 private 操作 seam。验收在初始 open-file 观察后确定性替换、截短或扩展所选文件，不使用 sleep 或概率竞态；三种变化都返回 retryable `SourceChangedDuringCapture`。
45. `P1-F15` 每次失败都逐项比较 source / body / fragment / tip / binding / audit / FTS 计数，以及 tip、binding、audit 和 FTS 行投影；旧来源保持可召回，候选 source 不存在且不产生 receipt。并发截短的校验顺序已修正为先比较读取前后观察，再判断读取结果是否为空，避免把捕获期间变化误报为普通 `EmptyFile`。
46. `P1-F16` 在 source / fragment / FTS / tip / binding / audit 全部写入并完成派生与入口状态复验后、SQLite transaction commit 前注入真实缺表错误；失败保留底层 SQLite cause，事务回到 autocommit，旧 tip 与全部八类事实 / 派生行保持逐项一致。该 seam 是 adapter private 泛型操作，不增加 public port、schema、fallback 或通用故障框架。
47. `P1-F16` 同时复验 export 临时写入 timeout 与并发占用目标：前者保留真实 `TimedOut` cause 与 retryable 状态，后者返回稳定 `DestinationExists`；两者都不发布或覆盖目标，只清理当前任务临时文件。
48. `P1-F17` 使用含 front matter、链接、HTML / script、伪指令和图片 URL 的合成 Markdown 贯通 exact-byte capture 与 FTS recall；本地 nonblocking TCP observer 没有收到连接，proposal / decision / record / state-event 四类 memory facts 全部为零，Markdown 只形成一个 SourceArtifact、一个 SourceFragment 与一行 FTS。
49. `P1-F18` 集中渲染 read / export request、snapshot、capture、SQLite result、SourceArtifact / SourceFragment / search hit、数据库 Debug、失败消息和 capture / export receipt；合成正文、完整路径、allowed root、导出目标、被拒绝字节标记及路径摘要均未出现，用户已有导出目标保持不变且没有遗留任务临时文件。file-entry / SQLite library manifests 没有日志依赖，production source 也没有 `println!`、`eprintln!`、`dbg!`、`log::` 或 `tracing::` sink，因此当前切片没有额外普通日志面；仓库检查器会拒绝这些 library source 新增未评审诊断 sink。
50. 定向 file-entry / SQLite 全量 locked test 与 all-targets / all-features Clippy `-D warnings` 已通过；文档同步后 `./scripts/check-repo.sh` 通过 107 个仓库文件检查、workspace format、Clippy `-D warnings` 与全部 locked test，`source_capture` 当前为 21 个测试。`Cargo.lock`、SQLite v6 schema、第三方 package / feature、native build 与产品网络能力均未变化；本轮改动尚未暂存、提交、push 或创建 PR。

## M0 基线完成（2026-08-26）

`M0-I02` 三个评审单元、`M0-I03 SQLite entry / storage` 与 `M0-I04 fixture runner` 保持在职责对应 package、直接测试、依赖记录和必要检查器内：

1. 四个 core 直接依赖与其全部传递依赖已经 lock，并记录解析版本、许可证、feature、build script / proc macro、网络与 native code 影响；
2. 错误只暴露稳定 category / reason，解析 cause 可追溯，但不保存被拒绝的时间、JSON 或正文；
3. 时间比较先归一到 UTC，同时保留原始 RFC 3339 表示、offset 与小数秒精度；`ValidTime` 四种 mode 和半开区间已覆盖；
4. canonical JSON 显式按 Unicode code point 排 key、保留 array 顺序、规范化转义与普通十进制 number，并输出无空白、无尾随换行的 UTF-8 bytes；
5. 冻结 digest vectors、完整 suite digest 与偏移时间、无效区间、NFC、转义、key 排序、integer / fraction、未知 profile、摘要不匹配、重复 key、`null` 等负例已通过本地 package 检查和正式仓库入口。
6. 九种顶层对象以私有 validated wrapper 表达，`schema_version`、`object_type`、本地 delivery / deletion scope、初始 confirmed 状态和事件目标状态由类型固定，不允许调用方构造矛盾常量字段；
7. `Identifier`、非空文本、正版本、有限 `[0, 1]` 数值、引用、actor / producer、retention、local-only governance、MemoryValue 与全部 M0 digest profile 已成为共享值类型；
8. 来源类型 / media type、正文长度与摘要、proposal operation、decision result、record 摘要、状态转换、ContextPack 预算 / citation / ordinal / 截断、删除闭包 / 计数 / 失败字段 / 最小保留依据和 completed 证据均执行字段级失败关闭；
9. schema 补齐了此前只写语义而未冻结内部字段的 `TruncationFacts`、`FilterCount`、`FrozenTargetClosure` 与 `retention_basis`；这些值只保留计数、稳定引用、摘要和 policy evidence，不复制被截断或删除正文；
10. 6 个 object contract 测试覆盖九种合法对象、代表性条件负例、错误稳定 reason 和正文不进入错误或 Debug；与 12 个 primitive 测试共同通过本地 core package 验证。
11. 跨对象校验接收调用方显式解析出的对象切片，不持有数据库或全局 registry；缺失引用、重复对象、namespace、身份、治理、召回、来源切片、物化、事件链、状态投影、supersede、citation、删除计划与时间漂移使用稳定 reason 区分；
12. SourceFragment 按 UTF-8 byte range 回查 active SourceArtifact；proposal 验证完整来源闭包和治理继承；accept decision、record 与初始 confirmed event 形成可复验闭环；无序事件集合按 previous ID 恢复唯一无分叉链并核对当前投影；
13. supersede 约束同 lineage、精确目标集合、单调版本、旧记录关闭事件与新记录有效起点；ContextPack 只解析 active 来源和 confirmed 普通记忆，并要求 citation map 精确、可回源且与 item evidence 关联；
14. DeleteRequest 的语义目标必须已进入非 active 状态，DeletionEvidence 的 component key、类型、目标闭包、动作和计数必须与冻结计划一一对应；7 个 invariant contract 测试覆盖合法闭环及代表性失败，且错误不复制来源或 ContextPack 正文。
15. `rusqlite 0.40.2` 关闭 default features，仅开启 `bundled`；lockfile 实际固定 `libsqlite3-sys 0.38.2`、SQLite `3.53.2` 与 11 个新增传递 package，并记录 MIT / Apache、SQLite public-domain、build script、预生成 binding 和 C 工具链影响；
16. `SqliteDatabase::open` 在迁移前强制 foreign keys、`trusted_schema=OFF`、`synchronous=FULL` 与非 WAL journal policy，核对运行时精确版本、`SQLITE_ENABLE_FTS5` 编译选项并实际创建 / 删除临时 FTS5 虚表；缺失能力不回退到扫描或另一存储；
17. `0001_sqlite_entry.sql` 只建立严格 migration metadata，事务内记录 adapter version、migration name 和 core `radishmemory.m0/1` schema version，再单调更新 `user_version`；当前版本重复打开只校验不改写；
18. 未知较新 `user_version`、未版本化外来 schema、当前版本缺失 / 篡改 migration history 均使用稳定错误失败关闭；公开错误和 Debug 不复制数据库路径或 SQL，测试复核失败后 schema 未被认领或迁移。
19. core 新增且仅新增 M0 capture / segment 已真实需要的 `SourceVault` port：不可变来源写入、单来源片段批量写入、namespace + source ID 读取和有序片段读取；没有预建通用 repository、runner operation 或异步抽象；
20. adapter schema v2 分离 `radishmemory_source_artifacts` metadata、`radishmemory_source_bodies` BLOB、source supersedes、fragment metadata 与 heading path；migration 在提交前校验历史前缀和精确业务表集合，v1 → v2 失败会整体回滚；
21. SourceArtifact metadata 与 exact UTF-8 body 在同一 `IMMEDIATE` transaction 写入，重复 ID / lineage version 返回稳定 conflict 且不覆盖原正文；supersedes 必须解析到同 namespace、同 lineage 的更早已存版本；
22. SourceFragment 以单来源非空批次写入，写入前对持久化 source 执行 namespace、治理、byte range、正文切片和摘要复验；片段表不复制 content，读取时从 source BLOB 按原始 UTF-8 byte range 重建并再次通过 canonical constructor；
23. source storage 测试覆盖 CRLF / Unicode exact bytes、BLOB 类型、完整 metadata 往返、namespace 隔离、heading 顺序、版本关系、不可变冲突、批量回滚、body 摘要篡改与无效 UTF-8；错误只暴露稳定 code / reason，不保存或输出正文。
24. core 新增且仅新增 M0 propose / decide / materialize / state event 已真实需要的 `MemoryStore` port：proposal 与 decision 不可变写入、accept 物化、非 supersede 状态事件追加，以及按 namespace + ID 读取四种对象；没有引入通用 repository、异步接口或 runner mapping；
25. adapter schema v3 以十张 STRICT 表分离 proposal / decision / immutable record / append-only event facts 及其有序引用关系；record 表刻意不保存 `current_state` / `last_state_event_id`，v1 / v2 都能在单个 migration transaction 单调升级到 v3；
26. proposal 写入与读取都从已持久化 Source Vault 解析完整 fragment / source 闭包并复验 namespace、active governance、原始 byte slice 和摘要；相同去重语义的候选返回稳定 conflict，不以新 ID 重复污染 proposal truth；
27. decision 写入要求现存同 namespace proposal，并把首个决定或 defer 后续决定追加为唯一无分叉链；accept / reject 终态拒绝任何后续决定，accept 的 `result_memory_id` 在 materialization 前保持独立可审计引用；
28. accept materialization 在单个 `IMMEDIATE` transaction 内验证 proposal / decision / record / initial event 闭环，写入 record facts 与初始事件；supersede 同时验证 lineage、递增版本、精确目标集与 effective time，并追加旧记录关闭事件。通用 append 不允许绕过该事务写 superseded event；
29. MemoryRecord 读取通过完整事件链派生当前状态，复验 proposal、decision、来源、事件 cause、相关记忆和 supersede 闭环；memory storage 测试覆盖完整往返、namespace 隔离、语义去重、defer → accept、终态拒绝、真实写后冲突回滚、旧事实行不改写、事件分叉拒绝、v2 → v3 升级、持久化链 / 正文 / lineage 篡改与错误脱敏。
30. core 新增且仅新增冻结 `search` operation 已真实需要的 `LocalSearch` port：请求显式携带 namespace、非空查询、`as_of`、正 `top_k` 和允许 sensitivity，结果只返回完整 SourceFragment 或 MemoryRecord，不暴露 SQLite rowid、FTS 分数或 adapter schema；
31. adapter schema v4 建立统一 FTS5 派生表与最小 memory current projection；投影只保存 memory ID、namespace、事件链当前状态和 tip event ID，FTS 只复制 active fragment / confirmed record 的可重建正文与最小过滤 metadata；
32. SourceFragment 批量写入、accept materialization、supersede 和通用终态事件已把 canonical facts、FTS 和当前投影收口到同一 `IMMEDIATE` transaction；索引或投影写失败会回滚对应 fragment、record 与 event facts；
33. 本地搜索先从已验真的 canonical facts 形成 namespace、sensitivity、deletion state、memory state、retention、capture / create time 与 valid time 资格集合，再用转义后的普通词项执行 FTS5；排序使用 bundled SQLite 的明确 BM25 与 object kind / stable ID tie-break，proposal、pending fragment、superseded / retracted memory 和未来时间对象不进入结果；
34. 新库及 v1 / v2 / v3 升级都在 v4 migration transaction 内从事实首次重建派生数据；打开、搜索和显式 verify 会比较 live FTS / 投影与全量复算结果，显式 rebuild 可修复派生漂移，但 canonical fact 损坏会使迁移整体回滚而不是认领新版本；
35. local search 测试覆盖 fixture 使用的连字符查询、namespace / sensitivity / `as_of` 隔离、稳定同分排序、proposal 排除、accept 后召回、pending fragment 排除、supersede / retract 后资格变化、来源 / 记忆派生写失败回滚、漂移失败关闭、重建一致性、v3 事实升级和无效事实升级回滚；错误与 Debug 不复制篡改正文。
36. core 新增且仅新增冻结删除流程实际需要的 `DeletionStore` port 和 `LocalDeletionExecution` 输入：计划持久化、逐组件执行、请求 / 证据读取与证据追加保持在领域边界；最小保留结果必须显式携带 `PolicyBasis`，冻结闭包会重算并核对覆盖完整有序 `ObjectRef` 列表的 canonical JSON digest；
37. adapter schema v5 以八张 STRICT 表分离 DeleteRequest、语义目标、计划组件、adapter 物理执行闭包、追加式 execution attempt / result 与 DeletionEvidence；component result 从不可变计划补回类型、目标、动作和计数，证据绑定一个完整不可变 attempt 并通过 previous evidence 建立无分叉链；
38. 删除计划只接受冻结 `m0-local-purge` 十组件 profile 和 SourceArtifact / MemoryRecord 语义目标；源仍被 active memory 引用而未显式纳入计划时失败关闭。计划提交原子地把 source、fragment、proposal、record 置为 pending，并移除对应 FTS 和当前投影，因此执行前普通读取与搜索已经停止召回；
39. 本地执行按外键安全顺序真实 redact proposal / record、删除来源 body / fragment、最小保留 metadata / decision / event / audit，并复验 FTS absence 与 M0 context cache 不持久化事实；每个组件使用独立 transaction，单项失败仍生成完整十项结果，最小审计将可审计目标收口为 failed，后续幂等重试可建立新的 attempt / evidence 而不改写历史；
40. 删除测试覆盖完整 source + memory purge、计划即停止召回、十组件结果、证据 round-trip / namespace 隔离、证据链、幂等执行、重建不恢复、源依赖未展开拒绝，以及 FTS 单组件故障下其余组件真实执行、failed evidence 和目标持续关闭；migration 测试覆盖 v4 → v5，workspace 全量测试保持通过。
41. runner 在执行前复验 suite digest、九种稳定 ID vector 和三类 digest vector，再为每个场景建立独立内存 SQLite；12 个场景不共享连接、registry、ContextPack、查询缓存、时钟或删除副作用，且 runner-only 入口仍复用 adapter 的 capability、migration、连接策略与派生校验；
42. 86 个 operation 使用显式 dispatch 调用真实 `SourceVault`、`MemoryStore`、`LocalSearch` 与 `DeletionStore`；application registry 只解析 fixture logical key，不复制 canonical object 校验、SQLite 行事实或第二套 schema；
43. 普通搜索先调用严格 FTS query，零结果时才使用与 expected key 无关的确定性本地词项变体逐次调用同一 `LocalSearch`；当前查询仍受 adapter 状态过滤，历史查询从不可变 record 与事件 effective boundary 重建 confirmed point-in-time 投影；
44. ContextPack 由真实召回对象、来源片段和 citation 构建并调用 core resolution invariant；删除失败通过 opt-in `fixture-runner` feature 选择已冻结组件并把 fixture error / retryable 真实写入 execution attempt，不向 production port 增加测试参数；
45. assertion 与 metric 都从运行事实计算，suite 聚合使用整数 count 与精确有理数，不从 oracle 复制 observed value；最小 JSON 包含 runner / adapter version、步骤、稳定 ID、带 profile 摘要、状态、错误和 gate，不包含正文、临时路径或删除内容；
46. runner 回归测试覆盖 12 场景 / 86 步 / 12 gate 成功、未知 operation / assertion 失败关闭、scenario / step 错误上下文、重复隔离运行字节级确定性、CLI JSON、预期删除失败和敏感内容缺席；本机 31 个仓库检查器单测与 `./scripts/check-repo.sh` 已通过。PR #1 首轮 run `32976944213` 真实暴露 Windows 文件数据库逐事务同步把重复 runner 测试拖到 `10m14s` 超时，提交 `918d045` 将 runner-only 场景改为独立内存连接后，run `32978669766` 的 Repo Hygiene、Linux `48s`、macOS `52s`、Windows `1m37s` 与聚合 `Candidate Quality` 全部通过；未通过放宽超时或减少断言掩盖失败。
47. PR #1 最终 head `6df0891` 在 run `32979128488` 再次通过 Repo Hygiene、Linux `42s`、macOS `1m6s`、Windows `1m33s` 与聚合 `Candidate Quality`，随后以 merge commit `fe8186a` 合入 `master`；`master -> dev` 以 `--ff-only` 回流并推送，两个远程分支计数为 `0 / 0`，回流后的 `./scripts/check-repo.sh` 通过。
48. 日终文档审阅纠正 README 的 M0 implementation-entry / SQLite v3 旧摘要、架构中 runner 临时文件与九对象持久化的过期描述，以及 fixture “未来 runner”输出措辞；长期产品、记忆、隐私、同步和 RadishMind 边界未因 M0 完成而扩大。

## 当前门禁

- 产品、架构、记忆或隐私语义变化必须同步更新对应真相源，不能只改入口摘要或检查器。
- ADR 0006 的 `P1-F01` 至 `P1-F18` 已由本机合成 importer / exporter 边界运行通过；这只证明当前 macOS 工作区的首个 application contract，不证明三平台 Phase 1、production host / UI、平台 bookmark、真实个人资料授权面或产品可用性。
- P1-I01 的 file snapshot 单独成功仍只证明一次本地读取；只有通过 P1-I02 `SourceCaptureStore` 返回的 receipt 才证明当前事务中的 managed body / canonical facts / FTS / binding / tip / audit 已共同提交。P1-I03 与 P1-I04 分别只证明本机 exact export 和 lineage deletion contract；完整 18 场景的本机通过仍不证明对抗性文件系统能力、未来平台兼容、三平台 CI 或产品入口。
- M0 语言、首批直接依赖范围和 SQLite / FTS5 已冻结；新增依赖必须审查许可证、原生构建、网络与数据影响并更新 lockfile。UI、服务端、向量实现和 Provider SDK 仍未冻结。
- 仓库只允许代码、规范、治理资产和合成 / 明确脱敏的 fixture；真实个人资料、记忆库、ContextPack、Embedding 输入和密钥不得进入 Git、Issue、PR 或 CI。
- GitHub 远端以 `master` 为默认稳定分支、`dev` 为常态开发分支，启用 merge commit 与 rebase merge，并禁用 squash merge；Private vulnerability reporting、Secret scanning 和 push protection 已启用。Ruleset 与 required check 必须以 API、workflow run 和目标分支有效规则复核，不能把仓库模板本身当作已生效证据。
- 当前仓库检查证明 P1-I01 / P1-I02 / P1-I03 / P1-I04 与 `P1-F01` 至 `P1-F18` 的格式、lint、locked test 和本机合成运行证据成立；PR #1 最终 run `32979128488` 只证明已合并 M0 基线在当时 Linux / macOS / Windows locked CI 环境成立，尚未覆盖任何 Phase 1 file snapshot、atomic capture、exact export、lineage deletion、TOCTOU 或无副作用验收。不证明 production host / UI、平台 bookmark、PDF / 图片、向量、模型、同步或生产能力，也不把一次 CI 通过外推为未来平台兼容承诺。

## 当前不做

- 不制作虚拟形象、主动陪伴或大而全的聊天产品面；
- 不发明新的长期记忆算法或直接引入图数据库、消息队列和微服务；
- 不把模型推断自动写成已确认记忆；
- 不宣称零知识同步、端到端加密、可证明永久删除或生产可用；
- 不建立自动发布、tag Ruleset、装饰性 CODEOWNERS 或无真实评审人的审批门禁；
- 不复制兄弟项目的技术栈、业务清单、CI 组件或目录结构。

## M0 实现退出条件

- 已完成：经评审的 M0 采集、检索、引用、确认、更正和删除闭环；
- 已完成：与首个切片对应的字段级 canonical schema；
- 已完成：与首个切片对应的可执行合成 fixture 与指标 oracle；
- 已完成：明确的记忆状态机、时间与冲突语义；
- 已完成：首个同步信任模式决策；
- 已完成：可测量的召回、污染、删除与隐私指标；
- 已完成：记录实现栈选择、替代方案、迁移边界和风险的 ADR；
- 已完成：精确 Rust 工具链、三 package workspace、受审阅依赖锁和三平台 Rust CI 合同；
- 已完成：canonical core 第一 primitive 单元，包括稳定错误、时间、摘要和 canonical JSON；
- 已完成：九种 canonical 顶层对象、共同值类型与字段级条件校验；
- 已完成：同 namespace 引用、来源切片、记忆闭环、事件投影、ContextPack 和删除证据的 canonical 跨对象不变量；
- 已完成：SQLite 连接、bundled 版本 / FTS5 能力检查、版本化 migration 与失败关闭入口；
- 已完成：SQLite SourceArtifact / SourceFragment metadata、exact body BLOB 与最小 Source Vault port；
- 已完成：SQLite proposal / decision / immutable record / append-only event facts、accept / supersede 事务与最小 MemoryStore port；
- 已完成：SQLite FTS5 索引、当前投影、事务维护、全量重建与本地检索 port；
- 已完成：SQLite 删除请求 / 证据持久化、冻结组件闭包、逐组件本地执行、失败保持关闭与幂等证据链；
- 已完成：M0 runner 真实实现；
- 已完成：冻结 fixture 的全部 assertion 与 metric 在本机通过；
- 已完成：PR #1 最终 head `6df0891` 在 run `32979128488` 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`，并以 merge commit `fe8186a` 合入 `master`、回流 `dev`；M0 退出条件全部收口。

## 当前验证入口

macOS / Linux：

```bash
./scripts/check-repo.sh
./scripts/check-m0-fixtures.py
```

Windows：

```powershell
pwsh ./scripts/check-repo.ps1
```
