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

<a id="deletion-evidence-digest-review"></a>

## 2026-11-15 删除回执摘要契约审阅

状态：离线审阅及现有回归通过。完整回执摘要和兼容策略待所有者确认。

### 生成、持久化和回读

| 路径 | 实际契约与影响 |
| --- | --- |
| 当前明文 v6、当前显式加密与旧宿主 | 两个 application 入口共用 `compute_deletion_evidence_digest`。从 `a4150ce` 的明文宿主及 `bd0c11b` 的加密接入即可追到相同投影。以 `deletion-evidence-v1` 对回执 ID、请求 ID、总状态与按原计划顺序排列的组件键、处理数和状态做 SHA-256。未覆盖前一回执、namespace、device、时间、校验者、组件目标、动作、outcome、验证方法、错误及保留依据 |
| M0 fixture 合同和 runner | [摘要契约](../evaluation/m0-fixture-contract.md#摘要-profile)为同名 profile 定义完整 `DeletionEvidence` 移除摘要字段后的 canonical JSON，集合使用稳定次序。M0 runner 实际自行构造与 application 相同的缩减投影。现有 core / SQLite 测试还有用空对象或测试标记生成的摘要，属于合成占位，不能据此认定所有历史产品回执可复算或有效 |
| 写入与物理执行 | `store_evidence` 核对请求、计划组件、最新 attempt 的持久化结果、整体状态和原链头。加密写入另做对象与数据库复验并回读同一回执；对象退役和历史缺失有各自 checkpoint 与执行凭据。存储过程接受有合法形状的摘要值，未将值与内容比对 |
| 回读与恢复 | 共用 `load_evidence` 由请求和指定 attempt 重建各组件结果，并调用 `DeletionEvidence::new` 和 `validate_deletion_evidence` 校验结构。SHA-256 字段只验 profile 与十六进制格式，未重算。此限制同时影响直接回读、加密 latest / unfinished 查询与同请求重试返回既有 `Completed`，以及用该状态判定完成的召回格式切换条件 |

### 离线对账

在 `85d97fe` 的应用入口为独立合成库各执行两版本 lineage 删除。关闭重开后，明文 v6、加密 v11、v6 删除后再迁移到 v8 的旧回执均读回原 `Completed` 和十项成功结果，加密入口的同请求显式重试返回同一回执。检查了删除词零命中、行内正文及 FTS 行为零、原组件目标计数和正文验证方法。

对三个独立回执做过四次只读 SQLite 快照对账，含历史回执迁移前后。用独立 JSON / SHA-256 实现按持久化组件 ordinal 复算，四次均等于存储值。按组件键排序则四次均不同。明文与加密合成用例的投影输入相同，因此得到相同摘要，两条真实正文结果的验证方法却不同。迁移前后历史回执字段、摘要和各组件结果保持一致。因而已知运行时格式可从现有持久事实复算，历史兼容必须保留原有顺序。证据来自合成执行和只读复核，未评估未知旧生产者的历史资料。

M0 fixture 契约与嵌入 runner 通过，覆盖 12 场景、86 操作及 12 个指标 gate。core、SQLite、Source Vault 删除与 application 定向回归 74 项通过，含原请求历史接管、已有回执保留和结构损坏拒绝。这些是当前行为的基线。

### 对有限验收的判断

既有通过场景中，原请求、真实组件结果和正文缺失的证据继续适用。当前实现缺少对回执摘要的值复验，且运行时字段覆盖比规范窄，因此 `Completed` 仍缺完整摘要可复验的证据。它影响当前新写回执，也影响以旧回执状态判断恢复是否结束。将其列为本批删除回执验收的阻塞，之前的实际删除和重建无复活结果继续按原场景记录。无密钥摘要只支持一致性比对，信任边界仍限本地执行和存储。

### 待确认的兼容方案

先冻结已知旧投影的规范输入及原 ordinal 顺序，提供覆盖生成、写入、读回的 golden vector。新回执使用明确的版本标识与统一全字段 codec，区分已验证旧投影和已验证全字段回执，并保留现有请求 / attempt / 组件证据校验。实现前需确认 profile 命名、明文 v6 与加密库的读写兼容、记录版本的持久化位置，以及已知旧生产者的兼容资格。全字段 codec 须冻结可选字段、引用、时间及集合排序规则。未知或不匹配的历史记录保留原字节，以有界错误报告核验失败，既不当作已验证完成，也不以失败回执推导重做删除的授权。

实施验证覆盖当前明文与加密生成、旧回执迁移前后、失败后重试追加链、未知或不匹配摘要、仅修改被旧投影遗漏的字段，以及回读与恢复发现的失败关闭。此审阅不修改 canonical schema、profile、业务代码或既有回执。

## 工作区与下一步

上一批 verify / rebuild 已提交为 `543108e`，本批删除实现随后提交为 `292ed57`；未 push、未执行远程 CI、未启动 GUI / VM / 长期服务。测试清理各自的隔离库、子进程和端口，保留常规编译缓存与任务专用临时验证日志，不纳入 Git。当批下一步为历史未完成删除请求的迁移兼容，再按当前状态评估并推进 application 数据流接入，再完成 macOS 宿主、真实 Keychain 及集中跨平台验收；具体系统操作须另行授权。

后续历史请求迁移兼容见[兼容记录](phase1-source-vault-legacy-deletion.md)，当前顺位以[当前状态](../status/current.md)为准。

日终交接：截至本日结束，维护、删除兼容和认证读取 / 导出组合验收均已形成独立切片；完整提交回顾见[9 月 26 日记录](../status/2026-09-26-source-vault.md)。当前首项为 application 生命周期接入，早期批次中的待实现描述保留其当时范围。
