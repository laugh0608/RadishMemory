# RadishMemory 文档索引

本目录承载 RadishMemory 的产品、架构、记忆、隐私、集成和实施真相。

## 建议阅读顺序

1. [当前状态](status/current.md)：现在处于什么阶段、下一步、停止线和当前验证入口。
2. [产品范围](product-scope.md)：为什么做、为谁做、解决什么问题。
3. [系统架构](architecture.md)：数据如何进入、组织、检索、同步和提供给模型。
4. [记忆模型](memory-model.md)：什么是记忆，如何形成、更新、冲突、召回和遗忘。
5. [M0 Canonical Schema](schema/m0-canonical-schema.md)：首批对象的逻辑字段、类型、必填性和跨对象不变量。
6. [隐私与威胁模型](privacy-threat-model.md)：用户信任谁，数据可能在哪里泄露，系统如何失败关闭。
7. [RadishMind 边界](radishmind-boundary.md)：两个项目如何协作而不混淆数据真相。
8. [MVP 路线图](mvp-roadmap.md)：先验证什么，哪些能力后置。
9. [ADR 0002：M0 本地记忆闭环](adr/0002-m0-local-memory-loop.md)：首个可执行切片的范围、处理顺序和失败关闭规则。
10. [M0 合成验收](evaluation/m0-local-memory-loop.md)：固定场景、指标、证据和退出条件。
11. [M0 Fixture 与指标契约](evaluation/m0-fixture-contract.md)：JSON mapping、操作序列、稳定 ID、摘要和指标聚合方法。
12. [ADR 0003：首个同步采用零知识服务](adr/0003-zero-knowledge-sync-first.md)：服务端、受信设备、密钥、恢复、删除和可信计算节点边界。
13. [ADR 0004：RadishMind 首次以可选 Gateway 接入](adr/0004-radishmind-optional-gateway-entry.md)：首次进入阶段、逻辑交换、外发、失败和后续 Workflow 边界。
14. [ADR 0005：M0 实现栈与模块边界](adr/0005-m0-implementation-stack.md)：Rust workspace、SQLite / FTS5、依赖、迁移和验证入口。
15. [Rust 依赖基线](implementation/m0-rust-dependency-baseline.md)：当前 lockfile、第一方依赖图、供应链边界和三平台证据状态。
16. [ADR 0006：阶段 1 文本 / Markdown 文件入口](adr/0006-phase1-text-markdown-file-entry.md)：显式选择、路径边界、字节、版本、导出、删除和合成验收。
17. [参考系统与研究问题](references.md)：可借鉴的公开实现和需要自行验证的问题。

## 治理入口

- [Agent 协作与执行规则](governance/agent-collaboration.md)：工作区、授权、数据边界、验证和交接细则。
- [仓库治理](governance/repository-governance.md)：规则层级、仓库资产、PR、CI、Ruleset 和同步矩阵。
- [ADR 0001：分支、PR 与 Ruleset 治理](adr/0001-branch-and-pr-governance.md)：`dev` / `master` 拓扑、合并方式和回流决策。
- [Ruleset 运维说明](../.github/rulesets/README.md)：远程启用顺序、核对、更新与回滚。

## 文档维护规则

- 本目录描述目标、边界和长期事实，不把愿景写成已实现能力。
- 阶段状态、近期顺位、临时门禁和“当前不做”只更新 `status/current.md`，不复制回 Agent 根入口或长期专题。
- 架构、协议、存储、加密和删除语义发生变化时，必须更新对应文档并记录决策理由。
- 仓库、协作、分支、PR、CI 或 Ruleset 变化时，同步检查治理专题、ADR、模板、workflow 和检查器。
- 历史推演、实验结果和完整验证流水进入未来的记录或归档，不堆入索引、根入口或当前状态。
