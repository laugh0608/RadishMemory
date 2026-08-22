# M0 Canonical Schema

状态：Frozen for M0 implementation

版本：`radishmemory.m0/1`

本文冻结 [M0 Local Memory Loop](../adr/0002-m0-local-memory-loop.md) 的逻辑字段、类型、必填性、条件约束和对象关系。它是首批实现、fixture 和测试的共同契约，不指定数据库表、语言类型、ID 编码、传输协议或 JSON 布局。

## 适用范围

M0 冻结九种顶层对象：

1. `SourceArtifact`
2. `SourceFragment`
3. `MemoryProposal`
4. `MemoryDecision`
5. `MemoryRecord`
6. `MemoryStateEvent`
7. `ContextPack`
8. `DeleteRequest`
9. `DeletionEvidence`

`CaptureRequest`、`CaptureReceipt`、`SearchRequest` 和 `SearchCandidate` 是运行接口或评测证据，不是长期记忆真相；它们在 M0 评测中的操作表示已经由 [M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 冻结，但 production API 仍由实现阶段决策。Embedding、数据库行号、Provider 请求、模型内部表示、设备同步字段和备份协议不属于本版本。

本文使用“必须”“不得”“仅当”表达规范性要求。标为“可选”的字段缺失表示没有该事实；除字段类型明确允许外，`null`、空字符串和缺失不得互换。

## 共同逻辑类型

### 标识与引用

- `Identifier`：非空、不透明、大小写敏感的稳定标识。实现不得要求调用方从标识中解析数据库、时间、对象类型或设备信息。
- 每个顶层对象使用自己的 ID 字段；ID 创建后不得复用。删除后也不能把旧 ID 分配给新对象。
- `ObjectRef` 由 `object_type` 与对应对象 ID 组成。M0 的引用必须位于同一 `namespace_id`；跨 namespace 引用失败关闭。
- `EvidenceRef` 包含 `evidence_type` 与 `evidence_id`。M0 允许引用 `source_fragment`、`memory_proposal`、`memory_decision`、`memory_record`、`delete_request` 和 `policy_basis`；引用目标必须存在且可审计。
- `ActorRef` 包含必填的 `actor_type` 与 `actor_id`，以及可选的 `actor_version`。M0 允许的 `actor_type` 为 `user`、`device`、`rule`、`parser`、`test_fixture` 和 `system`。
- `ProducerRef` 包含必填的 `producer_type`、`producer_id` 和 `producer_version`。规则、解析器与测试桩必须可区分，不能统一伪装成 `system`。

### 时间

- `Timestamp` 表示带时区的绝对时刻。任何外部文本表示必须保留 RFC 3339 语义；比较前转换为 UTC，但不得丢失原始精度事实。
- `observed_at` 是来源事实被观察到的时间；`created_at`、`proposed_at`、`decided_at`、`occurred_at` 和 `compiled_at` 是系统事件时间，二者不得互相代替。
- `ValidTime` 包含 `mode`、`start_at`、`end_at` 与 `precision`。`mode` 只能为 `unknown`、`instant`、`interval` 或 `open_ended`。
- `unknown` 不包含边界；`instant` 只包含 `start_at`；`interval` 同时包含 `start_at` 与 `end_at`，采用半开区间 `[start_at, end_at)`；`open_ended` 只包含 `start_at`。
- `precision` 至少支持 `exact`、`day`、`month`、`year` 和 `unknown`。缺少有效时间不是“从无限过去一直有效”。

### 内容与摘要

- M0 原始正文必须是 UTF-8 文本或 Markdown；正文原始字节和换行必须保留。
- `Digest` 包含 `algorithm`、`profile` 和 `value`。M0 的 `algorithm` 必须为 `sha256`，`value` 为小写十六进制。
- 原始正文使用 `exact-bytes-v1` profile；语义文本使用 `utf8-nfc-text-v1` profile。复合对象摘要使用对象专属 profile，其 canonical byte mapping 已在 [M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 冻结，实现不得自行发明第二种编码。
- `MemoryValue` 在 M0 只允许 `kind = text`，并包含非空 `text` 与 `content_digest`。结构化值留给兼容性评审后的新 schema 版本。
- 摘要用于完整性、幂等和最小审计，不代表访问授权，也不能替代被删除的正文。

### 治理标签

`Governance` 是所有可召回内容的必填值对象：

| 字段 | 必填 | 约束 |
| --- | --- | --- |
| `sensitivity` | 是 | `personal`、`sensitive` 或 `restricted` |
| `egress_policy` | 是 | `local_only`、`trusted_device_only`、`trusted_server_only` 或 `cloud_allowed` |
| `retention` | 是 | `RetentionRule` |
| `deletion_state` | 是 | `active`、`pending`、`failed` 或 `deleted` |
| `policy_basis` | 是 | 非空、可审计的策略或授权标识 |

`RetentionRule.mode` 只能为 `until_deleted`、`until_time` 或 `policy`。`until_time` 必须提供 `expires_at`；`policy` 必须提供 `policy_id`；其它组合不得携带不适用字段。

M0 所有包含 Governance 的对象，其 `egress_policy` 必须为 `local_only`。sensitivity 的严格度为 `personal < sensitive < restricted`；派生对象取所有来源的最严格值。egress policy 按允许目的地集合求交集，retention 不得晚于任一来源；无法无损表达交集时回退为 `local_only`，不能选择更宽标签。

Governance 中除 `deletion_state` 外的字段属于对象创建时的不可变策略快照；`deletion_state` 是由 DeleteRequest 和 DeletionEvidence 计算的物化投影。未知 sensitivity、未知 egress policy、缺失 policy basis、投影与删除证据不一致，以及 `pending`、`failed` 或 `deleted` 删除状态都必须排除普通召回。

### 版本、不变性与集合

- 每个顶层对象必须包含精确值为 `radishmemory.m0/1` 的 `schema_version` 和与对象一致的 `object_type`。
- 来源正文变化创建新的 `SourceArtifact` 版本；记忆语义变化创建新的 `MemoryRecord`；决定与状态变化创建新事件。不得通过 `updated_at` 原地改写历史事实。
- `version` 是同一 `lineage_id` 内从 1 开始的正整数。版本必须单调增加，但不得仅依赖时间推断前后关系。
- 表示集合的列表必须去重；为计算摘要或 fixture 比较时按对象类型与稳定 ID 排序。表示顺序的列表必须显式包含 `ordinal`。
- 任何未知 `schema_version`、未知枚举值、悬空引用、重复 ID 或不满足条件字段的对象都不得静默降级。

## SourceArtifact

`SourceArtifact` 是用户输入正文的原始真相。正文可以物理内联或外置，但在对象处于 active 状态时必须能由受信 Source Vault 按 `source_id` 和摘要取回。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `SourceArtifact` |
| `source_id` | 是 | `Identifier`；当前不可变版本 ID |
| `lineage_id` | 是 | `Identifier`；同一来源版本链 |
| `version` | 是 | 正整数 |
| `namespace_id` | 是 | `Identifier` |
| `source_kind` | 是 | `text` 或 `markdown` |
| `media_type` | 是 | 分别为 `text/plain` 或 `text/markdown` |
| `content` | 是 | 受保护的 UTF-8 原始正文 |
| `content_length` | 是 | 原始 UTF-8 字节数，正整数 |
| `content_digest` | 是 | `Digest`；`exact-bytes-v1` |
| `title` | 否 | 非空文本；不得代替正文 |
| `origin_kind` | 是 | M0 为 `synthetic_fixture` 或 `explicit_user_input` |
| `origin_ref` | 否 | 不透明来源引用；不得要求真实本地路径 |
| `observed_at` | 是 | `Timestamp` |
| `captured_at` | 是 | 系统完成本次采集的 `Timestamp` |
| `supersedes_source_ids` | 是 | 去重的 `source_id` 集合；首版为空 |
| `governance` | 是 | `Governance` |
| `producer` | 是 | `ProducerRef` |
| `created_at` | 是 | `Timestamp` |

相同内容的两次独立采集可以具有不同 `source_id`；内容摘要相同不能自动合并来源、授权或保留策略。

## SourceFragment

`SourceFragment` 是对单个 `SourceArtifact` 的确定性、连续字节范围引用。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `SourceFragment` |
| `fragment_id` | 是 | `Identifier`；同一来源、分段器版本和范围应稳定 |
| `namespace_id` | 是 | 与来源相同 |
| `source_id` | 是 | 现存 `SourceArtifact.source_id` |
| `ordinal` | 是 | 同一来源内从 0 开始的整数 |
| `byte_start` | 是 | UTF-8 原始正文字节偏移，含起点 |
| `byte_end` | 是 | UTF-8 原始正文字节偏移，不含终点；大于 `byte_start` |
| `heading_path` | 否 | Markdown 标题路径的有序非空文本列表 |
| `content` | 是 | 原始正文 `[byte_start, byte_end)` 的精确 UTF-8 文本 |
| `content_digest` | 是 | `Digest`；必须匹配片段精确字节 |
| `segmenter` | 是 | `ProducerRef` |
| `governance` | 是 | 不得弱于来源 |
| `created_at` | 是 | `Timestamp` |

片段不得跨来源，不得只保存无法回到正文的归一化文本。引用解析时摘要或字节范围不匹配即失败。

## MemoryProposal

`MemoryProposal` 是不可变候选，不是已确认记忆。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `MemoryProposal` |
| `proposal_id` | 是 | `Identifier` |
| `namespace_id` | 是 | `Identifier` |
| `operation` | 是 | `create` 或 `supersede` |
| `memory_type` | 是 | `observation`、`claim`、`episode`、`preference` 或 `procedure` |
| `subject_ref` | 是 | 非空、namespace 内的主体引用 |
| `proposed_content` | 是 | `MemoryValue` |
| `source_fragment_refs` | 是 | 非空、去重的 `SourceFragment` 引用 |
| `target_memory_ids` | 是 | `create` 时为空；`supersede` 时非空 |
| `observed_at` | 是 | `Timestamp` |
| `valid_time` | 是 | 创建时断言的 `ValidTime`；有效终点还受后续状态事件约束 |
| `confidence` | 是 | 闭区间 `[0, 1]` 的有限数值 |
| `importance` | 是 | 闭区间 `[0, 1]` 的有限数值 |
| `governance` | 是 | 不得弱于任一来源 |
| `producer` | 是 | `ProducerRef` |
| `reason_code` | 是 | 非空稳定代码 |
| `proposed_at` | 是 | `Timestamp` |

proposal 不包含 confirmed 状态。规则、解析器或测试桩无论置信度多高，都不能省略 `MemoryDecision`。namespace、operation、内容摘要、排序后来源和目标全部相同的 proposal 具有相同去重语义；内部索引键不是 canonical 字段。

## MemoryDecision

`MemoryDecision` 是针对一个 proposal 的不可变决定事件。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `MemoryDecision` |
| `decision_id` | 是 | `Identifier` |
| `namespace_id` | 是 | 与 proposal 相同 |
| `proposal_id` | 是 | 现存 `MemoryProposal.proposal_id` |
| `previous_decision_id` | 否 | 同一 proposal 的前一 defer decision |
| `decision` | 是 | `accept`、`reject` 或 `defer` |
| `decided_by` | 是 | `ActorRef` |
| `authorization_basis` | 是 | 非空授权或确定性规则标识 |
| `reason_code` | 是 | 非空稳定代码 |
| `reason_text` | 否 | 最小化文本，不得复制不必要正文 |
| `result_memory_id` | 条件 | `accept` 时必须存在；其它决定不得存在 |
| `decided_at` | 是 | `Timestamp` |

`defer` 后可以追加一个引用它的决定；`accept` 与 `reject` 是该 proposal 的终态。终态决定不得被另一决定覆盖；纠错必须创建新 proposal 或状态事件。

## MemoryRecord

`MemoryRecord` 保存一次已确认的不可变语义版本；当前状态是由 `MemoryStateEvent` 计算出的必填投影。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `MemoryRecord` |
| `memory_id` | 是 | `Identifier`；不可变版本 ID |
| `lineage_id` | 是 | `Identifier`；同一逻辑记忆版本链 |
| `version` | 是 | 正整数 |
| `namespace_id` | 是 | 与 proposal 相同 |
| `memory_type` | 是 | 与 proposal 相同 |
| `subject_ref` | 是 | 与 proposal 相同 |
| `content` | 是 | `MemoryValue` |
| `source_fragment_refs` | 是 | 非空、去重，且来自 proposal |
| `origin_proposal_id` | 是 | 被接受的 proposal |
| `accepted_by_decision_id` | 是 | 对应 accept decision |
| `observed_at` | 是 | `Timestamp` |
| `valid_time` | 是 | `ValidTime` |
| `confidence` | 是 | `[0, 1]`；不表示授权 |
| `importance` | 是 | `[0, 1]`；不表示保留承诺 |
| `governance` | 是 | 不得弱于 proposal 与来源 |
| `initial_state` | 是 | `confirmed` |
| `current_state` | 是 | `confirmed`、`superseded`、`contradicted`、`retracted` 或 `expired` |
| `last_state_event_id` | 是 | 生成当前投影的最后事件 |
| `supersedes_memory_ids` | 是 | 新版本替代的记忆集合；可为空 |
| `contradicts_memory_ids` | 是 | 明确冲突的记忆集合；可为空 |
| `content_digest` | 是 | 与 `content.content_digest` 相同 |
| `created_at` | 是 | `Timestamp` |

`current_state` 或 `last_state_event_id` 缺失、投影与事件链不一致时不得进入普通召回。`supersede` proposal 产生的 record 必须与目标处于同一 lineage，版本递增，并通过状态事件关闭旧记录的当前适用性。

## MemoryStateEvent

`MemoryStateEvent` 是记忆状态的追加事实；它比 `MemoryRecord.current_state` 投影更权威。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `MemoryStateEvent` |
| `event_id` | 是 | `Identifier` |
| `namespace_id` | 是 | 与 memory 相同 |
| `memory_id` | 是 | 现存 `MemoryRecord.memory_id` |
| `previous_event_id` | 否 | 同一 memory 的前一事件；初始 confirmed 事件缺失 |
| `event_type` | 是 | `confirmed`、`superseded`、`contradicted`、`retracted` 或 `expired` |
| `from_state` | 条件 | 初始 confirmed 事件不得存在；其它事件必须存在 |
| `to_state` | 是 | 与 `event_type` 对应的状态 |
| `cause_ref` | 是 | 指向决定、相关记忆、删除请求或保留策略的 `EvidenceRef` |
| `related_memory_ids` | 是 | 去重集合；没有相关记忆时为空 |
| `actor` | 是 | `ActorRef` |
| `reason_code` | 是 | 非空稳定代码 |
| `effective_at` | 条件 | 初始 confirmed 事件不得存在；其它事件的状态生效时间必须存在 |
| `occurred_at` | 是 | `Timestamp` |

M0 允许 `none → confirmed` 以及 `confirmed → superseded | contradicted | retracted | expired`。事件链分叉、循环、跳过 previous event 或出现其它转换时必须报告冲突，不得最后写入覆盖。point-in-time 查询使用创建时 `valid_time` 与状态事件 `effective_at` 的交集；不得为补写 `valid_to` 修改旧记录。

## ContextPack

`ContextPack` 是一次本地任务的受控构建产物，不是新的记忆真相。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `ContextPack` |
| `context_pack_id` | 是 | `Identifier` |
| `namespace_id` | 是 | `Identifier` |
| `request_id` | 是 | 本次本地请求的 `Identifier` |
| `task` | 是 | 非空本地任务文本 |
| `task_digest` | 是 | `Digest`；语义文本 profile |
| `as_of` | 是 | 查询的 point-in-time；默认值也必须显式物化 |
| `compiled_at` | 是 | `Timestamp` |
| `delivery_scope` | 是 | M0 精确值 `local` |
| `governance` | 是 | `egress_policy = local_only` |
| `budget` | 是 | `Budget` |
| `items` | 是 | 有序 `ContextItem` 列表；可以为空 |
| `citation_map` | 是 | 去重的 `Citation` 列表 |
| `filter_summary` | 是 | 按稳定 reason code 汇总的允许、拒绝和截断计数 |
| `content_digest` | 是 | 整个逻辑 ContextPack 的 `Digest`；profile 为 `context-pack-v1` |

`Budget` 包含 `unit`、`limit` 与 `used`。M0 的 `unit` 必须为 `utf8_bytes`，且 `0 <= used <= limit`；未来使用 `tokens` 时必须通过新版本同时冻结 tokenizer 标识。

每个 `ContextItem` 必须包含唯一 `item_id`、`ordinal`、`item_type`、`object_refs`、`rendered_content`、`content_digest`、非空 `evidence_refs`、`citation_ids`、非空 `selection_reason_codes`、`temporal_role` 与截断事实。`item_type` 允许 `source_fragment`、`memory_record`、`conflict_notice` 和 `constraint`；`temporal_role` 允许 `current`、`historical`、`conflict` 和 `not_applicable`。除 constraint 外，`object_refs` 必须非空；constraint 可以只引用 `policy_basis` evidence。

每个 `Citation` 必须包含唯一 `citation_id`、`source_id`、`fragment_id`、字节范围与片段摘要。所有 citation 必须解析到 active 来源并与正文匹配。普通 `memory_record` item 只允许 current state 为 confirmed；冲突内容必须进入显式 `conflict_notice`，未确认 proposal 永远不能成为 ContextItem。

## DeleteRequest

`DeleteRequest` 表达删除意图与执行前冻结的单设备影响面，不代表完成。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `DeleteRequest` |
| `delete_request_id` | 是 | `Identifier` |
| `namespace_id` | 是 | `Identifier` |
| `requested_by` | 是 | `ActorRef` |
| `authorization_basis` | 是 | 非空授权标识 |
| `requested_guarantee` | 是 | `stop_recall` 或 `local_purge` |
| `scope` | 是 | M0 精确值 `local_device` |
| `device_id` | 是 | 受信本地设备 `Identifier` |
| `target_refs` | 是 | 非空、去重的语义目标集合 |
| `planned_components` | 是 | 非空、去重的 `DeletionTarget` 集合 |
| `reason_code` | 是 | 非空稳定代码 |
| `requested_at` | 是 | `Timestamp` |

`DeletionTarget` 包含稳定 `component_key`、`component_type`、`target_ref`、正整数 `target_count` 和 `required_action`。`target_ref` 可以是 canonical `ObjectRef`，也可以引用某一组件内不可变、已排序的目标闭包；后者必须把完整目标集合纳入引用摘要，不能只保存模糊查询条件。它不得是数据库行号或真实文件路径。M0 的 `component_type` 至少可表达 `source_body`、`source_metadata`、`source_fragment`、`memory_proposal`、`memory_decision`、`memory_record`、`memory_state_event`、`full_text_index`、`context_cache` 和 `minimal_audit`；`required_action` 为 `delete`、`redact` 或 `retain_minimal`。

执行前必须先完成 target expansion 并冻结 `planned_components`。无法枚举影响面时请求保持失败关闭，不能边猜测边声明完成。

## DeletionEvidence

`DeletionEvidence` 对一个 DeleteRequest 的每个计划组件给出可复验结果。

| 字段 | 必填 | 类型或约束 |
| --- | --- | --- |
| `schema_version` | 是 | `radishmemory.m0/1` |
| `object_type` | 是 | `DeletionEvidence` |
| `deletion_evidence_id` | 是 | `Identifier` |
| `delete_request_id` | 是 | 现存 `DeleteRequest.delete_request_id` |
| `previous_evidence_id` | 否 | 同一请求的前一证据快照 |
| `namespace_id` | 是 | 与请求相同 |
| `scope` | 是 | M0 精确值 `local_device` |
| `device_id` | 是 | 与请求相同 |
| `overall_status` | 是 | `pending`、`partial`、`failed` 或 `completed` |
| `component_results` | 是 | 与 `planned_components` 一一对应的结果集合 |
| `started_at` | 是 | `Timestamp` |
| `finished_at` | 条件 | `overall_status` 非 pending 时必须存在 |
| `verified_by` | 是 | `ProducerRef` |
| `evidence_digest` | 是 | 不包含被删除正文的 `Digest`；profile 为 `deletion-evidence-v1` |

每个 `ComponentResult` 必须包含对应 `component_key`、`component_type`、`target_ref`、`required_action`、`target_count`、非负 `processed_count`、`status`、`outcome`、`verification_method` 与 `checked_at`。`status` 为 `pending`、`succeeded` 或 `failed`；`outcome` 为 `deleted`、`redacted`、`retained_minimal`、`not_found` 或 `not_applicable`。`succeeded` 要求 `processed_count = target_count`；集合中任一目标无法复验时整个组件不能成功。失败结果还必须包含稳定 `error_code` 与 `retryable`，但不得复制正文、密钥或真实路径。

每个 DeletionEvidence 都是不可变快照；重试或进展必须创建新证据并引用前一快照。仅当全部计划组件都有唯一结果、必需动作验证成功，且 `retain_minimal` 具有明确保留依据时，`overall_status` 才能为 `completed`。`not_found` 只有在验证对象确实不存在且不会被索引或缓存恢复时才算成功。M0 证据不得扩展成其它设备、服务端或备份已删除的声明。

## 跨对象不变量

1. SourceFragment 必须解析到同 namespace 的 active SourceArtifact，并通过字节范围和摘要复验。
2. MemoryProposal 必须至少引用一个 SourceFragment；来源治理标签以最严格值向下继承。
3. accept MemoryDecision、confirmed MemoryRecord 与初始 confirmed MemoryStateEvent 必须形成一一可追溯闭环。
4. 未终态 accept 的 proposal 不得产生可召回 MemoryRecord。
5. supersede 必须创建新 MemoryRecord 和旧记录的 superseded 事件；事件 `effective_at` 与新记录有效起点在已知时必须一致，不得为补写旧记录的有效终点而修改旧内容。
6. ContextPack 必须先按 namespace、治理标签、状态和删除状态失败关闭，再选择内容；citation 无法复验即整体构建失败。
7. DeleteRequest 创建后，受影响对象至少进入 pending 并立即退出普通召回；失败不能恢复成 active。
8. DeletionEvidence 只能报告本次请求冻结的组件集合，不能用未枚举组件的成功推断完整删除。
9. 日志、错误和证据只保留稳定 ID、摘要、状态、时间和最小原因，不复制正文或完整 ContextPack。
10. 所有 M0 fixture 与输出必须是合成数据，且核心闭环不得产生网络请求或 Provider 记录。

## 演进边界

- 兼容实现可以增加非 canonical 的内部字段，但不得暴露为同版本契约、改变摘要输入或影响规范行为。
- 增加可选字段仍需评审其隐私、摘要和迁移影响；改变字段含义、必填性、枚举闭集、时间语义、摘要 profile 或引用关系必须升级 schema 版本。
- 未知版本必须返回显式 unsupported schema 错误；不得按最新版本猜测解析。
- fixture 的具体 JSON 表示、操作序列和指标计算已经由 [M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 映射到本文；若映射暴露语义缺口，应先修订并重新评审本规范，而不是在测试代码中建立第二套 schema。
