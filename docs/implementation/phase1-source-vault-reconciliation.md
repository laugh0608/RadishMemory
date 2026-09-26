# Phase 1 Source Vault inventory reconciliation

日期：2026-09-26。状态：`P1-S04 inventory reconciliation slice implemented — synthetic acceptance`。

本批承接正文迁移与加密 capture，新增独立对象核对入口。适用完成迁移的维护 v8 / v9；普通 `SqliteDatabase::open` 仍只允许 v6。不增加 schema、依赖、第三方包或公共 canonical 字段，不接入 application / UI 或真实 key store。

## 核对与清理边界

ADR 0008 要求只有“没有 committed reference、没有仍可恢复的 active attempt、目标身份稳定”的对象才允许作为 orphan 清理。当前 v9 每个未提交对象仍绑定 `prepared` capture attempt，没有取消或永久放弃状态。因此不能把未提交、请求暂未提供或重试失败解释为可删除。

`PlatformKeyProvider::reconcile_library_objects` 复用既有 exclusive SQLite session 和认证读取：

1. 验证同一应用目录下的数据库文件身份、schema、namespace / device / provider checkpoint、`objects_ready` 和存储状态；v7 或未完成正文迁移先拒绝，不读取 key。
2. 只加载既有 key，核对完整目录 inventory，认证全部对象与 pending attempt，并用对象正文校验 canonical、fragment、binding、audit、记忆来源、FTS 与派生投影。
3. 所有事实通过后，逐项重新读取 committed reference。只有已发布对象真实存在、认证通过且 staging 是同一文件身份的 hard link 时，才移除这个多余的 staging 目录项；对象文件与数据库事实保持不变，目录同步后复验目标。
4. 最后再次核对完整 inventory 与全部事实，再返回不含路径或身份的 `ReconciliationReport`。失败返回错误，不把已发生的部分 link 清理说成回滚或整体成功；重试重新核对实际状态。

报告包含 `committed_objects_verified`、`pending_capture: Option<AttemptState>` 和本次 `committed_staging_links_removed`。pending 可以是尚无文件、仅 staging、已发布或 staging + published 四态；这些状态均保留，调用者必须用原始 `SourceCapture` 继续恢复。没有 pending 也不等于产品已开放或删除验收完成。

未知文件、无登记的密文、截断对象、缺失 committed object、错误 key、内容 / locator / attempt / 文件身份冲突和 metadata / 派生损坏均失败关闭并保留现场。即使 staging 的认证内容正确，也不从它重新发布缺失的 committed object。核对不创建 key、不 seal、不创建来源、不回退 inline BLOB 或外部原件。

本批还收紧 `inspect_attempt`：两个路径同时存在时，除认证内容一致外，必须是同一文件身份；相同字节的独立复制文件也属于 ambiguous state。此约束统一用于 migration、capture 和 reconciliation，避免幂等路径绕过身份冲突检查。

## 验证与限制

专项合成测试覆盖健康 v8 不升级 schema、四种 pending 状态保留且可继续 capture、已提交 staging link 精确清理与重复核对、数据库文件及对象逐字节不变、全库 preflight 拒绝未知 / 缺失 / 损坏内容、独立复制文件拒绝、preflight 后文件替换、key 失败与诊断脱敏、四个错误 checkpoint、并发写锁、最终回读发现晚到损坏、v7 在 key 访问前拒绝，以及清理前后独立子进程直接退出后的恢复。

本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：197 个仓库文件、notices、fmt、all-targets / all-features locked Clippy 与 236 个 Rust tests；其中本批新增 12 项核对测试。4 个 ignored helper 均由父测试实际启动，不另计入总数。35 项检查器回归、M0 fixture 契约（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 与 `git diff --check` 通过。既有 P1-F17 本机临时端口测试需要沙箱外权限，完整检查沿权限流程运行，未跳过或放宽检查。上述测试只使用合成库与测试 key，子进程退出不等于真实断电；Windows / Linux 当前代码、真实密钥库、宿主生命周期与磁盘满未验收。

本批不是完整 orphan retirement：未建立已放弃 attempt 的持久化状态，也未开放取消或删除未引用对象的入口。现有 v9 不存在满足冻结清理条件的可删除 orphan，不能伪造此类成功证据。后续应先明确显式放弃 / 取消的恢复语义，再实现对应精确清理；完整 verify / rebuild、删除执行与宿主验收仍待完成。

核对仍会全量认证已有对象并在内存构造 active sources，没有大库性能验收。FTS 仍含完整可读正文；对象加密和 link 清理不构成整库静态加密、历史明文擦除或备份清除。

## 工作区交接

此前正文迁移与加密 capture 已按项目所有者要求提交为 `a7db975`。本批核对实现单独保留工作区差异供后续审阅；未 push、未执行远程 CI、未启动 GUI / VM / 长期服务。测试自行清理隔离目录和子进程，常规编译缓存保留。

同日后续[显式放弃切片](phase1-source-vault-abandonment.md) 已扩展维护 v10；本页上述 v8 / v9 验证保留为原批次事实。核对报告现增加 `abandonment_pending` 与 `abandoned_attempts`，只报告待完成清理，不自动作出放弃决定。原 v9 `prepared` 继续保留；已持久化 `abandoning` 经同一显式入口恢复。
