# Phase 1 Source Vault 正文迁移与恢复

日期：2026-09-26。状态：`P1-S04b body migration slice implemented — synthetic acceptance`。

本页记录 ADR 0008 下的首个正文迁移切片，不表示 P1-S04 或十八项产品场景全部完成。普通 `SqliteDatabase::open` 仍只允许 v6；维护入口的 v7 / v8 不接入 application / UI。本批只使用合成资料和测试 key，没有访问真实系统密钥库、修改依赖或 lockfile。

## 入口与内部存储契约

`PlatformKeyProvider::migrate_library_bodies` 是显式维护入口：先验证同一对象 capability 根下的 `library.sqlite3`、schema 和 namespace / device / provider checkpoint，再只读既有 key。首次初始化继续由 P1-S04a 负责；新库、v6、缺 key、未知 schema 或对象事实不得被本入口转化为创建资格。

SQLite 的 `BodyMigrationDatabase` 不暴露 SQL handle。它设置并复验 `locking_mode=EXCLUSIVE`，取得 `EXCLUSIVE` transaction 后重新检查版本、schema、checkpoint 和事实；独占锁跨后续 checkpoint commit 保持到连接关闭，阻止其它维护进程交错。宿主仍必须在调用前关闭旧 v6 连接并暂停普通操作，不能把维护锁解释为已完成产品多实例生命周期治理。

v8 由唯一 migration `0008_source_vault_objects.sql` 定义，全部是 adapter-private 表，不改变 canonical schema：

| 表 | 字段与约束 | 职责 |
| --- | --- | --- |
| `radishmemory_source_vault_migration` | 单例；`migrating` / `objects_ready` | 本批正文 inventory 的整体 checkpoint |
| `radishmemory_source_vault_attempts` | `source_id` 主键；namespace、digest、length、media type 快照；state；唯一 locator 与 attempt token | 每个剩余 inline body 对应一个迁移项，包括历史版本和删除残留 |
| `radishmemory_source_vault_references` | `source_id` 主键；唯一 locator / attempt；复合 FK 绑定精确 attempt | 已提交密文引用，回读必须重新查询此表 |

`planned` 没有 token；其它状态必须具有严格小写十六进制 token。locator / attempt 均为内部相对标识，不允许任意路径。metadata 快照必须与 canonical source 一致；active source 或剩余 BLOB 缺迁移项、引用分叉、状态与 BLOB / reference 不一致均拒绝。新 capture 的 attempt 与原子 metadata 提交尚未在此切片实现，不用未运行的 capture 表占位；后续扩展继续复用同一 migration 真相源与对象引用边界。

## 提交与恢复

1. v7 首次进入时复用既有完整 inline、来源 / fragment、binding 与派生校验，并要求对象 / staging 目录为空。加载既有 key 成功后，原子建立 v8 inventory。
2. 每次恢复都复验数据库完整性、canonical namespace、binding、inventory 与对象目录；所有剩余 BLOB 先复验 exact digest / length，已有对象先认证，包含已经移除旧正文的 `retired` 项。
3. 单项 `planned → prepared`：内存加密后先持久化 locator / attempt，再写入加密 staging。不保存明文 staging。
4. durable publish 后，在 SQLite `IMMEDIATE` transaction 内插入 reference 并将项标为 `referenced`。
5. 提交后重新读取 committed reference，验证 locator、精确 attempt、envelope、AEAD、exact digest / length，并复用 canonical source / fragment decoder 和 file capture audit 检查。
6. 回读成功后，删除该项 inline BLOB 与 `referenced → retired` 在同一事务提交。最后再次认证所有引用与 inventory，才写入 `objects_ready` 并返回只含对象数量的报告。

`prepared` 项若两个精确路径均不存在，可以重建内存密文并更新同一项的 attempt token；不会创建新来源、版本或 audit。已有认证 staging / published 对象必须复用，重新完成文件与目录 sync；只移除已经验证为同一文件身份的本次 staging hard link。已提交引用缺对象、错误 key、篡改、未知文件、不同 attempt、复制出的不同文件身份或不完整 staging 均失败关闭并保留现场，不回退 inline body 或外部原件。

这不是完整 orphan reconciliation：本批不清理无关对象、不尝试修复截断密文。已 deleted 且无 BLOB 的 source 不创建对象；若有删除残留，则迁移残留但保持原 deletion state，不能由此声称删除执行已完成。

## 验证与限制

本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：187 个仓库文件、notices、fmt、workspace Clippy 和 208 个 Rust tests。35 项检查器回归、M0 fixture 契约（12 场景 / 86 操作 / 12 gate）及 1 个 compile-fail doctest 另行通过，`git diff --check` 通过。两个 ignored helper 分别由父测试实际执行，不另计入 208。首次完整检查中既有 `P1-F17` 网络观察测试被沙箱拒绝绑定本机端口；按权限流程在沙箱外原样重跑通过，没有跳过测试或放宽门禁。

本机专项测试通过 17 项，另有 1 个 ignored helper 由父测试实际启动：

- 健康的多版本 / 多 provenance 合成 v6 库，经 P1-S04a 进入 v7，再迁移到 v8；所有迁移对象认证恢复 exact bytes，重复执行保持对象字节、数量与 attempt 行数；相同正文的不同来源不去重。
- 对全部 canonical 表、binding、audit、lineage、fragment 与 FTS 表做迁移前后逐值快照比较；原本为空的记忆 / 删除表仍为空，不据此声称复杂记忆 / 删除工作流已验收。
- 六个 checkpoint 的错误中断重开；publish、reference commit、read-back、inline retirement 四个边界的独立子进程直接退出后恢复，不依赖 Rust drop 回滚。
- 已提交对象缺失 / 篡改、错误 / 缺失 / 锁定 / 拒绝 key、历史正文损坏、部分迁移后的损坏、未知文件、状态 / schema 漂移、截断 staging、数据库替换、对象 symlink、回读后对象变化阻止旧正文移除、诊断脱敏。
- 同进程第二连接在 checkpoint 间写锁竞争被拒绝；该项不等于已完成跨平台 / 宿主并发验收。

本批对照 `P1-SF14` 至 `P1-SF16` 验证维护迁移范围，不把它们写成普通产品路径全部通过。真实断电、磁盘满、当前 Windows / Linux 编译和运行、真实系统 key store、上游 logger 过滤、application / UI 与旧连接生命周期未验收。

首次 v7 preflight 包含派生校验；v8 恢复校验正文、来源 / fragment、binding / capture audit 和存储状态，不提供完整 object-backed FTS verify / rebuild。`objects_ready` 不表示派生数据、普通搜索 / 导出或新 capture 已开放。FTS 仍含完整可读正文；移除 active inline BLOB 不等于清除 SQLite 空闲页、journal 历史、交换区、快照或备份中的明文。

## 后续顺位

继续收口新 capture 协调、完整 orphan reconciliation、object-backed verify / rebuild 和删除执行，再进入 P1-S05 application / macOS 宿主、真实 Keychain 与集中 Windows / Linux 检查点。PDF / 图片、模型、同步与发行保持原停止线。R01 至 R06 质量缺口仍按独立计划跟踪。

本批变更留在本地 `dev`，未提交、未 push、未执行远程 CI 或发布。测试自行清理合成库、密文对象与子进程；未启动长期服务、GUI 或 VM。常规忽略的编译缓存保留。

同日后续[加密 capture 切片](phase1-source-vault-capture.md) 已将新来源 / 版本协调推进到维护 v9，正文迁移入口仍只接受 v7 / v8；本页上述验证数量与未实现范围保留为迁移批次事实，现行顺位以[当前状态](../status/current.md)为准。
