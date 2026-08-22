# RadishMemory 仓库治理

本文面向维护者、贡献者和自动化协作者，统一说明仓库内规则、GitHub 远程强制项及其演进方式。具体分支决策见 [ADR 0001](../adr/0001-branch-and-pr-governance.md)。

## 规则层级

发生冲突时按以下职责判断，不用低层文件覆盖高层边界：

1. `LICENSE` 与第三方许可证决定法律授权。
2. `SECURITY.md` 决定漏洞报告与披露方式。
3. [产品范围](../product-scope.md)、[系统架构](../architecture.md)、[记忆模型](../memory-model.md)、[隐私与威胁模型](../privacy-threat-model.md)和 [RadishMind 边界](../radishmind-boundary.md)决定产品与数据长期真相。
4. `docs/adr/` 决定已接受的长期架构与治理决策。
5. 本文决定仓库操作、PR、CI 和远程设置的一致口径。
6. `AGENTS.md` / `CLAUDE.md` 与 [Agent 协作规则](agent-collaboration.md)决定协作者在任务中的执行方式。
7. [当前状态](../status/current.md)决定近期顺位、临时门禁和当前验证入口。
8. `.github/` 和 `scripts/` 实施可自动执行的门禁，不自行创造与文档冲突的新政策。

如果规则与实现不一致，应先判断哪一方已经过期，再在同一变更中统一修正；不得仅修改检查器来掩盖政策漂移。

## 仓库内治理资产

| 资产 | 职责 |
| --- | --- |
| `AGENTS.md` / `CLAUDE.md` | 启动级长期协作、执行边界、隐私红线和任务路由 |
| `docs/governance/agent-collaboration.md` | 按任务读取的稳定协作、工作区、验证和交接细则 |
| `CONTRIBUTING.md` | 外部贡献者的最小入口与数据安全要求 |
| `CODE_OF_CONDUCT.md` | 社区讨论、证据准确性和隐私保护边界 |
| `SECURITY.md` | 私下漏洞报告与安全问题范围 |
| `.editorconfig` / `.gitattributes` / `.gitignore` | 编码、换行、本地状态、凭据和个人资料边界 |
| `scripts/check-repo.*` | 无第三方依赖的本地与 CI 仓库基线 |
| `.github/PULL_REQUEST_TEMPLATE.md` | 影响面、证据、风险和回流记录 |
| `.github/ISSUE_TEMPLATE/` | 缺陷、长期提案与安全报告分流 |
| `.github/workflows/pr-check.yml` | PR 自动检查和稳定聚合 context |
| `.github/rulesets/` | 远程 `master` 保护的声明式模板与运维说明 |
| `docs/adr/` | 已接受治理决策及理由 |

## 仓库与个人数据边界

Git 仓库只保存项目代码、文档、schema、合成 fixture 和可公开审查的治理资产。以下内容不得提交，即使仓库当前是私有的：

- 真实文件、对话、音频、图片、网页快照、个人资料和记忆库；
- 未脱敏的 `ContextPack`、Provider 请求 / 响应、Embedding 输入或向量；
- 本地 SQLite、Source Vault、同步操作日志、导出、备份和删除证据；
- API Key、加密密钥、恢复码、生产连接串、Cookie、Token 和私密路径日志；
- 未公开安全报告、第三方专有数据和许可不明的训练 / 评测材料。

测试与文档示例使用合成数据。需要代表真实分布时，应单独定义脱敏准则、授权和存储边界；“只去掉姓名”不构成充分脱敏。CI 日志、artifact 和缓存视为仓库信任边界外的数据外发面。

`.gitignore` 用于降低误提交风险，`scripts/check-repo.py` 对被强制加入 Git 的敏感路径和扩展名再次失败关闭；两者都不能替代代码评审和秘密扫描。

## 分支与提交

- `master` 是稳定主线；`dev` 是常态开发与集成分支，初始治理基线建立后作为 GitHub 默认分支。
- 普通贡献进入 `dev`，阶段性稳定化和 hotfix 才进入 `master`。
- 主题分支使用 `feature/*`、`fix/*`、`docs/*`、`proposal/*`、`experiment/*`、`chore/*` 或 `hotfix/*`。
- 共享分支禁止 force push 和破坏性历史重写。
- 提交遵循 Conventional Commits；允许 Git 生成的正常 merge commit。
- 提交使用真实贡献者身份，不加入 AI 协作者署名。

## PR 审查重点

所有 PR 都应说明目标、范围、实际验证、未验证内容、风险和回滚。以下变化还必须额外说明：

| 变化 | 必需说明 |
| --- | --- |
| 产品范围或数据所有权 | 用户承诺、非目标、迁移和现有数据影响 |
| Artifact / SourceFragment | 原件完整性、稳定引用、解析失败和来源追溯 |
| MemoryRecord / MemoryProposal | 来源、确认、版本、时间、冲突和污染路径 |
| 权限或外发策略 | 失败关闭、派生数据、Provider、用途和审计 |
| ContextPack | 最小化、Token 截断、引用映射和 manifest |
| 同步或密钥 | 信任模式、设备身份、撤销、冲突和恢复 |
| 删除或保留 | 覆盖面、传播状态、备份失效和可验证证据 |
| RadishMind 集成 | 数据库隔离、授权范围、候选回写和失败回退 |
| 依赖或解析器 | 来源、许可证、隐式联网、隔离和供应链风险 |

合并门禁验证实现和仓库满足已声明规范，不替代对规范本身是否完整、安全和符合用户意图的审查。

## Ruleset 基线

当前目标状态：

| 项目 | 策略 |
| --- | --- |
| 保护分支 | 仅 `master` |
| GitHub 默认分支 | 初始基线后为 `dev` |
| PR 要求 | 必须 |
| 删除 / force push | 禁止 |
| required context | `Candidate Quality`，来源限定为 GitHub Actions App |
| strict / up-to-date | 启用 |
| review conversation | 必须解决 |
| 审批数 | 单人阶段为 `0` |
| unattributed Copilot PR 额外审批 | 单人阶段默认关闭，启用时必须作为独立决策记录 |
| CODEOWNERS | 暂不启用 |
| 合并方式 | merge commit、rebase merge |
| squash merge | 禁用 |
| 管理员 bypass | 仅 Pull Request 内 |
| commit signature | 暂不强制 |
| tag / release rules | 版本与发布方案冻结后另行设计 |
| Merge Queue | 暂不启用；启用前先补 `merge_group` 触发与队列验证 |

远程 GitHub 设置才具有强制力；仓库模板负责审阅、复现和防止口径丢失。修改远程前后都应导出或读取实际状态，并确认没有创建重复或重叠 Ruleset。

## CI 契约

`Candidate Quality` 是 Ruleset 唯一绑定的稳定聚合 job，并限定由 GitHub Actions App 产生，避免其他拥有写权限的主体伪造同名成功状态。当前组件 `Repo Hygiene` 覆盖：

- 必需治理文件与核心真相源是否存在；
- UTF-8、BOM、LF、末尾换行与尾随空格；
- JSON 可解析性与 Markdown 相对链接；
- 路径、文件大小、缓存、环境文件和敏感数据文件名；
- `AGENTS.md` / `CLAUDE.md` 同步；
- Issue、PR、Ruleset 和 workflow 合同；
- 检查器单元测试、PR diff 空白和 Conventional Commits。

Workflow 使用最小 `contents: read` 权限、禁用 checkout 凭据持久化，并把 GitHub 官方 Actions 固定到带版本注释的完整 commit SHA；升级 Action 时必须核对官方发布、同步检查器合同并通过 PR 验证。

技术栈冻结后按风险把实现检查作为独立 job 接入聚合，包括 schema 兼容、记忆状态、时间冲突、权限拒绝、ContextPack 最小化、删除传播、同步冲突、构建测试和供应链检查。组件名称可以演进，`Candidate Quality` 保持稳定。

## 变更同步矩阵

| 变更 | 必须同步检查 |
| --- | --- |
| 分支、默认分支或合并策略 | ADR、本文、Ruleset README / JSON、PR 模板、贡献指南 |
| required context 或 CI 组件 | workflow、Ruleset README / JSON、检查器、ADR |
| Agent 协作或执行边界 | `AGENTS.md`、`CLAUDE.md`、Agent 协作专题、相关检查器或模板 |
| 当前阶段或临时门禁 | `docs/status/current.md`；公开摘要变化时再更新 README |
| 产品、架构或记忆语义 | 对应真相源、PR 影响面、兼容与评测 |
| 隐私、同步、外发或删除承诺 | 威胁模型、架构 / 记忆专题、SECURITY、测试与 PR 模板 |
| RadishMind 协议或所有权 | RadishMind 边界、架构、契约和集成测试 |
| 许可证或贡献授权 | `LICENSE`、CONTRIBUTING、README |

## 演进停止线

- 没有真实所有权结构时不创建装饰性 CODEOWNERS。
- 没有可稳定执行的检查时，不把占位 job 设为 required。
- 没有版本载体、支持矩阵、兼容 / 回滚验收时，不创建自动发布和 tag 保护幻象。
- 不复制兄弟项目的语言栈、业务目录、应用检查或发布流程。
- 不把当前阶段、临时门禁和易过期命令复制到 Agent 根入口。
- 不用更多文档替代自动化；稳定规则一旦可机器验证，应进入仓库检查或正式 CI 组件。
