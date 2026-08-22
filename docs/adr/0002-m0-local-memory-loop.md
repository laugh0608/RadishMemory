# ADR 0002：M0 本地记忆闭环

日期：2026-08-22

状态：Accepted

## 背景

RadishMemory 的长期路线包含多媒体采集、混合检索、多模型、同步和个人伴侣体验，但这些能力不能同时作为首批实现的前置条件。进入实现前需要一个足够窄、能够验证核心不变量、又不会把临时技术选择固化为长期格式的垂直切片。

当前真相源已经要求来源可追溯、模型只能提出候选、时间变化保留历史、权限失败关闭以及删除状态可复验。缺少的是这些要求在首个可执行切片中的精确边界与验收顺序。

## 决策

首个可执行切片命名为 `M0 Local Memory Loop`。它只证明本地单设备上的最小记忆治理闭环，不代表完整 MVP、生产能力或隐私保证。

### 运行边界

| 项目 | M0 决策 |
| --- | --- |
| 用户与 namespace | 单用户、单默认 namespace |
| 设备 | 单个受信本地设备 |
| 输入 | 合成短文本与 Markdown |
| 原始资料 | 保留用户输入正文及稳定内容摘要 |
| 片段 | 确定性分段并保留稳定来源引用 |
| 检索 | 本地全文基线与确定性过滤 |
| 模型 | 不调用云端或本地生成模型 |
| 网络 | 核心验收期间禁止网络外发 |
| 记忆形成 | 规则或测试驱动的 `MemoryProposal`，必须经过 `MemoryDecision` |
| 上下文 | 本地生成带 citation map 的 `ContextPack` |
| 更正 | 新版本与 `supersedes`，不覆盖旧记录 |
| 删除 | 单设备范围的 `DeleteRequest` 与 `DeletionEvidence` |
| RadishMind | 不进入运行链路，只保留未来契约边界 |
| 同步 | 不实现；首个同步信任模式仍由后续阶段决策 |

M0 不支持 PDF、图片、OCR、音频、Embedding、向量索引、图数据库、Provider SDK、后台模型整理、多设备同步、备份清除、虚拟形象或通用聊天界面。

### 最小处理闭环

```text
CaptureRequest
  → SourceArtifact
  → SourceFragment
  → SearchCandidate
  → ContextPack + citation map
  → MemoryProposal
  → MemoryDecision
  → confirmed MemoryRecord
  → correction / supersedes
  → DeleteRequest
  → DeletionEvidence
```

处理顺序冻结为：

1. 采集先持久化原始正文、摘要、来源和敏感度，再生成派生片段。
2. 检索先执行 namespace、敏感度、状态和删除过滤，再进行全文候选选择。
3. `ContextPack` 中的每个资料片段和记忆必须能回到 `SourceFragment` 或明确的用户决策事件。
4. 规则、解析器、测试桩和未来模型都只能产生 `MemoryProposal`；它们不能直接创建 confirmed `MemoryRecord`。
5. 用户显式“记住”可以在一次交互中同时产生 proposal 与 accept decision，但两项事实必须分别可审计，不能省略决策事件。
6. 更正创建新记录并指向被替代版本；历史查询仍能看到旧值及其有效时间。
7. 删除先标记目标和影响面，再清理正文、片段、确认记忆、全文索引和缓存，最后按实际结果生成证据。

### 失败关闭规则

- 缺少来源、namespace、敏感度、状态或删除状态时，不进入普通召回。
- citation 无法解析到现存来源时，不能把候选编入 ContextPack。
- proposal 未获得 accept decision 时，不得作为已确认事实或偏好使用。
- 更正的时间或被替代目标不明确时，保留冲突并要求显式决定，不执行静默覆盖。
- 删除任一已枚举本地组件失败时，证据保持 `pending` 或 `failed`，不能报告完成。
- M0 不产生已发送的 `OutboundContextManifest`、Provider trace 或模型用量；测试观察到网络请求即失败。

### Canonical 边界

M0 冻结对象职责和关系，不在本 ADR 中冻结数据库表、序列化格式、ID 编码或语言类型。字段级 canonical schema 由下一工作包定义，但必须覆盖：

- 来源、片段和内容完整性；
- proposal、decision、record 与状态事件；
- observed time、valid time、版本、`supersedes` 和 `contradicts`；
- namespace、sensitivity、retention 和删除状态；
- ContextPack 选择理由、citation map 和 Token / 大小预算；
- DeleteRequest 的目标集合与 DeletionEvidence 的逐组件结果。

Canonical 数据不能包含 Embedding 向量、单一 Provider 类型、数据库行号或不可迁移的模型内部表示。

## 验收

M0 必须用合成数据证明：

1. 保存文本后可以通过全文查询找回，并引用到稳定片段。
2. 未确认 proposal 不会污染当前事实、偏好或 ContextPack。
3. 用户接受后，confirmed record 保留来源和 decision。
4. “以前是 A、现在是 B”通过新版本与有效时间表达，当前和历史查询结果不同且正确。
5. `local_only` 内容不会产生外发记录或网络请求。
6. 删除后正文、片段、确认记忆、全文索引和缓存不再可检索；证据准确列出各组件结果。
7. 不安装模型、不配置 Provider Key、断网时仍能完成整个闭环。

详细场景与证据要求见 [M0 合成验收](../evaluation/m0-local-memory-loop.md)。

## 后果

收益：首批实现可以直接验证 RadishMemory 的差异化不变量；检索质量、记忆治理、隐私和删除不再依赖模型演示；后续替换存储或增加向量检索不会改变 canonical 关系。

代价：首个切片不提供自然语言生成回答、多媒体导入、多设备同步或真实伴侣体验；全文基线只用于契约验证，不能代表最终召回质量。

## 后续决策

完成 M0 实现前仍需依次冻结：字段级 canonical schema、合成 fixture 格式与指标口径、首个同步信任模式、RadishMind 首批参与方式，以及实现栈与迁移边界 ADR。
