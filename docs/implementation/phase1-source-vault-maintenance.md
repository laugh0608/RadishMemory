# Phase 1 object-backed verify / rebuild

日期：2026-09-26。状态：`P1-S04 object-backed verify and rebuild implemented — synthetic acceptance`。

本批提供完成正文迁移后的独立维护入口，支持维护 v8 / v9 / v10。普通 `SqliteDatabase::open` 仍只允许 v6；本批不改变 canonical schema、依赖、lockfile、密钥生命周期或数据所有权，不接入 application / UI。

## 操作契约

- `PlatformKeyProvider::verify_library_objects`：验证数据库身份、冻结 schema、结构 / 外键、key checkpoint 和 `objects_ready`，加载既有 key，认证全量对象与 pending attempt，校验 active canonical source、fragment、capture fingerprint、binding / audit、记忆来源闭包和派生行，并运行 FTS5 `integrity-check` 验证内部倒排结构。成功才返回不含路径、身份和正文的 `VerificationReport`；不清理文件、不自动修复。
- `PlatformKeyProvider::rebuild_library_derivations`：经过同样的打开与认证边界，允许派生行缺失或内容漂移。在 SQLite exclusive transaction 中从 canonical 事实重建来源 lineage tip、memory current projection 和 recall FTS，复用原有派生算法。提交前再次认证对象并验证全部事实；提交成功后再完整认证回读。前置验证或提交失败回滚派生写入；提交后回读失败仍返回错误，不能把已经提交的重建说成回滚。重试重新核对真实状态。

数据库文件身份在 key 读取、重建和回读边界反复检查，exclusive session 跨越整个维护调用。来源绑定预期改由 canonical 最新来源版本计算，不再依赖待修复 tip；binding / capture audit 本身不属于可重建数据，损坏仍拒绝。确认记忆的来源闭包通过已认证对象正文解析，不读取旧 BLOB。

报告包含认证 committed object 数量、pending capture 数量、是否存在 `abandoning`、`abandoned` 数量和本次是否执行重建。上述状态只报告、不推进；维护不能代替显式 capture 重试或放弃。canonical、attempt、reference、schema、对象及 staging 文件均不由重建修改。已删除对象不得因派生重建复活，现有 canonical 删除过滤规则保持；加密对象的删除执行另行落地。

缺失对象、错误 key、认证失败、未知文件、ambiguous inventory、canonical 损坏、未完成 migration、schema 漂移、数据库结构或外键损坏失败关闭。这里只修复结构完好的派生表中的行漂移，不承诺损坏 SQLite 页面、FTS 内部结构或外键非法行的自动修复。不回退 inline BLOB 或外部原件，不重新发布、重新加密或替换 canonical 对象。

## 验证与限制

合成测试覆盖来源历史与当前版本、已确认记忆的完整 provenance、FTS / tip / projection 同时损坏后恢复、canonical 逻辑快照和对象字节保持、v8 不升级、v9 pending 和 v10 abandoning / abandoned 保留、坏对象 / 缺对象 / 未知文件 / proposal / audit / binding / schema 损坏拒绝、FTS 可见行完整但内部倒排结构损坏拒绝、key 失败脱敏、v7 在 key 访问前拒绝、独占连接阻止并发写、提交前晚到损坏回滚、提交后晚到损坏不返回成功、各 checkpoint 错误及独立子进程在 commit 前后退出恢复。SQLite 专项使用真实 deferred foreign-key COMMIT 失败验证回滚，未以默认成功代替真实提交。

本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：206 个仓库文件、notices、fmt、all-targets / all-features locked Clippy 与 265 个 Rust tests；本批新增 11 项维护协调测试和 1 项 SQLite 真实 COMMIT 失败测试。6 个 ignored helper 均由父测试实际启动，不另计入总数。35 项检查器回归、M0 fixture 契约（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 与 `git diff --check` 通过。既有 P1-F17 临时端口测试在沙箱内遇到 PermissionDenied，按权限流程在沙箱外重跑完整检查后通过，未跳过或放宽检查。所有测试使用合成库与测试 key，不访问真实系统 key store。子进程退出不等于真实断电；磁盘满、Windows / Linux 当前代码、真实密钥库与宿主生命周期尚未验收。

维护仍全量认证对象并在内存构造 active sources，未做大库性能验收。FTS 仍含完整可读正文；本批不构成整库静态加密、历史明文擦除、备份清理或恢复承诺。真实 provider 调用继续要求宿主授权、暂停普通操作和过滤敏感上游 logger。本批独立入口未关闭 R03 的产品启动失败修复缺口。

## 后续顺位与交接

下一步推进 object-backed 删除执行，再接入 application / macOS 宿主及集中跨平台验收。既有显式放弃与 orphan retirement 已提交为 `3f5cc0f`；本批维护实现保留为新的工作区差异。未 push、未执行远程 CI、未启动 GUI / VM / 长期服务。测试自行清理隔离库、子进程与端口，保留常规编译缓存；本批验证日志位于任务专用临时目录，不纳入 Git。
