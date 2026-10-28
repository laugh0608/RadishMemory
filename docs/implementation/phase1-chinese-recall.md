# Phase 1 版本化中文检索落地

日期：2026-10-28

状态：`production dual-entry implemented; synthetic verification recorded; full Q01 acceptance open`

## 范围与实现

项目所有者在候选评审后确认[ADR 0009](../adr/0009-versioned-chinese-recall.md)的汉字连续短语、旧库显式升级、旧客户端拒绝及显式降级范围。输入为 `dev` / `f2f8e10` 加本任务尚未提交的候选实验；本记录覆盖随后的生产实现。没有新增依赖、改动 lockfile、调用模型或更改 canonical / 正文版本。

SQLite adapter 使用 `radishmemory.recall.han-basic-a/1` 标记派生格式。新建库默认启用；已有库保持原格式，正常打开不会升级。索引和查询共用冻结字符规则，基本汉字及扩展 A 按相邻有序短语检索，非汉字仍使用锁定 SQLite 的 unicode61。每次查询限制 4096 UTF-8 bytes / 128 个非空项，超限明确返回 `SearchQueryLimit`，没有截断或扩大权限 fallback。

健康默认 v6 和正文迁移完成的显式加密入口均提供原子切换。切换前验真 canonical、全部历史正文、派生数据及未完成请求，事务内重建 FTS，commit 后回读真实格式。对象模式沿原独占会话认证密文，commit 前再次验证；不创建 key、不读取外部原件。未知 / 部分 profile、schema 漂移和 pending capture / 删除阻止切换。正文版本、原始 bytes、对象、记忆状态及删除证据保持。

application 与桌面单 worker 已接线。健康视图显示最后刷新得到的格式，升级 / 降级有兼容性确认；Cancel 不派发，busy / pending / 无有效视图禁用。完成后清空搜索并刷新；操作失败撤回视图，commit 后独立刷新失败与写入结果区分。后续显式 Refresh 以数据库实际格式恢复，不假定失败一定未提交。

## 质量与故障证据

固定 `q01-synthetic-v1` 的 12 份资料 / 24 个查询及来源标签保持，top-k 5。默认 v6 与显式加密 production 入口，各在导入、重开、重建后取得：相关来源 **30/30**，有答案查询 **20/20**，完全匹配 **24/24**，额外来源 **0**，无答案误召回 **0/4**。原句中文短词遗漏已在固定样本中修复。历史 15/30 观测保留于[基线记录](../evaluation/phase1-chinese-retrieval-baseline.md)，没有改写为历史成功。

独立 production 反例覆盖分散 / 倒序汉字、标点与 Latin 边界、完整英文词；codec 回归覆盖字符区间端点、扩展 B 不冒充子串支持、简繁不转换、组合字符、variation selector、私用字符、重音与 FTS 操作符字面处理。SourceFragment / 已确认 MemoryRecord 的 namespace、敏感度、时间、撤回、更新与删除继续按原资格过滤；proposal 不直接成为记忆。

格式往返验证包括旧库打开保持、两连接看到实际格式、旧版本精确导出、更新后旧词排除、删除后不复活、新格式随 key bootstrap / 中断正文迁移 / capture / 删除恢复链保留。真实 SQLite deferred foreign-key commit 失败在两个方向均回滚；对象协调的 commit 前后故障、对象晚期损坏与独立进程退出分别验证旧 / 新完整状态及 canonical / 密文保持。桌面 headless 使用真实 worker，覆盖确认、取消、单次派发、原请求阻止切换与提交后刷新失败恢复。

首次全量测试中，历史正文迁移 fixture 直接写旧 FTS 列，却由新建库得到新格式，17 项中的 16 项在 bootstrap 阶段失败。已让该 fixture 显式建立旧格式，定向 17 项通过；生产新格式迁移由 `q01_upgrade` 保持覆盖，没有放宽一致性检查。

## 真实旧代码兼容

从 `f2f8e10` 的 `git archive` 构建真实旧 SQLite adapter，并与本批当前源码探针使用独立 target 目录。每步核对探针身份、退出码、profile 表及 `user_version`：旧格式可由旧代码打开；升级后旧代码返回 `SchemaDrift`；显式降级后再次可打开。六步均符合预期，基础正文版本一直为 v6。

首轮共享 target 目录复用了旧探针产物，已判无效并保留原记录；有效结果来自独立构建后的重新执行。该证据只覆盖空合成 v6 和实际 adapter，不代表旧 GUI 或所有加密版本兼容。完整资料的双入口升级、导出、更新、删除另由 production 集成测试覆盖。

## 有限长文成本

手动执行 `cargo test -p radishmemory-application --test q01_cost --locked -- --ignored --nocapture`。本机 macOS ARM64、debug 构建、32 份固定重复合成资料，共 704,918 bytes；无随机 seed。每种查询依次测 5 次，表中为中位毫秒。只做有限观测，无冷 / 热缓存控制，不设未经评审的 CI 时间阈值；加密使用合成 provider，未包含真实系统凭据延迟。

| 入口 / 格式 | 数据库文件 bytes | 使用页 bytes | 英文唯一 marker ms | 中文短词 ms |
| --- | ---: | ---: | ---: | ---: |
| 默认 / 旧 | 1,961,984 | 1,933,312 | 21.050 | 20.802 |
| 默认 / 新 | 3,518,464 | 3,518,464 | 57.279 | 58.723 |
| 加密 / 旧 | 1,990,656 | 1,290,240 | 1,530.080 | 934.709 |
| 加密 / 新 | 2,875,392 | 2,875,392 | 1,741.119 | 4,680.263 |

默认 / 加密升级耗时分别 327 / 1,160 ms。英文查询均命中 1 份；旧中文查询遗漏为 0 命中，新格式达到 top-k 5，因此两种中文耗时并非相同输出工作量。新格式的汉字 token、完整性复算与索引存储增加成本；既有加密全库认证和结果回源继续有明显代价。数据库统计不含对象文件、临时 journal、进程内存或备份；不将 SQLite 文件大小说成整个资料库大小。这不是性能优化或生产容量验收，Q04 保持开放。

## 验证与交接

完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：298 个文件的仓库基线、fmt、locked Clippy、29 个 Rust suites 通过，合计 427 passed、0 failed、13 ignored（12 个父测试调用的隔离 helper、1 个已单独手动执行的成本测试）。共享计量回归在两个测试目标重复执行，计数不当作独立质量样本。默认 `cargo build -p radishmemory-desktop --locked` 成功。

收尾复核修正了“提交后格式被另一连接改变”的错误文案，随后 fmt、全 workspace / all-targets / all-features locked Clippy、SQLite 全目标 85 项测试（0 failed）与默认 desktop 构建再次通过。定向质量、升级、存储故障、正文迁移、桌面 headless、手动成本与旧代码兼容均已执行；原始日志、计数、探针及源码摘要保留在本批忽略目录，仅含合成数据。

未执行新切换 UI 的原生可见验收、真实系统 key store、Windows / Linux、真实断电或大规模 release 性能测试；候选标注仍待独立人工评审，完整 CJK、简繁同义词与向量检索不在本 profile。Q01 / Q04 和 P1-S05 不整体关闭。FTS 精确正文及新增派生字段仍为 SQLite 明文，token 编码不是加密，也不保证旧页面 / 备份擦除。

本批未启动 GUI、VM 或常驻进程，没有远程动作，未提交、未 push；不修改或清理既有原生验收库、系统测试 key 与冻结产物。

日终更新：上述候选实验、ADR、production 实现及对应文档已按项目所有者要求提交为 `4c9b0be`，此前“未提交”描述是实施收尾快照。完整日终回顾和当前后续顺位见[10 月 28 日记录](../status/2026-10-28-chinese-recall.md)及[当前状态](../status/current.md#后续事项)。
