# Keychain 重复提示有限诊断与原生关闭验收方案

更新：2026-10-24。状态：D01 / D02 已收尾；D02 证明首对位于同一 SecretSearch 调用区间，第二提示拒绝后真实 KeyStoreCancelled 失败关闭。M01 首次执行在静态身份前置阶段失败，目标查询计数全部为零；离线定位后，副本范围 v2 已实现、合成验证并重新冻结，新 native helper 尚未执行。10 月 24 日重新执行普通文件门禁失败，定位为后续平台条件编译提交导致旧清单中的一份生产源码摘要漂移；v2 产物与历史回执保持，原生执行暂缓，详见[当前后续事项](../status/current.md#后续事项)。副本结果不能代表原 app 授权，未来须先审阅证据绑定，再另行精确授权。实际范围与限制见后文各批记录。

初始计划批次由项目所有者授权复核既有证据、完成有限诊断方案和原生验收操作单；当时明确禁止启动 GUI 或访问真实 Keychain。本文承接 [provider 记录](../implementation/phase1-source-vault-key-provider.md#2026-10-13-secitem-候选的单次原生结果)与[原生验收矩阵](phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)，不替代密钥读取契约，也不构成未来系统操作授权。

本页按批次保留历史状态；“尚未实现 / 执行”“未提交”和提交领先数均以所在批次为准。M01 相关记录已于日终先提交为 `49e2f59`，当天完整提交回顾、文档复核及后续安排见[10 月 18 日日终记录](../status/2026-10-18-source-vault.md)。当前停止线和下一步只以[当前状态](../status/current.md#后续事项)为准，不沿用已收尾批次的授权。

## 复核结果与证据强度

本地基线为 `6159a42`，`dev` 工作区起始干净，与本地 tracking ref 无 ahead / behind；未刷新远程。`bf8dddf` 至基线的 apps、crates、Cargo manifest / lockfile 与工具链没有差异。

| 证据 | 本次实际复核 | 可以支持与不能支持的结论 |
| --- | --- | --- |
| 旧 API 八提示 | 29 份索引文件的长度及 SHA-256 全部匹配；冻结二进制摘要匹配；八份符号文件逐一核对入口 / 末轮及第一 / 第二读，并与原栈的 legacy API / decrypt 等待核对 | 四个静态 secret 位置各有两个提示；没有调用 begin/end，不能证明每对属于同一调用实例 |
| SecItem 八提示 | 34 份索引文件的长度及 SHA-256 全部匹配；冻结二进制、DWARF 摘要及两者 Mach-O UUID 匹配；八份符号与栈逐一核对 | 新 API 在此次运行中未减少提示；不是应用 fallback，不证明两个系统内部授权的具体原因 |
| 补丁实施前的实现 | provider、macOS provider、snapshot 三个冻结源码摘要与当时文件一致；worker 只派发一次固定 snapshot，busy 拒绝第二命令 | 首尾两轮 loader、各双读的预算仍成立；不能用静态预算代替原生调用计数 |
| 既有空合成库 | 已审阅的只读 snapshot 使用 SQLite `mode=ro` / `query_only`；数据 / profile 摘要、schema v11、目录身份 / mode、五活动表、pending / deleting、objects / staging 和根清单与保存基线一致 | 只证明文件 / SQLite 状态，没有读取系统凭据、查 ACL 或证明当前 slot 存在 / 可访问 |
| 较早九提示 | 保留原用户汇总及原记录；后两次逐提示运行均未重现 | 额外一次仍未知；不归为误报、解锁或 metadata，不为重现它反复运行 |

原始栈、二进制、dSYM、目标身份和摘要清单只留在本地忽略目录。索引复算证明当前文件与保存索引一致，不是独立签名证明，也不能补回历史缺失的遥测。两轮共 63 份文件的核对不包含新的系统调用观察。

当前正常成功路径为：一次 snapshot、两次 `load_existing`、四次 secret data search、十二次 attributes search（八次 read 前后检查与四次 label 检查）。当前 User domain 查询预算为 25 次（connect 一次、十二次 attributes 各前后一次）；旧 legacy 路径的 29 次保留为历史预算。四次 data 与十二次 attributes 合计十六次 `SecItemCopyMatching`，不是十六个提示。锁定的 `security-framework 3.7.0` 本地 `ItemSearchOptions::search` 源码在每次 search 中只调用一次该 C API，未见应用层重试循环。

## 有限诊断决定

不再安排与既有八提示运行相同的 sample-only 重复观测。下一项有信息增益的诊断是：**区分两个相同静态位置的提示是否跨越应用 search 调用的返回边界**。这只回答应用是否重复调用及提示落在哪个调用区间，不承诺解释 macOS 内部授权机制或减少提示。

| 假设 | 所需最小证据 | 观察后的处理 |
| --- | --- | --- |
| H1：同一次 secret search 尚未返回时出现两个提示 | 两个逐提示交接点之间没有该 call ID 的 end；该 ID 的 begin 已完整写出并可读，sample 正向落在 secret search 内 | 记录同一应用调用区间内的多个提示；停止 API 替换试错。若仍要求一次授权，另评审系统授权模式与复验契约，不能直接减四读 |
| H2：存在预算外应用 search / 动作 | 完整 trace 中多出 search begin、第三次 loader、第二个动作或非预期次序 | 停止原生测试，先定位生产调度 / provider 的具体重入或重复路径，再划定最小修复 |
| H3：额外提示来自 attributes / domain 阶段 | 对应阶段的活动 call ID 与正向栈同时成立 | 记录新增事实；独立评审该检查路径，不依据一次提示直接删校验 |
| H4：证据缺口或观测扰动 | begin/end 丢失、无法刷新、调用阶段不明、签名 / 构建漂移或提示与 call ID 无法关联 | 本次诊断无结论并停止；不提高提示预算，不把没有 end 等同于同一实例 |

### 最小观测补丁的原设计边界

旧冻结二进制没有 call ID / begin-end 输出；下述为实施前冻结的设计。有限观测补丁须完成范围确认、代码审阅、合成验证及单独的构建 / 操作单冻结后，D01 才具备执行资格。该补丁涉及跨 worker / provider 的诊断输出边界；本批不把设计授权扩大为修改生产日志策略，也不偷偷用 debugger、进程内存或系统全局日志替代。

- 仅 macOS 显式隔离验收入口可启用；默认构建 / 正常入口不输出，不能借用全局 logger 或打印 `Debug` / 原始错误。
- 记录固定枚举与计数：action ID、entry / release、read 1 / 2、attributes pre / post / label、domain / secret、嵌套 call ID、begin / end、单调时间和有界结果。范围是应用调用区间，不把它命名为 securityd 内部授权事件。
- 观测点包围真实 `search()` / domain 调用，不更换 API。保留完整双读、唯一性、identity、label、domain、首尾 key 一致性及最终对象 / 数据库复验。不得新增读取、写 key、重试、等待人工放行、跨动作缓存或延长 key 生命周期。
- 输出只写新建的本批专用文件，逐事件可见、有严格条数 / 大小上限并验证顺序。开始记录失败时不得触发 key API；结束记录失败时清理已返回的 secret 并中止动作，UI 不能发布部分结果。不得为使 trace 完整而再次访问系统。
- 不记录路径、slot、对象身份、正文、key 字节 / 长度 / 摘要、metadata 值、用户输入或原始 OS 错误文本；panic / unwind 不写成功 end，残缺 trace 判为无结论。
- 合成验收先证明事件配对、四读 / 十二 attributes 正常序列、各阶段失败即停止、输出失败的资源清理、无秘密输出、默认关闭及无额外读写；沿用 provider / reader / worker 拒绝与撤回回归。不得为观测引入新依赖、公共 schema 或持久业务 checkpoint。
- 实施前进一步冻结精确修改文件和接口接线。由 Astra 审阅方案后将确定切片交给 `gpt-6.1-sol`、`high`；返回后独立检查 diff 和验证，不把本设计当作补丁已交付。

### D01：一次空库调用实例观测

D01 与下面 N 批相互独立，不能捆绑授权。D01 的补丁已提交，专用冻结构建 / dSYM、精确操作单和读回预演已准备，见本文末节。启用参数为完整三项隔离参数加 `--acceptance-key-read-trace`；命令与唯一目标只保存在本地忽略操作单。准备结束时尚未授权原生执行；随后经本任务单独授权、复核冻结身份与实时基线后执行一次，实际限制见末节。本批已结束，不能沿用本次授权重试。

冻结操作单规定最多启动一个指定 GUI、一次空库 `OpenEncrypted`；只读取唯一既有合成 slot，不增加 security metadata / ACL 探针。预计 20–35 分钟，另留 5 分钟收尾。每提示先确认请求者与目标、保持未选择，保存最新 call ID / 阶段与一次最多 1 秒、20ms 间隔的该 PID sample，再由用户单次允许。最多八次 Allow Once；第九次只作至多一次定位后交接拒绝。出现目标不明、无完整 trace 或未知阶段时立即停止允许并交接，不要求用户事后回忆总数。

若无提示，记录本次零提示并将配对假设记未覆盖；不改 ACL / 签名 / key 来制造提示。若第八次后成功，保存完整 trace 并正常关闭；若拒绝，记录真实错误和失败关闭，不能冒充末轮取消已覆盖。若首对提示已能判别 H1，就交接用户拒绝当前提示，等待对应 end 与动作失败交付后正常关闭；结论只覆盖该调用区间，不宣称四个区间或完整动作预算均已实测。其它停止分支也必须处理当前提示并等待真实结果，不能留下未处理提示就退出观测。只有未提前停止且动作正常完成，才核对完整成功路径预算；不扩张 Refresh、有资料、写入或另一 API 对照。索引、trace 和原栈本地保留；只有匿名化结论进入正式记录。

## N 批：不经认证的空库原生关闭检查点

这是一项更小的原生验收，不依赖 D01 补丁。复用已核对的 `bf8dddf` 默认二进制 / 包装（包含 busy 关闭修复，与受限补丁实施前源码一致），不重签、不新建 key、不导入资料。精确路径、摘要、唯一 profile、命令与证据名放入本批忽略操作单。

N 批需要另行授权启动 GUI 和对固定合成 SQLite 短暂持锁。**不申请真实 Keychain 访问**：启动旧入口预期拒绝 v11；N01 在外部 EXCLUSIVE 锁存在时点击一次 `OpenEncrypted`，应在 `reader::open` 的数据库阶段失败，尚未调用 loader。源代码顺序与既有合成竞争测试支持这一设计；实际原生结果仍待验。若锁提前释放，可能走到系统提示，因此操作单要求未出现 GUI 错误前不得主动释放，并将意外提示列为停止条件。

| 场景 | 执行 | 判据与限制 |
| --- | --- | --- |
| N00 空闲关闭 | 第一次启动，等待旧入口 v11 拒绝且 worker 空闲；正常关闭 | 无 key 提示；原进程正常退出；只读基线不变 |
| N01 busy 关闭 | 第二次启动并等初始失败；helper 对唯一数据库取得空 EXCLUSIVE transaction，确认 ready 后点击一次 OpenEncrypted；实际看到 busy 后请求关闭 | 该关闭请求被取消、窗口保留；SQLite 锁错误真实交付后仍保持窗口，不自动退出；旧视图 / 恢复选择不暴露 |
| N02 结果后关闭 | 锁错误已经交付后释放 helper，核对 rollback 回执及进程退出；再次请求正常关闭 | worker join 后 GUI 退出；库 / profile / 对象 / staging / 权限与基线一致；不为检查恢复再点 Open / Refresh |

helper 仅在本批忽略目录实现，CLI 固定目标、显式授权参数、round 1–3，拒绝陈旧文件、符号链接、身份或摘要漂移；只执行空 transaction 与 rollback，不运行 DML。每轮 30 秒到期释放，最长三轮，仅在未观察到 busy 且数据不变时重试；OS 调度或阻塞不受硬实时保证。每轮必须保留准确 ready / release / released 证据。预演只使用临时合成数据库，不持锁既有库。

N 批预计 15–25 分钟加 5 分钟收尾；最多两次 GUI 启动、同一时刻一个 GUI 和一个 helper。SQLite 锁失败若太快不能观察 busy，应记未覆盖，不增大生产 timeout、不用暂停进程或 key prompt 冒充该时序。毫秒级结果与关闭同帧、接受关闭与按钮同帧仍只引用 headless 回归，不能由 N 批自然点击证明。

任何意外 key 提示不得允许：保持待选，报告当前阶段，交接用户拒绝 / 取消并等待真实结果；这一收尾不算授权读取 secret，也不能保证 OS 没有发起过访问尝试。无需为了恢复界面再认证。停止时只释放本批 helper、正常关闭本批 GUI，保留全部资料和证据；无法正常返回时报告实际状态，不能强杀或锁定整个 keychain。

持有原 capture 的 Cancel / Confirm、末轮取消、显式 Refresh、有资料旧视图撤回另列待验。N 批没有原请求，不以正常关闭代替其确认；不重新批准旧操作单中的导入、chmod、放弃、删除和恢复动作。以上缩小批次不能关闭 P1-S05，也不解除 PDF / 图片停止线。

## 2026-10-18 受限观测补丁实施范围

项目所有者随后明确要求推进补丁并委派实现；授权仅包括本地源码、合成验证与构建，继续禁止 GUI 和真实 Keychain。本节冻结实施边界，前述“尚未实现”为设计批次状态；最终实际结果另补记。

### 允许修改的文件

下表路径均相对仓库根。子代理不得修改表外文件；需要扩张时先向主代理说明理由。主代理负责检查实际 diff，并独立更新本方案、当前状态、provider / host / 原生计划及隐私文档；既有未提交文档不交给子代理覆盖。

| 文件 | 允许改动 |
| --- | --- |
| `crates/radishmemory-source-vault/src/lib.rs` | 导出隐藏的本地验收观测入口，不改变既有业务导出 |
| `crates/radishmemory-source-vault/src/key_read_observation.rs` | 新增线程作用域、固定枚举事件、编号 / 配对 / 限额与失败关闭 |
| `crates/radishmemory-source-vault/src/key_read_observation/file.rs` | 新增独占文件 sink、路径 / 身份检查及逐事件可见输出 |
| `crates/radishmemory-source-vault/src/key_read_observation/tests.rs` | 新增合成 sink、输出故障、序列、嵌套 / unwind / 线程隔离及无敏感输出测试 |
| `crates/radishmemory-source-vault/src/error.rs` | 仅添加明确的观测失败分类，不改现有错误映射 |
| `crates/radishmemory-source-vault/src/provider.rs` | 仅在既有读取 / label 检查周围接入阶段事件 |
| `crates/radishmemory-source-vault/src/provider/macos.rs` | 仅围绕既有 domain / attributes / data search 加观测，并保护记录失败时的 secret 返回缓冲 |
| `crates/radishmemory-source-vault/src/provider/macos/tests.rs` | 覆盖相同 macOS helper 的合成事件 / 错误 / 缓冲清理 |
| `crates/radishmemory-source-vault/src/provider/tests/snapshot.rs` | 验证观测开启 / 关闭均保留四读、label、首尾一致性与原错误 |
| `crates/radishmemory-source-vault/src/reader/snapshot.rs` | 明确 entry / release 阶段，不更换 reader、锁或最终认证顺序 |
| `apps/radishmemory-desktop/src/startup.rs` | 仅在 macOS 完整隔离参数下接受 `--acceptance-key-read-trace`，解析不创建文件 |
| `apps/radishmemory-desktop/src/startup/tests.rs` | 参数 / 平台 / 路径 / 默认关闭及无副作用测试 |
| `apps/radishmemory-desktop/src/worker.rs` | 隔离观测模式的初始 OpenPlain 只返回就绪提示；仅一次 OpenEncrypted 执行业务读取，禁止其它业务动作与重试；失败撤回完整视图 / 恢复选择 |
| `apps/radishmemory-desktop/src/worker/tests.rs` | 只增加新测试模块声明 |
| `apps/radishmemory-desktop/src/worker/tests/key_read_observation.rs` | 合成 worker 验证单动作门禁、拒绝路径、输出失败与旧视图撤回 |
| `apps/radishmemory-desktop/src/error.rs` | 仅新增有界的观测配置 / 已消费等拒绝原因，不回显参数 |

生产 `logging.rs` 保持逐字节不变。不修改 Cargo manifest / lockfile、第三方源码、cipher / canonical schema、Provider 业务 trait、application / core / SQLite、UI 布局、默认 v6 路径或非 macOS provider。若仓库检查器需登记新 sink，主代理仅在独立审阅实现后追加精确约束与回归，不扩大日志白名单。

### 接口约束

1. 新增一个隐藏的诊断入口，封装单次调用作用域；desktop 只在已验证隔离 profile 的首次 OpenEncrypted 中进入。内部通过线程局部且 RAII 管理的上下文传递固定事件，不把上下文存进 key、reader 或返回快照；不能变成跨动作缓存。没有活动作用域时仅执行原操作，不创建文件 / 时钟记录 / 系统访问；活动作用域损坏时失败关闭，不能降为无观测继续。
2. trace 文件必须是显式给出的绝对新文件，父目录已存在且为私有目录；拒绝祖先 / 目标符号链接、已有文件、资料根与默认资料库重叠、路径漂移及不支持的平台。使用 create_new / 0600；解析只检查、不创建。新文件及完整事件同步写出，不向 stdout/stderr 或全局 logger 输出，不接受环境变量开关或网络 sink。
3. 单作用域 action ID 固定为 1；所有事件携带固定版本、递增序号、call / parent ID、entry / release、read 1 / 2、固定操作枚举、begin / end、单调耗时和固定结果分类。不能序列化任意字符串、源错误、路径、slot、ID、key 长度 / 字节 / 摘要或返回值。最多 256 条事件、64 KiB；超限 / 配对异常 / sink 错误立即 poison，之后任何被观测系统调用均不得开始。
4. search 的 begin 必须成功写出才调用系统 API；end 在 API 返回后写出。成功的 secret SearchResult 在任何可能失败的 end 输出前必须已受零化 guard 保护；畸形结果同样清理。不得通过少读、改变 query、删除 attributes / label / domain、重试或 fallback 获得观测成功。原 key 与锁作用域保持；观测 I/O 会增加耗时，不增加可复用 secret 副本或延长持有到动作之外。
5. 成功路径保持两次 loader、四次 secret data search、十二次 attributes search及 25 次 User domain 查询。默认其它读写动作仍走原实现。若业务失败与 end 输出失败同时发生，不能伪报成功或吞掉原有业务失败；trace 不完整需判为诊断失败。动作 end 失败不得发布已生成快照 / 恢复选择。unwind 清理线程上下文但不写成功事件，部分文件保留且不可复用。
6. 观测模式是单次只读诊断：要求已有普通数据库文件；首次自动 OpenPlain 仅返回固定的观测就绪提示，不调用 application / provider、不打开或初始化明文库、不创建 trace，资料与恢复视图保持空。由此避免错误配置指向缺库 / 旧 schema 时触发普通入口迁移或写入。一次 OpenEncrypted 后无论成功或失败均不可再次读取，也不可 Refresh / Verify / Import / Delete 等。关闭通过现有 worker 生命周期完成。无参数或普通三参数验收行为不变。首个 OpenEncrypted 前先复验 profile 和目标路径。

### 验收标准与委派

委派给 `gpt-6.1-sol`，推理强度 `high`。子代理返回文件清单、改动摘要、实际测试结果与未解决问题；不提交、不启动原生进程、不访问凭据、不读写保留合成库。主代理审阅实际 diff 后独立执行关键负向测试、完整仓库检查与默认 desktop 构建。

- desktop 的 action-end 故障使用严格 `cfg(test)` 的私有 after-action hook，仅破坏测试自己的 trace 文件以触发真实文件完整性检查；默认构建无该字段 / 调用，不导出、不改变 sealed Provider 或生产故障入口。
- 通过相同 macOS helper 的合成调用验证事件顺序、每个 secret / attributes / domain 失败位置、结果分类及无重试；不是仅手工调用 emitter 检查字符串。
- 四次读取 / 十二次 attributes 的正常序列及每位置失败即停、缺 key、歧义、label / domain 漂移、同轮 / 首尾 key 替换保持原失败关闭。
- 开始 / 中途 / 结束写入失败、flush / 限额 / 配对失败均不继续访问；返回 secret 与畸形多结果在观测失败时清理，未生成成功快照。
- 新文件独占创建、0600、祖先 / leaf symlink、已有文件、非法路径、不完整 / 重复参数、非 macOS 拒绝；默认入口无 trace 副作用。
- 无跨动作 / 跨线程串线；嵌套 scope 拒绝；panic / unwind 后上下文释放、文件不伪报完成。trace 和错误展示不包含合成敏感标记；现有 logger 抑制回归继续通过。
- 运行 fmt、targeted Rust tests、locked Clippy、完整 `check-repo.sh` 和默认 locked/offline desktop 构建；依赖图不启用 acceptance-test-support。仅记录本机合成证据，不宣称原生提示改善或其它平台通过。

## 前序离线方案批次交付与验证

正式文档保存结论 / 判据，本地操作单保存冻结目标、命令和证据清单。当前状态及原生计划同步现行路由，既有八 / 九提示历史不覆盖。

空库持锁 helper 由 `gpt-6.1-sol`、`high` 子代理实现，允许写入范围仅本批忽略目录中的 helper、合成测试、验证摘要及自建 scratch。主代理已审阅完整新增文件，独立重跑 17 项 stdlib 测试通过：真实独立进程争锁、显式 / 到期释放、SIGTERM 收尾、未取锁不伪报 released、控制文件与符号链接拒绝、验证失败释放及无授权 CLI 提前拒绝；各测试库字节保持不变。主代理另执行固定合成库的只读 inspect，验证完整空库 guard 通过，没有执行其 hold。工具持锁期间不打开 / 关闭同一数据库的第二个 fd，避免破坏 POSIX 进程锁；只在连接前和关闭后计算该数据库摘要。

本批未修改生产 Rust、公共接口、依赖 / lockfile、日志策略或加密语义；没有 GUI、Keychain、ACL、既有库排他持锁、VM、远程写入或提交。默认 desktop 没有重新构建：使用已核对摘要与匹配 dSYM 的冻结默认产物，且当前源码未变。原生行为、观测补丁、Windows / Linux 仍未验收。`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 首轮在 P1-F17 合成本地端口绑定处受到沙箱 `PermissionDenied` 限制；按权限流程在沙箱外完整离线重跑退出 0，266 个仓库文件、notices、fmt、locked all-targets / all-features Clippy、25 个 suites 共 386 项 Rust tests 通过，11 个 ignored helper 由父测试调用。未跳过测试或放宽检查。补记后重跑文本 / 链接检查与 `git diff --check`；未单独重跑未受影响的 M0 fixture runner、Python 检查器回归或 compile-fail doctest。

本批结束时 `dev` 有五份未提交文档（含本方案新增文件）；操作单、工具、原始证据复核摘要及预演结果均在已忽略的本批目录，未暂存 / 提交 / push。合成测试进程已结束，scratch 为空；既有合成库摘要、目录身份 / 权限和零活动状态再次复核保持，旧 key / 包装 / 证据全部保留。


## 受限观测补丁实现与独立审阅

该实现由 `gpt-6.1-sol`、`high` 按上述文件清单完成，主代理承担实际 diff 审阅、边界收口和独立验证。没有改变四次 secret、十二次 attributes、25 次 User domain 的正常成功预算，也没有省略唯一性、identity、label、domain、双读、首尾 key 一致性及最终对象 / 数据库复验。`SecretSearch` 和 `AttributesSearch` 的 end 表示系统 search 返回后的应用记录点，结果先受清理 guard 保护；query 结果的后续解析失败由外层读取阶段报告，不能将 API 返回 `ok` 解读为整个动作成功。

显式诊断模式仅接受已有隔离 profile / 普通数据库文件和一个库外的新 trace 路径。自动 OpenPlain 只交付固定就绪提示；一次 OpenEncrypted 不论成功或失败均消费额度，其它业务命令被拒绝。trace 的 Action end 完整成功之前不发布目录 / 正文 / 恢复选择；业务错误与输出失败同时出现时保留原业务错误，残缺 trace 仍判无结论。默认入口和普通三参数验收入口不启用该模式。

文件使用独占创建、0600、祖先身份绑定、leaf 身份 / 单链接 / 长度 / 权限及 mtime / ctime 漂移检查；每事件 `write_all` 与 `flush`，最多 256 条 / 64 KiB。这里保证正常运行时可读的应用事件，不承诺 `fsync`、断电持久性、原子行写入或抵抗同一用户在检查间隙恶意替换路径；部分写入、缺失 end 或任何配对异常都不能推定成功。调用 RAII guard 在 unwind 时毒化作用域，后续观测调用被阻止，作用域退出清理 TLS；不在 panic 收尾伪造 end。原始 SearchResult 的 Rust Data 及已提取 secret 受既有 zeroization 约束，CF / OS 内部副本仍不在保证范围内。观测 I/O 会增加调用耗时，不能将其测量当作无扰动性能基线。

生产 `logging.rs` 保持字节不变；受限文件 sink 是单独显式启用的本地验收输出，固定字段禁止携带路径、slot、内容、key 或原始错误。检查器对事件生成器及文件 sink 分别钉住已审阅源码摘要，不扩大 log / tracing / print 白名单。trace 是本地诊断材料，不进入业务 schema、普通日志、仓库或 CI artifact；由未来操作单单独确定保留和清理，本补丁不自动删除证据。

前序 N 批操作单仍只对应其旧 `bf8dddf` 冻结产物，不代表当前补丁源码；不得覆盖旧证据或沿用旧 dSYM 解释新构建。D01、N 批、其它平台及真实 Keychain 提示均未执行；本次不宣称减少提示、解释系统内部授权原因或关闭 P1-S05。


### 本机独立验证结果

主代理已独立审阅全部 Rust diff、新增文件、检查器约束及文档；审阅时收紧初始 OpenPlain 为无业务副作用的就绪动作、将 SearchResult guard 放在系统返回与 end 之间，并补齐嵌套 / 被捕获 unwind 后阻止继续调用及真实文件故障的覆盖。独立 `cargo test -p radishmemory-source-vault --lib --all-features --locked --offline`：190 项通过、8 个 helper 按父测试约定执行；其中实际 macOS helper 的合成成功序列保持 4 / 12 / 25，41 个底层调用失败位置逐一验证，128 条事件各自的 write / flush 故障均阻止预算外调用。正常及畸形多 Data 返回在 secret end 失败时通过严格 `cfg(test)` 清理观察证明 Rust 缓冲已清空。

完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 首轮只在既有 P1-F17 合成本地观察端口因沙箱 `PermissionDenied` 失败；按权限流程在沙箱外离线重跑退出 0：270 个文件、notices、fmt、locked all-targets / all-features Clippy，25 个 suites 共 398 项 Rust tests 通过，11 个 ignored helper 由父测试调用。desktop 的 57 项测试包含单动作限制、成功 / 业务失败 / 输出失败后撤回和禁止重试。另独立运行 45 项 Python 检查器回归通过；精确受限 sink 源码通过、缺失 / 改动均被拒绝，原 logger 输出限制继续生效。

默认 `cargo build -p radishmemory-desktop --locked --offline` 通过；默认 normal / build / features 依赖图不含 `acceptance-test-support`。生产 logger 原 SHA-256 保持，Cargo manifest / lockfile、第三方代码与认证契约无差异。验证日志仅保存本机临时目录。末次文档补记后再次运行仓库 Python 基线与 `git diff --check`；未单独重跑未受影响的 M0 fixture runner、compile-fail doctest、Windows / Linux、GUI 或真实 Keychain 验收。

收尾时仍在 `dev`，共有 25 个改动 / 新增文件（含前序五份文档），全部未暂存、未提交、未 push；本次未刷新远程。检查 / 构建进程已退出，未启动长期服务；没有操作保留库、原 key、ACL、旧包装 / dSYM 或原始证据。此次默认构建不是 D01 专用冻结产物，后续原生执行须按上文另行冻结与确认。


## 2026-10-18 D01 冻结准备结果

项目所有者随后要求提交本批改动并准备 D01，明确暂不 push、启动 GUI 或访问真实 Keychain。25 份实现 / 文档 / 检查器改动已提交为 `1cfc8d9`；本节是其后的独立准备记录，不改写前序未提交状态。

以该提交执行默认 locked / offline desktop 构建，将逐字节相同的 binary 复制进独立本地包装，再由 dsymutil 生成本批 dSYM；二进制与 DWARF 的 arm64 UUID 同为 `80470321-B219-356D-B1D9-8199B1DF7426`。离线 atos 已正向映射 `checked_attributes` 与 `read_checked_secret` 到本次 macos.rs:190 / :171。仅保留链接器原有 ad-hoc 签名，没有重签、证书访问、安装或分发签名；Info.plist 未绑定签名，不以包装身份推断当前 slot 授权。旧包装、符号和原生证据全部保留。

本地 `freeze.json` 固定 29 份产物的长度 / SHA-256，含 binary、dSYM、目标清单、精确操作单、只读工具和预演证据；清单摘要为 `2e6f4c0fd2e56399bd7656bb63759a82ab974be068fd7cbc5f595fad3ce7df65`。清单是本地一致性校验，不是独立签名证明。默认依赖图不含 `acceptance-test-support`；原生 trace、GUI 日志和 native-preflight 回执尚不存在。所有路径、合成 slot、trace、样本和命令仅留本地忽略目录，不进入 Git / CI。

读回工具的 9 项 stdlib 测试通过，覆盖完整 128 事件、127 个合法前缀、各失败 end 的逐层退出、固定字段 / 编号 / 阶段 / 顺序 / 限额 / 重复 JSON key、文件与 CLI 拒绝、不覆盖证据。前置工具的一项参数化测试覆盖 9 个成功 / 漂移场景：摘要、源代码、工作区、旧输出、UUID、库基线、旧回执和越界清单路径。独立 Rust 小型 probe 直接链接本次默认 rlib，只执行文件观测作用域并返回合成业务错误；动作内可读 begin、返回后可读失败 end，未引用 / 调用 provider 或 GUI。该探针证明正常运行下的逐事件可见性，不证明断电持久或真实四读效果。

实际准备前置检查通过：29 份文件摘要、UUID、源码及工作区一致；固定空合成库的 state / extra 与保存基线完全匹配；真实 trace 不存在。只读基线没有验证系统 slot、ACL 或凭据内容。操作单限制最多一个 GUI、一次 OpenEncrypted；每提示先保存 trace 前缀、最多一次指定 PID 的 1 秒 / 20ms sample、再保存同一前缀并正向符号定位。首对已支持同一 SecretSearch 区间就交接拒绝收尾；上限仍为八次 Allow Once，第九次只定位后拒绝。目标不明、记录变化 / 残缺或预算异常即停止，不为补足完整预算继续认证。

准备文档更新后，完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 按已验证的本地端口权限方式在沙箱外离线通过：270 文件、notices、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 由父测试调用，45 项检查器回归。末次事实补记后再次运行 Python 仓库基线与 `git diff --check`；没有修改生产实现、依赖或 lockfile。准备结果以独立文档提交保存，冻结产物仍只在本地忽略目录。

准备结束时 D01 为“准备完成、待独立原生授权”，未执行任何 GUI / Keychain / sample、业务写入或远程操作；旧 N 批与其它验收仍独立。新操作单要求执行前重新核对精确产物与库基线，不能直接沿用准备时回执，也不能以新摘要替换冻结漂移。

## 2026-10-18 D01 单次原生结果与交接缺口

项目所有者随后明确授权按冻结操作单执行：先复核构建、dSYM 和空库，只启动一个 GUI、执行一次 OpenEncrypted，每个系统提示先记录再交接用户选择；首对确认同一调用区间即拒绝收尾，暂不 push。执行前工作区干净、`dev` 领先本地 `origin/dev` 两个提交；29 份冻结文件、binary / DWARF UUID、源码与空库 state / extra 全部匹配，未发现目标占用，trace 不存在。没有重新签名或改动冻结操作单。

只启动该冻结 GUI，核对 Synthetic acceptance 与观测就绪文案后，只点击一次 OpenEncrypted。首次提示前缀捕获返回 `unusable_or_unstable_trace` / exit 2，未建立有效逐提示记录；随后读取发现动作已完整结束。独立 `final` 校验通过：128 条事件、一个 Action，entry / release 各两读，共 4 次 SecretSearch、12 次 AttributesSearch、25 次 Domain，全部 begin/end 配对且结果为 ok；UI 显示加密打开成功、零活动来源。首次 capture 要求尚有活动 API，完整终态不满足该条件，不能把其拒绝结果当作业务失败或 trace 损坏。

用户事后报告八次提示全部选择允许。Agent 没有及时建立逐提示交接，没有取得系统提示请求者 / 目标的独立核对、逐提示 trace 前缀、sample 或运行态符号定位，也没有点击系统允许 / 拒绝按钮。该八次仅是汇总自述，不能补写为八条逐提示证据；四个 SecretSearch 的时长也不能证明每调用两个提示。**本轮调用预算成立，但 D01 的调用区间诊断未达目标，H1 / H3 未定；完整 trace 未发现预算外的受观测调用。首对停止条件未能评估，没有拒绝 / 取消验证。** 不以历史正向栈替代本次缺失关联，也不重新打开来补证。

成功结果后正常关闭唯一窗口，原进程退出码 0；确认退出后才执行固定只读 snapshot。数据库 / profile 摘要、schema v11、全部零活动表 / pending / deleting、空 objects / staging、目录身份 / 权限与根清单均保持基线。29 份冻结文件与 freeze 摘要再次复核未变；原始 trace、终态校验、汇总台账、退出后基线与独立 runtime 索引保留在本地忽略目录。无 sample 进程、第二个 GUI、Refresh、业务写入、强杀、额外 ACL / metadata 查询或远程动作；未改动生产代码或依赖。

本批已经收尾，不关闭 P1-S05。下一步先离线复核逐提示人工交接方式，确保用户在每个提示保持待选、记录完成后才选择；不为控制交接新增应用内等待放行、删认证检查或修改系统授权策略。若仍需 H1 / H3 证据，须另行冻结新的独占证据路径并确认一次有限原生范围，不能复用本批输出或把八提示汇总作为诊断通过。N 批、末轮取消、有资料 Refresh、busy / pending 关闭和其它平台继续独立待验。

收尾文档更新后，完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 在已核准的本地合成端口权限范围内离线通过：270 文件仓库基线、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 由父测试调用。末次事实补记再运行 Python 仓库基线与 `git diff --check`。本次只修改四份正式文档，未暂存 / 提交 / push；`dev` 保留此前两个未推送提交，原生 GUI 与检查进程均已退出。

## 2026-10-18 D02 首对同区间与拒绝收尾

项目所有者另行授权有限重验，并明确每个提示先保持未选择、记录完成后再操作；沿用原冻结构建 / 匹配 dSYM，新证据目录，一个 GUI、一次 OpenEncrypted，首对确认同一区间即拒绝，暂不 push。D01 原始证据、freeze 和二进制均未覆盖或重签。启动前重新核对原 29 份冻结产物、双 UUID、生产源码与空库 state / extra；工作区的四份 D01 未提交文档改动保持原样并另存差异摘要，新运行输出均不存在，未发现目标占用。

D02 只更换证据目录，应用名称仍为原冻结包装。Agent 最初预期系统弹窗显示固定 label，描述不准确；用户主动提供的截图显示请求者为冻结包装、目标为固定 service，与冻结查询匹配。SecurityAgent UI 工具因安全限制拒绝读取后未绕过；第一提示通过用户确认及其主动提供的截图核对请求者 / service，第二提示由用户确认文案相同。弹窗没有展示 account，不将其视为独立 slot / ACL 查询；密码仅由用户在系统提示内输入，不进入记录。

两次提示均先保持待选，再各执行一次已核实应用 PID 的 1 秒 / 20ms sample。每份 sample 前后均严格读回 trace；四份前缀逐字节相同，全部为 14 条事件，停在 action 1、entry / read 1、`SecretSearch call 9` 的 begin，期间没有该 call end。两份 sample 的进程、启动时间、Binary Images UUID / load address 与冻结构建一致，栈均正向经过 `read_checked_secret → ItemSearchOptions::search → SecItemCopyMatching`；使用各自观察地址和原匹配 dSYM 离线 atos，落在当前 `macos.rs:176` / `:122` 与上游 `item.rs:504`。

**H1 对本次入口第一读的这一应用调用区间成立：两个提示发生在同一 SecretSearch 尚未返回期间。** 这不是靠缺少栈帧、提示总数或持续时间推断。第一提示记录完成后用户单次允许；第二提示记录完成并核对文案后，按停止线拒绝，没有继续完成其余读取或再次打开。

拒绝后 call 9 的 end 为 `cancelled`，Read1 / Entry 同样失败退出，Action end 为 `business_failed`。严格终态校验得到完整失败序列，共 18 条事件；实际仅到达 1 次 SecretSearch、1 次 AttributesSearch、3 次 Domain，没有后续读取。UI 显示真实 `KeyStoreCancelled` 和资料库不可用，没有可用来源目录 / 正文或已核验恢复选项；通用 Inspect recovery / Retry 控件仍可见，但未执行，恢复状态明确未核验。

正常关闭唯一窗口，原进程退出码 0，两次 sample 也均退出 0。GUI 退出后才执行固定只读 snapshot：数据库 / profile 摘要、schema v11、零活动资料 / pending / deleting、空对象与 staging、目录身份 / 权限及根清单全部保持；原 29 份冻结产物再次复核未变。两提示前后 trace、原始 sample、匹配符号化、逐次用户选择、终态与退出后基线共 23 份文件进入独立 runtime 索引，全部留本地忽略目录。无第二 GUI、重试、Refresh、额外 key / ACL / metadata 探针、业务写入或远程动作。

该结论不解释 securityd 内部为何多次授权，也不推广到余下三读、历史全部八 / 九提示或其它操作。D01 已记录的完整四读预算与 D02 的提前拒绝是两个独立范围；本次不声称完整四读成功、末轮取消、有资料 Refresh、busy / pending 关闭或 P1-S05 全部通过。后续先离线审阅系统授权模式与现有复验契约，不能据此直接减少四读、删除认证检查、跨动作缓存或采用永久许可。新增原生实验另定精确范围，不沿用已收尾批次的授权。

D02 收尾文档更新后，完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 离线通过：270 文件基线、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 由父测试调用；末次事实补记后再次运行 Python 仓库基线与 `git diff --check`。四份文档包含 D01 / D02 结果，仍未暂存或提交，`dev` 保留此前两个未推送提交；没有 push，全部本批 GUI / sample / 检查进程已退出。

## 2026-10-18 M01 固定目标元数据核对方案

D01 / D02 四份记录随后按项目所有者要求提交为 `4c3a0bd`，未 push。项目所有者继续授权公开源码审阅、准备并审阅本方案，明确保留四次读取及全部认证检查，禁止真实 Keychain 访问、GUI 与 push。本方案准备阶段只交付方案、API 审阅和本地目标绑定；当时未实现或运行元数据 helper，也未验证当前 slot / ACL。后续本地实现见本页下一节。旧 D01 / D02 原生授权均已结束。

### 来源与证据边界

现有样本记录的 Security 为 `62460.1.3`；官方标签及 `Security-62460` 前缀查询未找到对应版本。本次参考 `Security-61901.120.67`、commit `97c3a4296c1ea06b0fe1877a7e616aa84450b5b2`，不宣称与当前运行二进制一致。源码仅作本地审阅，保留 Apple 版权、APSL 2.0、原始文件及 URL / SHA-256 清单，不集成进产品。[官方标签](https://github.com/apple-oss-distributions/Security/tags)

- 常规 ACL 后另有 partition 校验；解密在允许 UI 且 partition 不匹配时可以再次提示。普通一次允许不执行持久授权回调。此机制支持双提示假设，但不证明 D02 的实际授权类型。[服务端检查](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/securityd/src/acls.cpp#L121)、[提示回调](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/securityd/src/acl_keychain.cpp#L246)
- 该版本的 Keychain 访问确认本身要求密码；flags 为零不排除密码提示，两个密码框不证明两次解锁。[提示实现](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/securityd/src/agentquery.cpp#L494)
- `SecTrustedApplicationCopyData` 返回路径提示，旧工具的路径摘要不是代码要求。`SecTrustedApplicationCopyRequirement` 才取出已存要求，但属于私有 SPI；成功且 NULL 表示未保存要求，不表示匹配。[实现](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/OSX/libsecurity_keychain/lib/SecTrustedApplication.cpp#L63)、[SPI 定义](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/OSX/libsecurity_keychain/lib/SecTrustedApplicationPriv.h#L70)
- partition subject 在该版本的 ACL 简单内容中以十六进制 payload 暴露；普通提示 description 和 Integrity 条目不是 partition。此编码为版本相关实现细节，非稳定公共协议。[ACL 解析](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/OSX/libsecurity_keychain/lib/ACL.cpp#L418)、[内容转换](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/OSX/libsecurity_keychain/lib/SecACL.cpp#L123)

### 固定输入与禁止交互

M01 只针对 D02 已绑定的唯一合成 service / account、历史 User domain 默认 keychain 路径和原 D01 executable；不按当前窗口、helper 自身、bundle 名称或默认构建目录猜测目标。精确路径、slot、保存的 CDHash、原 freeze / targets 摘要只在独立忽略操作单中保存。历史路径绑定不是当前 keychain 身份已验证。

本轮准备只读既有文件：复算原 D01 29 份冻结产物、源码、D02 目标及保存的签名记录，绑定 binary / Info.plist / dSYM。CDHash 是签名元数据，不能用整文件 SHA-256 代替。保留 ad-hoc、无 TeamIdentifier、`Info.plist=not bound` 的限制；不重签、创建证书或启动 app，本轮不调用代码签名 / Keychain API 补证。

未来 helper 为单次、单线程的任务专用 CLI。先验证精确清单、显式运行门禁和独占输出，再在任何 Keychain 查询前成功调用 `SecKeychainSetUserInteractionAllowed(false)`，以 `SecKeychainGetUserInteractionAllowed` 读回 false。失败即退出；设置只限 helper 进程，不改全局偏好，不在错误路径重新开启 UI。禁用交互是调用契约，不能在未验证当前 OS 前承诺绝对零 UI。

交互被禁用、拒绝、锁定、缺失和其它错误均为真实停止结果，不重试、不解锁、不改 flags、不输入密码；不采用跳过需认证条目的查询或 data-protection backend。意外弹窗即越界，无允许额度；记录阶段并交接用户取消，不能自动点击、再次调用或把取消写成成功。

### 方案冻结时的调用顺序

以下为方案冻结时的调用规格；实现和命令冻结结果见下一节。真实访问仍须独立授权。

1. **冻结身份**：对固定 executable 建立 `SecStaticCodeRef`，先完整静态校验，指定 `kSecCSNoNetworkAccess`，不跳过 executable / resources、不启用网络或 Gatekeeper 操作。错误即停止，不降低校验或重签。此结果仅是离线静态结论，不等于在线信任或 securityd 动态校验。私有 SPI 符号须在 Keychain 访问前确认可用；缺失即停止，不枚举其它私有入口、解析系统文件或 fallback 到路径比较。
2. **限定 keychain**：解析一次 User domain 默认 keychain，核对保存路径及文件身份，拒绝非预期 symlink / 类型；漂移即停，不修改默认值或扩大 search list。结束前只再核对一次相同 domain / 文件身份。无需读取整个 login keychain 文件内容、计算摘要或要求其 mtime 不变，避免把其它正常系统活动当成目标漂移。
3. **唯一目标查询**：第一轮 `SecItemCopyMatching` 使用一个固定 keychain、GenericPassword、精确 service / account、`kSecMatchLimitAll`，仅返回 attributes、item ref 和 persistent ref，绝不请求 `kSecReturnData`。**label 不进过滤条件**；结果必须恰好一项，再核对 class / service / account / label，否则停止。这样不会以 label 掩盖重复或异常目标。persistent ref 只在内存比较前后条目身份，不保存或输出。
4. **复制 ACL**：对唯一 item 使用 `SecKeychainItemCopyAccess`、`SecAccessCopyACLList`，逐项使用 `SecACLCopyAuthorizations` / `SecACLCopyContents`。不支持结构或错误即停止。禁止 secret API、add / update / delete、SetAccess / ACL setter、key 导入导出、锁定 / 解锁及管理员操作。metadata-only 仍是实际系统访问，可能需要认证、产生内部缓存或访问痕迹，不能承诺 OS 零写入。
5. **比较和二次复核**：按下表在内存比较，然后对相同固定查询及 access / ACL 做且仅做一次第二轮读取，要求 persistent ref、精确属性、完整 ACL 内容和已存要求内容一致；对所有已支持 ACL 的应用项复制已存要求用于稳定性比较，仅对 Decrypt / Any 项评估匹配；编码保留顺序、重复、空 / NULL 区别，不只比较计数或路径。最后复核 domain、冻结输入与文件身份；任何变化记 `unstable`，没有第三轮。两次一致不等于原子快照，也不排除中间变更后恢复；未证明框架缓存已失效，不能把第二轮称为独立刷新后的系统状态，不为刷新增加第三轮或私有缓存操作。

预算上限为 2 次 item search、2 次 item access copy、2 次 ACL list copy、2 次 domain 解析。每轮至多 32 项 ACL、每项 64 个应用且总应用数至多 128；单个 requirement 编码至多 32 KiB，partition 解码至多 64 KiB / 128 项 / 每项 256 ASCII 字节；越限停止，不截断后下结论。这些是诊断接受限额，不保证 OS API 内部分配受同样限制。secret、业务动作、GUI 和系统提示的允许预算均为零。

### 比较方法与结果

| 对象 | 比较方法 | 结论边界 |
| --- | --- | --- |
| 冻结构建 | 整文件摘要匹配；静态签名的 arm64、ad-hoc、identifier、无团队 / 证书、CDHash 与保存证据匹配，完整静态校验通过。只对这个已确认 ad-hoc 切片推导 `cdhash:` 加 `kSecCodeInfoUnique` 的小写十六进制；签名类别变化即停止 | `frozen_identity_match` / `drift` / `unknown`。推导值称 candidate partition，不称 securityd 已观测值；不能用 helper CDHash、bundle ID、整文件 SHA-256 代替 |
| partition | 仅接受一个 authorizations 只含 PartitionID 的简单条目；description 按严格偶数位 hex 解码为有界 XML plist，根为单一 `Partitions` 字符串数组。拒绝重复键、外部实体 / DTD 外取、未知结构；与 candidate 完整字符串精确比较 | `candidate_present` / `candidate_absent`。缺失 / 多个条目、空数组、重复成员、格式不支持单列异常，不当 absent；不调用修改命令修复 |
| 可信应用要求 | 对含 Decrypt 或 Any 授权的简单 ACL，用 SPI 取**已存** `SecRequirementRef`，以冻结 static code 执行 `SecStaticCodeCheckValidity(..., kSecCSNoNetworkAccess, stored_requirement)`；不用 CreateFromPath 重建要求，不比较要求文本相等，不执行 ACL 中的路径 | 每项 `offline_static_match` / `requirement_mismatch` / `no_stored_requirement` / `error`。明确的要求不匹配与其它错误分开；NULL 要求保留 legacy 未知。NULL app list 单列 `allow_all_form`、空列表单列 `prompt_only`。未知项阻止“全部不匹配”的否定结论 |

candidate 推导依据[公开客户端分类](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/securityd/src/clientid.cpp#L187)；要求比较对照[服务端代码要求验证](https://github.com/apple-oss-distributions/Security/blob/97c3a4296c1ea06b0fe1877a7e616aa84450b5b2/securityd/src/codesigdb.cpp#L74)。静态匹配不等于整个 ACL 通过，仍有提示策略、其它 subject、实时进程与版本差异；helper 的访问结果也不能代表冻结 app 的认证结果。

| 稳定结果 | 解释 |
| --- | --- |
| 未找到静态信任匹配且 candidate absent | 加强“常规授权再加 partition 授权”候选解释，仍不证明 D02 实际分支与顺序；否定结论要求所有适用项均完整解释且没有 allow-all |
| 有静态信任匹配且 candidate absent | 支持继续核对 partition，不能解释常规提示为何出现，保留运行态 / 策略差异 |
| candidate present | 当前元数据不支持 candidate 不匹配，不能倒推 D02 当时状态，也不排除版本推导误差 |
| 漂移、SPI 缺失、未知结构、拒绝或交互被禁用 | 记录未完成 / 无结论，不补查 secret、不扩大权限、不反复采集 |

### 输出、收尾与审阅结论

原始 partition、requirement 文本 / blob、description、可信应用路径、attributes 和 persistent ref 只在内存比较，退出前释放，不打印 CF 对象 / Debug / 原始错误描述。不声称释放能清除 OS 缓冲、swap 或系统日志。0700 目录内独占创建 0600 结果文件，最多 64 KiB，只含版本、阶段、固定错误枚举 / 数值 OSStatus、计数、局部编号和比较枚举；第二轮及收尾复核全部通过后才写成功。部分结果不是成功，正式文档只提炼匿名结论，原始运行文件不进 Git / CI、不外发。

预计将来正常 CLI 小于 30 秒，人工核对及收尾约 5–10 分钟。30 秒未结束记超时 / 未完成，不新开 helper 或继续查询，实际退出前不称收尾。执行操作单须冻结超时监督与单个 helper 的终止方式，不假设同步 Security API 可硬取消，也不结束 securityd / SecurityAgent。意外 UI 交接取消后等待真实返回；无法返回则报告未完成，不扩大系统操作。方案不申请 key / ACL 写入，因此没有相应回滚动作，也不承诺恢复 OS 内部痕迹；输出保留，清理另行授权。

实施后的合成检查至少覆盖：无授权 / 漂移时零调用；禁 UI 设置 / 读回失败；缺失 / 重复目标与 label 异常；partition 编码正反例、重复键 / 超限 / 未支持；要求匹配 / 不匹配 / NULL / SPI 缺失；NULL 与空应用列表；两轮 item / ACL / domain 漂移；输出失败、超时及无第三次调用。使用 mock 系统边界与自建合成数据，不连接真实 slot，不把 parser 通过写成当前 OS 可用。

静态审阅已收口旧工具的 label 预过滤、路径摘要误作身份、helper 身份混淆、partition description 误读、NULL 要求误判和隐式 UI 重试问题。**方案冻结时可进入本地 helper 实现与合成验证评审，不具备真实执行资格。** SPI 与版本相关编码只用于本地诊断候选，不进入生产 provider、依赖或公共协议。当时 helper 源码 / 编译产物 / 精确命令、超时实现和合成结果尚未交付；后续准备见下一节，不能提前执行原生访问。

生产四次 secret、十二次 attributes、25 次 User domain 成功预算，以及唯一性、identity、label、domain、双读、首尾 key 一致性和最终对象 / 数据库复验全部保持。M01 两个 metadata 观察点是未来独立诊断预算，不替换生产检查；P1-S05、其它原生验收和历史第九次提示仍未关闭。

本轮实际准备复核原 D01 freeze 摘要、29 份产物和 11 份源码摘要全部匹配；从已保存签名记录提取 candidate，没有调用签名 / Keychain API。独立本地操作单、目标绑定、30 份官方源码来源清单与静态审阅结果均留忽略目录。完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 在既有合成本地端口权限范围内离线通过：270 文件基线、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 按父测试约定执行。末次补记后复核 Python 仓库基线与 `git diff --check`；未新增 helper 测试、单独重跑检查器回归或进行原生 / 跨平台验证。只修改本方案和当前状态两份文档，未暂存 / 提交 / push；`dev` 保留三个未推送提交，检查进程已退出，无 GUI 或长期服务。

## 2026-10-18 M01 本地实现、合成验证与冻结

项目所有者随后授权按 M01 方案实现本地 helper、完成合成测试与独立审阅、冻结精确命令 / 超时收尾 / 输出边界；明确保留四次读取及全部认证检查，暂不执行真实 Keychain 访问、不启动 GUI、不 push。本批交付只在独立忽略目录，未修改产品代码、依赖、lockfile 或公共协议。这里的审阅为主代理在实现完成后单独进行的静态复核，不是第二位人员或子代理审阅。

### 已实现和冻结

- 共享 Objective-C 引擎负责禁交互前置条件、两轮固定目标与完整 ACL 稳定性、partition 严格解析、已存要求三态和尝试预算；原生 Security adapter 只编译，没有执行，包括无参数门禁也未运行。Foundation 合成 backend 与原生 adapter 分开链接，合成程序不能转入真实 provider。
- 原生路径先检查固定文件、门禁与输出，再禁交互 / 读回，确认私有 SPI 来源、原 D01 arm64 ad-hoc 静态身份和完整离线校验；每个阶段均有输出监督 ACK。item 查询不带 label，返回唯一结果后再核对；不请求 data、不修改 key / ACL。两轮上限为 domain / item / access / ACL list 各两次，计数在实际调用前递增；失败后不进入下一阶段。
- Python 监督器只接受离线检查或一个固定目标原生命令；普通文件检查无原生访问。精确绝对路径、目标常量、命令、编译器 / SDK / Python 信息和输出位置仅记本地操作单。21 份源文件、编译产物、合成日志、操作单与静态审阅结果已冻结；`freeze.json` 的 SHA-256 为 `916bc7d5e9c564acff8e4efee2ec3ecd6b8575ad5fb4b1b47434d428fe54b7f6`。原 29 份产物、11 份源码和原 freeze 共 41 份普通文件由不链接 Security adapter 的文件 guard driver 两遍核对通过；监督器的真实 `--check-offline` 也已通过，明确回执 `native_started=false`。

### 超时、输出与停止条件

监督器从子进程建立后计时 30 秒，超时记未完成；只对自己持有的唯一子进程发一次 TERM，5 秒未退出则一次 KILL，再等至多 5 秒回收。native 自身另设 35 秒 alarm 覆盖门禁 / ACK 等待；子进程已退出但管道未关闭，最多再等 1 秒并记异常。此为有界监督策略，不承诺不可中断内核调用、进程启动或文件 I/O 的硬实时终止；未回收必须如实报告，不自动重跑或杀系统服务。

独占输出为 0700 目录内 0600 单链接文件；已有输出目录即拒绝，每阶段 ACK 前检查目录 / 文件身份、权限和空文件状态。stdout 帧限量解析，stderr 只记出现与否且出现即失败，不落盘原文。结果最多 64 KiB，只保留比较枚举、局部编号、计数、固定错误 / 数值 OSStatus、所拥有子进程的退出 / 信号 / 耗时。原始 attributes、partition、requirement、应用路径、description 和 persistent ref 不写日志。成功须同时满足二轮比较及文件复核、写入 / fsync / 读回验证、子进程已回收、CLI 退出 0 和 `result_verified=true`；部分文件不是成功证据。

### 合成测试与审阅证据

35 项合成测试通过，包含各调用失败的精确前缀 / 尝试计数、UI 前置失败、身份 / label / persistent ref 漂移、完整 ACL / 非 Decrypt 要求漂移、partition 异常与限额、要求匹配 / 不匹配 / NULL、NULL / 空应用列表、协议严格类型、隐私哨兵、输出异常、TERM / KILL 回收及退出后遗留管道。首轮沙箱中 Xcode Python 启动器的额外 stderr 导致三项监督测试失败；保留原始日志，按权限流程在沙箱外使用相同合成入口复跑通过，没有过滤 stderr 掩盖问题。最后复跑仍为 35 项通过。

实现后的静态复核发现并修正 CF / ARC 生命周期、尝试计数、DOCTYPE 位置、退出后管道等待、JSON 布尔版本 / 深层解析、增长文件读取上界及回收后结果生成等问题；相关可合成场景进入回归。原生函数白名单和固定 query 内容完成静态核对。没有把测试替身成功当成系统 API 兼容证据。

实现冻结时尚未验证：私有 SPI 在当前 OS 的可用性 / ABI、legacy SecItem 的组合返回形状、当前 partition 编码、缓存行为、禁 UI 实际效果和冻结 app 的真实静态校验结果。公开源码与运行版本差异保持；本地清单也不是抗同用户篡改的签名。**本地实现与准备已完成，当时真实访问未执行、未获准备批次授权**；后续只可按冻结操作单另行精确授权，遇未知结构 / 拒绝 / 意外提示即停止，不能因本批通过而解释 D02 的实际内部路径。

生产四次 secret、十二次 attributes、25 次 User domain 及唯一性、identity、label、双读、首尾 key 一致性和最终对象 / 数据库复验全部保持。P1-S05、其余原生窗口及历史第九次提示继续待验。没有 GUI、真实 key / ACL 操作、提交、push 或其它远程写入；忽略产物保留，当前仅两份正式文档存在未提交改动，`dev` 保留三个未推送提交。

本批完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 已离线通过：270 文件基线、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 按父测试约定执行。末次事实补记后再次运行 Python 仓库基线与 `git diff --check`；补查首次误用系统 Python 3.9，因仓库 notices 检查所需 `tomllib` 缺失失败，改回完整检查所用的默认 Python 3.14.5 后通过，原错误日志保留。M01 监督器自身仍以 Python 3.9.6 合成验证，不需要该模块。本批未单独重跑 45 项检查器回归、默认 desktop 构建、真实 Keychain / 签名 API 或 Windows / Linux 验证；此前证据不扩大。全部合成测试和仓库检查进程已退出，无原生 helper / GUI / 长期服务。

## 2026-10-18 M01 单次执行与身份前置停止

项目所有者明确授权按冻结操作单执行一次固定目标元数据核对，允许真实 Keychain 元数据访问，禁止 secret 读取、key / ACL 修改、提示允许、重试、GUI 和 push，继续保留生产四读及全部认证检查。执行前核对工作区仍为 `dev` 领先远端三个提交、两份既有文档改动；本地 21 份冻结文件和离线门禁通过，唯一原生输出目录尚不存在。只执行一次冻结原生命令，没有修改 helper、目标或冻结清单。

**本次结果为 `incomplete`，不是元数据核对通过。** 监督器退出码 2，终端回执 `result_verified=true`、`child_reaped=true`；原生子进程退出码 2，监督耗时 127 ms，无 stderr、未发送 TERM / KILL。项目所有者确认没有出现系统提示。

| 观测 | 实际结果与边界 |
| --- | --- |
| 停止阶段 | `identity`，`static_code_failed`，OSStatus `-67056` |
| 前置条件 | 按冻结控制流，普通文件前置检查、禁 UI 设置 / false 读回、SPI 符号及来源检查、thin arm64 header 检查已通过；SPI 获取已存 requirement 的实际调用尚未发生，不能据此证明 ABI / 条目兼容性 |
| 错误含义 | 本机 SDK 的 `CSCommon.h` 定义为 `errSecCSResourcesNotFound`：代码对象未找到签名所要求的资源。静态对象创建和完整校验共用本 helper 的错误枚举，现有回执不能区分具体调用，也不能确定资源布局、对象路径选择或其它根因 |
| 查询预算 | domain、item、access、ACL list 实际尝试计数全部为 0；尚未解析目标 domain 或取得条目、partition、可信应用要求 |
| 数据与修改 | 没有 secret 读取、key / ACL 修改或业务动作；没有获得 candidate present / absent 或要求匹配结论，不解释 D02 的内部授权顺序 |
| 收尾 | 唯一子进程已真实回收；0700 输出目录内只有 0600 单链接结果文件，写入 / fsync / 读回验真通过。原始回执与批次索引保留在本机忽略目录，没有清理或重跑 |

监督器内的普通文件后置检查未报告漂移，随后又仅用文件读取复算 21 份 helper 冻结文件、原 29 份产物、11 份源码、原 freeze、M01 目标单和 D02 targets，全部保持；没有为补证再次调用签名 / Keychain API。既有四次 secret、十二次 attributes、25 次 User domain 及全部认证检查未改变。CLI 设置的禁交互状态随进程结束，不宣称 OS 内部缓存 / 访问记录已撤销。

本次单次授权已消耗并收尾；不得直接换路径、重签、跳过 resource 校验、降低静态校验或修改 helper 后重试。下一步仅建议离线核对冻结签名资源布局与 static-code 路径契约，先形成可审阅判断；任何修订及新的原生尝试另定精确范围。当前 metadata、partition 和已存要求均未知，P1-S05、其它原生验收及历史第九次提示继续待验。无 GUI、提交、push 或远程状态修改，运行产物保留，两份正式文档继续未提交。

本次结果文档的完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 离线通过：270 文件基线、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 按父测试约定执行；事实补记后复核 Python 仓库基线与 `git diff --check`。本次没有重跑 M01 合成套件、重建 helper 或默认 desktop，也没有其它平台 / GUI 验证。全部本次命令与检查进程已退出，未提交 / push，`dev` 继续领先远端三个提交。

## 2026-10-18 M01 签名资源布局与静态路径离线定位

项目所有者授权继续离线核对冻结签名的资源布局和 helper 的静态代码对象路径，定位 `-67056` 并给出最小修正方案；不重签、不跳过校验、不再次访问 Keychain、不启动 GUI、不 push，保留四次读取及全部认证检查。本批没有执行原 M01 helper / supervisor，没有改变原冻结包装、helper 或一次运行结果。

### 原因与直接证据

**原因已定位为静态代码对象范围与签名形态不一致：冻结 Mach-O 是独立的 linker-signed ad-hoc executable，放入手工 `.app` 包装后，M01 传入其 executable 路径仍被系统识别为整个 bundle；完整 bundle 校验遇到缺少资源封装的布局而返回 `-67056`。** 这不是已证明的二进制损坏，也不是已证明某个曾经签封的资源被删除。

当前冻结包装只有 `Contents/Info.plist` 和 `Contents/MacOS` 下的 executable，没有 `_CodeSignature/CodeResources`。冻结时的保存记录已明确 `linker-signed`、`Info.plist=not bound`、`Sealed Resources=none`。本轮直接读取 Mach-O / SuperBlob / CodeDirectory，确认只有一个 CodeDirectory、没有 CMS、flags `0x20002`、special slots 为 0；15,490 个 4 KiB 代码页的 SHA-256 全部匹配，完整文件摘要、identifier 和 CDHash 也与冻结证据一致。此页摘要检查不冒充系统完整验证。

本机 SDK `SecStaticCode.h` 明确说明：传入 bundle 内主 executable 的 URL，通常仍识别整个 bundle；`SecCodeCopyPath` 对该对象返回 bundle 根。专用离线探针只引用四个公开静态代码函数：`SecStaticCodeCreateWithPath`、`SecCodeCopyPath`、`SecCodeCopySigningInformation` 和 `SecStaticCodeCheckValidity`，没有 Keychain API、私有 SPI、目标程序启动或签名功能。以三个固定文件路径做一次对照，全部使用 `kSecCSNoNetworkAccess`，没有任何 executable / resource 跳过标志：

| 输入 | 创建对象 | 系统识别的对象路径 | 完整静态校验 |
| --- | --- | --- | --- |
| 原包装内 executable | 成功 | 原 `.app` 根 | `-67056` |
| 原 `.app` 根 | 成功 | 原 `.app` 根 | `-67056` |
| bundle 外逐字节一致的独立副本 | 成功 | 副本文件自身 | 成功 |

三者均返回与保存记录一致的 identifier / CDHash 和 `0x20002` flags，无 CMS、证书或 TeamIdentifier；探针退出码 0、无 stderr。这次对照将复现的失败定位到 `CheckValidity`；历史 M01 回执仍保留共用错误枚举的事实，不改写为当时已有逐 API 证据。把原传入路径简单改为 `.app` 根不会修复错误。整个过程没有联网、重签、Keychain 查询或原 M01 重试，静态探针已退出。

### 最小修正方案及语义停止线

以下记录离线定位阶段的方案；当时没有修改 helper 或新增真实访问资格，后续实现见下一节：

1. **分离原目标与校验副本**。保留原 bundle / executable / `Info.plist` 和原 M01 证据；下一版单独冻结 bundle 外的同字节、单链接副本。原 executable 继续承担来源、完整摘要、thin arm64 和首尾稳定性检查；副本须与原文件逐字节 / SHA-256 一致，首尾复核身份，完整离线静态校验必须通过，不允许错误后临时换路径。
2. **同时修正路径与身份断言**。对副本建立 static code，严格检查 `SecCodeCopyPath` 和 `kSecCodeInfoMainExecutable` 都指向该副本，再核对原冻结 identifier / CDHash、ad-hoc / linker flags、无证书 / TeamIdentifier。不能只替换 `CreateWithPath` 参数而保留旧 main-executable 路径断言；固定 Keychain target、原 app 和 candidate 来源均不改变。
3. **限制结果口径**。结果只能说明完整验证的 standalone Mach-O 副本；不能表示原 bundle 或其 `Info.plist` / 资源已被签名保护。已存 requirement 若在副本环境评估，也只是 copy-scoped observation：bundle metadata / 路径上下文可能不同，尤其不能把副本 mismatch 当作原 app 不受信任，不能沿用“全部不匹配且 candidate absent”的原因判据。在没有可靠等价性证据前，原 app 要求匹配结论保持 unknown。
4. **拆分错误并补验证**。区分静态对象创建与完整校验失败，保留 OSStatus；同步配置、文件 guard、共享输出协议、监督器白名单与合成用例，覆盖副本漂移、识别对象路径异常、bundle-dependent 要求不能升级为原 app 结论等边界。下一版另行审阅和冻结，不复用已消耗的原生授权。

该方案改变 M01 的静态比较对象和结果范围，须先确认该诊断范围再实施；它不是让原 bundle 的失败无声转为成功。若仍要求原 `.app` 整体完整有效作为前提，在本轮“不重签、不改冻结包装、不降低校验”的约束下，没有仅靠换路径即可通过的修正，继续失败关闭。生产四次 secret、十二次 attributes、25 次 User domain 及全部认证检查不受该方案影响。

精确路径、普通文件分析、静态探针源码 / 结果、本机 SDK 头文件摘要和最小方案只留本地独立忽略目录；原 21 份 helper 冻结文件、D01 29 份产物 / 11 份源码、目标单 / freeze 及 M01 单次结果已在收尾复核保持，副本与原 executable 的完整摘要仍一致。没有应用修正、重新冻结旧清单、提交或 push；当前 partition / 已存要求仍未取得，P1-S05 与其它停止线保持。

本批诊断文档的完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 离线通过：270 文件基线、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 按父测试约定执行；事实补记后再次核对 Python 仓库基线与 `git diff --check`。本次没有重跑 M01 合成套件、默认 desktop 构建、Keychain / GUI 或其它平台验收。诊断探针与检查进程已退出，仅两份正式文档未提交；`dev` 保留三个未推送提交，未 push。

## 2026-10-18 M01 副本范围 v2 实现与重新冻结

项目所有者明确授权实施原目标 / 校验副本分离、限定副本结果、拆分创建与校验错误，完成合成测试、审阅及重新冻结；保留原冻结件、单次回执、四次读取和全部认证检查，禁止本批 Keychain 访问、重签、跳过校验、GUI 和 push。本版置于新的独立忽略目录，未改写旧 helper、freeze、包装或 `-67056` 单次回执，也未把新方案实施视为原 app 资源签封问题已修复。

### 实现与结果范围

- **双路径绑定**：`M01OriginalExecutable` 仍是原 D01 包装内 executable；`M01ValidationCopy` 是独占创建的 bundle 外同字节副本。原生和监督器两种文件 guard 均核对摘要、逐字节相等、不同 inode、单链接、无 alias、同用户、0700 副本及仅含该文件的 0700 目录；记录首尾文件 / 目录身份，漂移即停。原生 guard 共绑定 45 份输入，包含旧 freeze / 单次结果 / 运行回执；监督器还复核旧 21 份 helper 冻结文件。
- **完整静态身份**：共享 `static-identity` 模块只对预先冻结副本建立 static code，核对原件与副本 thin arm64、`SecCodeCopyPath` / main-executable 路径、原冻结 identifier / CDHash、精确 `0x20002` flags、无 CMS / 证书 / TeamIdentifier；完整校验仍仅使用 `kSecCSNoNetworkAccess`，没有 skip 标志，也没有原 bundle 失败后的路径 fallback。原 native 查询、ACL、domain、candidate 和超时主函数代码保持原样。
- **协议 v2**：所有成功 / 未完成帧及监督器终态固定声明 `comparison_scope=standalone_macho_copy`、`original_app_authorization=unknown`、`original_bundle_validation=not_performed`。身份成功为 `offline_static_copy_match`；要求结果仅为 `copy_requirement_match` / `copy_requirement_mismatch` / `no_stored_requirement`。协议拒绝旧版帧、旧无范围匹配字段或任何原 app 已授权 / 已拒绝、bundle 有效的声明。
- **错误分离**：创建、完整校验、对象路径读取分别报告 `static_code_create_failed`、`static_code_validate_failed`、`static_code_path_failed`，保留数值 OSStatus 和阶段，不打印原始错误文本。其它拒绝 / 未知结构仍失败关闭，不新增重试或 fallback。

`complete` 只表示副本范围的观察流程完成。副本匹配 / 不匹配不能推导原 app 授权，原包装的 `Info.plist` / 资源也未因此被签封；原 app 的要求结论始终 unknown，不恢复“全部不匹配且 candidate absent”的因果判据。原 bundle 整体校验的 `-67056` 事实保留，P1-S05 或原生验收未因此通过。

### 验证、审阅与冻结

47 项合成测试通过：保留原 35 项失败关闭 / 计数 / 稳定性 / parser / 输出 / 超时覆盖，补充两种文件 guard 对副本差异、摘要漂移、路径 alias、权限与兄弟项的拒绝，创建 / 校验错误分离，身份路径异常，bundle-context 要求不能升格为原 app 结论，以及旧命令 / 旧协议 / 伪造授权结果拒绝。测试只使用合成 backend、临时普通文件和短时子进程；带 Keychain adapter 的新 native helper 只编译，未执行，包括无参数门禁。

独立的离线身份程序复用本版真实 fileguard / static-identity 模块，只引用四个公开静态代码函数，没有 Keychain 入口或私有 SPI；45 份输入及副本目录身份前后检查、完整副本校验和原冻结签名字段比对均通过，退出码 0、无 stderr。该证据只覆盖副本完整静态身份，不覆盖 Keychain 查询或原 app 授权。

实现后由主代理单独进行静态审阅，非第二位人员 / 子代理评审；核对新结果范围、错误映射、CF / ARC 对象生命周期、文件边界和原查询代码保持，移除抽取身份模块后遗留的无用工具函数，并补充伪造 app 授权结果的端到端回归。本地准备范围内未发现剩余阻断项；私有 SPI 实际 requirement ABI、当前 ACL / 返回结构、composite native 路径、查询时禁 UI 效果和缓存刷新仍未验证，不能把合成通过解释为这些边界已通过。

新清单冻结 24 份源文件、编译产物、副本、测试日志、审阅结果和操作单；`freeze.json` SHA-256 为 `a268e45aa7915b521a3980bd9836920d178f981934be705b8d7872063004c1c0`。精确绝对命令、v2 gate、固定输出和目标绑定仅存本地操作单；普通文件 `--check-offline` 已实际通过，回执 `native_started=false`，新 `native-01` 输出目录不存在。新的原生命令使用独立 flag，不接受旧命令，也不能沿用已消耗的旧 M01 授权；本批未执行。

新操作单保留 30 秒监督超时、TERM 后 5 秒 / KILL 后 5 秒回收、native 35 秒 alarm、退出后遗留管道最多 1 秒及真实回收状态；不扩大为系统服务终止。输出仍为独占 0700 目录内 0600 文件、最大 64 KiB，逐阶段 ACK 检查、严格协议、写入 / fsync / 读回验真；原始元数据只留内存，成功不能绕过二轮观察和首尾复核。

生产四次 secret、十二次 attributes、25 次 User domain，以及唯一性、identity、label、双读、首尾 key 和最终对象 / 数据库认证检查全部保持。没有新增依赖、修改 lockfile、重签原件 / 副本、启动 GUI、提交或 push；新产物与旧证据均保留在本机忽略目录，两份正式文档继续未提交，`dev` 保留三个未推送提交。

本批完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 离线通过：270 文件基线、fmt、locked Clippy、25 个 suites 共 398 项 Rust tests，11 个 ignored helper 按父测试约定执行。收尾复核新 24 份冻结文件、旧 21 份 helper 文件 / freeze、原单次结果与运行回执、D01 29 份产物 / 11 份源码均保持；事实补记后再次运行 Python 仓库基线与 `git diff --check`。没有默认 desktop 构建、真实 Keychain / GUI 或其它平台验证；全部合成、静态身份与仓库检查进程已退出，无长期服务。
