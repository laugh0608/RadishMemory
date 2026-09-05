# 2026-09-05 项目审阅记录

审阅基线：`dev` 的 `3c10cd2`，审阅时工作区干净。用途：记录已观察的事实、证据边界和后续建议；不是安全认证、实现完成声明或新增操作授权。当前状态与顺位由[当前状态](../status/current.md)维护，后续修复应在该入口关联实际提交和验证结果，不改写本次观察。

## 总体判断

项目已具备真实 canonical core、SQLite 事务存储、本地文件入口、桌面宿主和合成验证。原始资料优先、proposal / decision 分离、不可变版本与事件、派生索引可重建，以及 RadishMind 可选接入是应保留的基础。主要风险是底层工程和治理建设领先于日常可用性、检索质量和恢复能力。

产品价值验证宜聚焦同一个长期项目的资料、关键事实、更正和受控上下文；这不取消长期愿景，也不改变当前 ADR 或自动提前实现模型、同步。

## 发现与证据

“已复现”指本次用纯合成材料运行了实际 application / SQLite 路径；“静态确认”指直接核对实现；“待测风险”不代表已有性能曲线或生产事故。

| ID | 类别 | 观察及影响 | 证据入口 | 后续关闭条件 |
| --- | --- | --- | --- | --- |
| R01 | 已复现 | 导入“项目默认使用蓝色主题。”，查“蓝色”“主题”均为 0，查无句号整句为 1；中文词语找回不成立 | [FTS schema](../../crates/radishmemory-sqlite/migrations/0004_local_recall.sql)、[查询](../../crates/radishmemory-sqlite/src/derived_index.rs) | 通过 Q01 中文与混合语言检索；不以 runner 专用扩展替代 production 修复 |
| R02 | 静态确认 | controller 只取 `list_sources(0, 200)`，无翻页；`select_lineage` 又只接受缓存中的来源，搜索命中的旧来源可能无法选择 | [controller](../../apps/radishmemory-desktop/src/controller.rs)、[UI](../../apps/radishmemory-desktop/src/ui.rs) | 通过 Q02 第 201 条、跨页定位和更新 / 导出 / 删除 |
| R03 | 已复现 + 静态确认 | 仅删除 FTS 派生行后 application reopen 失败；UI 仅在 controller 存在时暴露 rebuild，启动错误状态只有重试 | [SQLite open](../../crates/radishmemory-sqlite/src/lib.rs)、[UI](../../apps/radishmemory-desktop/src/ui.rs) | 按 ADR 0007 通过 Q03 受限维护与损坏拒绝；不能放宽正常打开检查 |
| R04 | 待测风险 | search 全量装载 / 复验候选后检索，收集全部匹配后才截取 top-k；目录读取加载正文后在内存分页；UI 同步调用 | [索引](../../crates/radishmemory-sqlite/src/derived_index.rs)、[目录](../../crates/radishmemory-sqlite/src/source_catalog.rs)、[UI](../../apps/radishmemory-desktop/src/ui.rs) | 先完成 Q04 性能测量；优化后继续证明权限、版本与完整性边界 |
| R05 | 静态确认 | `policy-filter-ran-first` 返回常量 `true`；`query_at` 在 runner 内重建历史投影；零结果词项扩展不在 production application 中 | [runner](../../apps/radishmemory-m0/src/operations/context_search.rs)、[application](../../crates/radishmemory-application/src/lib.rs) | 按 Q05 区分底层、runner 与 production 证据，并用真实拒绝 / 历史路径关闭缺口 |
| R06 | 静态确认 | 回源点击主要选中版本并显示 metadata；预览固定为正文前 400 字符，不能定位靠后命中；更新后没有清除或重算已有搜索结果 | [UI](../../apps/radishmemory-desktop/src/ui.rs)、[controller](../../apps/radishmemory-desktop/src/controller.rs) | 通过 Q06 长正文回源和更新后的结果失效 / 刷新 |
| R07 | 已复现，已接受权衡 | 当前 whole-file fragment 的 FTS `content` 与原文完全相等；ADR 0008 仅加密原始对象后仍会留下完整可读文本 | [FTS 写入](../../crates/radishmemory-sqlite/src/derived_index.rs)、[隐私边界](../privacy-threat-model.md#阶段-1-加密-source-vault-信任边界) | 准确告知保护范围；变更加密范围须独立决策；Q07 记录实际明文面 |
| R08 | 设计缺口 | 首批无 KEK 恢复，单原件导出不等于来源关系、记忆、决定、策略与版本的整体迁移 | [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)、[application](../../crates/radishmemory-application/src/lib.rs) | Q08 区分已有导出与待设计恢复 / 迁移；产品依赖前完成相应范围评审和演练 |

## 合成复现方法与结果

复现使用当前已编译第一方 application 和项目 bundled SQLite 3.53.2，无 GUI、网络、真实个人资料或系统 key store。临时探针不进入产品代码、fixture 协议或正式测试计数。

1. 在任务专用临时目录创建合成 `.md` 与独立数据库；使用 `LocalLibraryConfig::phase1_local` 和合成 ID / UTC runtime 打开 `LocalLibrary`。
2. 以 `FileReadRequest` 显式指定文件与直接 parent allowed root，调用 `import_new_source` 导入合成句子“项目默认使用蓝色主题。”及末尾换行。
3. 通过 `search_sources(query, 5, [Sensitivity::Personal])` 依次查询“蓝色”“主题”“项目默认使用蓝色主题”，观察命中数 `0 / 0 / 1`。
4. 只对该合成数据库读取 `radishmemory_recall_fts.content`，与输入 exact bytes 对比相等。此结果证明完整正文副本，不证明未来对象加密已实现。
5. 关闭 library，仅删除该合成数据库的 FTS 行，保留 canonical 原件，再通过 `LocalLibrary::open` 打开：返回 `OpenLibrary / Storage / StorageFailure`，`retryable=false`。UI 的重建不可达由源码确认，本次没有运行 GUI。

## 验证范围

- `./scripts/check-repo.sh` 首次受沙箱 loopback bind 限制；获准在沙箱外重跑后成功，检查 152 个文件并通过格式、Clippy 与 140 个 Rust 测试。
- 默认 features 的 `cargo check --workspace --all-targets --locked --offline` 通过。
- 原有 M0 fixture 版本、摘要、12 场景 / 86 操作 / 12 gate 均未修改；R05 指出通过结论的解释限制，不改变冻结 oracle 来掩盖问题。
- 未重新运行三平台 GUI、远程 CI、性能基准、恢复演练或独立密码学审计。历史平台运行只按其记录范围成立。
- 审阅未修改产品代码、依赖、lockfile、系统配置或远程状态。后续文档整理的验证另由任务交接记录。

## 待决策建议

下列事项只记录问题与预期收益，尚未选择实现、修改承诺或授予权限：

| 事项 | 需要明确的范围与影响 |
| --- | --- |
| 读取优化与维护入口 | 如何减少全量读取并保持校验覆盖；维护态允许哪些操作；canonical / binding 损坏继续关闭 |
| 原始对象加密以外的保护 | 明确攻击者能取得对象目录、SQLite、备份或已解锁设备中的哪一项，再评估完整正文副本的保护收益 |
| 恢复与整体迁移 | 选择备份内容、密钥保管、重装 / 丢 key 后果与验证方法；不预设口令、恢复码、escrow 或 rotation |
| 阶段依赖调整 | 先验证窄文本价值路径；若要提前阶段 2 / 3 能力，需单独明确 schema、隐私、模型调用和验收影响 |
| 外部试用与自部署授权 | 明确分发渠道、用户授权和维护范围；现行 `LICENSE` 不因产品愿景而改变 |

## 维护建议

保持 Rust 模块化单体与现有 package 依赖方向。`memory_store.rs`、`deletion_store.rs` 已超过协作规则建议篇幅，后续实际修改时按领域拆分私有模块，保留公开接口与事务边界；不以全面重构作为本轮文档收口条件。

当前状态只保存现行结论和路由，历史基线保留在[归档](../status/2026-09-03-baseline.md)。文档检查应区分当前安全边界、历史证据和已知缺口，不依靠重复“已完成”文字判断产品成熟度。
