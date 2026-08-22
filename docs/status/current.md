# RadishMemory 当前状态

更新时间：2026-08-22

## 当前阶段

`documentation-first / pre-implementation`

当前目标是冻结能够约束首个实现批次的产品、记忆、隐私、评测和仓库治理基线，而不是提前选择完整技术栈或堆叠产品功能。

## 已冻结的首个切片

`M0 Local Memory Loop` 已通过 [ADR 0002](../adr/0002-m0-local-memory-loop.md)冻结为单用户、单设备、本地、合成文本 / Markdown、无模型和无网络的最小闭环。它验证来源、引用、proposal / decision、时间更正、失败关闭和单设备删除证据，不包含 PDF、向量、Provider、RadishMind 或同步实现。

M0 字段级 canonical schema 已在 [M0 Canonical Schema](../schema/m0-canonical-schema.md) 冻结为九种顶层对象及共同逻辑类型。它确定字段、必填性、条件约束、时间、治理标签、事件和删除证据关系，但不绑定数据库、生产 ID 编码或语言类型。

[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 已冻结合成 JSON mapping、fixture ID、摘要 profile、12 个场景的 86 个有序操作和 12 个指标 gate。仓库校验器只验证这些输入与 oracle 自洽；真实 M0 runner 和产品能力仍未实现。

## 当前顺位

1. 明确首个同步信任模式。
2. 明确 RadishMind 在 M0 之后首次进入哪一运行阶段；M0 已决定无依赖。
3. 在以上决策稳定后，评审首个实现栈、目录结构、依赖和运行验证入口。
4. 实现真实 M0 runner，并以冻结 fixture 证明采集、检索、确认、更正、删除和无网络闭环。

## 当前门禁

- 产品、架构、记忆或隐私语义变化必须同步更新对应真相源，不能只改入口摘要或检查器。
- 技术栈、依赖、数据库、消息系统、向量实现和 Provider SDK 尚未冻结；实验建议不得写成长期格式或已接受架构。
- 仓库只允许代码、规范、治理资产和合成 / 明确脱敏的 fixture；真实个人资料、记忆库、ContextPack、Embedding 输入和密钥不得进入 Git、Issue、PR 或 CI。
- GitHub 远端以 `master` 为默认稳定分支、`dev` 为常态开发分支，启用 merge commit 与 rebase merge，并禁用 squash merge；Private vulnerability reporting、Secret scanning 和 push protection 已启用。Ruleset 与 required check 必须以 API、workflow run 和目标分支有效规则复核，不能把仓库模板本身当作已生效证据。
- 当前仓库检查只证明治理、文本、链接和配置合同成立，不证明产品功能、隐私协议、删除或同步已经实现。

## 当前不做

- 不制作虚拟形象、主动陪伴或大而全的聊天产品面；
- 不发明新的长期记忆算法或直接引入图数据库、消息队列和微服务；
- 不把模型推断自动写成已确认记忆；
- 不宣称零知识同步、端到端加密、可证明永久删除或生产可用；
- 不建立自动发布、tag Ruleset、装饰性 CODEOWNERS 或无真实评审人的审批门禁；
- 不复制兄弟项目的技术栈、业务清单、CI 组件或目录结构。

## 阶段退出条件

进入实现前至少应具备：

- 已完成：经评审的 M0 采集、检索、引用、确认、更正和删除闭环；
- 已完成：与首个切片对应的字段级 canonical schema；
- 已完成：与首个切片对应的可执行合成 fixture 与指标 oracle；
- 明确的记忆状态机、时间与冲突语义；
- 首个同步信任模式决策；
- 可测量的召回、污染、删除与隐私指标；
- RadishMind 首批参与方式的明确决定；
- 一份记录实现栈选择、替代方案、迁移边界和风险的 ADR。

## 当前验证入口

macOS / Linux：

```bash
./scripts/check-repo.sh
./scripts/check-m0-fixtures.py
```

Windows：

```powershell
pwsh ./scripts/check-repo.ps1
```
