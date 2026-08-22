# RadishMemory 记忆模型

## 设计目标

记忆不是一段被向量化的文本。RadishMemory 必须表达：

- 用户保存了什么原始资料；
- 系统从中观察或推断了什么；
- 事实在什么时间有效；
- 用户是否确认；
- 新信息如何更新或否定旧信息；
- 哪些内容可以在什么场景召回；
- 用户要求忘记后哪些副本需要处理。

## 核心对象

### SourceArtifact

用户保存的原始对象，例如文件、短语、网页快照、图片、音频、对话或应用事件。SourceArtifact 是来源，不等于记忆推断。

### SourceFragment

SourceArtifact 中可稳定引用的片段，例如 PDF 页、Markdown 标题段、音频时间码、图片区域或对话 turn。回答引用和记忆来源应尽量指向 SourceFragment。

### Observation

对原始来源的直接记录，例如“用户在 2026-08-22 明确说希望服务端自行部署”。Observation 应尽量接近原文，不把模型解释伪装成用户陈述。

### Claim

关于用户、人物、项目或世界的结构化陈述。Claim 可以来自明确陈述、资料解析或模型推断，必须标记生成方式与置信度。

### Episode

有时间顺序的经历或事件，包含参与者、上下文、动作、结果和用户反馈。Episode 用于回答“发生过什么”和选择相似经验。

### Preference

用户偏好、习惯或风格倾向。Preference 通常会变化，必须允许适用范围、强度、有效时间、例外和确认状态。

### Procedure

用户希望系统如何做事的规则、流程或技能，例如“代码评审时先给阻塞问题”。Procedure 比普通偏好更接近可执行指令，因此需要更严格的来源、权限和变更确认。

### Entity

人物、组织、项目、地点、设备、主题或其它可持续引用的对象。Entity 需要别名、去重和命名空间，但系统不能仅凭名称相似自动合并敏感身份。

### Relation

Entity、MemoryRecord 与 SourceArtifact 之间的关系。关系可以有类型、方向、来源、置信度和有效时间。时间变化通过新关系、失效时间与版本表达，而不是覆盖旧边。

### Projection

由其它对象生成的画像、摘要、时间线、主题页、Markdown 文件或图视图。Projection 不是唯一真相，必须记录输入摘要与生成器版本，并能重建。

## MemoryRecord 基础字段

字段级 canonical schema 必须在首批实现前冻结；数据库表、序列化布局和语言类型可在实现栈 ADR 中决定。所有长期记忆至少应表达：

```text
memory_id
memory_type
namespace
subject_ref
content / structured_value
status
source_refs[]
observed_at
valid_from / valid_to
confidence
importance
sensitivity
retention_policy
created_by
created_at / updated_at
version
supersedes[]
contradicts[]
generator / generator_version
content_digest
```

其中：

- `confidence` 表示事实或推断可信度，不代表访问授权；
- `importance` 影响召回排序，不代表永久保留；
- `sensitivity` 和权限由 Policy Engine 硬控制；
- `source_refs` 为空的模型推断不能自动进入已确认长期记忆；
- `generator` 必须区分用户、规则、解析器和具体模型。

## 状态机

建议的基础状态：

```text
proposed
   ├── accept decision ──► confirmed
   ├── reject decision ──► rejected
   └── expiry event ─────► expired

confirmed
   ├── superseded
   ├── contradicted
   ├── retracted
   └── expired
```

`defer` 决定不会产生新的状态，proposal 保持 `proposed`，等待后续决定或过期事件。

- `proposed`：模型、解析器或规则提出，尚不是长期确认事实。
- `confirmed`：用户明确确认，或满足用户授权的确定性规则。
- `rejected`：用户拒绝，不应被相同证据反复提出。
- `superseded`：新版本取代当前适用值，但历史仍可查询。
- `contradicted`：存在相互冲突的证据，尚未安全收敛。
- `retracted`：用户撤回或来源被判定错误，默认不得用于回答当前事实。
- `expired`：超过适用期或保留期，不再用于普通召回。

状态变化应形成追加事件，当前状态可以作为投影物化。

## 记忆写入

记忆写入分为三条路径：

### 用户显式写入

例如“记住我更喜欢简洁的中文说明”。可以在一次交互中产生 proposal 和 accept decision，但仍须分别保存候选、决定、来源、作用域和时间，不能跳过可审计的决策事件。

### 热路径候选

模型在回答当前请求时生成 `MemoryProposal`。优点是及时、可向用户展示；缺点是增加延迟并可能干扰主任务。默认不得静默确认高影响记忆。

### 后台整理候选

后台任务对近期资料做去重、归纳、冲突检测和低频总结。它只产生候选或派生投影，不能绕过用户策略。

## MemoryProposal

模型或解析器不能直接修改 MemoryRecord，应提交结构化候选：

```json
{
  "operation": "propose_upsert",
  "memory_type": "preference",
  "subject_ref": "user:self",
  "content": "用户偏好简洁的中文技术说明",
  "scope": "technical_communication",
  "source_refs": ["turn:example"],
  "confidence": 0.82,
  "valid_from": null,
  "sensitivity": "personal",
  "reason": "用户在当前对话中明确表达"
}
```

规则层负责 schema、来源、权限、重复、冲突和敏感度检查，用户或显式授权策略负责最终决定。

## MemoryDecision

MemoryDecision 是对一个具体 MemoryProposal 的不可变决定事件，至少区分 accept、reject 和 defer。它必须记录 proposal 引用、决定者、授权依据、原因和决定时间；未来允许确定性自动规则时，还必须记录规则与版本。

accept decision 可以创建或连接一个 confirmed MemoryRecord；reject decision 使候选进入 rejected，并保留阻止相同证据重复提议所需的最小摘要；defer 不改变候选的未确认性质。已记录的决定不可修改；需要纠错时创建新的撤回 / 更正事件和必要的新 proposal，不原地改写历史决定。

## M0 状态与操作约束

M0 只实现足以证明治理闭环的对象和转换：SourceArtifact、SourceFragment、MemoryProposal、MemoryDecision、confirmed MemoryRecord、状态事件、ContextPack、DeleteRequest 和 DeletionEvidence。

- 规则和测试桩与未来模型一样，只能创建 proposal。
- accept decision 创建 confirmed record；reject decision 保留去重所需摘要，但不能进入确认召回。
- 更正创建新 confirmed record，并通过 `supersedes` 和有效时间连接旧记录。
- 无法安全收敛的互斥来源进入 contradicted 状态，不自动选择得分更高者。
- 删除事件必须先枚举本地影响面，再逐项记录完成、pending 或 failed。

M0 的完整运行与验收边界见 [ADR 0002](adr/0002-m0-local-memory-loop.md)。

## 时间与冲突

RadishMemory 不把用户建模成永远不变的单份 Profile。对于“以前喜欢 A、现在喜欢 B”，应保留：

- A 的来源与有效时间；
- B 的来源与生效时间；
- B 是否 supersede A；
- 查询是在问当前状态还是历史状态。

无法判断时，系统应同时返回冲突证据或向用户澄清，而不是选择 Embedding 得分更高的一条。

## 召回流程

召回不是简单 `top_k vector search`：

1. 确定主体、命名空间、任务和时间语义；
2. 权限、敏感度、外发和撤回状态硬过滤；
3. 全文、向量、实体、时间、来源和关系多路召回；
4. 去重并扩展必要来源上下文；
5. 根据相关性、时间、重要度、来源可信度和覆盖率 rerank；
6. 识别冲突、过期和仅为候选的内容；
7. 在 Token 预算内构建 ContextPack；
8. 生成 citation map 和外发清单。

召回结果必须保留“为什么选中”以及“为什么不能使用”的可解释信息。

## 记忆压缩与分层

长期资料可以形成多层表示：

- 工作记忆：当前会话和任务状态；
- 近期事件：最近发生、尚未稳定归纳的 Episode；
- 语义记忆：已确认事实、概念和偏好；
- 程序性记忆：用户确认的做事规则与技能；
- 档案记忆：低频使用但可检索的历史资料；
- 派生画像：为特定场景生成的可重建 Profile。

压缩不能删除原始来源。摘要失真时，应能回到片段和原件重新编译。

## 遗忘与删除

需要区分：

- **不再召回**：从普通检索中撤回，但保留受控审计记录；
- **逻辑删除**：对象进入删除流程，等待索引、设备和备份传播；
- **物理清除**：正文、对象和派生索引已删除或通过密钥销毁变得不可恢复；
- **备份到期**：受控备份超过保留期或完成重加密/销毁。

DeleteRequest 表达用户要求删除或停止召回的目标、范围、期望保证、请求者和请求时间。它是删除意图，不等于删除完成。

删除应生成 `DeletionEvidence`，列出 Source Vault、Memory Store、全文索引、向量索引、图投影、缓存、同步设备和备份的状态。系统只能在证据满足对应信任模式时声明完成。

## 评测要求

记忆系统至少需要覆盖：

- 单跳与多跳召回；
- 时间变化与 point-in-time 查询；
- 同名人物和实体消歧；
- 矛盾来源；
- 用户明确纠错；
- 无关历史干扰；
- 记忆候选误写；
- 敏感资料禁止外发；
- 删除后全文、向量和图残留；
- Embedding 或模型更换后的重建与迁移。
