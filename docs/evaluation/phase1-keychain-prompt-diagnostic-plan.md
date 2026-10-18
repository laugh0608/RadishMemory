# Keychain 重复提示有限诊断与原生关闭验收方案

日期：2026-10-18。状态：既有证据已离线复核；随后推进受限观测补丁，本机合成验证结果见末节。未执行新的原生验收。

本批由项目所有者授权复核既有证据、完成有限诊断方案和原生验收操作单；明确禁止启动 GUI 或访问真实 Keychain。本文承接 [provider 记录](../implementation/phase1-source-vault-key-provider.md#2026-10-13-secitem-候选的单次原生结果)与[原生验收矩阵](phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)，不替代密钥读取契约，也不构成未来系统操作授权。

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

### D01：未来一次空库调用实例观测

D01 与下面 N 批相互独立，不能捆绑授权。D01 的补丁进展见末节；原生运行仍缺本次专用的冻结构建 / dSYM 和精确操作单，因此不提供可立即执行的命令。启用参数为完整三项隔离参数加 `--acceptance-key-read-trace`；执行前还须冻结构建身份、确切新 trace 路径并完成逐事件读回预演，再单独确认系统操作范围。

未来运行最多启动一个指定 GUI、一次空库 `OpenEncrypted`；只读取唯一既有合成 slot，不增加 security metadata / ACL 探针。预计 20–35 分钟，另留 5 分钟收尾。每提示先确认请求者与目标、保持未选择，保存最新 call ID / 阶段与一次最多 1 秒、20ms 间隔的该 PID sample，再由用户单次允许。最多八次 Allow Once；第九次只作至多一次定位后交接拒绝。出现目标不明、无完整 trace 或未知阶段时立即停止允许并交接，不要求用户事后回忆总数。

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
