# M0 Fixture 与指标契约

状态：Frozen M0 baseline

版本：`radishmemory.m0-fixture/1`

本文把 [M0 Canonical Schema](../schema/m0-canonical-schema.md) 和 [M0 合成验收](m0-local-memory-loop.md)映射为机器可读、可重复执行的测试输入与 oracle。规范 fixture 位于 [`fixtures/m0/local-memory-loop.v1.json`](../../fixtures/m0/local-memory-loop.v1.json)，仓库校验入口为：

```bash
./scripts/check-m0-fixtures.py
```

校验器只证明 fixture 格式、操作顺序、稳定 ID、摘要向量和指标聚合自洽，不证明 Source Vault、检索、记忆、删除或网络隔离已经实现。

## 信任与数据边界

- suite、场景、正文、人物、项目、设备和标识全部是合成数据。
- 每个场景在独立逻辑存储中运行，不能依赖前一场景的对象、缓存、时钟或副作用。
- runner 必须禁用真实网络、Provider、模型、系统密钥链和用户资料路径；fixture 中的内容始终作为不可信数据处理。
- runner 输出只能保存 fixture ID、稳定对象 ID、摘要、状态、计数、时间和最小错误代码；不得把完整 ContextPack 或全部正文复制到普通日志。
- fixture suite 不是生产导入格式、同步协议或用户导出格式。

## JSON 映射

### 顶层结构

suite 顶层必须包含：

| 字段 | 约束 |
| --- | --- |
| `fixture_contract_version` | 精确值 `radishmemory.m0-fixture/1` |
| `canonical_schema_version` | 精确值 `radishmemory.m0/1` |
| `suite_id` | 稳定合成 suite 标识 |
| `data_classification` | 精确值 `synthetic` |
| `namespace_id` | 所有场景共享的合成 namespace |
| `device_id` | 单个合成受信本地设备 |
| `canonical_json_profile` | 精确值 `radishmemory-canonical-json-v1` |
| `fixture_id_profile` | 精确值 `radishmemory-fixture-id-v1` |
| `governance_profiles` | 命名且不可变的合成治理标签 |
| `deletion_profiles` | 删除影响面的组件类型与要求动作模板 |
| `id_vectors` | 九种 canonical 对象的稳定 ID 测试向量 |
| `digest_vectors` | 原始字节、NFC 文本和 canonical JSON 摘要向量 |
| `metric_gates` | suite 级指标类型、比较器和阈值 |
| `scenarios` | 按 `M0-E01` 至 `M0-E12` 排序的场景 |
| `suite_digest` | 排除自身后对 suite 计算的摘要 |

可选字段必须省略，不使用 `null`、空字符串、非有限数值或未声明扩展字段表示“未知”。JSON object key 顺序不承载语义；array 是否有序由字段定义决定。

`deletion_profiles` 只定义组件类型和动作模板。`plan_delete` 必须先把语义目标扩展成已排序目标闭包，为每个组件冻结 `target_ref` 与 `target_count`；一个组件可以引用集合，但集合任一成员未处理都不能把该组件报告为 succeeded。

### `radishmemory-canonical-json-v1`

用于 fixture 和复合对象摘要的 canonical byte mapping 冻结为：

1. 输入必须是 RFC 8259 语义的 JSON object、array、string、number、boolean 或 null；本版本 fixture 进一步禁止 null。
2. object key 按 Unicode code point 升序排列；不得出现重复 key。
3. array 保留原顺序；schema 中表示集合的 array 必须在进入 canonicalizer 前按稳定 ID 排序并去重。
4. string 使用 JSON 必需转义，未被要求转义的 Unicode 字符直接编码为 UTF-8；不做隐式 Unicode normalization。
5. integer 使用最短十进制表示，不允许前导零或负零。
6. fraction 不允许指数、前导加号、尾随零或负零，必须使用最短普通十进制表示。
7. separator 精确为 `,` 与 `:`，不包含空白；最终字节不追加换行。

fixture 文件本身可以使用缩进便于评审；所有摘要都对上述 canonical bytes 计算，而不是对仓库中的 pretty-printed bytes 计算。

## 摘要 profile

`Digest` 均使用 `algorithm = sha256` 和小写十六进制 value：

| profile | 输入 |
| --- | --- |
| `exact-bytes-v1` | 原始 UTF-8 字节，保留换行与 Unicode 形式 |
| `utf8-nfc-text-v1` | 文本先做 Unicode NFC，再编码 UTF-8 |
| `canonical-json-v1` | `radishmemory-canonical-json-v1` 输出 |
| `fixture-suite-v1` | suite 删除顶层 `suite_digest` 后的 canonical JSON |
| `context-pack-v1` | ContextPack 删除 `content_digest` 后的 canonical JSON |
| `deletion-evidence-v1` | DeletionEvidence 删除 `evidence_digest` 后的 canonical JSON |

任何摘要不匹配、未知 profile 或对包含自身摘要字段的对象计算摘要都必须失败。摘要只证明给定输入一致，不证明内容可信或已授权。

## Fixture 稳定 ID

fixture 对象使用：

```text
urn:radishmemory:fixture:<scenario-lower>:<object-type-kebab>:<logical-key>
```

- `scenario-lower` 是 `M0-E01` 这类 ID 的 ASCII lowercase。
- `object-type-kebab` 由 canonical object type 固定映射，例如 `SourceArtifact` 映射为 `source-artifact`。
- `logical-key` 只能使用 ASCII lowercase、数字和单个 `-` 分隔符，且包含足以区分版本的语义名称。
- fixture ID 只用于可重复测试；生产 ID 仍是不透明实现决策。调用方不得通过解析前缀获得授权或对象类型。
- 同一 identity material 必须得到相同 ID；不同 scenario、object type 或 logical key 必须得到不同 ID。

规范 suite 的 `id_vectors` 必须覆盖九种顶层对象，校验器逐项重算。

## 场景结构

每个 scenario 必须包含：

| 字段 | 约束 |
| --- | --- |
| `scenario_id` | `M0-E01` 至 `M0-E12`，有序且不重复 |
| `title` | 简短合成场景名称 |
| `isolation_key` | 独立逻辑存储与缓存作用域 |
| `operations` | 冻结的有序操作列表 |
| `metric_observations` | 对 suite gate 的整数或有理数贡献 |

每个 operation 包含唯一 `step_id`、`op`、`input` 和 `expect`。`step_id` 使用 `m0-e01-s01` 格式；`expect.status` 为 `succeeded`、`rejected`、`pending` 或 `failed`，`expect.assertions` 是非空稳定代码列表。runner 可以保存更多诊断，但通过与否只能由冻结 assertion 和 metric 决定。

### 冻结操作

| `op` | 语义 |
| --- | --- |
| `assert_environment` | 证明无模型、无 Provider Key、网络被禁止 |
| `capture` | 从合成正文创建 SourceArtifact |
| `segment` | 使用 `m0-lines-v1` 生成稳定 SourceFragment |
| `search` | 先失败关闭再执行本地全文 top-k |
| `compile_context` | 构建本地 ContextPack 与 citation map |
| `propose` | 创建 MemoryProposal，不改变 confirmed truth |
| `decide` | 追加 accept、reject 或 defer MemoryDecision |
| `materialize_memory` | 由 accept 决定创建 MemoryRecord 和初始状态事件 |
| `attempt_duplicate_proposal` | 验证被拒绝候选不会按相同证据重复出现 |
| `query_at` | 执行明确 point-in-time 查询 |
| `query_current` | 以显式 `as_of` 查询当前适用值 |
| `detect_conflict` | 标记互斥证据且不自动确认任一值 |
| `plan_delete` | 展开并冻结 DeleteRequest.planned_components |
| `execute_deletion` | 对每个计划组件产生真实 component result |
| `emit_deletion_evidence` | 生成不可变 DeletionEvidence 快照 |
| `assert_no_network` | 断言请求、manifest、Provider trace 和用量均为零 |
| `seed_noise` | 通过固定 seed 生成无关合成资料 |
| `compare_source_set` | 比较噪声前后的标注来源集合 |

操作不能省略中间治理事实。例如 `materialize_memory` 不能在没有 accept decision 时运行；`execute_deletion` 不能在 planned components 冻结前运行；`compile_context` 不能把 proposal 当成 confirmed record。

## 固定场景顺序

校验器对每个场景要求精确的 `op` 序列：

| 场景 | 操作目标 |
| --- | --- |
| `M0-E01` | capture → segment |
| `M0-E02` | capture → segment → search → compile_context |
| `M0-E03` | capture → segment → propose → search → compile_context |
| `M0-E04` | capture → segment → propose → decide → materialize_memory → search → compile_context |
| `M0-E05` | capture → segment → propose → decide → attempt_duplicate_proposal |
| `M0-E06` | 两轮 capture / segment / propose / decide / materialize_memory → query_at → query_current |
| `M0-E07` | 两轮 capture / segment / propose → detect_conflict → compile_context |
| `M0-E08` | capture → segment → search → compile_context → assert_no_network |
| `M0-E09` | capture → segment → propose → decide → materialize_memory → plan_delete → execute_deletion → emit_deletion_evidence → search |
| `M0-E10` | capture → segment → plan_delete → execute_deletion → emit_deletion_evidence |
| `M0-E11` | assert_environment → 完整确认、更正、删除闭环 → assert_no_network |
| `M0-E12` | capture → segment → search → seed_noise → search → compare_source_set |

修改顺序、增加操作或放宽预期必须同时更新真相源、fixture、校验器与契约测试。

## 指标聚合

所有输入计数均为非负整数。ratio 用整数 numerator / denominator 聚合，先对 suite 内观察值分别求和，再以精确有理数比较；不得逐场景四舍五入。denominator 为零时 ratio gate 失败。

| metric | 类型 | M0 gate |
| --- | --- | --- |
| `citation_resolve_rate` | ratio | `1 / 1` |
| `retrieval_recall_at_5` | ratio | `1 / 1` |
| `unconfirmed_context_count` | count | `0` |
| `duplicate_reproposal_count` | count | `0` |
| `silent_overwrite_count` | count | `0` |
| `silent_conflict_selection_count` | count | `0` |
| `policy_violation_count` | count | `0` |
| `network_request_count` | count | `0` |
| `deletion_component_coverage` | ratio | `1 / 1` |
| `false_complete_deletion_count` | count | `0` |
| `model_free_loop_completion_rate` | ratio | `1 / 1` |
| `relevant_source_set_drift_count` | count | `0` |

`retrieval_recall_at_5` 的 numerator 是 top 5 中命中的唯一标注相关对象数，denominator 是全部唯一标注相关对象数。重复命中不重复计数；无标注相关对象的查询不进入该指标。

`deletion_component_coverage` 的 numerator 是具有唯一 ComponentResult、且结果 `target_count` 与计划一致的 component key 数，denominator 是 DeleteRequest 中唯一 planned component key 数；结果成功与否另由状态和 false-complete gate 判断。计划集合为空、目标闭包未冻结或 processed count 超过 target count 都直接失败。

`model_free_loop_completion_rate` 只观察 `M0-E11`：所有要求步骤和 assertion 通过计 1，否则计 0。预期失败行为，例如 `M0-E10` 的组件失败，不降低该指标；如果它错误报告 completed，则增加 false-complete count。

## Runner 输出协议

当前 runner 输出以下确定性最小证据：

```text
fixture_contract_version
suite_id / suite_digest
implementation_id / implementation_version
canonical_schema_version
adapter id / schema_version
logical_clock
scenario_id / step_id / operation
expected_status / observed_status / passed / assertion results
emitted logical keys / object IDs / digest profiles and values
metric observations / suite aggregates / metric gates
network_interceptor mode / request_count / passed
started_at / finished_at
error_code / retryable
```

`started_at` / `finished_at` 是冻结 suite 的逻辑证据边界，不是 wall-clock 性能测量；runner 报告不得包含临时路径或以运行耗时制造性能承诺。没有 error 的步骤省略 `error_code` / `retryable`，而不是使用 `null`。任何新增输出都必须保持确定性、最小化并遵守本节数据边界。

缺失步骤、未识别 assertion、未识别 metric、额外网络请求、unsupported schema、悬空引用或摘要不一致都必须令本次 suite 失败。runner 不得用默认成功、跳过、重试后隐藏首个错误或把 expected failure 当成未运行。

## 变更规则

- 修正文案但不改变机器语义时，更新文档即可。
- 改变 JSON 字段、op 顺序、ID profile、digest profile、metric 公式或 gate 必须更新 fixture contract 版本或明确证明向后兼容。
- fixture 内容变化必须重算 `suite_digest`；校验器拒绝陈旧摘要。
- fixture 暴露 canonical schema 缺口时，先修订 schema 与真相源，再更新 fixture，不能只在 runner 中增加 fallback。

## 当前实现的证据边界

2026-09-05 审阅在 `3c10cd2` 确认：来源、记忆和删除操作调用真实 core / SQLite，但 runner 还承担词项扩展、历史投影与 ContextPack 编排。suite 通过证明当前组合路径在固定合成输入上满足已执行断言，不自动证明 production application 具备对应历史查询、查询扩展或上下文编译接口。

特别是 `context_search.rs` 中 `policy-filter-ran-first` 当前返回常量 `true`，该项自身没有观察过滤顺序；`query_at` 遍历 runner 内存记录并构造历史投影；零结果词项扩展不在 production application 中。这些是待修复的实现 / 证据缺口，不符合以实际结果证明断言的目标，也不代表已经发现权限泄露。

原有禁止默认成功、隐藏错误及改变 oracle 掩盖问题的要求继续有效。后续须以实际可观察的拒绝证据和正式历史 / 检索路径收口，不通过放宽断言或修改预期使缺口消失。新增 production 质量场景见[本地资料库质量验收计划](phase1-local-library-quality.md)，不修改当前 fixture schema、摘要、86 个操作或指标公式。
