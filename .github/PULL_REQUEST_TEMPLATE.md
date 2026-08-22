## 目标与范围

请说明本次变更解决的问题、采用的方案以及明确不包含的内容。

## 关联与目标分支

- 关联 Issue / ADR / 真相源：
- 目标分支：`dev` / `master`
- 如目标为 `master`，阶段性收口或 hotfix 理由：

## 变更类型

- [ ] 产品 / 架构 / 协议提案
- [ ] 功能或实现
- [ ] 缺陷修复
- [ ] 记忆语义 / schema
- [ ] 隐私 / 安全 / 同步 / 删除
- [ ] 测试 / 评测
- [ ] 文档
- [ ] CI / 仓库治理
- [ ] 依赖 / 供应链

## 数据、记忆与信任影响

请勾选所有受影响边界，并在有影响时说明兼容性、迁移、失败模式和验证证据。

- [ ] 无数据、记忆或信任边界变化
- [ ] SourceArtifact / SourceFragment / 原始资料
- [ ] Observation / Claim / Episode / Preference / Procedure
- [ ] MemoryProposal / 状态机 / 时间 / 冲突 / 版本
- [ ] 权限 / 敏感度 / 保留 / 外发策略
- [ ] 检索 / 派生索引 / ContextPack / 引用
- [ ] Provider / OutboundContextManifest / 审计
- [ ] 同步 / 设备 / 密钥 / 恢复
- [ ] 删除 / 备份 / DeletionEvidence
- [ ] RadishMind 边界或跨项目协议

影响与迁移说明：

## 真相源同步

- [ ] 不需要修改产品或治理真相源
- [ ] 已同步 `docs/product-scope.md`
- [ ] 已同步 `docs/architecture.md`
- [ ] 已同步 `docs/memory-model.md`
- [ ] 已同步 `docs/privacy-threat-model.md`
- [ ] 已同步 `docs/radishmind-boundary.md`
- [ ] 已同步 `docs/mvp-roadmap.md` / `docs/status/current.md`
- [ ] 已同步 ADR、仓库治理、Ruleset、workflow 或协作入口

说明不适用项或新增真相源：

## 隐私与测试数据

- [ ] fixture、示例、日志和截图只使用合成或明确脱敏数据
- [ ] 未提交真实个人资料、私密对话、原始 ContextPack、Embedding 输入、密钥、恢复码或生产连接信息
- [ ] 导入资料与模型输出仍按不可信数据处理
- [ ] 权限与外发拒绝路径失败关闭，不通过摘要、缓存、图或 fallback 绕过
- [ ] 隐私、同步和删除声明没有超过实际证据

如有例外，说明授权、最小化、保留和清理方式：

## 验证记录

只填写实际执行过的命令及结果；未执行的验证放在“未验证、风险与回滚”。

```text
./scripts/check-repo.sh
pwsh ./scripts/check-repo.ps1
```

实现开始后，追加与改动范围匹配的格式化、静态分析、测试、兼容性、权限拒绝、状态迁移、删除或同步验证。

## 检查清单

- [ ] 改动符合 `docs/status/current.md` 与核心产品真相源
- [ ] 目标、明确非目标、兼容性和失败模式已写清楚
- [ ] 没有把模型推断直接写成已确认记忆
- [ ] 没有把自托管、局部测试或设计目标表述为零知识、生产可用或已完全删除
- [ ] 第三方代码、数据、模型和资产已记录来源、版本与许可证
- [ ] 提交符合 Conventional Commits，且未添加 AI 协作者署名
- [ ] 已执行风险匹配的最小验证，并记录未验证内容
- [ ] 目标为 `master` 时，本 PR 来自 `dev` 或已说明 hotfix 例外
- [ ] 目标为 `master` 时，已指定合并后的 `master -> dev` 回流负责人和方式

## 未验证、风险与回滚

- 未验证内容：
- 已知风险：
- 回滚、兼容或数据迁移处理：

## `master` 合并后回流

仅目标为 `master` 时填写。合并完成后、下一批 `dev` 开发开始前必须收口。

- 回流负责人：
- 预期方式：`fast-forward` / `merge commit`
- 如发生冲突，补充验证：
