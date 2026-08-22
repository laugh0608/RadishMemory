# RadishMemory

`RadishMemory` 是一个用户拥有、模型无关、隐私优先的个人长期记忆与上下文系统。

它允许用户持续保存短语、灵感、文件、对话、网页、图片、音频和其它个人资料，通过可追溯的记忆生命周期、混合检索与上下文编译，为 GPT、Gemini、Claude、Grok、DeepSeek、本地模型及未来模型提供长期记忆。

本项目只有一个产品名称：`RadishMemory`。个人伴侣、虚拟形象、聊天、写作和编程助手都是它之上的产品体验，不拆成独立项目。

## 核心定义

RadishMemory 不是“无限聊天记录”，也不只是向量数据库或传统个人知识库。它的核心职责是：

> 把无限、私密、持续变化的个人资料，编译成当前任务需要的有限、可靠、可追溯上下文。

模型上下文窗口无论增长到多大，都仍然是一次推理的工作内存；RadishMemory 承担跨会话、跨设备、跨模型的长期记忆真相、隐私策略和检索治理。

## 产品原则

- **用户拥有**：用户拥有数据、密钥、部署和迁移权，不依赖必须存在的托管云。
- **本地优先**：单机必须可用；多端同步通过用户自部署服务完成。
- **模型无关**：任何模型只能消费受控上下文并提出记忆候选，不能成为记忆真相源。
- **来源可追溯**：回答和记忆都应能回到原始资料、时间与处理记录。
- **写入可治理**：模型推断默认是候选；确认、更新、冲突、过期和撤回都有明确状态。
- **索引可重建**：全文、向量、图、摘要和缓存都是派生索引，不是唯一真相。
- **遗忘可验证**：删除必须覆盖正文、索引、缓存、同步副本和受控备份生命周期。
- **隐私是硬边界**：权限、敏感度和外发策略在检索前过滤，不能靠相关性分数软控制。

## 文档入口

- [协作入口](AGENTS.md)
- [文档索引](docs/README.md)
- [当前状态](docs/status/current.md)
- [产品范围](docs/product-scope.md)
- [系统架构](docs/architecture.md)
- [记忆模型](docs/memory-model.md)
- [M0 Canonical Schema](docs/schema/m0-canonical-schema.md)
- [M0 Fixture 与指标契约](docs/evaluation/m0-fixture-contract.md)
- [隐私与威胁模型](docs/privacy-threat-model.md)
- [与 RadishMind 的边界](docs/radishmind-boundary.md)
- [MVP 路线图](docs/mvp-roadmap.md)
- [仓库治理](docs/governance/repository-governance.md)
- [分支、PR 与 Ruleset ADR](docs/adr/0001-branch-and-pr-governance.md)
- [参考系统与研究问题](docs/references.md)

## 当前状态

当前处于 `documentation-first / pre-implementation` 阶段：先冻结产品边界、记忆模型、隐私假设和验证目标，再选择技术栈并进入实现。阶段顺位、停止线和当前验证入口以[当前状态](docs/status/current.md)为准。

首个可执行切片已冻结为 [M0 Local Memory Loop](docs/adr/0002-m0-local-memory-loop.md)：只用合成文本 / Markdown、本地全文基线和确定性 proposal / decision 流程验证来源、引用、时间更正、失败关闭和删除证据，不依赖模型、网络、RadishMind 或同步。

M0 的九种顶层对象已经在 [M0 Canonical Schema](docs/schema/m0-canonical-schema.md) 中冻结为实现中立的字段级契约；[M0 Fixture 与指标契约](docs/evaluation/m0-fixture-contract.md)进一步冻结 JSON mapping、稳定 ID、摘要向量、12 个场景的 86 个操作和指标 oracle。两者都是实现约束，不代表产品能力已经完成。

首个多设备同步信任模式已由 [ADR 0003](docs/adr/0003-zero-knowledge-sync-first.md) 冻结为零知识同步服务：默认服务端只中继密文和最小元数据，解密、索引、检索和记忆计算留在受信设备；这仍是待实现、待密码协议评审和待验证的目标边界。

RadishMind 首次运行接入已由 [ADR 0004](docs/adr/0004-radishmind-optional-gateway-entry.md) 后置到完整 MVP 阶段 3，并且只作为可选 Model Gateway；M0、单机资料库和记忆生命周期不依赖它，首次不接 Workflow、Tooling 或共享业务数据库。

M0 的首个产品实现栈已由 [ADR 0005](docs/adr/0005-m0-implementation-stack.md) 冻结为 Rust 2024 模块化单体，使用独立 core、SQLite adapter 和 runner package；本地小文本与结构化事实使用 SQLite，全文基线使用 FTS5。该决定不冻结未来 UI 或服务端语言，也不代表加密存储已经实现。

当前不把以下内容声明为已实现：

- 长期记忆算法；
- 加密多端同步；
- 生产可用部署；
- 多模型兼容；
- 虚拟形象或主动陪伴；
- 可证明删除；
- 零知识服务端。

## 仓库数据边界

本 Git 仓库只承载代码、规范、治理资产和合成 / 明确脱敏的测试材料，不是用户资料库或 Source Vault。真实个人文件、对话、记忆、ContextPack、Embedding 输入、密钥、本地数据库、同步状态和备份不得进入 Git、Issue、Pull Request 或 CI。

本地仓库检查入口：

```bash
./scripts/check-repo.sh
```

## 许可证

本仓库采用 [RadishMemory Source-Available License](LICENSE)，不是开放源码许可证。未经版权所有者书面许可，不授予复制、修改、再分发或商业使用权。
