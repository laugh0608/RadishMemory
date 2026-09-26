# P1-S04 历史删除请求迁移兼容

状态：`P1-S04 legacy deletion compatibility implemented — synthetic acceptance`（2026-09-26）。

本批承接 `292ed57` 的对象删除执行，处理旧 v6 明文库中已登记、执行中断或组件失败的 `DeleteRequest`，也允许同一已完成请求的显式重试。不改变 canonical schema、十组件协议、依赖、lockfile、数据所有权或删除保证。普通 `SqliteDatabase::open` 仍只允许 v6；兼容执行限定于已完成正文迁移的独立维护入口。

## 接管契约

1. 调用方提交原 `DeleteRequest`，必须与 SQLite 原记录逐字段一致，namespace / device 匹配密钥 profile，十组件定义仍满足既有协议。先对实际对象全量认证，再检查旧请求的 frozen execution closure；不开辟第二条授权入口。
2. 复核来源 / 记忆根、完整 lineage、仍存在的 fragment 归属、记忆 / proposal 依赖、decision / event 关联、FTS 和最小审计范围。所有目标必须已经关闭召回。旧执行器已移除的 fragment、已脱敏且失去来源链接的 proposal 只能由原请求真实成功组件结果解释；没有可复验依据则拒绝接管。
3. 单个 SQLite `IMMEDIATE` transaction 升级至维护 v12，冻结原请求 / 闭包摘要，并关闭残留对象的 committed reference，登记既有 `deleting` checkpoint。正文已在旧执行器中消失的来源，在 `radishmemory_legacy_body_retirements` 保存原请求、source ID 与旧执行 attempt ordinal；不补造 attempt、对象或 locator。事务失败保留原请求、原引用和原 schema。
4. 对象仍存在时，沿用 v11 认证、精确 unlink、目录同步及缺失复验。正文迁移前已消失时，必须确认无 inline BLOB、无对象 attempt / reference / deletion，且原请求的 `source-body` 结果为 `succeeded`，outcome 为 `deleted` / `not_found`，方法为 `sqlite-row-absence-v1`，processed count 完整、时间合法且匹配原 attempt，无错误或不适用 retention 字段。缺少旧执行凭据时失败关闭；空表本身不构成成功证据。
5. 接管后执行并持久化新的十组件结果，旧 attempt / result / evidence 均保留。包含历史正文缺失凭据的 body 结果使用 `source-vault-and-legacy-body-absence-v1`；全为历史缺失时报告 `NotFound`，不声称本次删除了物理对象。证据仍由既有 canonical 校验器核对真实结果与 evidence chain。
6. 后续 verify / rebuild / reconciliation 和删除重试均复验 v12 凭据及实际缺失；原结果漂移、凭据删除、正文重现或冻结计划变化导致失败关闭。`deleted_objects` 只计真实对象退役，不把历史缺失来源计成新对象。

接管只发生在显式删除调用中，不由打开库或迁移自动触发；pending capture / abandonment 继续阻止接管。合法重试不重新开放召回。v12 后续 capture、删除与显式放弃应保留 v12 schema。

## 验证与限制

定向合成验证覆盖：pending 请求、真实 body / metadata / fragment 组件 SQL 失败、已消失正文、部分 attempt 结果、原证据链保留、后续新 capture / 删除、闭包漂移、凭据缺失 / 计数错误 / 时间错误 / 失败状态 / 方法错误、凭据篡改及 BLOB 重现。SQLite 通过真实 deferred foreign-key COMMIT 失败验证接管原子性；独立子进程在接管、unlink、退役、执行后退出，由父进程重新打开并完成原请求。

本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：216 个仓库文件、notices、fmt、all-targets / all-features locked Clippy 与 289 个 Rust tests。新增 8 项 Source Vault 兼容测试和 1 项 SQLite 接管事务测试；7 个 ignored helper 由父测试实际执行，不另计入通过数。35 项 Python 检查器回归、M0 fixture 契约（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 与 `git diff --check` 通过。既有 P1-F17 本机临时端口测试在沙箱内失败，按权限流程在沙箱外重跑完整检查通过，未跳过或放宽检查。所有测试仅使用隔离合成库和测试 key；没有访问真实系统 key store、启动 GUI / VM 或更改依赖。子进程退出不代表真实断电，Windows / Linux 当前实现、真实 Keychain 和宿主生命周期仍待集中验收。

本地 SQLite 请求摘要和执行凭据用于检测状态漂移，不是对恶意整库一致改写的密码学证明；历史已删行的 provenance 不能从正文恢复，只能结合原冻结闭包与已持久化执行结果复核。资料仍存活时，FTS 仍含完整可读正文；本批不扩张静态加密范围，不销毁 library key，不保证历史明文页、备份、快照、外部副本或取证级删除。

## 工作区与下一步

上一批删除执行已按用户要求提交为 `292ed57`。本批兼容实现为新的工作区差异；未 push、未运行远程 CI。代码复核确认 `LocalLibrary` 仍直接持有 `SqliteDatabase`，目录、检索结果回源与精确导出均走明文 source loader。下一步先补齐认证读取、目录、检索及导出的 object-backed 存储适配，复用既有 `SourceVault` / `SourceCatalog` / `LocalSearch` 语义，再接入 application 的显式打开流程和完整操作链路；不能以独立维护接口通过代替普通产品已切换。真实系统凭据、宿主交互及集中跨平台检查仍按具体范围授权。
