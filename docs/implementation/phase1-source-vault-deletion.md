# Phase 1 object-backed 删除执行

日期：2026-09-26。状态：`P1-S04 object-backed deletion implemented — synthetic acceptance`。

本批承接独立 verify / rebuild，落实 ADR 0008 的本地对象删除执行。适用完成迁移的维护 v8 / v9 / v10 / v11；第一次删除请求事务升级到维护 v11。普通 `SqliteDatabase::open` 仍只允许 v6。本批不改变 canonical schema、十组件协议、数据所有权、依赖或 lockfile，不接入 application / UI 或真实 key store。

## 授权与执行顺序

宿主显式批准 canonical `DeleteRequest` 后，调用 `PlatformKeyProvider::execute_library_deletion`。请求继续使用既有 `local_purge`、完整 lineage 与依赖 memory closure、十组件计划和 `LocalDeletionExecution`。提供请求不等于外部调用者已获授权；真实入口仍须暂停普通操作并过滤敏感 logger。

1. 验证数据库身份、schema、namespace / device / provider 和完成迁移 checkpoint，加载既有 key，认证完整 inventory、canonical 与派生事实。错误 key、缺失或损坏的 committed 对象、未知文件及 ambiguous staging 均在请求提交前拒绝。
2. 同一 SQLite 事务验证 semantic targets，存储原 `DeleteRequest`、十组件闭包及其内部摘要，关闭召回、删除 committed reference，登记 `deleting` 对象。维护 v11 升级也在此事务内；失败不留下半升级、半请求或提前关闭的引用。已有 pending capture / abandonment 或未完成删除时，拒绝新的删除请求；已冻结请求精确重试可继续。
3. 对每个 `deleting` 对象使用原 locator / attempt / metadata，认证所有现存对象及 staging，持有文件身份，精确 unlink 两个目录项并同步目录。两处都缺失且复验通过后，才提交 `deleted` 物理 checkpoint。中断允许部分文件已经移除，重试依据持久化意图继续；原始对象不能由 staging 重新发布。清理失败返回有界原始错误，请求继续保留，尚未执行的组件不产生虚假成功。
4. 全部对象退役并再次认证后，执行既有十组件删除。`source_body` 改为核对退役 checkpoint 与引用关闭，返回 `source-vault-authenticated-absence-v1`，不再将迁移后为空的 BLOB 表当作密文删除证据。其它九个组件复用既有事务、失败结果和最小审计规则；组件失败保留真实 `Failed` 结果，整体 canonical 状态为 `failed`，重试不重新开放召回。
5. 再次认证、校验派生数据并核对数据库身份后返回 `ComponentResult`。`store_library_deletion_evidence` 接受既有 canonical `DeletionEvidence`，先验真对象状态、实际持久化结果与 evidence chain，再保存并回读；它不是独立于结果的新删除协议。文件或数据库异常不返回整体成功。

内部 `source_vault_delete_plans` 保存 canonical 请求、组件定义和 frozen execution closure 的确定性摘要；`source_vault_deletions` 只保存对应对象的执行 checkpoint。请求字段或闭包漂移时拒绝恢复，不能把删除范围静默扩大。attempt 保留对象身份以识别终态后重现文件；终态不能重放旧 source capture。完成清理后的新来源仍按独立 source / lineage 身份导入。

全程持有跨 checkpoint 的 exclusive session，反复验证数据库和文件身份。`deleting` 允许认证对象尚存或已缺失；`deleted` 要求对象及 staging 均缺失，重现则保留现场并失败关闭。verify / rebuild / reconciliation 只观察这些状态，不代替删除执行。报告的 `deleted_objects` 是物理对象退役数，不等于整个十组件请求成功。

## 数据与隐私边界

对象 envelope 内的 wrapped DEK 随精确文件清理，不销毁 library key，不影响另一 provenance 的独立对象。FTS、fragment、metadata、memory 等组件各自报告实际结果；删除后的重建遵循 canonical 删除状态，不复活召回。

仍存活资料的 FTS 仍含完整可读正文。清理目录项和 wrapped DEK 不证明底层介质、SQLite 历史页、快照、备份、用户原文件或导出副本不可恢复，也不等于整个资料库静态加密或远端删除已传播。原始对象、请求或 schema 损坏不自动强制删除。

本切片建立时仅支持在对象维护入口新建并冻结的请求及其重试，对旧 v6 未完成请求保留并拒绝接管。后续[历史请求兼容切片](phase1-source-vault-legacy-deletion.md) 已增加维护 v12 的精确接管和旧执行凭据校验；证据不足的请求仍保留并拒绝，不补造授权或成功。

## 验证与限制

本批合成测试覆盖：历史 lineage 与已确认记忆闭包、同字节另一来源保持不变、十组件结果和 canonical evidence 回读、v8 升级、所有协调 checkpoint 中断恢复、对象 / staging 持久化缺失、删除后重建零复活、partial lineage / 缺 memory target / pending capture 拒绝、坏对象 / 缺对象 / 错误 key / 未知文件 / 独立 staging 拒绝、请求 / 闭包漂移、并发连接拒绝、晚到文件替换 / 重现保留、v11 后新 capture 与显式放弃，以及独立子进程在意图提交、unlink、退役提交和组件执行后退出恢复。

SQLite 专项通过真实 deferred foreign-key COMMIT 失败验证请求 / schema / 引用事务回滚与退役 checkpoint 回滚；通过真实组件 SQL 失败验证 `Failed` 结果及后续重试。本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：212 个仓库文件、notices、fmt、all-targets / all-features locked Clippy 与 280 个 Rust tests。本批新增 13 项 Source Vault 删除测试及 2 项 SQLite 删除事务 / 组件测试；7 个 ignored helper 均由父测试实际启动，不另计入总数。35 项检查器回归、M0 fixture 契约（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 和 `git diff --check` 通过。既有 P1-F17 本机端口测试在沙箱内遇到 PermissionDenied，依权限流程在沙箱外重跑完整检查后通过，未跳过或放宽检查。

上述测试只使用隔离合成库及测试 key，不访问真实系统密钥库。子进程退出不等于真实断电；磁盘满、真实 OS 凭据、Windows / Linux 当前代码和宿主生命周期仍待验收。全量认证与库大小相关，未完成大库性能验收。本批独立维护入口不等于普通产品已使用加密 Source Vault。

## 工作区与下一步

上一批 verify / rebuild 已提交为 `543108e`，本批删除实现保留为新的工作区差异；未 push、未执行远程 CI、未启动 GUI / VM / 长期服务。测试清理各自的隔离库、子进程和端口，保留常规编译缓存与任务专用临时验证日志，不纳入 Git。下一步先收口历史未完成删除请求的迁移兼容，再按当前状态评估并推进 application 数据流接入，再完成 macOS 宿主、真实 Keychain 及集中跨平台验收；具体系统操作须另行授权。

后续提交与推进：本批已提交为 `292ed57`；历史请求迁移兼容的实现与最新工作区状态见[兼容记录](phase1-source-vault-legacy-deletion.md)，当前顺位以[当前状态](../status/current.md)为准。
