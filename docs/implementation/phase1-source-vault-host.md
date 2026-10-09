# P1-S05 桌面加密宿主接入

状态：`P1-S05 encrypted desktop integration — native acceptance pending`（2026-10-01）。

项目所有者已授权实现 macOS 宿主接入，并在本任务批准下述依赖连线。恢复前置改动已提交为 `b982cec`；desktop worker、共用 controller、显式加密 / 维护界面和诊断抑制已提交为 `6a413f8`。默认启动保留 SQLite v6 入口，不以合成测试宣称真实系统凭据或 GUI 验收已完成。

## 实现与操作边界

所有存储、全库认证、迁移和 provider 操作均在一个串行 worker 内执行。UI 只发送显式动作并接收只读快照；系统文件选择器仍在主线程，选择结果作为一次性请求进入 worker，消费后不存入 host profile 或诊断。worker busy 时不接受第二条操作，退出时发送停止信号并 join；线程失败显示 `WorkerStopped`，不能伪造成功或空目录。

`LibraryController` 使用共用的目录、版本和选择逻辑，内部区分旧 `LocalLibrary` 与显式 `EncryptedLibrary`。桌面提供如下动作，初始化和迁移前销毁旧 controller / SQLite 连接；普通打开失败不自动执行准备、迁移、重建或明文 fallback。

| 动作 | 真实行为 |
| --- | --- |
| Open legacy plaintext | 默认旧 v6 入口；遇 v7 至 v12 仍拒绝 |
| Open encrypted objects | 只加载既有 key，认证打开已迁移库并发现恢复状态 |
| Prepare key | 确认后调用既有初始化协调器；可能创建系统凭据，已有 checkpoint 缺 key 时不重新生成 |
| Migrate / resume bodies | 确认后使用既有 key 执行 v7 / v8 正文迁移或精确恢复；之后须显式打开 |
| Verify / Rebuild recall | 分别只核对或显式重建派生行；加密打开失败时仍使用同一目录 / profile，拒绝修复损坏 canonical 或自动推进 pending |
| Refresh | 重新读取目录与恢复状态；不重复先前写入 |

界面明确显示 FTS 仍保留完整可读正文。初始化、迁移及放弃的确认窗口冻结其它操作，避免用户确认期间当前选择或 inspected target 被 UI 动作替换。各 coordinator 仍在实际执行时复验状态，抵抗其它进程在确认期间改变目录。

## 原请求和恢复

worker 在执行 capture 前持有完整 canonical `SourceCapture`。写入失败保留同一快照和 provenance，阻止新写入、切换库和重新准备；“Retry original request”不重新读外部文件，也不生成新 ID。提交成功立即释放快照，之后目录 / 选择刷新失败单独显示，保留“已提交”的真实结果并引导 Refresh，不提供重复 capture 的错误暗示。一般读取错误隐藏旧 UI 数据，显式 Refresh / reopen 成功后再恢复显示。

原 capture 仅在内存持有，不新增明文磁盘 journal，不承诺在崩溃后保留快照或清零全部进程内副本。关闭时若仍有原请求，提示快照损失与后续限制；busy 时取消关闭请求，等待真实执行结果。进程被强制结束不具备该交互保证。重启后已提交 capture 通过目录恢复；未提交 capture 若快照丢失，不允许从 fingerprint 或外部文件伪造精确重试。认证 inspect 只保留不透明 target，明确确认后才调用精确放弃；没有自动清理。未知文件、已提交来源和其它库 target 不在其授权范围。

删除失败保留原 `DeleteRequest`。`unfinished_delete_requests` 在认证 reader 内发现同 namespace 尚无 Completed 最新 evidence 的原授权，包括执行已结束但 evidence 尚未落盘的窗口。它先复验最新 evidence 的身份、时间、计划和真实持久化组件结果，再排除 Completed；不只过滤 status 字段。重启恢复按钮绑定原 request ID 和完整请求，明确继续时不重新规划当前选择的闭包。完成、失败或部分状态按真实 evidence 展示；外部原件、导出、备份和其它设备不属于本地回执。

若请求在 durable mutation 之前因永久冲突失败，worker 仍保留它以免误判提交边界；用户可以保留窗口进行诊断，或确认关闭以丢弃内存副本。只有实际存在并认证通过的 uncommitted capture 才出现精确放弃入口，不能凭 UI 错误状态合成删除授权。

## 有界诊断与日志策略

`DesktopError` 只保留 application operation / code / reason、Source Vault code 以及数据库枚举和数值错误码。key missing、locked、denied、cancelled、authentication 与 database 原因可区分，不保存路径、slot identity、正文、密钥或上游错误链。

创建生产 worker 前安装 process-wide 无输出 `log::Log`，设置等级 Off；即使上游提高等级，这个 logger 仍没有 sink。已有 logger 导致安装失败时不创建 worker、不访问 provider。panic hook 丢弃任意 payload，worker 通过 channel 断开报告线程失败。该策略牺牲进程原始 panic 诊断以保护可能携带私密内容的 payload；不新增文件、网络或 telemetry 输出。

检查器对 `logging.rs` 的完整已审阅内容保存 SHA-256，变更必须重新审阅；只放行该精确文件，其余第一方源文件继续禁止 log / tracing / print sink。检查器回归覆盖精确实现通过、该文件追加输出被拒绝，以及其它文件的输出仍被拒绝。隔离子进程测试验证 logger、等级被上游提高与带合成敏感标记的 panic 都不输出该标记；另在已有 logger 的隔离进程证明安装失败会阻止 worker 创建。此证据不覆盖原生库 stderr、系统崩溃报告或真实 Keychain 的诊断行为，仍需原生验收。

## 已批准的依赖连线

| 位置 | 声明 | 用途与影响 |
| --- | --- | --- |
| desktop runtime | `radishmemory-source-vault.workspace = true`，workspace `=0.1.0` | 直接引用已经可达的 sealed provider 和有界错误，不新增第三方 package |
| desktop runtime / workspace | `log = "=0.4.34"` | 沿用已锁定 MIT / Apache-2.0 版本，安装无输出策略 |
| desktop dev-dependency | Source Vault `acceptance-test-support` | 以固定合成 key 驱动相同 worker / controller 流程；默认生产构建不启用 |

离线 Cargo 解析仅增加 desktop 的两条 lockfile 边；全部 453 个 package 的 name / version / source / checksum 集合与改动前逐项一致。依赖基线、manifest 精确检查与 notices lockfile 摘要同步更新。没有下载或升级第三方包，没有访问真实系统密钥库，也没有新增持久 schema、依赖宿主 journal 或第二套删除算法。

## 验收证据

前置 application 批次通过完整检查：245 个仓库文件、25 个 Rust test suites 共 334 项通过、10 个 ignored helper 由父测试调用，42 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）及 compile-fail doctest 通过。本批 desktop 已通过 targeted fmt、locked all-targets / all-features Clippy 与 26 项测试（11 项新增）；另一个 ignored 日志子测试由父测试在隔离进程执行。

新增覆盖：显式准备与缺 key 不重建；publish / commit 后原 bytes / provenance 重试；阻止新请求替换；提交成功但目录刷新失败不保留 retry；原请求重试成功后恢复检查失败单独回报；重启丢失 capture 后只观察并明确放弃；删除 intent / execution / evidence commit 前后四个恢复窗口；provider 失败诊断及旧视图隐藏；真实 worker 线程执行、串行 busy 拒绝、停止和 join；failed-open 后维护位置保留；加密更新、搜索和历史版本导出；隔离日志 / panic 抑制。

最终 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 在沙箱外完整通过：249 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，25 个 Rust test suites 共 345 项通过；11 个 ignored helper 由父测试调用。43 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest、默认 desktop 构建和 `git diff --check` 通过。默认 normal / build / features 依赖图未启用 `acceptance-test-support`。首次完整检查仅在既有 P1-F17 临时端口处受沙箱拒绝，获准重跑；最终代码与已审阅日志源码摘要已复验。所有测试使用合成资料、固定测试 key 或随机合成 ID，清理自身隔离目录、子进程和临时端口；无长期测试服务。日志留在仓库外任务临时目录，不包含真实个人资料。

## 未完成范围

既有 `DeletionEvidence` loader 只检查 `evidence_digest` profile / 格式，不重算摘要；历史 fixture 摘要内容不统一。本批不改变摘要语义或兼容性，不将该字段视作整库未被恶意改写的密码学证明。摘要契约、历史兼容与统一复算需要独立收口；恢复判断使用经复验的请求、计划与持久化组件结果。

截至 2026-10-01，真实 Keychain 读写、用户授权提示、取消 / 锁定、实际 GUI 关闭确认、多实例和端到端验收未执行；Windows / Linux 保持后置集中检查点。下一批需先列出隔离测试目录、合成 namespace / device 对应的精确 slot、运行命令、预计时长、系统提示及清理 / 保留方式，再取得当前任务授权。删除测试 slot 前必须确认对应合成对象库不再需要，不能将删除 key 当作普通回滚。

默认启动仍是 SQLite v6 inline plaintext body；显式原始对象加密不等于整个资料库静态加密，FTS 保留完整正文，旧 SQLite 页、快照和备份也未证明物理清除。中文检索、目录第 201 条、性能和默认 v6 的 failed-open 修复等质量缺口不在本批闭环范围，不授权真实个人资料使用。

2026-10-01 记录的原生隔离入口缺口已由 2026-10-06 的准备切片接线：原生启动严格解析独立根与两个合成身份参数，worker 每条命令复验既有 profile，不自动补建或切回默认库。正常无参数启动仍保持旧入口。具体参数、失败关闭边界、精确 slot 授权材料与待执行场景见[原生验收计划](../evaluation/phase1-source-vault-native-acceptance.md)。不替换 HOME 或全局配置，不新增依赖、持久 schema 或密钥映射。准备切片结束时真实 Keychain / GUI 尚未执行；后续结果见下文最小真实闭环。


## 2026-10-06 隔离入口准备证据

新增 `startup` 解析与目标绑定，通过既有路径、profile 和 worker 进入同一 production 链路；默认入口保留。新增 9 项合成回归覆盖参数不完整 / 重复 / 非法身份、缺失 / 错误 profile、运行期间 profile 丢失、默认目录重叠及别名、符号链接、Unix 权限、隔离旧库迁移 / 历史精确导出 / 重开。桌面单元测试共 35 项通过；日志子测试仍由父测试在隔离进程调用。

本机完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过：253 个仓库文件、notices、fmt、locked all-targets / all-features Clippy、25 个 Rust test suites 共 354 项通过，11 个 ignored helper 由父测试调用。另有 43 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 和默认 desktop 构建通过；默认 normal / build / features 图不启用 `acceptance-test-support`。首轮完整检查仅在既有 P1-F17 本地观察端口处受沙箱拒绝，按权限流程在沙箱外重跑同一离线检查通过，没有跳过或放宽门禁。

本批未修改 manifest / lockfile、canonical schema、slot 映射或 logger 实现；提前在原生窗口创建前安装同一诊断策略。已在忽略的任务目录准备合成输入、预建 profile、文件摘要及精确操作单，未创建验收数据库、启动 GUI 或访问系统 keychain。相关失败路径证据仍使用合成 provider，不能当作 macOS 原生验收；Windows / Linux 当前新增入口仍待集中检查点。未提交、未推送，不改变任务开始前已有提交。


## 2026-10-06 macOS 最小真实闭环

隔离入口提交为 `85a1722`。项目所有者在本任务确认精确操作单及真实系统范围后，在独立合成目录、随机合成 namespace / device 和一个精确 User Keychain slot 上执行原生验收。启动前 metadata-only 查询确认该 slot 不存在；未输出 secret、未导出全 keychain，也未锁定或修改默认 keychain / ACL。

裸 Cargo 可执行文件能够打开隔离 v6，但 UI 工具无法绑定无 bundle 的进程；项目所有者正常关闭它后，在忽略的任务目录生成临时 `.app` 元数据并复制同一二进制，字节摘要一致。包装未安装、未签名，验收进程的启动命令均显式携带三个验收参数；默认目录未作为测试目标。此项是本机验收适配，不是发行包证据。

在原二进制上经真实 GUI / native picker 完成：

- 导入 139 字节合成 Markdown v1，`nebula` 检索返回来源；Prepare key 创建唯一 slot 并完成 checkpoint；正文迁移、加密打开成功。
- 加密导入另一份 71 字节合成资料，原来源更新为 151 字节 v2；目录为两个 lineage，版本历史保留 v1 / v2，`nebula` 和 `comet` 分别返回预期来源。
- 选择 v1 经 native save panel 精确导出；SHA-256 与导入前合成 v1 一致，与已经更正的外部 v2 不同。
- 正常退出且进程返回 0；用同一参数重开。默认旧入口拒绝维护库属预期，显式加密打开使用既有 key，来源 / 版本恢复；Verify 显示 3 个对象、0 pending capture / deletion，未隐式 rebuild。重启后第二份资料的导出与输入 bytes 一致。
- 分别确认删除两条合成 lineage；两份 `DeletionEvidence` 均为 Completed，各有 10 项真实组件结果。多版本来源的正文 / fragment 为 2/2；最终目录为零，`nebula` / `comet` 无结果，Verify 为 0 对象、0 待处理。
- 关闭后只读复核维护 v11：三条对象删除 checkpoint 为 `deleted`，body / fragment / committed reference / FTS 行均为零；20 项持久化组件结果均为 succeeded。attempt 身份及最小审计按既有删除契约保留，不把 attempt 行数非零当作正文仍存活，也不删除这些记录来迎合验收计数。
- 两次包装进程均正常返回 0，profile 摘要未变，原输入与两份导出保留。精确 metadata-only 查询再次确认同一默认 User Keychain 下 service / account / label 匹配，slot 保留；没有删除 key。

原二进制的创建和重开未观察到系统授权提示，因此不据此声称取消、拒绝或锁定通过。stdout 为空；两次包装进程的 stderr 各有一条 150 字节 macOS TSM CapsLock 诊断，未发现合成正文、namespace / device、slot account 或测试数据目录标记。Rust 无输出 logger 不抑制这些原生诊断；此检查不覆盖 secret 的所有可能编码、系统崩溃报告或 OS 内部副本。

此次窗口截图暴露了三个真实 UI 缺口：Search 输入框的无限期望宽度挤出按钮；目录和结果的 ScrollArea 共用默认 ID，在两区同时存在时出现调试警告；中文正文以缺字方框显示。前两项以预留按钮宽度和独立 ID 作局部修复，不新增依赖；中文字体及其跨平台来源 / 许可证仍需质量批次解决。最终布局构建在同一隔离库 / slot 复验：搜索行保持单行高度，重新加密导入 71 字节资料，经可见 Search 按钮检索命中；目录与结果同时存在时未再显示 ID 冲突。随后删除本次新 lineage，第三份 Completed 回执含 10 项真实组件结果，Verify 与检索再次确认 0 对象、0 pending 和无结果，正常退出码为 0。修改二进制后系统再次请求访问，项目所有者报告选择仅本次允许，随后既有 key 打开成功；这不构成取消 / 拒绝的通过证据。

本次只证明 macOS 合成资料的最小正常链路。busy / 关闭原快照、原请求重试、精确放弃、原删除授权恢复、真实损坏维护、多实例、缺 key / 锁定 / ambiguity、Windows / Linux、中文质量及生产发行仍未关闭；不宣布整个 P1-S05、Q01 至 Q08 或全部 P1-SF 场景完成。FTS 仍有完整正文，删除也不代表历史页、快照、备份或外部副本被清除。


### 原生复验收尾与异常

局部修复迭代中，关闭验收窗口后调用 UI 状态查询，额外出现无 Synthetic acceptance 标识的默认空库窗口；已立即正常关闭，未导入资料或执行凭据操作。只读文件元数据确认默认应用目录下 profile 与 SQLite 文件在该时刻新建。未读取其正文、未自动删除或修改目录；因此不能声称此次工具操作完全没有默认目录副作用。后续关闭后仅等待原进程退出，不再对已关闭应用查询界面。该异常属于验收工具 / 应用启动边界，未发现显式参数解析回退的证据；下次绑定工具前仍需确认窗口身份。

合成输入、精确导出、隔离库、验收日志、临时应用包装与唯一测试 slot 保留，未执行 key 删除、系统锁定、ACL 调整、安装或远程写入。最终只读复核：三份 Completed evidence、30 项 succeeded 组件结果、四条 deleted 对象 checkpoint；body / fragment / committed reference / FTS 均为零。run 4 / 5 均正常返回 0，最终进程核对未发现仍运行的 desktop；run 4 诊断为空，run 5 仅有同类 150 字节 TSM 诊断，合成正文 / 身份标记检查未命中。

布局修复后完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过：253 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，25 个 suites 共 354 项 Rust tests 通过，11 个 ignored helper 由父测试调用；43 项 Python 检查器回归、M0 fixture 与默认 desktop 构建通过。compile-fail doctest 使用同日准备切片的通过证据；本批未改变其接口。最终文档检查和 `git diff --check` 通过，未新增依赖或改变 manifest / lockfile。


## 2026-10-06 受损库重启与恢复扩展

在 `6376def` 基线上，项目所有者确认了独立操作单后，复用前批的合成 profile、隔离目录及唯一 Keychain slot，以默认 production 构建完成两项原生扩展。没有启用故障 provider；派生行和对象字节由任务脚本在 GUI 正常退出后精确注入，不能当作自然故障、真实断电或全部恢复窗口证据。输入为 96 字节合成 Markdown，初始导出、派生修复后导出和对象还原后导出的 SHA-256 均相同。

| 场景 | 实际观察 | 复核 |
| --- | --- | --- |
| 派生损坏后重启 | 仅删除唯一 lineage tip 派生行并替换唯一 FTS 内容；加密打开与 Verify 均返回 `DerivedDataMismatch`，目录 / 正文隐藏；显式 Rebuild 返回 1 个认证对象与 `derivations rebuilt: true`；再打开恢复同一来源，检索命中且精确导出成功 | 打开 / Verify 后磁盘仍保留注入状态；重建后排除 FTS / tips 的表逐行摘要与全部密文 bytes 均未变，原 source / version 未重建 |
| 密文损坏失败关闭 | 保存唯一对象原字节后翻转最后一个字节，长度不变；重启后打开、Verify、Rebuild 均返回 `AuthenticationFailed`；界面无正文和导出入口 | 应用未改写损坏 bytes，canonical 表摘要不变；正常退出后由任务按损坏摘要核对并还原原字节，再打开 / 检索 / 导出恢复同一 source。任务还原不表示产品提供自动密文修复 |

本轮新增一项 desktop worker 回归，分别覆盖对象缺失和最后一个字节损坏：使用真实隔离启动解析、重建 Engine 生命周期和 opt-in 合成 key provider；旧入口、加密打开、Verify、Rebuild、Search 与 Export 全部拒绝。外部原件替换也不回源，不新建 key、不修改损坏对象；只恢复测试原密文字节后，原 source 与精确导出才恢复。该自动化证据没有访问真实系统凭据，不能替代上述原生运行。

收尾通过 GUI 删除本轮唯一 lineage，得到一份 Completed evidence、10 项 succeeded 组件结果；Verify 为 0 对象 / 0 pending，检索无结果。关闭后只读复核 body、fragment、committed reference、lineage tip、FTS 均为零，活动对象目录为空；原有最小审计与终态记录保留。未点击 Prepare key，未新增或删除凭据，profile 摘要不变。四轮进程均正常返回 0，关闭后未查询已退出应用界面。保留合成输入、三份导出、注入前数据库 / 对象备份和日志；这些测试备份位于受管库外，不计入删除回执。

四轮 stdout 均为空，第二、四轮 stderr 各有同类 150 字节 macOS TSM CapsLock 诊断，其余为空；合成正文 / namespace / device / 数据路径标记未命中，不据此推断 OS 内部副本或 secret 任意编码没有泄露。系统凭据访问成功不替代取消 / 拒绝 / 锁定测试。额外默认空库仍按前批记录保留，本批未以其为测试目标。

完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过：253 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，25 个 suites 共 355 项 Rust tests，11 个 ignored helper 仍由父测试调用；默认 desktop 构建、最终文档检查及 `git diff --check` 通过。本批只增加回归和验收文档，没有依赖、持久 schema 或生产行为变化；未重复运行未受影响的 Python 检查器回归、M0 fixture 与 compile-fail doctest，沿用同日上批证据。

本批补齐显式加密入口的两条真实故障边界；Q03 的默认 v6 failed-open、canonical / binding 损坏及取消路径仍未完整覆盖，Q07 / Q08 和 P1-S05 不整体关闭。busy / 关闭快照、原请求重试、精确放弃、原删除授权恢复、多实例、真实密钥失败及 Windows / Linux 当前代码集中检查仍待后续具体批次。中文质量、FTS 完整明文及历史页 / 备份删除限制保持不变。

## 2026-10-09 原请求恢复的文件系统故障预演

本批先核对 worker / backend / controller / UI 与既有合成中断测试，没有另造恢复入口或改变生产语义。加密会话逐次检查精确 schema，额外 SQL trigger 无法作为安全的 commit 故障构造。改为在测试创建的 Unix 合成对象目录撤销 owner 写权限，保留读取和 staging 写入，使真实发布或删除返回 Io；作用域 guard 在正常结束和断言失败时恢复原权限，再由 fixture 清理自身目录。

新增三项 desktop worker 回归，使用既有合成 key provider，不设置 coordinator 中断回调：

- 发布失败留下单份 staging；Inspect 与再次失败重试均保持密文，替换并移除外部原件后，恢复权限并 Retry，最终对象 bytes 与原 staging 一致，精确导出保留 BOM / CRLF / Unicode 原文，只有一次 key 创建。
- 已提交版本的更新发布失败；销毁并重建 Engine 后没有原快照，只能观察和精确放弃。Verify / Rebuild 不移除 pending，放弃只清理该 staging，原 committed 对象、source ID 和精确导出保持不变；重复放弃被拒绝。
- 两版本 lineage 删除在 unlink 失败前已保存原授权；重启后继续失败仍持有该请求，恢复权限后精确重试得到同一 request ID 的 Completed 十组件回执。目录当前选择是另一 lineage，恢复不重新规划它；对照对象与导出 bytes 不变，外部输入保留。

本机三项 targeted 回归通过，默认 locked / offline desktop 构建通过。当前 production 二进制与前批保留的原生包装摘要相同；本批只增加测试，没有修改生产代码、依赖、lockfile、schema 或系统凭据。这里只证明真实 Unix 文件系统故障经过 worker 的恢复行为，不证明原生窗口、Keychain 提示、Windows / Linux 或完整 P1-S05 通过。

完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过：255 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，25 个 suites 共 358 项 Rust tests；11 个 ignored helper 仍由父测试调用。首轮仅在既有 P1-F17 合成本地观察端口因沙箱 PermissionDenied 失败，按权限流程在沙箱外重跑同一离线检查通过，没有跳过门禁。最终文档检查与 `git diff --check` 通过；未重复未受影响的 Python 检查器回归、M0 fixture 或 compile-fail doctest，未执行 GUI / 真实 key store / 其它平台验收。测试自行退出并清理临时 fixture；本批忽略目录内的输入、原生操作单和验证日志保留，改动未提交、未推送。

已只读核对前批合成库：profile 匹配，schema v11，active references / bodies / fragments / tips / FTS 及对象 / staging 均为空。已在忽略目录准备本批输入、独立导出目标、权限 guard 与精确原生操作单，尚未启动 GUI、查询系统 slot、改变该原生库权限或修改其资料。下一步是在当前任务精确授权后复用原目录和唯一既有 key，执行[恢复场景](../evaluation/phase1-source-vault-native-acceptance.md#后续恢复和失败扩展)；不重新初始化 key。关闭提示 / Cancel、放弃确认 / Cancel、重启后原删除按钮及 busy 可见交互继续待验，提交后刷新失败沿用既有合成证据。

## 2026-10-09 原请求恢复的 macOS 原生验收

项目所有者在本任务明确批准上述精确操作单后，复用前批合成库、profile、唯一既有 User Keychain slot 和默认 production 包装二进制执行三条恢复路径。二进制 SHA-256 与 10 月 6 日恢复批次相同；未启用合成 provider 或 coordinator 中断回调。精确元数据查询首次在沙箱返回 44，按权限流程在沙箱外复验 service / account / label 匹配；未请求或输出 secret，不将沙箱错误记为真实缺 key 验收。三轮显式加密打开均成功，未观察到系统授权提示。

| 场景 | 原生观察 | 磁盘 / 导出复核 |
| --- | --- | --- |
| 发布失败后原请求重试 | 仅将专用对象目录 mode 从 `0700` 改为 `0500`，Import 返回 `ImportNewSource / SourceVaultFailure / Io`；原快照仍持有、切换库禁用；Inspect 保留 target；关闭提示出现，Cancel 后原请求仍在；未恢复权限时 Retry 再次失败 | 替换外部输入、恢复原权限后 Retry 成功，发布密文与失败时 staging bytes 一致；68 字节 BOM / CRLF 原文精确导出，与初始输入一致且与替换内容不同；仅一条新来源 |
| 重启后精确放弃 | 第二条已提交来源的 Update 因同类权限错误失败；确认关闭丢失内存快照，进程正常退出；恢复权限后同参数重开，无原 Retry 按钮，明确提示快照不可从文件重建；Verify / Rebuild 仍显示 1 pending；放弃 Cancel 保留，Confirm 后成功 | 两份 committed 密文在失败、重启、维护及放弃后逐字节不变；只移除该 pending staging；放弃前后两份 58 字节导出与原已提交版本完全一致，确认期间其它操作禁用 |
| 原删除授权恢复 | 第二条来源正常更新至 v2 后撤销对象目录写权限；删除返回 `DeleteSourceLineage / SourceVaultFailure / Io`，未虚构 Completed；确认关闭再重启，当前选中第一条对照来源，恢复按钮仍绑定第二条来源的原 request ID；恢复权限后点 Continue，得到同 request 的 Completed 十组件回执 | 失败时三份密文不变，两个版本的 deletion intent 已持久化；完成后只剩原对照密文，导出仍等于最初 68 字节；`comet` 无结果。原生观察到执行期间 busy 文案、恢复及其它按钮禁用 |

故障是任务主动撤销专用合成目录 owner 写权限，不是自然断电或 SQLite commit 故障。权限 guard 每次复验固定 profile、目录 inode、mode 和预期数量，收尾恢复原 `0700`；未改父目录、staging 权限、系统 ACL 或凭据。每次重启均保留完整隔离参数，核对 Synthetic acceptance 标识；三轮只在原进程启动后操作 UI，正常退出后不再查询已关闭应用界面。

最后仅经 GUI 删除本批剩余对照来源，得到另一份 Completed 十组件回执；`nebula` 无结果，Verify 为 0 对象、0 pending capture / deletion、无 abandonment。三轮进程退出码均为 0。只读复核两份本批 evidence 各仅一条、20 项持久化组件结果均 succeeded；active references / bodies / fragments / tips / FTS 与对象 / staging 均为空，profile 摘要不变。最小审计、abandoned / deleted 终态、输入、4 份导出、旧备份、日志、临时应用包装和原 key 继续保留，不计作已物理清除。未操作默认资料库、创建或删除 key、安装、启动 VM、push 或写入远程状态。

三轮 stdout 为空，前两轮 stderr 为空，第三轮仅一条 macOS TSM CapsLock 原生诊断；本批合成正文、namespace / device、account 和数据目录标记未命中。这不覆盖 secret 任意编码、系统崩溃报告或 OS 内部副本。部分 egui 文本框的 AX 赋值未改变实际输入，改用原生聚焦 / 键盘输入后复核实际检索词与结果；未将未生效动作记为通过。

本批关闭上述权限故障对应的三条 macOS 原生恢复路径及关闭 Cancel / 明确关闭、放弃 Cancel / Confirm；busy 动作禁用已有可见证据，但 busy 与关闭的竞争时序仍待验。publish 后 SQLite commit 失败、提交成功后刷新失败、evidence 写入前后中断仍仅有既有合成证据；Keychain 取消 / 拒绝 / 锁定、多实例、Windows / Linux 当前代码验收、中文质量及完整 P1-S05 继续待验。没有生产代码、依赖、schema 或默认 v6 入口变化，FTS 仍保存完整明文。

原生收尾后的 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 再次完整通过：255 个仓库文件、notices、fmt、locked Clippy、25 个 suites 共 358 项 Rust tests，11 个 ignored helper 由父测试调用。已知 P1-F17 沙箱端口限制按权限流程使用沙箱外离线检查；最终文档检查和 `git diff --check` 通过。默认构建沿用本日准备阶段证据，未重复 Python 检查器回归、M0 fixture、doctest 或其它平台验收；验收收尾时测试与文档未提交、未推送，没有遗留本批运行进程。随后按项目所有者要求提交为 `347058f`，未推送。

## 2026-10-09 Keychain 与多实例准备

沿当前顺位检查实际会话锁与 macOS provider：桌面每次加密操作使用排他数据库会话，空闲窗口本身不占锁；provider 绑定 User domain 默认 keychain 的唯一 service / account，当前接口不支持另选独立测试 keychain。既有系统 key 连续免提示成功读取，不能把再次成功当成真实取消 / 拒绝。下一原生范围与分类判据见[操作计划](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-09-keychain-与多实例准备)。

新增 `worker::tests::locking::competing_reader_blocks_export_and_hides_stale_view_until_explicit_refresh`，通过独立 reader 的真实排他锁覆盖桌面 Export / Refresh 的失败关闭、无 key 读取、无导出、失败连接不破坏锁、释放后显式刷新与精确 bytes 导出。使用既有合成 provider，没有依赖、生产代码或 schema 变化。任务专用 helper 仅操作固定合成数据库的空 `BEGIN EXCLUSIVE` transaction，最长 120 秒释放；独立临时 SQLite 的子进程预演已验证 SQLITE_BUSY、提前释放、超时释放及数据库 bytes 不变。

准备阶段仅只读复核既有合成 profile、默认二进制摘要、零活动资料 / 对象 / staging，并创建新合成输入与精确操作单；未启动 GUI、访问 Keychain、在该原生库持锁或变更任何访问策略。计划限定先观察目标项访问控制，不为了制造提示而删除唯一 key、修改 ACL 或锁定整个 keychain。真实取消 / 拒绝、多实例 GUI 与其它平台仍待执行。

准备验证：新增桌面定向测试、helper 两条释放预演通过；完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过 256 个仓库文件、notices、fmt、locked Clippy 及 25 个 suites 共 359 项 Rust tests，11 个 ignored helper 由父测试调用。首轮仍只因 P1-F17 合成本地端口被沙箱拒绝而失败，经权限流程在沙箱外完整离线重跑通过。未重复默认 desktop 构建、Python 检查器回归、M0 fixture、doctest 或其它平台运行；本批新增测试与文档未提交，无遗留本批进程。

## 2026-10-09 原生锁竞争与 Keychain 只读核对

项目所有者明确批准上述精确操作单后，复核既有 profile、默认 production binary、输入摘要和唯一既有 key 元数据。A 在空的显式加密合成库导入 v1、Verify 成功；B 以同一完整参数另进程启动，但原生 UI 工具通过 bundle / path 仍绑定 A，完整窗口清单查询超时。按操作单停止条件未继续双 GUI 交替；先经 UI 正常关闭 A、确认退出码 0，再绑定尚未退出的 B。B 打开并精确导出 A 已提交的 v1，只能证明跨进程顺序读取，不能声明两个存活窗口之间的交替刷新或重叠提交通过。v2 更新和跨窗口历史导出未执行。

| 场景 | 实际结果 | 边界 |
| --- | --- | --- |
| GUI 遇锁 | B 保留 v1 视图，外部 helper 对固定合成库取得空 `BEGIN EXCLUSIVE`；Export / Refresh 分别失败于 `ExportSource` / `ListSources`，均保留 `SourceVaultFailure / database=Migration / None / SQLite Some(5)`；busy 按钮禁用，返回错误后旧来源视图隐藏，导出文件不存在 | 真实 SQLite 锁与 production GUI / Keychain；持锁者是专用夹具，不是第二个 GUI 的业务事务 |
| 释放后恢复 | 两项失败均在 release 前核对；helper 收到精确释放文件后 rollback，退出码 0；Inspect 成功但不恢复旧视图，显式 Refresh 恢复同一 v1；前后两个 60 字节 BOM / CRLF 导出完全等于初始合成输入，对象摘要不变、staging 为空 | 原生执行验证提前释放；120 秒超时释放沿用独立临时 SQLite 子进程预演证据 |
| Keychain 只读核对 | 仅目标 generic-password 项的 service / account / label 匹配；Access Control 为“允许访问之前确认”、未允许所有应用、未勾选询问钥匙串密码，常许列表有四个同名验收 app 条目；Save changes 禁用，未显示 secret 或修改任何权限 | 名称不足以唯一识别代码签名 / 路径；没有 RadishMemory 系统授权提示，所以不覆盖实际取消 / 拒绝，更不推断为 locked |

本轮有一次坐标重试误打开 Prepare key 确认；只选择 Cancel，未运行初始化或迁移。检索输入曾受焦点与输入法影响，最终使用实际聚焦和粘贴，确认可见词为 `nebula` 才执行搜索。没有将未生效的动作记作成功。目标 key 的创建 / 修改元数据在收尾时与预检相同，未删除或创建 key、修改 ACL、锁定整个 keychain 或改系统默认项。

最终仅通过 B 的 GUI 删除本轮唯一来源，得到一份 Completed evidence，十项持久化组件结果全为 succeeded；`nebula` 无结果，Verify 为 0 objects、0 pending capture、无 abandonment、0 pending deletion。只读复核 active references / bodies / fragments / tips / FTS、objects / staging 均为空，schema 仍为 11，profile 摘要和对象目录原 `0700` 权限不变。A / B 及 helper 退出码均 0，本轮启动的 Keychain Access 正常退出，进程检查无残留。保留两个输入、两份精确导出、证据、日志、历史审计、旧备份、应用包装和原 key，不将本地回执扩大为这些副本已删除。

A 的 stdout / stderr 和 B 的 stdout 为空；B stderr 仅原生输入法 / TSM 诊断，本批合成正文、profile 身份、account 和目录标记未命中。不覆盖 OS 崩溃报告、任意编码的 secret 或系统内部副本。双 GUI 交替与真实 Keychain 取消 / 拒绝仍待另行构造，本批没有改变生产代码、依赖、schema 或默认 v6 入口，不关闭 P1-S05。

原生收尾后再次完整通过 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh`：256 个仓库文件、notices、fmt、locked Clippy、25 个 suites 共 359 项 Rust tests，11 个 ignored helper 由父测试调用。沿用本会话已确认的 P1-F17 沙箱端口限制，按权限流程在沙箱外离线执行；最终文本 / 链接检查与 `git diff --check` 通过。未另跑默认构建、Python 检查器回归、M0 fixture、doctest 或其它平台验收；本批新增锁回归与验收文档仍未提交、未推送。
