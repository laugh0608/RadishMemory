# RadishMemory 文档索引

本目录承载 RadishMemory 的产品、架构、记忆、隐私、集成和实施真相。

## 建议阅读顺序

1. [当前状态](status/current.md)：现在处于什么阶段、下一步、停止线和当前验证入口。
2. [产品范围](product-scope.md)：为什么做、为谁做、解决什么问题。
3. [系统架构](architecture.md)：数据如何进入、组织、检索、同步和提供给模型。
4. [记忆模型](memory-model.md)：什么是记忆，如何形成、更新、冲突、召回和遗忘。
5. [隐私与威胁模型](privacy-threat-model.md)：用户信任谁，数据可能在哪里泄露，系统如何失败关闭。
6. [RadishMind 边界](radishmind-boundary.md)：两个项目如何协作而不混淆数据真相。
7. [MVP 路线图](mvp-roadmap.md)：先验证什么，哪些能力后置。
8. [ADR 0002：M0 本地记忆闭环](adr/0002-m0-local-memory-loop.md)：首个可执行切片的范围、处理顺序和失败关闭规则。
9. [M0 合成验收](evaluation/m0-local-memory-loop.md)：固定场景、指标、证据和退出条件。
10. [参考系统与研究问题](references.md)：可借鉴的公开实现和需要自行验证的问题。

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
