# Phase 1 capture 显式放弃与 orphan retirement

日期：2026-09-26。状态：`P1-S04 capture abandonment implemented — synthetic acceptance`。项目所有者已确认上一轮评估方案并授权实施。

## 本批契约

本批只处理尚未提交 canonical source 的 capture，沿用 ADR 0008 的 orphan 清理条件。已提交资料继续由 `DeleteRequest` / `DeletionEvidence` 管理。实现只用合成资料与测试 key；不访问真实密钥库、不接宿主、不改依赖或密码套件。

维护 v10 演进现有 attempts / references 表，capture 增加 `abandoning` 与 `abandoned`；migration 的 `retired` 不变。普通产品入口仍为 v6。状态为 `prepared → abandoning → abandoned`：第一次事务提交放弃决定后，原请求永久禁止再次提交，只能继续清理。禁止超时、重启或失败次数自动触发放弃。

先通过显式检查取得不暴露 Debug 身份的精确 target，绑定 namespace / device / provider、source、request digest、binding、locator / attempt 与对象 metadata。调用放弃入口前须由宿主取得明确操作授权；target 本身不是权限凭证。每次调用在 exclusive session 中重新匹配全部字段，过期 target 拒绝。无需原始完整 `SourceCapture`，避免丢失原请求后无法解除阻塞；全库及目标仍须验真。检查与放弃不创建 key。

第一次事务必须把 schema 升级与 `prepared → abandoning` 一起提交，提交失败不得删除文件。随后只清理该 target 的精确 staging / published 路径：无 canonical source、无 committed reference，存在的文件必须通过 metadata、AEAD、attempt 和文件身份验证；两个路径同时存在时必须为同一文件。移除前复验身份，移除后同步目录并确认两条路径均不存在，最后事务提交 `abandoned`。中断保留真实状态；已经消失的目标可在重试时继续收口，不重新发布、seal 或回退原件。

一库最多存在一个 `prepared` 或 `abandoning`。清理完成并提交终态前，所有新 capture 保持阻塞；`abandoned` 永久保留 source identity、请求摘要及原 attempt 信息，原 source ID 不可复用。重新导入使用新的 source ID 与当前有效版本关系。重复放弃同一 target 幂等；旧文件在终态后重新出现时失败关闭，不自动再次删除。

未知文件、截断或认证失败对象、不同文件身份、错误 / 缺失 key、损坏 canonical / 派生事实均保留并失败关闭。本批不提供坏文件强制清理。内部表与摘要不构成 metadata 加密；终态记录的保留不代表历史明文或备份已经清除。

## 核对与恢复

inventory 必须显式区分 committed、prepared、abandoning、abandoned，不能把非 prepared 统称为 committed。核对报告区分待恢复 capture 和待完成放弃，不自动作出放弃决定。`abandoning` 由同一精确 target 的放弃入口继续执行；`abandoned` 要求原精确路径不存在。维护恢复仍只支持完整迁移后的对象库。

## 验收范围

覆盖四种对象状态、旧请求重放、同 binding 的新 source、错误 / 过期 target、已提交对象拒绝、schema 升级与两次真实 COMMIT 失败、逐步异常及子进程退出、两条链接之间的中断、目录同步后的复验、文件替换、并发 capture、错误 key、未知 / 截断对象保留及终态对象重新出现。本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：203 个仓库文件、notices、fmt、all-targets / all-features locked Clippy 与 253 个 Rust tests。本批包含 15 项 coordinator / filesystem 测试及 2 项真实 SQLite COMMIT 失败测试；9 个 checkpoint 的异常与独立子进程退出均可恢复。5 个 ignored helper 由父测试实际执行，不另计入总数。35 项检查器回归、M0 fixture 契约（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 与 `git diff --check` 通过。既有 P1-F17 本机端口测试沿权限流程在沙箱外执行，未跳过或放宽门禁。

## 入口与交接

`inspect_library_capture_abandonment` 全量验真后返回 opaque `CaptureAbandonmentTarget`；`abandon_library_capture` 精确匹配后执行放弃，成功返回只含 `already_abandoned` 的 `AbandonmentReport`。target 不是公开协议、日志内容或授权证明，也不支持序列化；中断后可重新检查取得当前 `abandoning` target，继续已持久化的决定。没有 `prepared` / `abandoning` 时检查才返回 None，不能据此声称已经完成产品删除或宿主验收。

原子性仅覆盖各次 SQLite 事务，文件系统与 SQLite 仍无共同事务。清理中失败可能已有一个或两个路径消失；返回错误、保留真实 `abandoning` 并重试。终态提交后回读失败仍返回错误，不回滚文件或伪报成功。持久化终态保留内部 metadata，不声明整库静态加密、取证级删除或备份清除。大库性能、真实断电、磁盘满、Windows / Linux 当前代码、真实 key store 与 application / UI 未验收；FTS 仍含完整可读正文。

本批不改变 canonical schema、数据所有权、记忆状态、密码套件或依赖。实现已提交为 `3f5cc0f`，未 push、未运行远程 CI，未访问真实资料或密钥库，未启动长期服务、GUI 或 VM。合成测试自行清理隔离目录与子进程，常规编译缓存保留。当批下一步为完整 object-backed verify / rebuild，再接删除执行与宿主验收。

日终交接：截至本日结束，维护、删除兼容和认证读取 / 导出组合验收均已形成独立切片；完整提交回顾见[9 月 26 日记录](../status/2026-09-26-source-vault.md)。当前首项为 application 生命周期接入，早期批次中的待实现描述保留其当时范围。
