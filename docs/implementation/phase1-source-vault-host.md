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

真实 Keychain 读写、用户授权提示、取消 / 锁定、实际 GUI 关闭确认、多实例和端到端验收未执行；Windows / Linux 保持后置集中检查点。下一批需先列出隔离测试目录、合成 namespace / device 对应的精确 slot、运行命令、预计时长、系统提示及清理 / 保留方式，再取得当前任务授权。删除测试 slot 前必须确认对应合成对象库不再需要，不能将删除 key 当作普通回滚。

默认启动仍是 SQLite v6 inline plaintext body；显式原始对象加密不等于整个资料库静态加密，FTS 保留完整正文，旧 SQLite 页、快照和备份也未证明物理清除。中文检索、目录第 201 条、性能和默认 v6 的 failed-open 修复等质量缺口不在本批闭环范围，不授权真实个人资料使用。

原生验收准备的静态缺口：当前 `main` 不解析测试目录参数，生产 `Worker::start` 直接调用 `ApplicationPaths::resolve`；`Engine` 测试虽能注入独立目录，该能力没有暴露为原生启动入口。因此明天先确定安全的独立测试根与 profile / slot 选择方式，再请求真实运行授权，不直接启动默认 GUI 访问日常资料库，也不通过替换用户 HOME 等宽泛全局设置制造隔离。此项只记录为待办，今天没有改动启动代码。
