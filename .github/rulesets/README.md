# GitHub Rulesets

本目录保存可审阅的 GitHub Ruleset 模板。文件进入 Git 仓库不等于远程规则已经启用；任何创建或更新远程 Ruleset、默认分支或 Merge options 的操作都必须单独确认目标仓库、现有状态和变更范围。

GitHub 官方说明：[About rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets)、[Available rules for rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets)。

## 当前模板

`master-protection.json` 只保护稳定主线 `master`：

- 禁止删除和 non-fast-forward 更新；
- 所有变更必须通过 Pull Request；
- 要求解决全部 review conversation；
- 要求 strict / up-to-date 的 `Candidate Quality` 状态检查，并限定来源为 GitHub Actions App；
- 允许 merge commit 与 rebase merge，禁用 squash merge；
- 单人维护阶段审批数为 `0`，不要求 CODEOWNERS；
- 管理员只可在 Pull Request 内绕过，不开放直接 push。

Conventional Commits 由仓库检查器校验 PR commit range，不在 Ruleset 中添加提交正则，避免 GitHub 生成的正常 merge commit 被远程规则误拦截。

## `dev` 策略

`dev` 是日常集成分支，初始基线后应设为 GitHub 默认分支。当前单人阶段不启用强制 Ruleset：

- 直接 push 前执行风险匹配的本地验证；
- 目标为 `dev` 的 PR 自动运行 `PR Checks`，用于外部贡献和并行分支反馈；
- 每次 `master` 合并后，下一轮开发前必须把 `master` 回流到 `dev`；
- 达到多人维护、持续外部贡献、并行自动化写入或出现实际回归时，再评估 `dev` 保护。

## 启用前核对

1. 将治理基线提交并推送到远端 `master`。
2. 从同一提交创建并推送 `dev`，再把 GitHub 默认分支切换为 `dev`。
3. 用测试 PR 确认 `Candidate Quality` context 已实际产生且来源是本仓库 workflow；不要只依赖 `workflow_dispatch` 的结果。
4. 用 `gh api /apps/github-actions --jq .id` 核对 GitHub Actions App ID 与模板中的 `integration_id` 一致；当前模板记录为 `15368`。
5. 在仓库 Merge options 中启用 merge commit 与 rebase merge、关闭 squash merge，并确认现有 Ruleset 未启用 Merge Queue。
6. 在 Ruleset UI 中核对“unattributed Copilot Pull Request 额外审批”预览项；单人阶段若它会把审批数从 `0` 实际提升为 `1`，应关闭或把该差异记录为明确决策。
7. 读取仓库级和上级组织 Rulesets，确认没有重叠或冲突规则。
8. 复核 `master-protection.json` 的目标、bypass、审批数、required context 和来源 App。
9. 创建或精确更新 Ruleset，再用非受保护分支发起测试 PR 验证直接 push、force push、会话解决和 required check。

只读核对示例：

```bash
gh api -H "X-GitHub-Api-Version: 2026-03-10" \
  repos/laugh0608/RadishMemory/rulesets

gh api repos/laugh0608/RadishMemory \
  --jq '{default_branch,allow_merge_commit,allow_rebase_merge,allow_squash_merge}'
```

创建新 Ruleset 的示例：

```bash
gh api -H "X-GitHub-Api-Version: 2026-03-10" \
  repos/laugh0608/RadishMemory/rulesets \
  --method POST \
  --input .github/rulesets/master-protection.json
```

已有 Ruleset 时必须先读取其 ID，再使用精确的 `PUT /repos/{owner}/{repo}/rulesets/{ruleset_id}` 更新；不得重复创建同范围规则。修改前保存实际状态，修改后重新读取并用测试 PR 验证；回滚时优先恢复保存的完整配置，而不是临时放宽单项规则。

## 演进原则

- Ruleset 只绑定稳定聚合 context `Candidate Quality`；新增产品检查时接入聚合 job，不频繁修改远程 required context。
- 当前不启用 Merge Queue。若未来启用，必须先让 required-check workflow 监听 `merge_group`，再通过队列测试验证 context 确实产生。
- CODEOWNERS 和至少一名审批者只在形成真实多人评审安排后启用。
- 不要求线性历史，因为阶段性 `dev -> master` 使用 merge commit 保留拓扑闭环。
- 不默认要求签名提交；建立跨平台签名、身份和密钥恢复流程后再评估。
- 版本、tag、Release 和部署规则在发布载体与验收冻结后单独设计。
- Push Ruleset 可作为敏感路径和大文件的纵深防御，但不能替代仓库检查、评审和真实数据隔离。
- Ruleset、workflow、ADR、PR 模板、贡献指南与治理文档必须同步变更。
