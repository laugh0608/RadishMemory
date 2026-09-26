# Phase 1 Source Vault 加密 capture 协调

日期：2026-09-26。状态：`P1-S04 encrypted capture slice implemented — synthetic acceptance`。

本批承接[正文迁移与恢复](phase1-source-vault-body-migration.md)，完成新来源与新版本的对象发布、SQLite 原子提交和认证回读。普通 `SqliteDatabase::open` 仍只允许 v6；本入口未接入 application / UI，不表示 P1-S04 / P1-S05 或 ADR 0008 十八项产品场景全部通过。

## 范围与依赖

项目所有者批准继续推进新 capture，并明确批准 `radishmemory-source-vault` 直接依赖已有 `radishmemory-core`，复用 `SourceCapture` 与返回类型。Cargo 离线更新 lockfile，只增加这一条第一方连线；第三方 package、版本、feature、许可证和 notices inventory 不变。core 原本已通过 SQLite 可达，不扩大 Source Vault 的第三方依赖图；生成 notices 仅更新 lockfile 摘要。

本批不改变 canonical schema、crypto / AAD profile、数据所有权或治理语义。只使用合成资料与测试 provider，不访问真实密钥库、GUI、VM 或远程状态。SQLite 存储格式 v9 是维护内部格式，普通入口拒绝 v7 / v8 / v9。

## 入口与状态契约

`PlatformKeyProvider::capture_library_source` 接受同一应用目录 capability、namespace、device 与已有 canonical `SourceCapture`。先取得 SQLite 独占维护会话，验证 schema、namespace / device / provider checkpoint 和 `objects_ready`，再只读取既有 key；不创建 key，不回退 inline BLOB 或外部原件。新库、v6、v7 和未完成迁移的 v8 均拒绝。

首次写入从已完整认证的 v8 升至 v9；幂等读取不因调用本入口而升级 schema。`0009_source_vault_capture.sql` 演进同一 attempts / references 表，保留 v8 已迁移的来源及对象引用：

| 项目 | 约束 |
| --- | --- |
| migration attempt | `kind=migration`、`state=retired`，原快照、locator 与 attempt 不变 |
| capture attempt | `kind=capture`，`prepared → committed`；绑定 source、namespace、digest、length、media type、origin binding 与 request digest |
| prepared | 尚无 canonical source / committed reference；一库最多一个未决 capture |
| committed | canonical source 与 reference 同时存在，快照匹配，reference 以复合 FK 绑定精确 locator / attempt |

prepared 比 canonical source 早落盘，因此 attempts 不再对 source 设置直接 FK；已提交 references 保留 source FK，并在每次协调前后显式验证全部状态关系。未知 schema、快照分叉、缺引用、多余 inline body、未知对象文件和不完整 staging 都失败关闭、保留现场。

request digest 使用 adapter-private `radishmemory.capture-attempt/1` codec：SHA-256 输入包含固定前缀、长度前缀 UTF-8 字符串、big-endian u64 数字、optional presence 与集合长度。它覆盖 binding、来源全部 metadata / governance / producer / supersedes 以及按 ordinal 排序的完整 fragment metadata / heading / governance / segmenter；正文通过既有 exact digest 和已验证 fragment range 绑定。只持久化摘要，不落地第二份请求正文或使用 Debug 序列化。不把该摘要当作 canonical identity、授权证明或 metadata 静态加密。

## 提交与重试

1. 持有跨 checkpoint 的 SQLite exclusive session；复验目录与数据库文件身份，认证所有 committed 对象及已知 pending attempt，再用对象正文复用既有 canonical、binding、audit、FTS 和派生投影校验。
2. 复用现有 capture 的 binding / lineage / governance / exact bytes 判定。相同 binding 和当前正文保持幂等；不同 binding 的相同正文仍各有独立 source 与对象；正文变化新增版本并推进 tip。
3. 新请求先验证 source / fragment ID、版本、supersedes 与整数范围。在内存加密后提交 `prepared` 的请求摘要和locator / 随机加密 attempt，然后写入密文 staging 并 durable no-overwrite publish。
4. 重新认证精确发布对象；在一个真实 SQLite transaction 内写入 source metadata、fragments、binding、audit、FTS、lineage tip、committed reference 和 `committed` 状态。新 capture 不写 inline BLOB。
5. 提交后重新查询 committed reference 并认证解密，复验完整存储事实后才返回 `SourceCaptureResult`。read-back 失败明确返回错误；已经提交的事实由重试重新确认，不伪报未提交或成功。

提交前重试必须保留完整同一 `SourceCapture`，包括 ID、时间、治理和分段信息。不同请求不能占用或改写未决 attempt；调用者需要保留该请求，本批未实现取消或从持久化正文自动重建请求。两条精确对象路径都不存在时才可重新 seal 并更新 token；已有认证 staging / published 对象必须按原 attempt 恢复，不能重复创建来源或版本。已提交精确请求重复执行返回幂等结果，即使该 binding 的当前 tip 后来已经推进。

这些规则只恢复已知 attempt，不是完整 orphan reconciliation。未知对象、截断密文、错误 key、缺失或被篡改的 committed 对象均阻止新请求和幂等成功；本批不自动删除、修补或从外部原件 fallback。

## 验证与限制

专项合成测试覆盖新建 / 版本化 / 幂等、跨 provenance 独立对象、v8 已迁移来源与新来源共存、四个 checkpoint 的错误与独立子进程退出恢复、staging-only / staging+published 恢复、请求变更拒绝、对象缺失 / 篡改、key 失败、碰撞与未知文件保留、metadata / fragment / binding / FTS / request digest 损坏、并发写锁和诊断脱敏。补充验收带有已确认记忆的 v6 库：迁移后新 capture 与幂等重试通过，记忆 proposal / decision / record 校验沿同一认证来源读取链执行；原 proposal 正文损坏仍拒绝。该证据不外推到全部复杂记忆 / 删除工作流。SQLite 专项通过 deferred foreign key 触发真实 COMMIT 失败，确认 canonical、binding、audit、FTS 与 reference 全部回滚，prepared 保留且可以重试。

本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：194 个仓库文件、notices、fmt、all-targets / all-features locked Clippy 与 224 个 Rust tests；包含 15 项 capture 合成测试和 1 项 SQLite 真实 commit 失败测试。3 个 ignored helper 由父测试实际启动，不另计入总数。35 项检查器回归、M0 fixture 契约（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 与 `git diff --check` 通过。完整检查因既有 P1-F17 本机端口测试需要，沿已确认权限流程在沙箱外执行，未跳过测试或放宽检查。子进程退出不等于真实断电；真实断电、磁盘满、Windows / Linux 当前代码编译和运行、真实 key store、logger 过滤、宿主生命周期均未验收。

内部 object-backed verify 已用于 capture 的拒绝判定，但没有公开完整 verify / rebuild 维护入口。当前每次 capture 会认证整个已知对象 inventory 并在内存保留 active sources；尚无大库性能、流式读取或索引修复验收。删除执行、普通搜索 / 导出和 application 数据流未接入加密对象。本批不宣称 R01 至 R06 产品质量问题已修复。

FTS 仍含完整可读正文；原始对象加密不等于整库静态加密，也不清除迁移前明文、SQLite 空闲页、交换区、快照或备份。core 返回的正文仍受既有进程内明文边界约束。

## 下一步与交接

下一步完成完整 orphan reconciliation、object-backed verify / rebuild 和删除协调，再进入 application / macOS 宿主、真实 Keychain 与集中 Windows / Linux 验收。PDF / 图片、模型、同步和发行停止线保持不变。

本批与此前正文迁移改动一起保留在本地 `dev`，未提交、未 push、未运行远程 CI。未启动长期服务、GUI 或 VM；合成测试自行清理临时库、对象与子进程，常规忽略的编译缓存保留。

同日后续 [inventory reconciliation](phase1-source-vault-reconciliation.md) 已完成独立核对与 committed staging link 精确清理；本页原验证与未完成范围保留为 capture 批次事实，现行顺位见[当前状态](../status/current.md)。
