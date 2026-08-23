# RadishMemory 当前状态

更新时间：2026-08-23

## 当前阶段

`M0 implementation entry`

产品、记忆、隐私、评测、同步信任、RadishMind 接入和 M0 实现栈基线已经冻结，最小 Rust workspace 与三平台检查合同已经建立。当前目标是按顺位实现 canonical core、SQLite adapter 和真实 M0 runner，不扩大到 PDF、Embedding、模型、UI、同步或服务端。

## 已冻结的首个切片

`M0 Local Memory Loop` 已通过 [ADR 0002](../adr/0002-m0-local-memory-loop.md) 冻结为单用户、单设备、本地、合成文本 / Markdown、无模型和无网络的 M0 代码范围。它按固定场景验证来源、引用、proposal / decision、时间更正、失败关闭和单设备删除证据，不包含 PDF、向量、Provider、RadishMind 或同步实现。

M0 字段级 canonical schema 已在 [M0 Canonical Schema](../schema/m0-canonical-schema.md) 冻结为九种顶层对象及共同逻辑类型。它确定字段、必填性、条件约束、时间、治理标签、事件和删除证据关系，但不绑定数据库、生产 ID 编码或语言类型。

[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 已冻结合成 JSON mapping、fixture ID、摘要 profile、12 个场景的 86 个有序操作和 12 个指标 gate。仓库校验器只验证这些输入与 oracle 自洽；真实 M0 runner 和产品能力仍未实现。

首个多设备同步信任模式已通过 [ADR 0003](../adr/0003-zero-knowledge-sync-first.md) 冻结为零知识同步服务：默认自部署服务端只中继密文对象、加密操作日志、密钥封装和已枚举最小元数据，不持有内容解密能力，也不运行语义索引、检索或 ContextPack 编译。可信计算节点后置为显式可选能力。该决定不把同步加入 M0，也不代表零知识同步已经实现。

RadishMind 首次运行接入已通过 [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md) 冻结在完整 MVP 阶段 3：只在 mock 或直接 adapter 基线成立后，以显式可关闭的 Model Gateway 接入。M0、阶段 1 单机资料库和阶段 2 记忆生命周期均不依赖它；首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回，也不复制兄弟项目业务 schema。

M0 实现栈已通过 [ADR 0005](../adr/0005-m0-implementation-stack.md) 冻结为 Rust 2024 模块化单体：`radishmemory-core`、`radishmemory-sqlite` 和 `radishmemory-m0` 三个 package，首个工具链固定为 Rust `1.96.0`，本地存储采用 bundled SQLite 与 FTS5。M0 不包含网络、异步运行时、ORM、模型 SDK 或静态加密，也不冻结未来 UI 和同步服务端语言。

`M0-I01` 已建立且仅建立上述三个可编译 package，提交初始第一方 lockfile，并把 fmt、Clippy 和 locked test 接入本地双平台入口与 PR 的 Linux / macOS / Windows matrix。

`M0-I02` 的第一个独立评审单元已实现稳定 core 错误、RFC 3339 / UTC 时间、`ValidTime`、`exact-bytes-v1` / `utf8-nfc-text-v1` / `canonical-json-v1` SHA-256，以及拒绝重复 key 和 `null` 的 `radishmemory-canonical-json-v1` parser / writer。`M0-I02` 的第二个独立评审单元已实现九种 canonical 顶层对象、共同值类型与字段级条件校验。`M0-I02` 的第三个独立评审单元已实现跨对象不变量，包括同 namespace 引用、来源切片、governance 继承、accept 物化闭环、事件链投影、supersede、ContextPack citation / 召回边界与删除计划 / 证据一致性。外部 JSON mapping 和应用操作尚未实现。

`M0-I03 SQLite entry` 已实现版本化 migration、连接安全基线、bundled SQLite 精确版本与 FTS5 双重 capability probe，以及对未知较新版本、现有外来 schema 和 migration metadata 漂移的失败关闭。当前 lockfile 包含五个经审查的直接第三方依赖和 40 个第三方 package，没有 Git dependency 或产品网络能力；`libsqlite3-sys` build script 会通过 `cc` 编译 bundled SQLite `3.53.2` C 源码。feature、许可证与 native build 影响见 [M0 Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)。该入口提交本身没有为未来假设新增空 core port；真实 port 随下述 storage 操作进入。

`M0-I03 SQLite storage` 的首个纵向切片已实现 SourceArtifact / SourceFragment 所需的真实 `SourceVault` core port，并把 adapter schema 单调升级到 v2。来源 metadata 与 exact body BLOB 分表原子写入；片段只保存 byte range、摘要、治理和生成器 metadata，读取时从已验真的 source body 重建 exact content，不建立第二份片段正文真相。写入保持不可变，namespace 读取失败关闭，重复对象不覆盖，批量片段冲突整批回滚，损坏或非 UTF-8 body 不进入领域对象。

`M0-I03 SQLite storage` 的第二个纵向切片已实现 MemoryProposal / MemoryDecision / MemoryRecord / MemoryStateEvent 所需的真实 `MemoryStore` core port，并把 adapter schema 单调升级到 v3。proposal 在写入和读取时解析完整 SourceFragment / SourceArtifact 闭包，相同 namespace、operation、内容摘要、来源集合与目标集合的候选不会重复落库；decision 以不可变、无分叉链追加，accept / reject 终态不能被覆盖。accept materialization 在同一事务创建不可变 record facts、初始 confirmed event，并在 supersede 时同时追加旧记录关闭事件；record 表不保存可变 `current_state` 或 `last_state_event_id`，读取时从已验真的事件链重建。FTS5 业务索引、物化当前投影、删除对象 / 执行和 runner 仍未实现。

## 当前顺位

1. `M0-I03 SQLite storage`：下一切片实现 FTS5 派生索引与可重建当前投影，使来源 / 记忆事实写入和对应索引 / 投影更新在同一事务收口；随后实现删除对象与本地删除组件。
2. `M0-I04 fixture runner`：按冻结操作顺序调用真实 core 与 adapter，执行全部 assertion / metric，输出最小 JSON 证据并报告零网络能力边界。

## 本次完成（2026-08-23）

`M0-I02` 三个评审单元与 `M0-I03 SQLite entry` 保持在职责对应 package、直接测试、依赖记录和必要检查器内：

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

下一实施单元继续 `M0-I03 SQLite storage`：建立仅索引 active source fragments 与 confirmed memory records 的 FTS5 派生表，以及由事件链可重建的最小当前状态投影；事实、索引和投影必须在同一 adapter transaction 更新并提供重建 / 漂移测试。删除和 runner 仍按后续独立评审切片推进。

## 当前门禁

- 产品、架构、记忆或隐私语义变化必须同步更新对应真相源，不能只改入口摘要或检查器。
- M0 语言、首批直接依赖范围和 SQLite / FTS5 已冻结；新增依赖必须审查许可证、原生构建、网络与数据影响并更新 lockfile。UI、服务端、向量实现和 Provider SDK 仍未冻结。
- 仓库只允许代码、规范、治理资产和合成 / 明确脱敏的 fixture；真实个人资料、记忆库、ContextPack、Embedding 输入和密钥不得进入 Git、Issue、PR 或 CI。
- GitHub 远端以 `master` 为默认稳定分支、`dev` 为常态开发分支，启用 merge commit 与 rebase merge，并禁用 squash merge；Private vulnerability reporting、Secret scanning 和 push protection 已启用。Ruleset 与 required check 必须以 API、workflow run 和目标分支有效规则复核，不能把仓库模板本身当作已生效证据。
- 当前仓库检查证明治理、文本、链接、配置合同、canonical core primitive、九种对象、字段级校验、跨对象不变量、SQLite 连接 / migration、最小 Source Vault、MemoryStore 不可变事实 / 追加事件与 FTS5 capability probe 的格式、lint 和测试在本机成立；不证明 FTS5 业务索引、物化当前投影、runner、真实删除执行或同步已经实现，也不等同于三平台 CI 已运行。

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
- 已完成：RadishMind 首批参与方式的明确决定；
- 已完成：记录实现栈选择、替代方案、迁移边界和风险的 ADR；
- 已完成：精确 Rust 工具链、三 package workspace、受审阅依赖锁和三平台 Rust CI 合同；
- 已完成：canonical core 第一 primitive 单元，包括稳定错误、时间、摘要和 canonical JSON；
- 已完成：九种 canonical 顶层对象、共同值类型与字段级条件校验；
- 已完成：同 namespace 引用、来源切片、记忆闭环、事件投影、ContextPack 和删除证据的 canonical 跨对象不变量；
- 已完成：SQLite 连接、bundled 版本 / FTS5 能力检查、版本化 migration 与失败关闭入口；
- 已完成：SQLite SourceArtifact / SourceFragment metadata、exact body BLOB 与最小 Source Vault port；
- 已完成：SQLite proposal / decision / immutable record / append-only event facts、accept / supersede 事务与最小 MemoryStore port；
- 待完成：SQLite FTS5 索引 / 当前投影 / 删除组件和 M0 runner 真实实现；
- 待完成：冻结 fixture 的全部 assertion、metric 和三平台运行门禁通过。

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
