# P1-S05 桌面加密宿主接入

状态：`P1-S05 encrypted desktop integration — native acceptance pending`（2026-10-01）。

项目所有者已授权实现 macOS 宿主接入，并在本任务批准下述依赖连线。恢复前置改动已提交为 `b982cec`；desktop worker、共用 controller、显式加密 / 维护界面和诊断抑制已提交为 `6a413f8`。默认启动保留 SQLite v6 入口，不以合成测试宣称真实系统凭据或 GUI 验收已完成。

2026-10-24 当前补充：macOS 空库自然 busy 关闭拒绝、错误交付保留与结果后退出已通过；Windows / Linux ARM64 各两轮普通用户真实 provider / GUI 正常链路、注销重登、五份精确导出和两条来源删除也已通过，见[跨平台执行记录](../evaluation/phase1-windows-linux-provider-gui-plan.md)。下面各日期记录保留当批证据范围；精确同帧、其余系统错误、pending 原请求与中文质量仍不因本批通过而关闭。

## 实现与操作边界

所有存储、全库认证、迁移和 provider 操作均在一个串行 worker 内执行。UI 只发送显式动作并接收只读快照；系统文件选择器仍在主线程，选择结果作为一次性请求进入 worker，消费后不存入 host profile 或诊断。worker busy 时不接受第二条操作，退出时发送停止信号并 join；线程失败显示 `WorkerStopped`，不能伪造成功或空目录。

`LibraryController` 使用共用的目录、版本和选择逻辑，内部区分旧 `LocalLibrary` 与显式 `EncryptedLibrary`。桌面提供如下动作，初始化和迁移前销毁旧 controller / SQLite 连接；普通打开失败不自动执行准备、迁移、重建或明文 fallback。

| 动作 | 真实行为 |
| --- | --- |
| Open legacy plaintext | 默认旧 v6 入口；遇 v7 至 v12 仍拒绝 |
| Open encrypted objects | 只加载既有 key，在一次只读会话内认证打开、目录 / 版本及恢复状态，首尾复验后交付快照 |
| Prepare key | 确认后调用既有初始化协调器；可能创建系统凭据，已有 checkpoint 缺 key 时不重新生成 |
| Migrate / resume bodies | 确认后使用既有 key 执行 v7 / v8 正文迁移或精确恢复；之后须显式打开 |
| Verify / Rebuild recall | 分别只核对或显式重建派生行；加密打开失败时仍使用同一目录 / profile，拒绝修复损坏 canonical 或自动推进 pending |
| Refresh | 显式加密刷新合并目录 / 版本与恢复状态并首尾复验 key；不重复先前写入，写后刷新仍为原独立路径 |

界面明确显示 FTS 仍保留完整可读正文。初始化、迁移及放弃的确认窗口冻结其它操作，避免用户确认期间当前选择或 inspected target 被 UI 动作替换。各 coordinator 仍在实际执行时复验状态，抵抗其它进程在确认期间改变目录。

## 原请求和恢复

worker 在执行 capture 前持有完整 canonical `SourceCapture`。写入失败保留同一快照和 provenance，阻止新写入、切换库和重新准备；“Retry original request”不重新读外部文件，也不生成新 ID。提交成功立即释放快照，之后目录 / 选择刷新失败单独显示，保留“已提交”的真实结果并引导 Refresh，不提供重复 capture 的错误暗示。一般读取错误隐藏旧 UI 数据，显式 Refresh / reopen 成功后再恢复显示。

原 capture 仅在内存持有，不新增明文磁盘 journal，不承诺在崩溃后保留快照或清零全部进程内副本。关闭时若仍有原请求，提示快照损失与后续限制；busy 时取消关闭请求，等待真实执行结果；2026-10-13 起，关闭帧开始仍 busy 时，即使本帧收到结果也取消这次关闭，保留结果供查看，用户须再次请求关闭。同一帧多次布局保持取消决定，接受关闭后不再派发同帧动作。进程被强制结束不具备该交互保证。重启后已提交 capture 通过目录恢复；未提交 capture 若快照丢失，不允许从 fingerprint 或外部文件伪造精确重试。认证 inspect 只保留不透明 target，明确确认后才调用精确放弃；没有自动清理。未知文件、已提交来源和其它库 target 不在其授权范围。

删除失败保留原 `DeleteRequest`。`unfinished_delete_requests` 在认证 reader 内发现同 namespace 尚无 Completed 最新 evidence 的原授权，包括执行已结束但 evidence 尚未落盘的窗口。它先复验最新 evidence 的身份、时间、计划和真实持久化组件结果，再排除 Completed；不只过滤 status 字段。重启恢复按钮绑定原 request ID 和完整请求，明确继续时不重新规划当前选择的闭包。完成、失败或部分状态按真实 evidence 展示；外部原件、导出、备份和其它设备不属于本地回执。

若请求在 durable mutation 之前因永久冲突失败，worker 仍保留它以免误判提交边界；用户可以保留窗口进行诊断，或确认关闭以丢弃内存副本。只有实际存在并认证通过的 uncommitted capture 才出现精确放弃入口，不能凭 UI 错误状态合成删除授权。

## 有界诊断与日志策略

`DesktopError` 只保留 application operation / code / reason、Source Vault code 以及数据库枚举和数值错误码。key missing、locked、denied、cancelled、authentication 与 database 原因可区分，不保存路径、slot identity、正文、密钥或上游错误链。

创建生产 worker 前安装 process-wide 无输出 `log::Log`，设置等级 Off；即使上游提高等级，这个 logger 仍没有 sink。已有 logger 导致安装失败时不创建 worker、不访问 provider。panic hook 丢弃任意 payload，worker 通过 channel 断开报告线程失败。该策略牺牲进程原始 panic 诊断以保护可能携带私密内容的 payload；logger 本身不新增文件、网络或 telemetry 输出。2026-10-18 的显式受限验收 trace 是独立入口，详见下述补丁记录。

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

## 2026-10-09 实例身份与系统拒绝方案准备

上一批在 `7724ac2` 提交后，继续准备双实例可区分操作及实际系统拒绝构造。新增内容仅为验收文档和忽略目录内的三个临时 app 包装、两份合成输入、只读 metadata 工具及精确操作单；production 代码、依赖、schema、默认启动和系统凭据均未改动。

A、B、Prompt 分别采用不同 bundle ID / 显示名，可执行文件 SHA-256 与原基线一致；签名 inspection 成功，但完整 bundle verification 对原包装及所有副本均报告缺少资源封装。没有把该已存在的包装限制隐藏成通过，也未为其改动产品发布流程。GUI 工具能否按新身份分别绑定，仍需本轮明确授权后的原生验证。

只读工具已使用公开 macOS SDK 编译，并运行无参数说明分支；未触发其固定目标查询。工具仅可核对唯一既有合成项的 ACL 元数据，不能读取 secret 或写回访问控制。公开路径信息不包含完整存储代码身份，无法作为四个同名应用条目的可恢复 ACL 备份；因此不实施去重或基于路径的重建。

拟执行范围为 A/B 跨窗口版本可见性及精确导出，再单独为 Prompt 临时副本更换 ad-hoc 签名身份，尝试触发真实系统拒绝；原 ACL 保持不变、无需依赖不完整的恢复快照。此方案尚未重签、启动 GUI 或访问 Keychain；提示是否出现、实际 OSStatus 和失败后恢复均未验证。完整判断依据、授权范围和停止条件见[验收计划](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-09-可区分实例与拒绝构造准备)。

本轮完整离线 `./scripts/check-repo.sh` 通过：256 个仓库文件、notices、fmt、locked Clippy、25 个 suites 共 359 项 Rust tests，11 个 ignored helper 由父测试调用。因已确认的 P1-F17 合成本地端口限制，经权限流程在沙箱外执行。最终文本 / 链接检查和 `git diff --check` 通过；未另跑 Python 检查器回归、M0 fixture、doctest、默认构建或其它平台。当前三份验收文档未提交，既有两个本地提交未推送；没有本批启动的 GUI 或系统操作残留。

## 2026-10-09 双实例原生验收与 Prompt 许可变化

准备文档提交为 `9e184ec` 后，按项目所有者当前任务授权执行。复用原合成 profile、schema 11、唯一既有系统 key 和默认构建；未修改 production 代码、依赖、schema 或默认入口。A 首次窗口控制服务连接失败，重试后成功；不同 bundle ID 的 A/B 两个进程能分别绑定，A 的模式切换不影响 B，已解除上一批工具定位限制。

| 实际场景 | 结果及限制 |
| --- | --- |
| A/B 交替读写 | A 导入 v1，B 打开 / 显式刷新并导出；A 更新 v2，B 刷新后导出历史 v1 和当前 v2。三份 bytes 分别为 60 / 60 / 54，与合成输入完全一致；B Verify 两个对象、零 pending。未构造重叠 GUI 事务 |
| Prompt 新签名身份 | B 退出后，只重签任务副本；完整 bundle verification 通过，identifier / cdhash / SHA-256 改变，原始包装与 A/B 字节未变。该临时签名不等于正式签名发行验收 |
| 实际系统交互 | Prompt 打开时 busy 且重复动作禁用；工具未能读取系统提示，项目所有者报告选择“始终允许”。最终打开成功，没有拒绝 / 取消 error。助手的按钮提醒晚于触发，未完成预定拒绝测试 |
| 凭据访问控制变化 | 只读输出显示五项常许应用，原为四项；新增路径摘要对应 Prompt bundle。原四项路径摘要及输出的 ACL 数量 / authorizations / flags 未变，但不是完整 ACL 身份备份。新增永久许可保留，未擅自撤销；不能记为访问控制已恢复或保持原样 |
| 对照与收尾 | Prompt 读取前后数据库、profile、两个密文对象摘要一致；退出后 A Refresh / Verify 和第四份 v2 精确导出通过。GUI 删除本轮两版本来源为 Completed、十组件 succeeded；检索为空、Verify 零对象 / 零 pending。三个进程退出码均为 0 |

磁盘最终为零活动 references / bodies / fragments / tips / FTS、零对象 / staging，schema 11、profile 摘要及目录权限保持原值；删除证据与十项持久化组件结果按本轮 request 复核。保留两份输入、四份导出、三个包装、日志、原 key、历史审计 / 旧备份及新增 Prompt 常许条目。凭据变化由本轮手动“始终允许”产生，不归入已完成的删除回执；撤销这一许可须另行精确授权。

A stderr 含一条原生 TSM 诊断，其余本批 stdout / stderr 为空；已核对的合成正文 / 身份 / account / 数据目录标记未命中，不扩大到系统崩溃报告或任意编码 secret。文件对话框首轮路径快捷操作未到目标，重新读取实际路径框后选中精确输入；检索框 AX 点击未聚焦，坐标聚焦后复核可见 `nebula` 再执行搜索。上述工具重试未改动额外资料。

双窗口交替与精确导出已通过；实际系统拒绝 / 取消、busy 关闭竞争、提交 / evidence 中断及集中跨平台检查仍未收口。新增 Prompt 常许许可的保留 / 撤销范围须先明确，后续系统提示应在触发前完成具体按钮交接，不能把允许成功当作拒绝覆盖。

## 2026-10-09 Prompt 许可精确撤销与拒绝重验

项目所有者另行明确批准仅撤销新增 Prompt 项并重验。Keychain Access 中按唯一 service / account / label 复核目标，访问控制列表只选择并移除 `RadishMemory Acceptance Prompt.app`，保留原四项及原 key。首次保存认证被用户拒绝，只读核对仍为五项，没有记作撤销成功；明确区分“允许保存”与后续“拒绝读取”并取得准备确认后，第二次保存完成。

保存后列表为四项，新增 Prompt 路径摘要消失；按 authorizations 集合匹配五组 ACL，其 prompt flags、应用列表类型 / 数量及路径摘要与原基线一致。API 返回的两个 ACL 组顺序交换，不能直接把原始输出逐字节相同作为判据。仍不声明存储代码身份或完整 ACL 二进制快照完全相同；没有重建原四项、删除 key 或修改其它项。Keychain Access 正常退出。

再次启动同一已重签 Prompt，先停在未打开界面；向用户明确交接只点“拒绝”，收到已准备好后才触发 Open encrypted objects。busy 期间重复动作禁用；随后界面实际显示 `OpenLibrary / SourceVault / SourceVaultFailure · vault=KeyStoreCancelled`、`Local library unavailable` 和新写入禁用，没有回退打开。该观察验证真实系统交互返回取消类错误后的失败关闭，不把它改称为 `KeyStoreDenied` 或 `KeyStoreLocked`。界面未直显原始 OSStatus，不将代码映射表当作独立运行态采集。

本次重验使用已经清理为空的既有合成库，因此只覆盖显式打开的系统 key 失败，不覆盖已有正文视图撤回或 capture / export 中途的系统失败。拒绝后数据库 / profile bytes 不变，objects / staging 均为零；ACL metadata 与撤销后输出一致，未重新添加 Prompt。Prompt 正常退出 0；原 A 对照免提示打开同一加密库、Verify 零对象 / 零 pending 后正常退出 0，数据库 / profile 仍未变。进程复核无本轮 GUI 或 Keychain Access 残留，两轮后续 stdout / stderr 均为空。

最终新增 Prompt 永久许可已撤销、原 key 与四项保留。两份输入、四份导出、三个临时包装、历史审计 / 旧备份和证据继续保留；上一节“新增许可保留”是首次子批次的中间状态，最终以本节为准。双 GUI 交替与本次真实 `KeyStoreCancelled` 打开失败已覆盖，其它系统错误分支、其它操作阶段、busy 关闭竞争、提交 / evidence 中断及集中跨平台检查仍待后续范围。

本轮完整离线 `./scripts/check-repo.sh` 在首轮原生收尾后通过：256 个仓库文件、notices、fmt、locked Clippy、25 个 suites 共 359 项 Rust tests，11 个 ignored helper 由父测试调用；已确认的 P1-F17 端口限制按权限流程处理。随后修正仅涉及已授权原生操作及记录更新，未改动代码或依赖；最终再次运行文本 / 链接检查和 `git diff --check`，不重复未受影响的 Rust 全量测试。未另跑 Python 检查器回归、M0 fixture、doctest、默认构建或其它平台。本轮五份验收记录按当前任务的提交要求单独提交，未执行 push。


## 2026-10-13 busy 关闭竞争修复与剩余窗口核对

本批在干净的 `dev` 工作区开始，领先本地 `origin/dev` 一个文档提交，未刷新远程。项目所有者授权核对、必要修复、仓库内验证和文档更新；GUI、系统凭据和 VM 留待具体范围另行确认。

复现并修复两处 UI 时序问题：原实现先 `poll` 再检查 busy，完成结果与关闭事件在同帧到达时可直接关闭，使成功 / 刷新失败或 worker 停止结果尚未展示就退出；原实现接受空闲关闭后仍绘制并处理按钮释放，可在该帧额外派发 Refresh 等命令。现在以本次渲染开始时的 busy 和帧号保留关闭取消决定；同帧多个布局 pass 不覆盖该决定，真正完成的消息不被等待提示替换。取消不会延期自动关闭；用户再请求关闭时，仍持有原请求才进入既有明确确认。接受关闭后立即停止 UI 动作处理。

为直接测试 production 界面，将 `eframe::App::ui` 的原界面体保留在私有 `render` 方法并由原入口调用。新增六项 headless egui 回归，控制测试 channel 的结果交付，覆盖持续 busy、提交成功与刷新错误同时交付、失败原请求及实际 Cancel / Confirm 点击、worker channel 断开、空闲关闭与按钮释放同帧、同一帧多次布局。测试在取得输出时清空无需渲染的纹理增量，避免失败断言触发纹理析构二次 panic；不建立原生窗口。另加真实 worker 的 Drop 回归：初始化测试门限有超时，已接受的 OpenPlain 完成后 engine 释放，Drop 返回前线程已 join，临时 SQLite 可重开；不以强制终止模拟关机。

回归对照临时恢复原关闭逻辑：六项 UI 测试中五项失败，持续 busy 取消一项通过；随即恢复修复代码。初次对照暴露了测试输出纹理未在断言前清理导致的析构 abort，修正测试资源处理后取得上述正常失败结果。worker 新测试初次将数据库文件名写错，修正为现有 `library.sqlite3` 后通过，未改变产品路径。修复后的 desktop 测试 47 项通过、1 项日志 helper ignored 并由父测试调用。

提交 / evidence 中断和成功写入后刷新失败的既有 worker 回归已核对，未发现本批需要改变恢复语义的缺口；不重复建立存储测试入口。逐窗口的触发、持久状态、UI 判据、原生限制与有限跨平台检查点见[验收矩阵](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)。合成回调错误不等于真实 commit 失败、进程中断或断电。

本批 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：259 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，25 个 suites 共 366 项 Rust tests；11 个 ignored helper 由父测试调用。首轮仅在既有 P1-F17 合成本地观察端口被沙箱拒绝，按权限流程在沙箱外离线重跑同一完整检查通过，未跳过门禁。默认 `cargo build -p radishmemory-desktop --locked` 离线构建通过。补记结果后的文本 / 链接检查与 `git diff --check` 亦通过；本轮未单独重跑未受影响的 Python 检查器回归、M0 fixture runner 或 compile-fail doctest。未启动 GUI、调用系统 key store、改变保留合成库 / 测试 slot、运行 VM 或远程动作。无新依赖、lockfile、canonical schema、加密 profile、删除契约、默认 v6 入口或日志策略变化；默认构建无新增故障入口。本批只收口关闭竞争的实现和合成证据，原生窗口行为、其它系统错误 / 操作阶段与 Windows / Linux 当前代码验收仍待完成，P1-S05 不整体关闭。

实现切片结束时变更尚未暂存、提交或推送，保留任务前领先本地 tracking ref 的一个提交；检查和构建均正常退出，没有本轮后台服务。测试自身临时库按 fixture 清理，核验日志保留在任务专用临时文件中。


## 2026-10-13 原生首轮的重复密钥提示与收尾

关闭竞争修复已提交 `f37078d`。项目所有者随后确认精确操作单，批准新构建 GUI、唯一既有测试 key 读取、临时 SQLite 持锁和指定对象目录权限变更。预检核对默认构建与独立包装摘要、profile、零活动资料 / pending、`0700` 权限及精确 key metadata；未请求 secret 的系统 metadata 预检通过。

第一轮启动后停在预期的 v11 legacy-open 拒绝界面；触发前交接只允许本次访问、不点“始终允许”，收到用户已准备好后只点击一次 Open encrypted objects。用户报告“允许之后又弹出来”，反复提示十几次。工具没有获得系统提示逐次计数；稍后用户报告当前无提示，GUI 实际显示加密打开成功、`0 active lineage(s)`。本次不是取消 / 拒绝通过，也不能把“单次允许”描述为整个桌面动作只弹一次。

静态调用链确认：空库 OpenEncrypted 依次进行 `EncryptedLibrary::open`、controller 的 `list_sources`、`inspect_capture_abandonment` 和 `unfinished_delete_requests`，各自加载现有 key；`provider::load_existing` 每次执行两次 `read_secret` 和 label 复验。四次加载对应八次 secret 读取调用，旨在维持独立认证用例和 key 替换检查；这是静态正常路径，不等于现场提示数量。原生体验暴露出桌面动作拆成多次认证会话的交互成本，不能归因于用户误操作或助手多次点击。操作单低估了该成本。

在导入前停止继续触发认证，正常关闭唯一窗口，进程退出码为 0。数据库与 profile 的 SHA-256 与读取前一致；五项活动表计数、对象 / staging、pending capture / deletion 均零，对象目录 inode 和 `0700` 权限保持。stdout / stderr 均为空。没有启动持锁 helper、撤销目录权限、创建 / 删除 key、导入 / 导出、放弃或删除来源，因此没有本批删除回执，不虚构 Completed。保留原 key、历史产物、新包装、输入、预检和诊断证据；本轮未操作 ACL，不以此声明已完成全部 ACL 身份审计。

busy 关闭竞争及原请求 Cancel / Confirm 的本批原生验收尚未执行。继续加密原生流程前，先评审桌面动作级认证会话、交互次数和取消 / key 变化时的失败关闭契约；不得简单删除二次复验、暗中跨动作缓存 key 或要求永久许可。密钥生命周期与安全边界变化需要明确范围后再实现；当前生产代码、依赖、schema 和信任模式未改变。

记录更新后，`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过，退出码 0：259 个仓库文件，25 个 suites 共 366 项 Rust tests，11 个 ignored subprocess helper 由父测试调用。沿用此前已确认的合成本地端口所需权限，在沙箱外离线运行，未跳过检查。补记后的文本 / 链接检查和 `git diff --check` 通过。新增三份文档记录未暂存、未提交；`dev` 领先本地 `origin/dev` 两个提交，未 push。本批 GUI 和检查进程均已退出，未启动持锁 helper；合成验收产物与日志保留在任务专用忽略目录及临时目录。

## 2026-10-13 只读动作认证会话实现与合成验证

此前三份原生验收记录已按项目所有者要求提交为 `4ecd775`。随后完成认证边界评审，项目所有者明确批准仅实现 `OpenEncrypted` 和显式 `Refresh` 的组合只读会话，并禁止本轮 GUI / 真实系统密钥库访问。[完整契约](phase1-source-vault-application.md#2026-10-13-只读动作认证会话)冻结首尾 double-read、同 store 身份观察、最终对象 / 数据库复验及无跨动作缓存。

实现复用现有 reader 和查询，在 Source Vault 内构造固定目录 / 版本 / 恢复快照；application 提供 `open_with_snapshot` 与 `read_snapshot`，desktop 仅将两个批准的 command 接入。成功后才安装完整 view 和恢复状态，错误路径撤回旧恢复选择并隐藏旧 view，仍保留原 capture / DeleteRequest；pinned profile 检查先失败时也撤回旧 target，不保留看似可操作的恢复状态。原生提示数量尚未测量，不能把代码级四次 secret 读取写成四次或一次弹窗。

新增 provider 合成测试使用现有 fake store，逐次验证两轮 double-read 的四次 secret / label 校验、每个 secret 读取位置的取消 / 拒绝 / 锁定 / 缺失 / ambiguity / metadata / unavailable、同轮与首尾 key 替换、无写入与下一动作重新认证。新增 reader 测试覆盖版本选择、精确 pending target、跨进程持锁、末次 key 读取期间对象损坏 / 消失与 Unix 数据库替换、profile 先于 key、unwind 后释放。desktop 合成测试覆盖空库 / 有资料库两轮加载、旧视图撤回、首尾失败分类、空库 key 替换、原 capture 与删除授权保留、pinned profile 失败时恢复状态失效，以及其它读用例未共用认证。

本批未改依赖、lockfile、canonical schema、加密 profile、bootstrap / 普通 load 双读算法、写入 / 删除 / 导出事务、默认 v6 入口、日志策略或 busy 关闭规则。Windows / Linux 当前代码和真实宿主验收仍待完成；本地合成结果不关闭 P1-S05，也不解除 PDF / 图片停止线。原 key、保留合成库及历史原生验收产物未触及。

验证完成：`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0，263 个仓库文件、notices、fmt、locked all-targets / all-features Clippy 和 25 个 suites 共 379 项 Rust tests 通过；新增 13 项回归，11 个 ignored subprocess helper 仍由父测试执行。完整检查沿用既有合成本地端口所需权限，在沙箱外离线执行，未跳过门禁。此前三个相关 package 的定向测试、7 项 snapshot 和首批 4 项 desktop read-session 定向测试亦通过；后补的两项 desktop 回归包含在完整检查中。`cargo build -p radishmemory-desktop --locked --offline` 默认构建退出 0；默认 normal / build / features 图不含 `acceptance-test-support`。补记后的仓库文本 / 链接检查及 `git diff --check` 通过。本轮未单独重跑未受影响的 Python 检查器回归、M0 fixture runner 或 compile-fail doctest，未运行 Windows / Linux 当前目标。

本批实现、测试及七份相关文档未暂存、未提交、未推送；`dev` 保留领先本地 `origin/dev` 的三个既有提交。测试 / 构建进程已正常退出，无 GUI 或长期后台进程；合成 fixture 按自身清理逻辑收尾，验证日志保留在任务专用临时文件中。未访问真实系统 key store、启动 VM、改变既有权限 / ACL 或执行远程动作。

## 2026-10-13 新构建原生入口取消与提示预算停止

只读动作认证会话经复核后按项目所有者要求提交为 `89d53b0`。提交后的默认 locked 离线构建通过，独立包装与默认二进制逐字节一致，生成匹配 UUID 的本地 dSYM，未额外签名或安装。项目所有者随后明确批准新操作单的 GUI、唯一既有合成 key、限定调用栈采样、SQLite 持锁及精确目录权限操作；每次系统认证前交接，阶段不明或提示超限停止新增场景。

预检确认提交、包装 / 输入摘要、原合成 profile、空库与目录身份 / 权限均匹配；只读解析 User domain 默认 keychain，再核对精确 service / account 的 metadata，没有使用 secret 输出选项。首轮仅启动本批一个 Synthetic acceptance 窗口，启动时 legacy v11 打开拒绝符合预期。

- A01：交接后仅点击一次 `Open encrypted objects`，用户让首个目标提示停留。对核对后的唯一 GUI PID 执行一次 1 秒、20ms 间隔的只读 sample；worker 的 39 个采样命中同一链。使用同 UUID 的 dSYM 离线符号化，正向确认 `snapshot.rs:43 → reader.rs:44 → provider.rs:546 → macos.rs:102 → SecKeychainFindGenericPassword`，因此首轮入口阶段已确认。用户确认首个提示点拒绝并消失；GUI 返回 `KeyStoreCancelled`、资料视图不可用、恢复状态未验证，没有自动重试。数据库 / profile 摘要与基线一致。
- A02：再次交接，仅点击一次空库加密打开。用户报告“一共九次，我都点了允许”；GUI 最终显示成功、`0 active lineage(s)`。实际报告 9 次超过操作单的 8 次上限；收到报告时提示已全部处理，不能写成第 9 次已拒绝。计数台账按用户汇总记录，单提示阶段与耗时未逐一观察；A02 没有采样，不能将额外提示确定归因于 metadata 或某个 secret 调用，也不能据此证明末轮阶段。四次 secret API 调用仍不能等同于四次系统提示。

按超限停止线，不执行 A03 及以后场景，也不进入 B/C。空库 Refresh、有资料 OpenEncrypted / Refresh、末轮取消、busy 关闭竞争和原请求 Cancel / Confirm 本轮均未覆盖。A01 的真实入口取消结果不扩张为其它阶段通过；本批仍未满足完整原生验收。

正常关闭窗口，GUI 及 sample 均退出 0，GUI stdout / stderr 为空。最终数据库 / profile 的 SHA-256 与预检及 A01 后一致；五项活动表、objects / staging、pending capture / deletion 均零，对象目录身份与 `0700` 权限保持。未导入 / 导出、启动持锁 helper、改变目录权限、放弃或删除来源，也未创建 / 替换 / 删除 key 或修改 ACL；因此没有本批删除回执，不虚构 Completed 或完整回滚。保留原 key、数据库历史、新旧包装、输入、dSYM、原始栈、符号化结果、台账与日志，仅正式摘要进入文档，不提交原始运行产物。

后续先诊断一次已合并只读动作内的真实提示来源，重新制定可接受的交互预算与分阶段观测方案；不自行提高上限、重复触发、要求永久许可或削弱首尾复验。任何新原生操作仍需具体范围与交接，当前实现与安全契约未改变。

记录更新后的 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整退出 0：263 个文件、notices、fmt、locked Clippy 与 25 个 suites 共 379 项 Rust tests 通过，11 个 ignored helper 由父测试调用。按已知合成本地端口需求在沙箱外离线运行，未跳过检查；补记后文本 / 链接和 `git diff --check` 再次通过。本轮未重跑未受影响的默认构建、Python 检查器回归、M0 fixture runner、compile-fail doctest 或其它平台。三份新增记录未暂存、未提交；`dev` 领先本地 `origin/dev` 四个提交，未 push。GUI、采样和检查进程均已退出，无本批后台进程。

## 2026-10-13 单次空库打开的八提示逐次定位

三份诊断文档已按项目所有者要求复核并提交为 `dffb40d`。随后获得新操作单的精确授权，仅启动指定默认构建、读取唯一既有测试 key，并对本批 GUI PID 进行限定调用栈采样；不承接此前 A/B/C 的写入、锁和权限场景。默认运行实现仍为 `89d53b0`；包装摘要、二进制 / dSYM UUID、数据库 / profile、零活动状态及目录身份均通过只读预检，没有运行额外 security metadata 或 ACL 探针。

本次只启动一个 Synthetic acceptance 窗口、点击一次 `Open encrypted objects`。用户在每个系统提示出现后保持未选择；核对目标和类型后，对同一个已核对 PID 各做一次 1 秒、20ms 间隔 sample，使用匹配 dSYM 离线 atos。每次取得正向调用点后，才分别交接用户仅允许当前一次并停住下一个提示。实际共八个提示、八次用户确认的 Allow Once、八次正常退出的 sample；第八次后用户报告无新提示，GUI 显示加密打开成功、`0 active lineage(s)`。没有第九个提示，因此没有执行拒绝，不记作本轮取消用例通过。

| 提示序号 | 已确认的读取阶段 | 正向源码调用点 | 系统 API |
| --- | --- | --- | --- |
| 1、2 | 入口第一读 | `snapshot.rs:43 → reader.rs:44 → provider.rs:546 → macos.rs:102` | `SecKeychainFindGenericPassword` |
| 3、4 | 入口第二读 | `snapshot.rs:43 → reader.rs:44 → provider.rs:549 → macos.rs:102` | `SecKeychainFindGenericPassword` |
| 5、6 | 交付前末轮第一读 | `snapshot.rs:60 → provider.rs:546 → macos.rs:102` | `SecKeychainFindGenericPassword` |
| 7、8 | 交付前末轮第二读 | `snapshot.rs:60 → provider.rs:549 → macos.rs:102` | `SecKeychainFindGenericPassword` |

第一个栈有 45 个 worker 采样，其余各 44 个；每对的 Security framework 栈相同，均经过 item data / content 获取和 `SecurityServer::ClientSession::decrypt` 等待。末轮定位来自 `snapshot.rs:60` 的正向符号证据，完整 worker 链不经过 `reader::open`，不是按提示序号猜测。用户逐次确认目标 / 类型相同，未提供口令或 secret；原始栈、符号化输出、八份逐提示记录和台账仅本地保留。

这次运行把八个可见提示全部定位在四个 legacy secret 静态调用位置，每处观察到两个提示；没有可见提示的采样落在 `attributes` / `SecItemCopyMatching`。因此本次重复提示的调查优先级转向 legacy secret 获取路径内部的系统交互，不支持先删除 metadata 校验。采样没有 API begin/end 实例标识，也没有 securityd 内部授权原因；不能把相同静态调用位置直接表述为已证明的系统调用实例 / 两项 ACL 原因。上一批 A02 用户汇总的九次没有在本次逐次观测中重现，保留原记录，不把额外一次归为误报、解锁或 metadata。新证据不保证其它状态、动作或系统版本也固定八次。

GUI 正常关闭、退出码 0，stdout / stderr 均为空。最终只读核对：数据库与 profile 摘要完全等于本轮基线；五项活动表、objects / staging、pending capture / deletion 均零，目录身份及 `0700` 权限保持。未导入 / 导出、持锁、chmod、初始化 / 替换 / 删除 key、修改 ACL、放弃或删除来源，无删除回执、无清理数据或回写数据库。原 key、历史资料、原包装 / dSYM 与全部本批证据保留；无强杀、安装、系统设置或远程动作，GUI 和采样进程已全部退出。

本次完成的是单动作正常打开的逐提示定位。末轮阶段已观察到，但末轮取消、有资料视图撤回、显式 Refresh、busy 关闭与 pending 原请求关闭均未在本单执行，P1-S05 不整体关闭。下一步针对 legacy secret API 路径评审最小候选改动及可测指标，沿用[修复提案](phase1-source-vault-key-provider.md#2026-10-13-合并会话后九次提示的静态诊断与修复提案)的完整安全约束；本次授权不扩张为替换 API、删减复验或改 key / ACL。

本轮记录更新后，`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：263 个文件，notices、fmt、locked Clippy 和 25 个 suites 共 379 项 Rust tests 通过，11 个 ignored helper 由父测试执行；沿用已知合成本地端口权限在沙箱外离线运行。补记后文本 / 链接检查及 `git diff --check` 通过。本轮未重跑未受影响的默认构建、Python 检查器回归、M0 fixture runner、compile-fail doctest 或其它平台。四份本轮观测记录未暂存、未提交；`dev` 领先本地 `origin/dev` 六个提交，未 push。29 份本地证据文件已建立 SHA-256 索引，GUI、全部 sample 与检查进程均已退出。


## 2026-10-13 SecItem secret 新构建逐提示观测

七文件最小读取替换已复核并提交 `bf8dddf`。项目所有者随后明确批准新构建单次空库打开的操作单：唯一指定 GUI、既有测试 slot、逐提示人工交接及限定调用栈采样，不扩展其它验收。新默认构建、冻结包装与 dSYM UUID / 摘要及完整合成库基线匹配后，仅启动一个窗口、点击一次 OpenEncrypted。没有额外 security metadata / ACL 探针或 key-store 预检。

共观察到八个系统提示，每个均由用户确认应用 / 目标匹配并保持未选择；对同一已核对 GUI PID 各执行一次 1 秒、20ms 间隔 sample，离线 atos 确认调用点后才逐次交接 Allow Once。八个 sample 正常退出，首个 worker 栈 35 个样本，后续分别 45、44、44、44、45、45、45 个；不使用旧 dSYM 或根据提示序号推断阶段。

| 提示 | 阶段 | 正向源码调用点 | 系统 API |
| --- | --- | --- | --- |
| 1、2 | 入口第一读 | `snapshot.rs:43 → provider.rs:546 → read_checked_secret:169 → macos.rs:117` | `SecItemCopyMatching` |
| 3、4 | 入口第二读 | `snapshot.rs:43 → provider.rs:549 → read_checked_secret:169 → macos.rs:117` | `SecItemCopyMatching` |
| 5、6 | 交付前末轮第一读 | `snapshot.rs:60 → provider.rs:546 → read_checked_secret:169 → macos.rs:117` | `SecItemCopyMatching` |
| 7、8 | 交付前末轮第二读 | `snapshot.rs:60 → provider.rs:549 → read_checked_secret:169 → macos.rs:117` | `SecItemCopyMatching` |

所有提示都正向定位新 data search，未落在 metadata 的 `macos.rs:60`。栈下游经过 `SecItemCopyMatching_osx → AddItemResults → SecKeychainItemCopyContent → SecurityServer::ClientSession::decrypt` 等待；这是新 API 的系统内部调用路径，不是应用回退旧 API。四个静态 secret 位置仍各出现两个提示，本次总数与旧构建的八次相同，因此更换 API **未在本次观测中减少提示**。新包装 / 二进制身份与旧轮不同，此对照不证明所有环境的 API 因果；未取得 OS 调用 begin/end 实例或系统内部授权原因，不能宣称确定两项 ACL 原因。更早九次汇总的额外一次仍未重现，旧记录保留。

第八次允许后用户确认无新提示；GUI 显示加密打开成功、零活动来源。没有第九次，所以没有执行拒绝，也不记作首尾取消通过。正常关闭窗口，GUI 退出 0，stdout / stderr 均为空。退出后只读快照逐项等于预检：数据库 / profile SHA-256、五项活动表、pending capture / deletion、objects / staging、目录身份、0700 权限与资料根文件清单均保持；无业务写入、持锁 helper、权限 / ACL / key 修改、强杀或远程动作，故无删除回执或需要虚构的恢复步骤。

34 份本地证据文件已建立 SHA-256 索引，八行逐提示台账、操作单实际结果、新旧包装 / dSYM、原 key 和数据库历史全部保留；原始栈和实际本机路径不进入 Git。GUI 与全部 sample 已退出。本轮仅完成新 API 的单次空库正常打开与提示定位；Refresh、有资料、首尾取消、busy / pending 关闭均未执行，P1-S05 继续开放。下一步先复核本次未改善结果和后续有限诊断范围，不自行增加提示预算、删减复验或采用永久许可。


本轮记录更新后的 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整退出 0：264 个文件、notices、fmt、locked Clippy 和 25 个 suites 共 386 项 Rust tests 通过，11 个 ignored helper 由父测试执行；按已知合成端口权限在沙箱外离线运行，未跳过检查。补记后文本 / 链接检查和 `git diff --check` 通过。未重复未受影响的默认构建（观测前已通过）、其它平台构建或原生验收。四份本轮记录未暂存、未提交；`dev` 领先本地 `origin/dev` 八个既有提交，未 push。所有本轮 GUI、sample 和检查进程已退出。

上述四份记录随后复核并提交为 `8393cc0`；当日完整提交回顾、相关文档修正与下一步建议见[10 月 13 日日终记录](../status/2026-10-13-source-vault.md)。此前未提交状态为该观测收尾时的快照。


## 2026-10-18 单动作受限观测入口

完整三项隔离验收参数之外，仅 macOS 可显式提供 `--acceptance-key-read-trace` 和一个库外的新绝对路径；解析验证现有 profile / 数据库、私有父目录及路径隔离，不创建 trace。诊断模式的自动 OpenPlain 只显示固定就绪提示、不触发业务读写；仅一次 OpenEncrypted 可进入观测作用域，成功或失败后都拒绝其它业务动作和重试。正常关闭仍经过既有 worker join 生命周期。

Action end 写出前不发布快照，观测错误撤回目录、正文及恢复选择；没有原请求或第二次读取可供重试。普通默认入口与原三参数验收不受单动作限制，`logging.rs` 保持原摘要。真实文件故障的 worker 回归用严格 `cfg(test)` 私有钩子破坏自己的合成 trace，生产构建没有该钩子，也没有 provider override。具体允许文件、失败关闭策略与独立验证见[受限补丁记录](../evaluation/phase1-keychain-prompt-diagnostic-plan.md#受限观测补丁实现与独立审阅)；补丁实现阶段未执行 GUI / 真实 Keychain，不能把该阶段合成结果作为提示或原生关闭验收证据。

后续独立授权的 [D01](../evaluation/phase1-keychain-prompt-diagnostic-plan.md#2026-10-18-d01-单次原生结果与交接缺口)取得完整 4 / 12 / 25 预算，但八提示缺少逐提示交接；[D02](../evaluation/phase1-keychain-prompt-diagnostic-plan.md#2026-10-18-d02-首对同区间与拒绝收尾)仅观察入口第一读，确认同一 SecretSearch 区间的两次提示，第二提示拒绝后真实 KeyStoreCancelled，GUI 正常退出、空库保持。两批均未覆盖末轮取消、有资料 Refresh 或 busy / pending 关闭。随后 M01 在独立 CLI 进行，未修改宿主；副本范围冻结不构成原 app 授权证据。当天提交回顾与后续窗口见[日终记录](../status/2026-10-18-source-vault.md)。

## 2026-10-24 空库锁等待下的原生关闭验收

准备文档已按项目所有者要求提交为 `b13c3b6`，随后按本次精确授权执行 N 批。运行的是 `5365e27` 默认 desktop 构建的独立冻结包装，提交仅涉及文档，161 份源码与构建输入保持；全量 169 份冻结输入在前后核对一致。固定目标为历史 v11 空合成库，沿用唯一既有 profile，不进行系统密钥查询或允许提示。首次进程核对受沙箱 `ps` 限制，按权限流程只读复核通过后才启动 GUI；未跳过占用检查。

| 场景 | 实际执行与结果 | 证据边界 |
| --- | --- | --- |
| N00 空闲关闭 | 第一次 GUI 自动旧入口打开拒绝，显示 StorageFailure 与资料库不可用；不点击业务按钮，正常关闭，原进程退出 0 | 关闭后空库只读状态保持，不以窗口消失替代进程退出 |
| N01 busy 关闭 | 第二次 GUI 就绪后，第一轮持锁在点击打开前遇到 UI 工具进程检查调用不可用；未触发业务动作，主动释放并核对 helper 退出 0、基线保持，记未覆盖。第二轮实际点击一次 OpenEncrypted，观察 Working、busy 指示与业务按钮禁用；请求一次关闭后窗口保留，明确提示等待真实结果。随后显示 `database=Migration / None / SQLite Some(5)`，窗口未自动退出、无资料视图或已验证恢复目标 | 第一轮不计通过。第二轮为自然可见 busy，未注入精确同帧事件；存在 Inspect recovery 入口不代表已交付恢复授权，未点击该入口 |
| N02 结果后关闭 | 锁错误交付后主动释放第二轮锁；helper 约 19.7 秒完成 rollback / close，退出 0。再请求窗口关闭，第二个 GUI 正常退出 0 | 第三轮未使用；没有在释放后执行 Open / Refresh / Verify 来扩大访问范围 |

两轮锁均因显式 release 结束，未等到 deadline、未强杀；两个 GUI 及两个 helper 均有真实退出 0 / 回收记录，最终只读进程检查确认已不存在，指定数据库无打开句柄。GUI stdout / stderr 和 helper stderr 均为空。前后数据库 / profile 摘要、文件身份 / 权限、v11、五项零活动表 / pending / deleting、空 objects / staging 与根目录清单保持。没有资料写入、导入、删除、迁移、权限 / ACL 修改、重签、安装、VM 或远程动作。

保存五份窗口截图、六份 AX 状态、锁 ready / release / released、进程回执、前后状态及摘要索引，全部留本机忽略目录。未出现系统密钥提示，未执行任何允许动作或额外 Keychain 探针；本批未启用系统调用观测，不仅凭“无提示”推断全部 OS 调用计数。M01 v2 保持暂缓，新 native 输出未创建，旧冻结件和唯一 key 保留。

本批关闭自然 busy / 锁错误交付 / 正常退出的有限缺口，不宣称精确同帧竞争、末轮 key 取消、有资料旧视图撤回、pending 原请求 Cancel / Confirm、其它系统错误或 Windows / Linux 原生通过。P1-S05 及 PDF / 图片停止线保持；剩余范围按[当前状态](../status/current.md#后续事项)推进。

原生记录更新后的完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：271 文件仓库基线、fmt、locked Clippy 与 25 suites 共 398 项 Rust tests 通过，11 个 ignored 子进程 helper 由父测试调用。准备阶段的默认 desktop 构建与 17 项持锁 helper 合成回归已通过，原生执行未改代码，不重复构建或扩充测试。补记后再次完成仓库文本 / 链接检查和 `git diff --check`；未运行其它平台、额外 Keychain 诊断或远程检查，未 push。
