# 2026-09-26 Source Vault 日终记录与明日事项

日期：2026-09-26（Asia/Shanghai）。本记录回顾当天 `dev` 的全部七个实现提交，并保存代码与文档核对及次日交接。现行顺位与停止线以[当前状态](current.md)为准。

## 当天提交回顾

| 提交 | 实际推进 | 证据与边界 |
| --- | --- | --- |
| `a7db975` `feat(source-vault): 实现正文迁移与加密 capture 协调` | v7 → v8 正文 inventory / attempt / reference 与恢复；维护 v9 的新来源、版本、原子提交和认证回读；增加既有 core 第一方连线 | [迁移记录](../implementation/phase1-source-vault-body-migration.md)、[capture 记录](../implementation/phase1-source-vault-capture.md)；批次全仓 224 个 Rust tests，普通入口仍为 v6 |
| `83f3fc1` `feat(source-vault): 增加独立对象核对与 staging 清理` | 独立全量核对与 pending 状态报告，只移除已提交对象同身份的 staging hard link | [核对记录](../implementation/phase1-source-vault-reconciliation.md)；批次全仓 236 个 Rust tests，不把可恢复 attempt 当 orphan 删除 |
| `3f5cc0f` `feat(source-vault): 实现 capture 显式放弃与孤儿清理` | 维护 v10 持久化 `prepared → abandoning → abandoned`，按精确身份清理，保留终态防重放 | [放弃记录](../implementation/phase1-source-vault-abandonment.md)；批次全仓 253 个 Rust tests，不强制清理坏文件或未知对象 |
| `543108e` `feat(source-vault): 实现对象校验与派生索引重建` | 独立 verify 与事务重建 FTS、lineage tip、memory projection，提交前后认证复验 | [维护记录](../implementation/phase1-source-vault-maintenance.md)；批次全仓 265 个 Rust tests，不修复 canonical、schema 或坏对象 |
| `292ed57` `feat(source-vault): 实现加密对象删除执行与恢复` | 维护 v11 冻结原请求及执行闭包，先关闭引用，再清理对象并执行既有十组件结果与 evidence 校验 | [删除记录](../implementation/phase1-source-vault-deletion.md)；批次全仓 280 个 Rust tests，物理对象退役数不等于整个请求成功 |
| `3dd312b` `feat(source-vault): 支持历史删除请求迁移恢复` | 维护 v12 接管精确原请求；残留对象走现有退役链路，旧正文已消失则复验原执行凭据 | [兼容记录](../implementation/phase1-source-vault-legacy-deletion.md)；批次全仓 289 个 Rust tests，不根据空表伪造删除成功 |
| `8d2c651` `feat(source-vault): 实现认证读取与精确导出衔接` | 独立 `LibraryReader`、共用目录 / 搜索查询和真实导出组合验收；修复 Unix 身份检查释放 POSIX 锁及数据库误用单对象大小限制 | [读取记录](../implementation/phase1-source-vault-reader.md)；全仓 309 个 Rust tests，新增跨进程锁证据；application / UI 尚未切换 |

各行数量是对应批次的全仓通过数，不能相加作为独立测试总数。日终另做一次文档提交，汇总本页并修正以下口径；不修改 Rust、manifest、lockfile、自动化规则或长期产品语义。全部提交保留在本地 `dev`，未 push、未创建 PR、未触发远程 CI 或发布。

## 对照代码的文档复核

| 代码与契约依据 | 核对结果与文档处理 |
| --- | --- |
| SQLite `migration.rs`、`source_vault_migration.rs`、`encrypted_capture.rs` 与 migrations 0008–0012 | 普通 `SqliteDatabase::open` 上限仍为 v6；正文迁移只接受 v7 / v8；迁移完成后的共用操作入口接受 v8 至 v12。修正当前状态只列维护 v8 / v9 / v10 的过期表述，并在核对 / 维护记录补齐 v12 支持，保留原批次测试范围 |
| `body_migration.rs`、`capture.rs`、`reconciliation.rs`、`abandonment.rs` | 迁移引用提交、认证回读、inline retirement 与 capture 提交顺序和文档一致；核对不自动放弃，可恢复 capture 仍需原请求。将早期“未提交”和“下一步实现”改为有批次范围的历史事实，补齐提交号与后续链接 |
| `object_deletion.rs`、`legacy_deletion.rs` 与 Source Vault 删除协调 | 删除继续使用原 `DeleteRequest` / 十组件结果，不是第二套删除协议；旧正文缺失凭据必须复验，不创建假对象。`deleted_objects` 只计真实对象退役；核对 / 维护不会替用户执行旧请求。保留历史凭据不能证明恶意整库一致改写未发生的限制 |
| `reader.rs`、SQLite `object_read.rs` / `source_catalog.rs` / `derived_index.rs`、reader 组合测试 | 查询前后认证、原目录排序 / 分页、检索过滤和删除关闭规则一致；实际调用既有导出器验证原字节与不覆盖。读取结果和用户导出是调用方持有的明文副本；认证读取不提供撤回已交付副本的保证 |
| `DatabaseObservation`、bootstrap / capture / deletion 与独立进程测试 | Unix 对数据库使用不打开额外描述符的路径身份检查，由 SQLite 连接持有 inode；已复现并修复 close 释放 POSIX 锁问题。同进程拒绝写入不替代跨进程证据，早期批次“并发通过”不能外推为当时已经覆盖此缺陷；Windows 新路径仍待集中验收 |
| `LocalLibrary` 的字段、open 与 export 实现 | application 仍直接持有 `SqliteDatabase`，open、回源与导出仍走 v6 loader。独立读取 / 导出组合测试没有自动完成 application 生命周期或 GUI 接入；明天首项应处理打开、关闭旧连接及操作会话边界 |
| manifest、Cargo.lock、notices 及 Rust 依赖基线 | 今天只增加已有 core runtime 连线和 file-entry test-only 连线，没有新增或升级第三方。453 个总 package、445 个第三方和 366 项分发 notices 保持；修正依赖表漏列 file-entry 测试依赖、旧更新时间，并补充最新 lockfile 摘要的 notices 复核记录 |
| 当前状态、索引、ADR 0008、架构 / 隐私 / 路线图 | 当前状态聚焦能力、近期顺位和限制，逐批事实转由本日记录与原记录承载；索引新增本日交接。ADR 的早期“维护与删除未完成”加上批次限定。架构 / 隐私 / 路线图已有独立读取和产品 v6 边界，无须改写长期承诺 |

这是按本日修改核对文档和证据，不是独立密码学审计或完整产品验收。canonical schema、用户数据所有权、记忆确认、RadishMind 职责、零知识同步目标和许可证均未改变，因此没有为日终回顾改写这些长期真相源。历史 macOS / Windows / Linux filesystem 证据保留原基线，不覆盖今天新增的协调代码。

## 验证与环境收尾

实现提交前的完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 已通过：220 个仓库文件、notices、fmt、all-targets / all-features locked Clippy、25 个 Rust test suites 共 309 项通过。9 个 ignored helper 由父测试实际作为子进程运行，不额外计入通过数。另有 35 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 与 `git diff --check` 通过。

日终文档变更后的同一仓库聚合检查完整通过：221 个仓库文件、notices、fmt、locked Clippy 与 309 项 Rust tests；差异复核确认最后一批只含 14 个文档文件。既有 P1-F17 的合成本地网络观察器在沙箱内绑定端口遇到 `PermissionDenied`；验证沿权限流程在沙箱外完整重跑，不跳过或放宽门禁。

- 测试只使用隔离合成资料库、对象和测试 key；测试负责清理自身临时文件、子进程和端口。常规忽略的编译缓存与任务专用临时验证日志保留，不纳入 Git。
- 未访问真实个人资料、真实 Keychain / Credential Manager / Secret Service，未启动 GUI / VM / 长期服务，未修改系统配置或远程状态；没有安排后台继续开发。
- Windows / Linux 当前代码的编译与运行、真实 OS 凭据和 logger、宿主多实例 / 交互、断电、磁盘满与大库性能仍待验收。每次读取仍全库认证并持有 exclusive session，不能以 201 条分页测试宣称解决性能或 R02 UI 分页缺口。
- FTS 仍含完整可读正文，独立对象加密不等于整库静态加密；不保证清除历史 SQLite 页、交换区、备份、快照或用户导出副本。当前阶段仍未通过 P1-S05 十八项完整产品场景。

## 明日事项（2026-09-27）

1. **先确认工作起点**：读取 `git status`、实际 HEAD 和[当前状态](current.md)，识别本地未推送提交。以上七个实现已经落地，下一步以 application 接入为起点，不重复实现对象读写或将普通 v6 打开简单放宽到 v12。
2. **先收口显式打开与会话生命周期**：对照 `LocalLibrary`、`PlatformKeyProvider`、`ObjectDirectory` 及现有宿主 profile，确定旧 v6 连接何时关闭、何时进入初始化 / 迁移 / 恢复、何时允许普通操作。首次准备与恢复必须分开，已有 checkpoint 只加载原 key；缺 key、坏对象、未完成迁移继续失败关闭。读取会话应覆盖一次用例所需的完整读取 / 导出阶段，结束后释放 exclusive lock，避免长期持锁阻塞 capture、删除或维护。
3. **形成最小 application 合成切片**：复用既有 SourceCatalog、LocalSearch、file-entry 和 canonical 请求 / 结果，先贯通已迁移库的显式打开、目录 / 历史版本、搜索回源与精确导出，再逐项接入 import / update、lineage 删除及 verify / rebuild。只提取实际需要的共用边界，不新增平行数据库、schema 或第二套用例。若需要 application → Source Vault 第一方依赖，先列明 manifest / lockfile 影响并确认范围；今天批准的测试依赖不包含该连线。
4. **把恢复与错误状态纳入用例**：pending capture 必须保留并使用原请求精确重试；原请求不可用时明确报告，不能生成新 ID 冒充恢复，也不能自动放弃。删除重试必须沿用原请求和 frozen closure；维护观察不能被解释为删除成功。读会话、写操作和错误退出均需证实锁与 key 的生命周期，错误分类继续脱敏且保留真实原因。
5. **按用户路径验证再扩大范围**：至少覆盖关闭重开、旧连接 / 第二进程竞争、迁移未完成、缺 key / 错 key、派生漂移、已删除来源不可见、导出原字节且不覆盖，以及失败后无静默 fallback。围绕首个切片完成有意义的 application 集成验收，再运行全仓检查；独立 adapter tests 不代替 application 证据。
6. **后续检查点**：application 链路稳定后，再按精确授权范围进行 macOS 宿主、真实 Keychain / logger 与多实例验收，随后集中补齐 Windows / Linux 编译和运行。真实系统操作、GUI / VM、依赖或远程动作需各自授权；本页只记录开发建议。
7. **继续保持停止线**：R01 至 R06 独立跟踪；PDF / 图片、模型、同步、恢复方案、签名发行与部署不提前进入本批。不把维护 v12、导出组合验收或本日全仓测试描述为普通产品已经完整加密或生产可用。

今天停止在提交与文档收尾；本页不创建提醒、自动化或后台执行任务。
