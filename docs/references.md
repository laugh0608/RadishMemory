# 参考系统与研究问题

本文件记录可借鉴的公开方向。引用现有系统用于建立基线，不代表 RadishMemory 采用其全部架构、托管模式或产品边界。

## 可借鉴系统

### OpenAI 模型上下文

当前模型可以提供很大的上下文窗口，但上下文仍是单次推理资源，而不是跨模型、跨设备、可治理的用户长期记忆。

- [OpenAI 模型文档](https://developers.openai.com/api/docs/models)

### LangGraph Memory

可借鉴的概念：

- 区分 thread-scoped 短期记忆与跨会话长期记忆；
- 语义记忆、情景记忆和程序性记忆的分类；
- 热路径写入与后台整理的不同取舍；
- 长上下文中的成本、延迟和无关历史干扰。

- [LangGraph Memory Overview](https://docs.langchain.com/oss/python/concepts/memory)

### Mem0

可借鉴的概念：

- 模型无关的记忆层；
- 自托管服务、Provider、向量库和 reranker 可配置；
- 记忆抽取、合并和检索的独立流水线；
- 实体关系与语义检索结合。

需要自行验证：隐私信任模式、时间冲突、可解释来源、删除完整性和用户可读仓库投影。

- [Mem0 Open Source](https://docs.mem0.ai/open-source/overview)
- [Mem0 Graph Memory](https://docs.mem0.ai/open-source/features/graph-memory)

### Letta

可借鉴的概念：

- 有状态 Agent；
- 持久且可编辑的 memory blocks；
- 工作记忆与档案记忆的分层；
- 自托管 Agent runtime。

需要自行验证：以用户资料为中心而非单 Agent 为中心的真相模型，以及多个外部模型共享同一用户记忆时的权限边界。

- [Letta Documentation](https://docs.letta.com/)

### Graphiti / Zep

可借鉴的概念：

- 增量时间知识图谱；
- 事实有效时间与失效时间；
- 向量、全文与图遍历混合检索；
- 多跳与实体中心召回。

需要自行验证：图规模、抽取错误、同名实体、隐私删除、模型成本，以及是否真的需要独立图数据库。

- [Graphiti Documentation](https://help.getzep.com/graphiti/getting-started/welcome)
- [Zep Temporal Knowledge Graph paper](https://arxiv.org/abs/2501.13956)

## RadishMemory 需要回答的研究问题

### 记忆形成

- 哪些内容值得成为长期记忆，哪些只应留在原始资料或近期事件中？
- 用户显式确认、确定性规则和模型自动确认的边界在哪里？
- 如何避免模型因表达频率而过度强化偶然偏好？

### 时间与冲突

- Preference、Claim 和 Relation 是否需要不同的时间模型？
- 如何在不要求用户频繁整理的情况下处理冲突？
- 如何区分事实改变、来源错误和适用场景不同？

### 检索与编译

- 全文、向量、实体、时间、关系和近期性在不同任务中的组合如何学习？
- 如何在固定 Token 预算下最大化覆盖而不重复？
- ContextPack 应多大程度保留原文，何时使用摘要？
- 如何量化“模型被无关历史干扰”？

### 隐私与同步

- 首个多端版本选择可信私有服务器还是零知识同步？
- 零知识模式下，跨设备一致索引和后台任务在哪里运行？
- 删除如何穿透离线设备、只读备份和内容寻址对象？
- 如何向用户准确解释云端模型外发边界？

### 个性化与伴侣体验

- 哪些偏好只用于表达风格，哪些可以影响决策？
- 如何避免“长期记忆”变成未经用户同意的心理画像？
- 主动提醒、情感表达和虚拟形象如何保持可控、可关闭和可解释？

## 研发策略

先建立可替换基线和评测框架，再研发新算法。只有当全文 + 向量 + 时间/实体过滤 + rerank + Context Compiler 在明确场景中持续失败，且失败不能通过数据模型、权限或提示策略修正时，才把新记忆算法作为独立研究目标。
