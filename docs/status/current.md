# RadishMemory 当前状态

更新时间：2026-08-26

## 当前阶段

`M0 remote-validated merge candidate`

产品、记忆、隐私、评测、同步信任、RadishMind 接入和 M0 实现栈基线已经冻结；最小 Rust workspace、canonical core、SQLite v5 事实存储、FTS5 派生索引、当前状态投影、本地删除闭环与真实 M0 runner 已经建立。本机已由冻结 fixture 实际通过 12 个场景、86 个有序操作和 12 个指标 gate；[PR #1](https://github.com/laugh0608/RadishMemory/pull/1) 的 head `918d045` 已在 [workflow run 32978669766](https://github.com/laugh0608/RadishMemory/actions/runs/32978669766) 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`。当前目标是审阅并决定是否把该阶段候选合入 `master`，在完成合并与 `master -> dev` 回流前不开始阶段 1 实现。

## 已冻结的首个切片

`M0 Local Memory Loop` 已通过 [ADR 0002](../adr/0002-m0-local-memory-loop.md) 冻结为单用户、单设备、本地、合成文本 / Markdown、无模型和无网络的 M0 代码范围。它按固定场景验证来源、引用、proposal / decision、时间更正、失败关闭和单设备删除证据，不包含 PDF、向量、Provider、RadishMind 或同步实现。

M0 字段级 canonical schema 已在 [M0 Canonical Schema](../schema/m0-canonical-schema.md) 冻结为九种顶层对象及共同逻辑类型。它确定字段、必填性、条件约束、时间、治理标签、事件和删除证据关系，但不绑定数据库、生产 ID 编码或语言类型。

[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 已冻结合成 JSON mapping、fixture ID、摘要 profile、12 个场景的 86 个有序操作和 12 个指标 gate。仓库校验器仍只验证输入与 oracle 自洽；`radishmemory-m0` 现在会把同一 suite 映射到真实 core / SQLite 操作并独立计算 assertion 与 metric。

首个多设备同步信任模式已通过 [ADR 0003](../adr/0003-zero-knowledge-sync-first.md) 冻结为零知识同步服务：默认自部署服务端只中继密文对象、加密操作日志、密钥封装和已枚举最小元数据，不持有内容解密能力，也不运行语义索引、检索或 ContextPack 编译。可信计算节点后置为显式可选能力。该决定不把同步加入 M0，也不代表零知识同步已经实现。

RadishMind 首次运行接入已通过 [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md) 冻结在完整 MVP 阶段 3：只在 mock 或直接 adapter 基线成立后，以显式可关闭的 Model Gateway 接入。M0、阶段 1 单机资料库和阶段 2 记忆生命周期均不依赖它；首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回，也不复制兄弟项目业务 schema。

M0 实现栈已通过 [ADR 0005](../adr/0005-m0-implementation-stack.md) 冻结为 Rust 2024 模块化单体：`radishmemory-core`、`radishmemory-sqlite` 和 `radishmemory-m0` 三个 package，首个工具链固定为 Rust `1.96.0`，本地存储采用 bundled SQLite 与 FTS5。M0 不包含网络、异步运行时、ORM、模型 SDK 或静态加密，也不冻结未来 UI 和同步服务端语言。

`M0-I01` 已建立且仅建立上述三个可编译 package，提交初始第一方 lockfile，并把 fmt、Clippy 和 locked test 接入本地双平台入口与 PR 的 Linux / macOS / Windows matrix。

`M0-I02` 的第一个独立评审单元已实现稳定 core 错误、RFC 3339 / UTC 时间、`ValidTime`、`exact-bytes-v1` / `utf8-nfc-text-v1` / `canonical-json-v1` SHA-256，以及拒绝重复 key 和 `null` 的 `radishmemory-canonical-json-v1` parser / writer。`M0-I02` 的第二个独立评审单元已实现九种 canonical 顶层对象、共同值类型与字段级条件校验。`M0-I02` 的第三个独立评审单元已实现跨对象不变量，包括同 namespace 引用、来源切片、governance 继承、accept 物化闭环、事件链投影、supersede、ContextPack citation / 召回边界与删除计划 / 证据一致性。fixture 外部 JSON mapping 与 application operation dispatch 已由 `M0-I04` 在 runner 层实现，不反向进入 core schema。

`M0-I03 SQLite entry` 已实现版本化 migration、连接安全基线、bundled SQLite 精确版本与 FTS5 双重 capability probe，以及对未知较新版本、现有外来 schema 和 migration metadata 漂移的失败关闭。当前 lockfile 包含五个经审查的直接第三方依赖和 40 个第三方 package，没有 Git dependency 或产品网络能力；`libsqlite3-sys` build script 会通过 `cc` 编译 bundled SQLite `3.53.2` C 源码。feature、许可证与 native build 影响见 [M0 Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)。该入口提交本身没有为未来假设新增空 core port；真实 port 随下述 storage 操作进入。

`M0-I03 SQLite storage` 的首个纵向切片已实现 SourceArtifact / SourceFragment 所需的真实 `SourceVault` core port，并把 adapter schema 单调升级到 v2。来源 metadata 与 exact body BLOB 分表原子写入；片段只保存 byte range、摘要、治理和生成器 metadata，读取时从已验真的 source body 重建 exact content，不建立第二份片段正文真相。写入保持不可变，namespace 读取失败关闭，重复对象不覆盖，批量片段冲突整批回滚，损坏或非 UTF-8 body 不进入领域对象。

`M0-I03 SQLite storage` 的第二个纵向切片已实现 MemoryProposal / MemoryDecision / MemoryRecord / MemoryStateEvent 所需的真实 `MemoryStore` core port，并把 adapter schema 单调升级到 v3。proposal 在写入和读取时解析完整 SourceFragment / SourceArtifact 闭包，相同 namespace、operation、内容摘要、来源集合与目标集合的候选不会重复落库；decision 以不可变、无分叉链追加，accept / reject 终态不能被覆盖。accept materialization 在同一事务创建不可变 record facts、初始 confirmed event，并在 supersede 时同时追加旧记录关闭事件；record 表不保存可变 `current_state` 或 `last_state_event_id`，读取时从已验真的事件链重建。

`M0-I03 SQLite storage` 的第三个纵向切片已实现真实 `LocalSearch` core port，并把 adapter schema 单调升级到 v4。FTS5 只保存 active SourceFragment 与 confirmed MemoryRecord 的派生正文，当前状态表只物化事件链 tip；来源 / 记忆事实、状态事件、索引和投影在同一 `IMMEDIATE` transaction 更新。检索先形成 namespace、敏感度、删除、状态、保留期与 `as_of` 资格集合，再执行本地 FTS5 top-k；普通查询不会返回 proposal、非 active 来源或非 confirmed 记忆。每次打开和搜索都会对事实全量复算并核对派生数据，显式重建也只从已验真的 canonical facts 生成；v1 / v2 / v3 升级会在 migration transaction 内完成首次重建。

`M0-I03 SQLite storage` 的第四个纵向切片已实现真实 `DeletionStore` core port，并把 adapter schema 单调升级到 v5。`DeleteRequest`、十项冻结组件、canonical 目标闭包、adapter 内部物理执行闭包、逐次执行结果和 `DeletionEvidence` 都以不可变或追加式关系持久化；计划入库与语义目标进入 `pending`、FTS / 当前投影移除位于同一 `IMMEDIATE` transaction。执行器真实处理 source body / metadata / fragment、proposal / decision / record / state event、FTS、M0 不持久化的 context cache 与最小审计；失败组件保留真实 error / retryable 结果并把目标收口为 `failed`，不会恢复 active，也不会把部分失败写成 completed。普通读取、检索和派生重建只接受 active facts；完整成功、单组件失败、幂等重试、未展开依赖拒绝、namespace 隔离、证据链和 v4 → v5 升级已由测试覆盖。该能力只证明应用可复验的本地行、索引、投影和 cache 边界，不证明 SQLite 空闲页、临时文件、备份、文件系统快照或介质已被取证级擦除。

`M0-I04 fixture runner` 已实现冻结 suite 摘要与向量复验、每场景独立内存 SQLite、logical key → canonical fixture ID registry、86 个操作的显式 dispatch、确定性本地词项扩展、point-in-time 事件投影、ContextPack citation 解析、删除失败注入、实际 metric 聚合和最小 JSON 证据。内存数据库入口与失败注入只存在于显式启用的第一方 `fixture-runner` feature：前者仍执行 production capability、migration、连接策略和 adapter 操作，但不持久化文件；后者不改变 production `DeletionStore` port。runner 输出不包含 fixture 正文、临时数据库路径或已删除内容；未知 operation、assertion、metric 与摘要漂移均失败关闭并保留稳定 scenario / step 上下文。

## 当前顺位

1. 审阅 PR #1 的 M0 阶段 diff、远程运行证据、失败历史和未覆盖边界；未获明确授权不自动合并。
2. 获得明确授权后以 merge commit 合入 `master`，再在下一轮开发前完成 `master -> dev` 回流与拓扑 / 仓库门禁复核。
3. 合并与回流收口后，再冻结阶段 1“单机资料库与可追溯问答”的首个真实资料切片；先定义真实文件边界、导入 / 导出、派生数据和删除验收，不直接跳到 PDF、向量或 Provider 实现。

## 明日事项（2026-08-27）

主任务收口 `M0-I04 fixture runner` 的本地退出证据，并为阶段 1 入口做窄范围准备：

1. 复核 PR #1 的阶段范围、真实远程证据与未覆盖边界，由项目所有者明确决定是否 merge。
2. 合并后完成 `master -> dev` 回流，确认 `origin/master` 是 `dev` 祖先并重新运行与实际回流 diff 相称的仓库门禁；保留首轮 Windows 超时与修复后成功的真实 CI 历史。
3. 只在 M0 合并与回流收口后起草阶段 1 首个切片：优先把已成立的本地文本 / Markdown 事实存储转成真实但受控的单机资料库入口，先冻结文件路径、来源身份、导入幂等、导出、删除与合成验收边界。
4. PDF / 图片解析、向量索引和模型 adapter 分别需要可替换边界、依赖 / 许可证 / native build 审查、隐私失败关闭和独立质量指标，不合并成一个大批次。

明日停止线：未经新真相源与验收冻结，不进入真实个人资料导入、PDF / OCR、Embedding、模型、UI、网络、同步或通用 workflow engine；未经当前任务明确授权，不 push、不创建 PR、不改变远端状态。

## 本次完成（2026-08-26）

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

## 当前门禁

- 产品、架构、记忆或隐私语义变化必须同步更新对应真相源，不能只改入口摘要或检查器。
- M0 语言、首批直接依赖范围和 SQLite / FTS5 已冻结；新增依赖必须审查许可证、原生构建、网络与数据影响并更新 lockfile。UI、服务端、向量实现和 Provider SDK 仍未冻结。
- 仓库只允许代码、规范、治理资产和合成 / 明确脱敏的 fixture；真实个人资料、记忆库、ContextPack、Embedding 输入和密钥不得进入 Git、Issue、PR 或 CI。
- GitHub 远端以 `master` 为默认稳定分支、`dev` 为常态开发分支，启用 merge commit 与 rebase merge，并禁用 squash merge；Private vulnerability reporting、Secret scanning 和 push protection 已启用。Ruleset 与 required check 必须以 API、workflow run 和目标分支有效规则复核，不能把仓库模板本身当作已生效证据。
- 当前仓库检查与 PR #1 run `32978669766` 共同证明治理、文本、链接、配置合同、canonical core primitive、九种对象、字段级校验、跨对象不变量、SQLite 连接 / migration、Source Vault、MemoryStore 不可变事实 / 追加事件、FTS5 业务索引、当前投影、检索过滤、派生重建、本地删除对象 / 组件 / 证据链和真实 M0 runner 的格式、lint 与测试在本机及 Linux / macOS / Windows 当前 locked CI 环境成立；不证明 PDF / 图片、向量、模型、同步或生产能力，也不把一次 CI 通过外推为未来平台兼容承诺。

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
- 已完成：PR #1 head `918d045` 在 run `32978669766` 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`；阶段候选仍待项目所有者审阅、合并和回流。

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
