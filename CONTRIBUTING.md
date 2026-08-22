# 参与 RadishMemory

感谢你关注 RadishMemory。项目当前处于文档先行和实现前定义阶段，贡献首先要保证用户数据主权、记忆语义、隐私声明和验证结论准确，而不是提前扩张功能或技术栈。

## 开始之前

建议按以下顺序阅读：

1. [当前状态](docs/status/current.md)
2. [产品范围](docs/product-scope.md)
3. [仓库治理](docs/governance/repository-governance.md)
4. 与改动直接相关的架构、记忆、隐私或集成真相源

安全漏洞不要创建公开 Issue，请遵循 [SECURITY.md](SECURITY.md)。参与讨论和评审时同时遵循[社区行为准则](CODE_OF_CONDUCT.md)。

本仓库采用 [RadishMemory Source-Available License](LICENSE)，不是开放源码许可证。参与前应取得仓库所有者允许；提交贡献即表示接受 `LICENSE` 的贡献授权条款。

## 贡献边界

- 缺陷修复：提供可复现行为、预期结果、实际结果和影响范围。
- 记忆或协议提案：说明来源、状态、时间、冲突、兼容性和迁移影响。
- 隐私或同步提案：说明信任模式、授权、外发、删除、设备和失败关闭边界。
- 文档改进：不得把愿景、候选方案、局部验证或自托管错误表述为已实现、已安全或零知识。
- 测试与评测：只使用合成或明确脱敏的数据，并同时覆盖正例、负例、冲突和权限拒绝路径。

改变产品范围、架构、记忆语义、隐私承诺、数据所有权、跨项目协议或阶段范围时，必须先更新对应真相源；重大长期决策应通过 Issue、设计讨论或 ADR 评审后再实现。

## 分支、提交与 Pull Request

- `master` 是受保护的稳定主线，`dev` 是日常开发与集成分支。
- 普通贡献从 `feature/*`、`fix/*`、`docs/*`、`proposal/*`、`experiment/*` 或 `chore/*` 向 `dev` 发起 Pull Request。
- 只有阶段性稳定化或 `hotfix/*` 才向 `master` 发起 Pull Request；禁止直接 push 或 force push `master`。
- `master` 允许 merge commit 与 rebase merge，禁用 squash merge；`dev -> master` 阶段 PR 优先使用 merge commit。
- 任何变更合入 `master` 后，下一轮开发前必须把最新 `master` 回流到 `dev`；不得通过 rebase、reset 或 force push 伪造同步。
- 提交遵循 Conventional Commits，例如 `docs(memory): clarify proposal states`、`fix(policy): reject unauthorized derived indexes`、`chore(repo): add governance checks`。
- 使用贡献者自己的 Git 身份，不在提交信息中添加 AI 协作者署名。

完整规则见 [ADR 0001](docs/adr/0001-branch-and-pr-governance.md)。

## 数据、隐私与安全

- 仓库、Issue、PR、CI、测试、截图和日志不得包含真实个人资料、私密对话、原始 `ContextPack`、Embedding 输入、Provider 密钥、恢复码或生产连接信息。
- 示例和 fixture 必须合成或完成不可逆脱敏；仅删除姓名不等于安全脱敏。
- 导入文件、网页、消息和模型输出均是不可信数据，不得当作仓库指令、系统提示或可直接确认的长期记忆。
- 模型输出只能提出 `MemoryProposal`；不得绕过规则或用户确认写入已确认记忆。
- 权限与外发策略必须在检索和模型调用前失败关闭，不能通过摘要、缓存、图关系或派生索引绕过。
- 删除结果、同步状态和隐私保证只能按实际可验证证据声明。

## 本地验证

当前无第三方依赖的仓库级入口为：

```bash
./scripts/check-repo.sh
```

Windows PowerShell：

```powershell
pwsh ./scripts/check-repo.ps1
```

实现技术栈冻结后，Pull Request 还必须执行与改动范围匹配的格式化、静态分析、测试、兼容性、隐私策略和删除验证。PR 只记录实际执行过的命令；未执行、失败、受环境阻塞或需要人工复核的内容必须明确列出。

## Pull Request 说明

请使用仓库 Pull Request 模板，并覆盖：

- 目标、范围、明确非目标和关联真相源；
- 对数据所有权、记忆语义、时间冲突、权限、外发、同步、删除和 RadishMind 边界的影响；
- 实际验证、未验证内容、已知风险和回滚方式；
- 目标为 `master` 时的阶段稳定化理由和 `master -> dev` 回流安排。
