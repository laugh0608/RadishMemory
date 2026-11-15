# 2026-11-15 macOS 文本资料库日终记录

提交以 Asia/Shanghai 日期为准。日终文档提交前，本地 refs 当天共 11 个提交，都在 `dev` 的第一父链上。现行顺位见[后续事项](current.md#后续事项)。

## 当日提交

| 提交 | 变更与证据 |
| --- | --- |
| `1a97375` | 桌面按只读格式探测分流启动，区分 v6 普通明文、正文迁移未完成及可显式打开的加密对象库。 |
| `6f579c2` | macOS 启动引导六个串行窗口、加密库 201 条分页及双入口 760×520 内容区实测。合成绑定的显式打开观测到 `KeyMissing`，普通启动未命中密钥操作。 |
| `3d3514f` | 所有者独立复核 24 个固定查询，标签全数一致。两条生产入口各三阶段达到 24/24 结果集合匹配、30/30 相关来源，零误召回。 |
| `df20d39` | 双入口 760×520 组合筛选及版本对照，加密手输两种格式，双入口删除后重开。 |
| `afb3825` | 明文 v6 派生修复遇读锁时保持普通视图关闭，解除故障后显式重建、重开并复验原资料。 |
| `d2ca04f` | 有资料的加密打开和刷新均正常完成。该轮未出现可交接的系统提示，凭据失败后的关闭及正文撤回仍待验。 |
| `a29683b` | 冻结 Q04 百来源离线预算并复测。加密中文五命中超预算，其余所选操作按记录对账。 |
| `f941c20` | 复核单次加密搜索的对象认证与数据库身份检查边界，记录减少中间复验的可观察性代价。 |
| `85d97fe` | 来源筛选、排序和精确回源复用一次加密读取会话。两轮各 610 条样本，加密五命中 p95 为 283.9 / 284.8 ms，全库认证为 3 轮，所选预算通过。 |
| `0ce38b0` | 复现旧回执缩减投影的原 ordinal 顺序和字段范围，定位读回与恢复缺少摘要值复验。 |
| `2d5f7ea` | 明文、加密和 M0 runner 共用 v2 全字段摘要，写入及读回复算。既有库显式升级，旧回执原值保留，升级后用原请求恢复并追加 v2。 |

详细场景见[资料库质量验收](../evaluation/phase1-local-library-quality.md)、[中文找回对账](../evaluation/phase1-chinese-retrieval-baseline.md#q01-human-recheck)及[规模复测](../evaluation/phase1-local-library-scale.md#macos-q04-single-search-retest)。

## 代码与文档复核

- `1a97375` 的 `inspect_library_format` 通过只读 SQLite 连接判定正文格式。worker 再按结果打开明文或保持关闭，等待显式维护或显式加密打开。已补宿主契约、架构和 application 文档的入口职责，更新总览及最小窗口专题索引。[宿主契约](../adr/0007-phase1-local-library-host.md#错误日志与恢复)、[实测窗口](../evaluation/phase1-local-library-quality.md#macos-catalog-and-min-viewport)
- `85d97fe` 只在单次搜索内复用独占认证会话。入口和结果交付处的复验保持，失败时拒绝交付整批，精确导出仍用自己的操作会话。读取实现记录、隐私边界和 Q04 确认口径一致。实测以隔离合成 provider 的本机 release 路径为限。[读取边界](../implementation/phase1-source-vault-reader.md#q04-single-search-review)、[隐私与威胁模型](../privacy-threat-model.md#阶段-1-加密-source-vault-信任边界)
- `2d5f7ea` 的回执迁移是 SQLite 历史中的独立记录，保留正文 `user_version`。v2 复算使用持久化 attempt 的原时间文本，旧 v1 按原计划顺序读取并复验。文档已同步 canonical schema、fixture 契约、宿主确认流程和隐私保证。已有库升级后旧客户端会拒绝重开，切回旧搜索格式不解除回执格式限制。[回执实施](../implementation/phase1-source-vault-deletion.md#deletion-evidence-v2-implementation)、[M0 schema](../schema/m0-canonical-schema.md#deletionevidence)、[旧搜索降级边界](../adr/0009-versioned-chinese-recall.md#原子升级降级与恢复边界)

README 的 Source Vault 文档检查同步核验当前入口、原始正文加密范围和 FTS 明文边界。当天提交未改变依赖、lockfile 或许可证。

## 验证与接续

v2 实现提交前，`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过，467 项 Rust 测试通过、0 失败、14 项忽略。默认 workspace 构建检查和 M0 fixture 契约通过。上一批实际生成的 v6、v8、v11 合成副本升级后重开均保留原 v1 回执，旧格式探测拒绝三个升级副本。加密应用回归覆盖原请求的 v8 失败回执及 v11 两个持久 checkpoint。实际升级窗口和真实 Keychain 仍待验。

日终离线全量检查通过，467 项 Rust 测试通过、0 失败、14 项忽略。收尾文案与文档规则同步后重新通过 337 文件的仓库基线、文档检查器 32 项回归和 `git diff --check`。

下次先准备明文旧库回执升级的独立操作单，按取消、确认、重开和新写 v2 的顺序取证。待加密方案绑定隔离库和合成凭据后，再独立确认执行。真实凭据失败与原请求关闭继续各自保留。具体准备条件及其它顺位见[后续事项](current.md#后续事项)。

当天已执行的原生窗口按各自记录收尾，隔离材料继续按对应操作单保留。日终只做本地 Git、文档、文档检查规则及离线回归。工作区实现与日终文档各自提交，未推送。日终文档提交后停止。
