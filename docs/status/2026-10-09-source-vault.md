# 2026-10-09 Source Vault 日终记录与明日事项

日期：2026-10-09（Asia/Shanghai）。按本地所有 refs 的当天提交时间回顾，共 4 个提交；本页随后单独作日终文档提交。现行阶段、顺位和停止线以[当前状态](current.md)为准。今晚到文档提交为止，不安排继续开发或自动运行。

## 今天的全部提交

| 提交 | 实际变更 | 证据与边界 |
| --- | --- | --- |
| `347058f` | 新增三项 Unix 文件系统权限故障 worker 回归；记录原请求重试、重启精确放弃及原删除授权恢复的 macOS GUI 验收 | 自动化使用合成 key，真实撤销对象目录写权限后复原，不启用协调器中断回调；全仓 Rust tests 从 355 增至 358。对应原生路径使用既有合成库和真实测试 key，不能替代 commit / evidence 中断窗口 |
| `7724ac2` | 新增一项真实 SQLite 竞争会话回归；记录外部进程持锁下 GUI Export / Refresh 失败与显式恢复，以及 Keychain Access 只读核对 | 全仓增至 359 项 Rust tests。自动化竞争 reader 与 worker 位于同进程，独立 helper 的原生持锁证据另记；当批未实现双 GUI 独立绑定，也未取得真实系统取消 |
| `9e184ec` | 准备不同 bundle ID 的 A/B/Prompt 包装、只读 ACL metadata 工具与精确操作单 | 纯文档提交；准备本身不代表 GUI 已执行、Prompt 已重签或系统拒绝通过 |
| `945aec1` | 记录 A/B 独立定位、交替版本刷新、四份精确导出，以及 Prompt 许可纠偏与真实系统取消验收 | 首次误点“始终允许”后，追加授权只撤销新增项；触发前重新交接，空合成库打开返回 `KeyStoreCancelled` 并失败关闭，原 A 对照通过。没有原始 OSStatus 运行态采集，不扩大到其它错误或操作阶段 |

日终开始时 `dev` 工作区干净，领先本地 `origin/dev` 4 个提交。未刷新远程状态，tracking ref 不代表实时远程结果；本次只作本地文档提交，没有 push、PR、tag、发布或部署。

## 代码—文档复核与更新

| 核对代码 / 证据 | 结论与文档处理 |
| --- | --- |
| `worker/tests.rs` 与 `worker/tests/filesystem_failures.rs` | 三项权限回归受 `cfg(unix)` 约束；断言同一 staging 密文字节重试、重启后仅放弃 pending update、删除沿用原请求且保留无关 lineage。不能记作 Windows 权限测试或真实系统 key 故障；宿主记录已准确区分自动化与原生范围 |
| `worker/tests/locking.rs`、worker 错误快照及 backend 原请求持有 | 锁期间 Export / Refresh 失败、无导出文件、无额外 key load；失败连接不释放原锁，InspectRecovery 不恢复旧视图，显式 Refresh 后恢复原 source 与精确导出。与当前生产逻辑一致；未发现需要在日终修改生产代码的文档偏差 |
| macOS provider 的 OSStatus 映射与 Prompt 实际界面 | 静态代码将 `-128` 映射到 `KeyStoreCancelled`；现场只观测到后者，不能把按钮“拒绝”写成已验证 `KeyStoreDenied` 或把静态映射当作原始状态采集。provider 与隐私模型的“全部真实取消仍待验”已过期，更新为有限范围通过 |
| 原生恢复、锁竞争、双实例与许可撤销记录 | 架构更新现行证据摘要，ADR 0008 追加带日期的后续结果；原日期下的准备、未覆盖及中间许可状态保留，不改写当时事实。最终 Prompt 新增许可已撤销，原四项及 key 保留，不声明完整 ACL 二进制或代码身份相同 |
| application、maintenance、宿主、原生验收计划与路线图 | application / 宿主 / 路线图已记录本日结果，无需重复改写；maintenance 的系统失败及默认 v6 边界仍成立。空库 OpenLibrary 的取消证据不关闭 maintenance 取消、已有正文视图撤回或写操作中途失败 |
| 当前状态与文档索引 | 后续事项路由到本页，明确 10 月 10 日建议与今晚停止安排；不再把已通过的原请求恢复、双 GUI 交替和空库打开取消重复列为下一步 |

今天的代码变化仅为四项回归及测试模块接线；没有生产行为、canonical schema、密码 profile、依赖、许可证、数据所有权或信任模式变更。日终只修正文档证据与路由，不修改测试或检查器，也不构成独立密码学审计或完整产品复审。产品范围、记忆模型和 RadishMind 边界无需人为重写；默认桌面仍为 v6，显式对象加密不覆盖 SQLite / FTS 完整明文，PDF / 图片仍受原停止线约束。

## 原生证据与环境交接

- 权限故障批次完成原 staging bytes 重试、重启精确放弃和原删除授权恢复；目录权限已复原，三轮 GUI 正常退出，两份 Completed 回执及零活动资料已复核。保留合成输入、导出与原 key。
- 外部锁批次完成 Export / Refresh 失败关闭、释放锁后显式恢复与两份精确导出；两个 GUI、持锁 helper 和 Keychain Access 已退出。当批只读访问控制核对没有修改许可。
- 双实例批次 A/B 对同一合成库完成 v1 / v2 交替刷新，四份历史 / 当前导出与原始 bytes 一致；本轮两版本 lineage 经 GUI 删除后，十组件 Completed、零活动 references / bodies / fragments / tips / FTS、零对象 / staging 已复核。
- Prompt 首次误允许新增一项常许许可。后续按精确追加授权撤销新增项；以 authorizations 集合匹配 ACL，原四项的数量、路径摘要及 prompt flags 保持，不能把返回顺序变化当作内容漂移，也不能据此声称完整 ACL 身份审计。再次打开在空合成库返回真实 `KeyStoreCancelled`，数据库 / profile bytes 与 ACL metadata 保持；原 A 仍可打开并 Verify 零对象 / 零 pending。
- 双实例阶段的 A/B/Prompt 三轮及纠偏后的 Prompt/A 两轮均正常退出，最后进程复核无本轮 desktop 或 Keychain Access 残留。部分运行有原生 TSM CapsLock 诊断；已检查的合成标记未命中，不外推到 secret 的任意编码、系统崩溃报告或 OS 内部副本。
- 忽略目录中的输入、导出、证据、旧数据库 / 对象备份及三个临时包装继续保留；仅 Prompt 副本完成授权的 ad-hoc 重签，原包装和 A/B 未重签。原测试 key 保留，新增 Prompt 常许许可已撤销。备份、导出和最小审计不计作已删除；本日终不启动 GUI、访问系统凭据或清理这些产物。

完整观察、偏差处理和精确证据边界见[宿主记录](../implementation/phase1-source-vault-host.md)，场景判据见[原生验收计划](../evaluation/phase1-source-vault-native-acceptance.md)。不能据本日结果关闭 P1-S05、全部 P1-SF 场景或 Q01 至 Q08。

## 验证

当日实现 / 原生批次最后一次完整离线检查通过：256 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，25 个 suites 共 359 项 Rust tests；11 个 ignored helper 由父测试调用。较早三项权限回归批次另通过默认 desktop 构建。各批测试数量是递进基线，不相加为独立测试总数；P1-F17 合成本地端口的沙箱限制已按权限流程处理。许可纠偏后只改系统许可和记录，最终文本 / 链接检查及 `git diff --check` 通过，没有以修正文档冒充重复原生验证。

日终文档修改后，`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：257 个仓库文件、notices、fmt、locked Clippy 与 25 个 suites 共 359 项 Rust tests，11 个 ignored helper 由父测试调用。按已确认的合成本地端口限制通过权限流程在沙箱外离线运行，检查已正常退出；日志保留在任务临时文件中。补记结果后再次通过文本 / 链接检查和 `git diff --check`。未重复未受影响的 Python 检查器回归、M0 fixture、doctest、默认构建、GUI、系统密钥库、Windows / Linux、VM 或远程验收。

## 明日事项（2026-10-10）

1. **核对交接并限定下一切片**：先读 Git 状态、本页与当前状态；确认保留的合成目录 / 唯一测试 slot。已完成的隔离入口、正常链路、受损库恢复、三条权限故障恢复、锁失败恢复、双 GUI 交替和空库取消不重复开发或重新创建 key。
2. **优先准备剩余宿主窗口**：对照既有测试列出 busy 关闭竞争、提交 / evidence 中断、成功写入后刷新失败的触发方式、持久状态、UI 判据和恢复 / 清理条件。明确哪些已由合成回归证明、哪些仍需原生观察；默认构建不能稳定构造的窗口保留合成注入边界，不用假 checkpoint 或不受控强制退出替代证据。若需要生产代码修改或新增 seam，先解释具体缺口与影响。
3. **按必要范围补系统失败阶段**：空库打开的取消已覆盖；有正文旧视图撤回、capture / export 阶段以及其它系统错误另列范围。只选择可控、安全且有明确恢复方式的场景，不删除唯一 key 或锁定整个用户 keychain。系统提示必须在触发前交接具体按钮，权限保存与拒绝读取分开；无法安全复现的分支保持待验。
4. **形成集中跨平台检查点**：为上述剩余切片写明完成条件，再推进 Windows / Linux 当前代码的 locked 编译、合成测试及约定的真实 provider / 宿主运行；分别记录架构、文件系统、普通用户权限、日志和清理。不把旧 filesystem 证据或 macOS 结果视为当前跨平台通过，也不因无法穷尽全部原生故障而无限扩大前置工作。
5. **保留独立质量与契约事项**：中文缺字 / 找回、201 条目录、性能、正文定位及默认 v6 failed-open 按质量计划推进；字体引入先明确来源、版本、许可证和分发影响。`evidence_digest` 覆盖与历史兼容先单独评审，不借验收静默改变协议。
6. **延续停止线和授权边界**：只用合成资料，不进入 PDF / 图片、模型、同步、整体备份恢复或签名发行。新的系统操作、依赖及远程动作按精确目标另行授权；测试 key、备份和导出的彻底清理也须明确范围，不把密钥销毁当普通回滚。

以上仅为明日工作建议，不创建提醒、定时任务或后台开发。
