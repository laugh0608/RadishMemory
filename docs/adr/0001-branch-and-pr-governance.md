# ADR 0001：分支、PR 与 Ruleset 治理

日期：2026-08-22

状态：Accepted

## 背景

RadishMemory 当前处于文档先行和实现前定义阶段。产品范围、记忆状态、隐私承诺、同步信任与删除语义都属于高影响长期边界；如果长期直接在稳定主线上累积提交，日常探索、阶段基线和紧急修复会混在同一条历史中，也难以形成可复核的晋级门禁。

Radish、RadishMind、RadishCatalyst、RadishFlow 和 RadishAxiom 已共同验证了“日常集成分支、稳定主线、PR 门禁、稳定主线合并后回流”的基本拓扑。本项目采用这些通用原则，但只保留技术栈无关的治理骨架，并把个人数据、记忆语义、隐私和删除声明作为自己的审查边界。

## 决策

### 分支职责

- `master`：稳定主线和 GitHub 默认分支，只通过 Pull Request 接收变更。
- `dev`：常态开发与集成分支。由于 GitHub 默认目标是 `master`，普通贡献创建 Pull Request 时必须显式选择 `dev`。
- `feature/*`：边界清楚的产品或实现功能。
- `fix/*`：非紧急缺陷修复。
- `docs/*`：不改变实现行为的文档工作。
- `proposal/*`：产品、记忆语义、隐私、同步、公共 schema 或跨项目协议提案。
- `experiment/*`：检索、模型、索引、同步或评测实验；实验结果不自动成为正式架构。
- `chore/*`：仓库、脚本、CI、依赖和治理工作。
- `hotfix/*`：仅用于必须直接修复稳定主线的问题。

不同时维护 `main` 别名。若未来迁移默认稳定分支，必须通过新的治理变更同时更新远程设置、Ruleset、工作流、模板和文档。

### 开发与合并拓扑

普通变更形成 `topic -> dev -> master -> dev` 闭环：

1. 主题分支默认向 `dev` 发起 PR；单人连续开发可直接进入 `dev`，但仍须执行风险匹配的本地验证。
2. 产品、文档、schema、治理或实现切片达到阶段稳定标准后，从 `dev` 向 `master` 发起 PR。
3. `dev -> master` 阶段 PR 优先使用 merge commit，以保留阶段边界并让未继续前进的 `dev` 可以 fast-forward 回流。
4. 仓库允许 rebase merge，但使用后必须接受提交 SHA 改变，并以普通 merge 把 `master` 回流到 `dev`。
5. 禁用 squash merge；仓库需要可审计的提交粒度，贡献者应在合并前整理无意义的临时提交。
6. 任何进入 `master` 的阶段 PR 或 hotfix PR 合并后，都必须在下一轮 `dev` 开发前完成 `master -> dev` 回流。
7. 共享 `dev` 不通过 rebase、reset、force push 或其它历史重写伪造同步状态。

回流后应确认：

```bash
git merge-base --is-ancestor origin/master dev
git rev-list --left-right --count origin/master...dev
```

第一条必须成功，第二条左侧计数必须为 `0`。可快进时使用 fast-forward；无法快进时先检查分支图，再以普通 merge 回流并执行与冲突或实际文件变化相称的验证。

### Pull Request 规则

- `master` 禁止直接 push、force push 和删除。
- 所有 `master` 变更必须通过 PR，并解决全部 review conversation。
- `master` PR 必须通过 strict / up-to-date 的 `Candidate Quality` 聚合检查。
- 单人维护阶段要求 `0` 名批准者；形成稳定第二维护者或持续外部贡献后，再提升审批数并评估 CODEOWNERS。
- 启用 unattributed Copilot changes 的额外审批；仅当 GitHub 无法把 Copilot 提交归因给 Pull Request 作者时，要求至少一名批准者。
- 产品范围、记忆状态、时间冲突、权限、外发、同步、删除、RadishMind 边界或公共 schema 变化必须说明兼容性、迁移、失败模式、隐私影响和验证证据。
- 只有静态检查、mock 或局部测试时，不得把结果表述为隐私保证、删除完成、零知识或生产可用。
- 管理员绕过仅限 Pull Request 内，不开放直接 push 绕过。

### CI 与 required context

远程 Ruleset 只绑定稳定 context `Candidate Quality`，并限定来源为 GitHub Actions App。当前它聚合 `Repo Hygiene` 与三平台 `Rust Quality`：前者覆盖治理文件、文本、链接、JSON、敏感路径、模板、Ruleset、workflow、提交信息和 diff 卫生，后者在 Linux、macOS 和 Windows 运行固定工具链的 fmt、Clippy 和 locked test。

随着实现推进，schema 兼容、权限拒绝、记忆状态、删除传播和供应链检查应按稳定职责加入聚合 job，而不频繁更换远程 required context。

Conventional Commits 由仓库检查器对 PR commit range 执行，不在 Ruleset 中添加提交信息正则，避免 GitHub 自动生成的 merge commit 与远程规则冲突。

### `dev` 的阶段性保护

当前单人维护阶段不保护 `dev`，普通 push 不自动触发 CI；目标为 `dev` 的 PR 会运行完整 `PR Checks`，供外部贡献和并行分支使用。

满足任一条件时重新评估 `dev` Ruleset：

- 有两名或以上稳定维护者；
- 持续接受外部贡献；
- 多个自动化协作者并行写入共享分支；
- 曾因绕过检查导致隐私、记忆语义、治理或构建基线回归。

### 暂不启用的规则

- 不创建 CODEOWNERS 或要求 code owner review；当前没有真实多人所有权结构。
- 不要求签名提交；跨平台签名、密钥恢复和机器人身份方案尚未建立。
- 不创建 tag Ruleset、自动 Release 或部署 workflow；当前没有冻结版本载体、支持矩阵、兼容承诺和发布验收。
- 不启用 Merge Queue；当前单人阶段没有并发合并收益，workflow 也未监听 `merge_group`。未来启用必须先补触发器并进行队列验证。
- 不把个人资料路径限制委托给远程 push Ruleset；仓库内检查始终执行，未来可在计划与仓库可见性允许时增加远程纵深防御。

## 远程落地

仓库中的 JSON 是声明式模板，不会自动修改 GitHub。启用顺序、现状核对和回滚要求见 [Ruleset 说明](../../.github/rulesets/README.md)。远程写入属于独立管理动作，必须先确认目标仓库、现有 Ruleset、Merge options、默认分支和 `Candidate Quality` 已实际产生。

## 后果

收益：稳定主线、日常探索和紧急修复边界明确；每次稳定合并都会回到下一轮 `dev` 祖先链；required context 可在检查组件增长时保持稳定；隐私和记忆语义变化在合并前获得显式审查。

代价：阶段合并后多一次强制回流和拓扑确认；禁用 squash 后需要维护可审计的提交历史；Ruleset、workflow、PR 模板、检查器和文档必须同步维护；`dev` 未保护阶段依赖直接提交者执行本地验证。

## 变更要求

调整分支职责、默认分支、合并方式、required context、审批数、bypass、CODEOWNERS、签名、发布或回流规则时，必须同步更新本 ADR、仓库治理文档、Ruleset 模板与说明、PR 模板、workflow、贡献指南和协作入口。
