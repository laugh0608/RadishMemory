# RadishMemory 系统架构

## 架构目标

RadishMemory 必须同时满足：

- 单机离线可用；
- 用户自部署多端同步；
- 原始资料、结构化记忆和派生索引相互分离；
- 模型、Embedding、数据库和 Provider 可替换；
- 权限、敏感度和外发策略在检索前执行；
- 所有回答、记忆写入和删除动作可追溯；
- 虚拟形象和聊天体验不拥有独立记忆真相。

## 总体结构

```text
采集入口
  短语 / 文件 / 网页 / 图片 / 音频 / 对话 / 应用事件
        │
        ▼
Source Vault ── 原始对象、元数据、内容摘要、版本和来源
        │
        ▼
Ingestion Pipeline ── 解析、OCR、转写、分段、分类、风险扫描
        │
        ├──────────────► Derived Indexes
        │                 全文 / 向量 / 实体 / 时间 / 图 / 摘要
        ▼
Memory Engine ── Observation / Claim / Episode / Preference / Procedure
        │
        ▼
Policy-aware Retrieval ── 权限硬过滤 + 混合召回 + rerank
        │
        ▼
Memory Compiler ── ContextPack + Outbound Context Manifest
        │
        ▼
Model Adapter / RadishMind ── GPT / Gemini / Claude / Grok / DeepSeek / Local
        │
        ▼
回答、引用、Trace、MemoryProposal
        │
        └──────────────► 规则校验 / 用户确认 / 记忆状态变更
```

多端同步独立于模型调用：

```text
Device A Local Store ── encrypted operation/object sync ── Self-hosted Server
                                                           │
Device B Local Store ◄── encrypted operation/object sync ──┘
```

## M0 本地架构边界

首个可执行切片只启用总体架构的本地最小子集：Capture Gateway 接收合成短文本与 Markdown；Source Vault 保存正文和摘要；Ingestion Pipeline 做确定性分段；Derived Indexes 只包含全文基线；Policy Engine 执行 namespace、敏感度、状态和删除过滤；Memory Engine 管理 proposal、decision、record、时间更正和删除事件；Memory Compiler 生成带 citation map 的本地 ContextPack。

M0 不调用 Model Adapter 或 RadishMind，不产生已发送的外发 manifest、Provider trace 或模型用量，也不实现同步。核心验收期间任何网络请求都视为失败。这个边界不删除长期组件，只避免在来源、状态、权限和删除契约尚未成立时引入模型与分布式复杂度。完整决策见 [ADR 0002](adr/0002-m0-local-memory-loop.md)，字段与跨对象不变量见 [M0 Canonical Schema](schema/m0-canonical-schema.md)，可执行操作与指标 oracle 见 [M0 Fixture 与指标契约](evaluation/m0-fixture-contract.md)。

## 核心组件

### 1. Capture Gateway

提供统一采集协议，接收文本、文件、URI、媒体和应用事件。采集必须快速完成原始资料持久化，耗时的 OCR、转写、Embedding 和记忆提取可以异步执行。

每次采集生成稳定 `source_id`、内容摘要、来源类型、采集时间、设备、命名空间和敏感度初值。

### 2. Source Vault

Source Vault 是用户原始资料的真相源，采用内容寻址对象与结构化元数据分离的方式。它负责：

- 保存原始字节或用户明确保存的正文；
- 内容摘要、MIME、大小、来源、版本和完整性检查；
- 加密、保留、归档、删除和同步状态；
- 派生文本、缩略图、转写和分段到原件的引用关系。

大对象不要求进入人类可读仓库或 Git 历史。

### 3. Ingestion Pipeline

导入流水线负责把不同来源转换为统一的可检索材料：

1. 类型识别与安全检查；
2. 文本提取、OCR 或音频转写；
3. 结构化分段与页码、时间码保留；
4. 语言、人物、项目、时间和主题候选提取；
5. Prompt Injection 与不可信指令标记；
6. 派生索引更新；
7. `MemoryProposal` 生成。

导入材料默认是“不可信资料”，其中的命令不得升级为系统指令或程序性记忆。

### 4. Memory Engine

Memory Engine 管理记忆类型、状态机、来源、时间有效性、冲突、确认、撤回和过期。它不依赖单一模型生成结果，所有写入都经过 canonical schema 和确定性校验。

详细语义见 [记忆模型](memory-model.md)。

### 5. Derived Indexes

派生索引可以包含：

- 全文倒排索引；
- 一个或多个版本化向量索引；
- 实体与别名索引；
- 时间、来源、项目、人物和敏感度索引；
- 关系边和时间图投影；
- 分层摘要和查询缓存。

所有索引必须记录生成器、模型、版本、维度、输入摘要和生成时间，并能从 Source Vault 与 Memory Engine 重建。Embedding 不进入长期 canonical 格式。

### 6. Policy Engine

Policy Engine 在检索、同步、导出和模型调用前提供硬边界：

- namespace、用户、设备和应用授权；
- `local_only`、`trusted_server_only`、`cloud_allowed` 等外发级别；
- 来源级、文件级、记忆级和字段级敏感度；
- 模型 Provider、地区、用途和保留策略限制；
- 临时授权、到期、撤销和审计。

拒绝结果不得回退到更宽权限，也不得通过摘要、缓存或 Embedding 绕过限制。

### 7. Retrieval Orchestrator

检索按以下顺序执行：

1. 解析任务、主体、时间范围和期望输出；
2. 执行权限、敏感度和外发硬过滤；
3. 并行进行全文、向量、实体、时间和关系召回；
4. 归并、去重和来源扩展；
5. 使用相关性、时效、重要度、来源可信度和任务覆盖进行 rerank；
6. 显式处理冲突、已撤回、已过期和不确定记忆；
7. 把候选交给 Memory Compiler。

安全策略不是打分项；未获授权的内容不能作为低分候选进入后续步骤。

### 8. Memory Compiler

Memory Compiler 根据任务和 Token 预算生成模型无关的 `ContextPack`。它类似编译器：长期资料是源代码，派生索引是中间表示，ContextPack 是本次模型调用的构建产物。

ContextPack 至少包含：

- task instruction；
- 已选择的资料片段与稳定引用；
- 已确认事实、偏好、相关经历和程序性规则；
- 时间与冲突说明；
- 不确定性和禁止推断边界；
- Token 预算、截断和覆盖说明；
- 可供回答引用的 citation map。

同时生成 `OutboundContextManifest`，记录接收 Provider、资料引用、敏感度、授权依据、摘要和时间，但审计日志不得复制完整私密正文。

### 9. Model Adapter Layer

模型适配层把 canonical 请求翻译为不同 Provider 协议。它负责能力发现、结构化输出、流式响应、错误分类、用量记录和超时取消，不负责决定哪些记忆是真实的。

RadishMind 可以实现这一层或作为其上游服务。RadishMemory 必须保留直接接入和本地模型的能力，避免数据层依赖单一平台。

### 10. Companion Experience

个人伴侣、虚拟形象、语音和主动提醒属于表现与交互层。它们只能通过公共记忆和上下文接口读取或提出修改，不能创建第二套画像、会话或长期记忆数据库。

## 三层数据真相

### 原始真相层

用户明确保存的原件、正文、对话和应用事件。系统不得用模型摘要替换原件。

### 结构化记忆层

经过状态管理的 Observation、Claim、Episode、Preference、Procedure、Entity 和 Relation。它们是可查询、可冲突、可撤回的长期语义。

### 派生投影层

Markdown 投影、用户画像、时间线、摘要、向量和知识图谱视图。它们服务阅读和检索，但可以被重建。

仓库式用户视图可以呈现为：

```text
AGENTS.md
PROFILE.md
MEMORY_POLICY.md
inbox/
knowledge/
timeline/
skills/
sources/
.memory/
```

这是一种可读交互投影，不要求底层直接用 Git 保存全部私密历史。

## 同步架构

长期目标支持两种明确的信任模式：

1. **可信私有服务器**：用户信任自己的服务器解密、索引和运行后台任务。
2. **零知识同步服务**：服务器只保存密文对象与加密操作日志，解密和语义索引在受信设备执行。

两种模式必须在配置和 UI 中明确区分，不能把“自托管”自动宣传为“零知识”。首个实现模式以 [MVP 路线图](mvp-roadmap.md) 的阶段决策为准。

同步事实建议采用不可变对象与追加操作日志；结构化记忆的用户修改保留版本与冲突，不简单依赖最后写入覆盖。设备撤销后必须阻止新的同步读取，并通过密钥轮换控制未来数据访问。

## Canonical 接口边界

M0 已冻结 `SourceArtifact`、`SourceFragment`、`MemoryProposal`、`MemoryDecision`、`MemoryRecord`、`MemoryStateEvent`、`ContextPack`、`DeleteRequest` 和 `DeletionEvidence` 的逻辑字段。精确契约以 [M0 Canonical Schema](schema/m0-canonical-schema.md) 为准。

M0 fixture 已冻结这些动作在评测中的输入与预期，但不把测试 operation 当作 production API。以下运行接口仍需在对应阶段冻结，而不是提前绑定某个 Provider SDK：

- `CaptureRequest / CaptureReceipt`
- `SearchRequest / SearchCandidate`
- `OutboundContextManifest`
- `ModelRequest / ModelResponse / UsageRecord`
- `SyncOperation / DeviceIdentity`

## 候选存储基线

以下内容是实现栈 ADR 的待评估候选，不是 M0 已接受决策。在没有规模证据前，应优先保持简单、可迁移：

- 本地结构化数据：SQLite；
- 本地全文：SQLite FTS 或等价可嵌入索引；
- 原始对象：加密的内容寻址文件存储；
- 服务端结构化数据：PostgreSQL；
- 服务端向量：PostgreSQL 向量扩展或可替换适配器；
- 实体与时间关系：先用关系表与递归查询验证，不默认引入独立图数据库；
- 后台任务：先使用数据库任务表或单进程 worker，不默认引入消息队列。

具体技术选择必须由 MVP 数据规模、延迟、离线和加密模式验证后决定。
