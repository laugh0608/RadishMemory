# M0 本地记忆闭环合成验收

本文定义 [ADR 0002](../adr/0002-m0-local-memory-loop.md)的可执行验收边界。它描述测试事实、操作、预期结果和必须保留的证据，不指定测试框架或实现语言。

测试对象和引用必须符合 [M0 Canonical Schema](../schema/m0-canonical-schema.md)；具体 JSON mapping、操作协议、稳定 ID、摘要向量和指标聚合以 [M0 Fixture 与指标契约](m0-fixture-contract.md)及其机器 suite 为准，不得另造平行字段。

## 数据边界

- 所有人物、项目、内容、时间、设备和标识均为合成数据。
- fixture 不包含真实用户资料、真实 Provider 请求、密钥、路径、数据库或 ContextPack。
- 每个场景显式声明 observed time、valid time、namespace、sensitivity 和预期来源。
- 同一 fixture 可以生成大量无关资料验证抗干扰，但正确来源集合必须人工标注。

## 固定场景

| ID | 场景 | 核心预期 |
| --- | --- | --- |
| `M0-E01` | 采集短文本与 Markdown | 原文、摘要、稳定 `SourceFragment` 和来源元数据一致 |
| `M0-E02` | 全文召回与引用 | 正确片段进入候选，citation 能解析到原文 |
| `M0-E03` | 未确认候选 | proposal 可审查，但不进入 confirmed 召回或 ContextPack |
| `M0-E04` | 接受候选 | decision 与 confirmed record 分离留痕，record 保留来源 |
| `M0-E05` | 拒绝与重复提议 | rejected proposal 不以相同来源和内容反复出现 |
| `M0-E06` | 时间更正 | B supersede A；当前查询返回 B，历史查询仍能返回 A |
| `M0-E07` | 冲突但无法收敛 | 同时保留冲突证据，不按检索分数静默选边 |
| `M0-E08` | `local_only` 策略 | 内容可本地检索，但无外发 manifest、Provider trace 或网络请求 |
| `M0-E09` | 删除传播 | 正文、片段、记忆、全文索引和缓存逐项处理并生成准确证据 |
| `M0-E10` | 删除部分失败 | 结果保持 pending / failed，不显示完全删除 |
| `M0-E11` | 无模型与断网 | 不配置模型或密钥仍完成采集、检索、确认、更正和删除 |
| `M0-E12` | 无关历史干扰 | 大量无关合成资料不改变标注来源和时间判断 |

## 关键 fixture 叙事

时间变化 fixture 使用合成人物 `user:sample` 和项目 `project:orchard`：

1. `2026-01-10` 明确记录“项目默认使用蓝色主题”，有效时间从当天开始。
2. 用户接受该 preference proposal。
3. `2026-03-20` 明确更正为“项目默认使用绿色主题”，并接受新 proposal。
4. 新记录 `supersedes` 旧记录；superseded 事件的生效时间与新记录的有效起点一致，查询得到的旧记录有效终点也落在该边界。
5. 查询“现在的默认主题”只把绿色作为当前确认值；查询“二月的默认主题”返回蓝色及原始 citation。

冲突 fixture 使用两个不同合成来源给出互斥值，但不提供用户决定。系统必须报告 conflict，不得用向量或全文得分把任一值改成 confirmed current value。

删除 fixture 必须至少枚举：

- SourceArtifact 正文；
- SourceFragment；
- MemoryProposal、MemoryDecision 与 MemoryRecord 的可保留 / 应清除边界；
- 全文索引；
- ContextPack 或查询缓存；
- 最小审计与 DeletionEvidence。

审计和证据只能保留完成验证所需的稳定 ID、摘要、状态和时间，不能复制已删除正文。M0 不声称覆盖其它设备、服务端副本或备份到期。

## 指标与门禁

| 指标 | M0 门禁 |
| --- | --- |
| 标注 citation 可解析率 | `100%` |
| 未确认 proposal 进入 confirmed 上下文 | `0` |
| 静默覆盖历史记录 | `0` |
| 策略违规或网络外发 | `0` |
| 已枚举本地删除组件覆盖率 | `100%` |
| 错误声明完全删除 | `0` |
| 无模型闭环完成率 | `100%` |
| 标注 Retrieval Recall@5 | `100%` |
| 无关历史导致的相关来源集合漂移 | `0` |

M0 fixture 已将全文基线固定为 Retrieval Recall@5，并用一个目标来源与 1000 条固定 seed 的无关资料验证来源集合不漂移。延迟和大规模压缩率仍需在实现栈、运行环境和数据规模具备代表性后设定，当前不使用小样本制造虚假的性能承诺。

## 每次验证必须保存的证据

- fixture 版本与内容摘要；
- 执行的操作序列和稳定对象 ID；
- proposal / decision / state event；
- 候选选择、过滤理由和 citation 解析结果；
- 当前时间与 point-in-time 查询结果；
- DeleteRequest 目标和逐组件 DeletionEvidence；
- 网络禁用或请求拦截结果；
- 未通过项、未覆盖项和实际错误原因。

证据可以是结构化测试输出，但不得包含真实资料、秘密或未最小化的完整私密正文。

## 退出条件

字段级 canonical schema、fixture 格式、操作序列和指标 oracle 均已冻结，仓库校验器可以验证契约自洽。真实 runner 必须在 canonical core 与 SQLite adapter 成立后调用同一套产品实现，把上述场景转成无网络、无 Provider Key、可重复执行的产品测试，不得在 runner 内复制领域或存储逻辑。全部强制门禁通过之前，不进入 PDF / OCR、Embedding、多模型或同步实现。

## 通过结论的使用范围

M0 满分是固定合成 suite 的回归证据，不是中文检索质量、文件数据库大规模性能或生产历史查询已成立的结论。runner 中默认成功断言、词项扩展和历史投影的实际限制见[fixture 证据边界](m0-fixture-contract.md#当前实现的证据边界)。后续 production 验收补充中文、高相似干扰、无答案、重启和回源场景；不降低本文件既有门禁。
