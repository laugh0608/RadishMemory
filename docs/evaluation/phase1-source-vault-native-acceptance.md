# P1-S05 隔离原生验收计划

日期：2026-10-13。状态：隔离入口已实现，macOS 最小正常链路、派生损坏重启恢复与密文损坏拒绝已完成；2026-10-09 另按精确授权完成权限故障下的原请求重试、重启精确放弃与原删除授权恢复，以及外部 SQLite 持锁下的 GUI 失败恢复和唯一测试 Keychain 项的只读核对。随后双 GUI 交替及四份精确导出通过；经精确撤销误加的 Prompt 常许许可并重新交接操作，真实系统交互返回 `KeyStoreCancelled` 的显式打开失败关闭已验收。2026-10-13 已修复 busy 结果与关闭同帧到达及接受关闭后继续派发动作的问题，并补齐 headless UI / worker 退出回归；本次未启动原生窗口。其它系统错误 / 操作阶段、其余原生失败窗口与集中跨平台检查仍待执行。实际范围见[最小闭环](../implementation/phase1-source-vault-host.md#2026-10-06-macos-最小真实闭环)、[恢复原生证据](../implementation/phase1-source-vault-host.md#2026-10-09-原请求恢复的-macos-原生验收)与[锁及凭据观察](../implementation/phase1-source-vault-host.md#2026-10-09-原生锁竞争与-keychain-只读核对)，本计划不等于全部场景已通过。

本计划承接[桌面宿主接入](../implementation/phase1-source-vault-host.md)与[ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)。先验收 macOS 当前代码，再进入 Windows / Linux 集中检查点；合成 provider、headless 测试和历史平台证据分别保留边界。

## 已实现的隔离入口

正常无参数启动继续使用默认目录与 SQLite v6。原生验收使用同一个默认构建产物与 production worker / provider，只显式选择另一目录和既有 host profile：

```bash
./target/debug/radishmemory-desktop \
  --acceptance-root "$ACCEPTANCE_DATA" \
  --acceptance-namespace "$ACCEPTANCE_NAMESPACE" \
  --acceptance-device "$ACCEPTANCE_DEVICE"
```

三个参数必须各出现一次，顺序不限；未知、重复、缺值、不完整组合和不合法身份均拒绝，不回退默认启动。namespace / device 沿用 `namespace-` / `device-` 加 32 位小写 hex，profile 沿用 `radishmemory.phase1-host-profile/1`，不增加 canonical schema、key slot 映射或依赖。

- `ACCEPTANCE_DATA` 是预建的绝对目录；拒绝父目录遍历、目录或祖先符号链接，以及默认资料库目录的自身、子目录和祖先。默认路径尚不存在时，也解析最近存在的祖先以排除别名重叠。
- Unix 验收根必须没有 group / other 权限，准备时使用 `0700`；入口不自动改权限。Windows ACL 与 reparse 行为仍须平台验证。
- `host-profile-v1.txt` 必须已经存在、格式合法且与两个显式身份完全相同；不自动生成、修复或替换。数据库文件的符号链接 / 非普通文件也拒绝。
- 参数解析、worker 创建及每条命令执行前复验目录和 profile；重开、原请求重试、failed-open 维护复用同一目标。profile 丢失或换身份时隐藏旧视图并失败关闭，恢复原 profile 后才可继续。
- 原生 UI 工具绑定后先检查窗口身份；正常关闭后通过原进程退出状态确认，不再查询已关闭应用的界面，以免工具重新启动无参数入口。发现默认窗口立即正常关闭并记录副作用。
- 窗口标题和界面标记 Synthetic acceptance；不显示路径、namespace、device 或 slot。无输出 logger / panic 策略提前到原生窗口创建之前安装，worker 内仍复验安装结果。

这提供显式目标绑定与运行前检查，不是抵抗同用户恶意并发替换、硬链接别名或所有 OS 文件别名的文件系统沙箱；不扩张已有 adapter 的保证。文件选择器仍沿用一次性选文件授权，验收操作只选择本任务的合成文件。不能复制日常库 profile 作为“合成”身份。

## 准备与授权材料

准备阶段只在本任务专用、已忽略的目录创建合成材料，不启动 GUI，不访问系统凭据。每批建立独立的 `data`、`input`、`export`、`evidence` 目录，创建随机合成 namespace / device 并写入既有 profile；`data` 中不预置数据库或 key。输入至少包含一个带 ASCII 检索词的 UTF-8 / 中文 Markdown 版本及其更正版本；中文分词质量不因 ASCII 找回通过而关闭。

运行前在仓库外或已忽略的任务操作单填写并核对：

| 项目 | 必须明确的内容 |
| --- | --- |
| 精确目标 | 实际绝对目录、输入 / 导出文件、namespace、device、二进制路径与摘要 |
| 系统 slot | macOS User domain 默认 keychain；generic password；service `io.github.laugh0608.RadishMemory.source-vault`；account `v1:<namespace>:<device>`；label `RadishMemory Source Vault key` |
| 执行动作 | 启动 / 关闭 / 重开本次 GUI；只查询该 slot 是否已存在；经 application 初始化协调器创建或复用、设置固定 label、读回验证；不直接注入 secret |
| 预期影响 | 写入隔离 SQLite、对象 / staging、合成导出和一个系统凭据；可能弹出 Keychain 授权提示，需要用户处理；无服务端、端口、网络、同步或真实资料 |
| 时长 | 最小闭环预计 30–45 分钟；系统提示、失败定位和恢复扩展另按实际评估 |
| 收尾 | 正常退出本次窗口并等待 worker join；保留隔离目录、证据和 slot 以便复验。失败同样保留，不自动删 key 或对象，不停止用户原有进程 |

随机身份降低碰撞概率但不证明 slot 空闲。授权后先精确检查；如出现非本批已记录的既有凭据，停止初始化，不复用或覆盖。不得输出 secret，系统工具不得使用显示密码的选项，不收集全 keychain dump。

当前基础授权范围不含删除 key、锁定整个 keychain、修改默认 keychain / ACL、系统设置、签名安装或远程操作。关闭进程能停止本批运行，不能撤销已发生的迁移或凭据创建；如需完全清理，先确认合成库不再需要，再单独明确授权精确删除 slot 与任务目录，不能将销毁 key 称为普通回滚。

## macOS 验收顺序与判据

| 顺序 | 操作 | 观察与通过条件 |
| --- | --- | --- |
| 1 | 精确 slot 预检后，带完整参数启动 | Synthetic acceptance 标识可见；隔离根创建 v6；profile 未被替换；无真实库访问 |
| 2 | 导入初始合成 Markdown，检索 ASCII 标记 | 可浏览、回源；记录初始来源 / 版本；不将该结果当作中文检索缺口已修复 |
| 3 | Prepare key → Migrate / resume bodies → Open encrypted objects | 每步按 UI 确认，系统提示由用户处理；创建 / 复用同一 slot，成功认证打开迁移资料；仍显示 FTS 完整明文边界 |
| 4 | 加密导入另一合成文件、更新既有来源、检索、导出历史版本 | 新版本关系正确；旧版导出与原始合成 bytes 精确一致；不重读外部原件冒充历史导出 |
| 5 | 关闭重开并显式 Open encrypted objects | 启动时旧入口拒绝已迁移库属预期；显式加密打开成功；原 profile、slot、版本与正文可复验，无新 key |
| 6 | Verify、显式删除本批来源、查看 evidence | 按真实组件结果记录 completed / partial / failed；删除后目录 / 搜索不再召回；原输入和此前导出仍在 |
| 7 | 正常退出、核对诊断与产物 | 本批 worker 退出；仅保留约定目标；诊断不含合成正文、身份或 secret；记录原生 stderr 与系统提示的实际观察范围 |

验收证据记录精确构建、场景、操作顺序、真实结果、合成文件摘要、是否产生系统提示及清理 / 保留状态；不记录 secret，不用“没有看到提示”替代取消 / 拒绝测试。日志无泄露检查不等于系统崩溃报告、交换区或第三方内存已清理。

## 后续恢复和失败扩展

按可安全构造的状态单独安排：busy 下重复动作、关闭时原快照提示、提交后刷新失败、原请求重试、重启精确放弃、原删除授权继续、failed-open 派生维护、多实例锁，以及真实提示的取消 / 拒绝。需要故障 seam 的场景必须标记为合成注入，不将默认 production 构建无法构造的状态记为通过。

2026-10-09 先完成恢复场景的本机自动化预演：仅在测试创建的 Unix 合成对象目录暂时撤销 owner 写权限（`0700 → 0500`），由真实 hard-link 发布 / unlink 删除返回失败，再恢复原权限。三项回归使用既有合成 key provider，不启用 coordinator 中断回调、不手写 checkpoint，也不修改 schema。随后项目所有者批准精确原生操作单，复用既有合成库 / 唯一系统 key / 默认 production 二进制完成下表 GUI 路径；两类证据分别记录，新增系统操作仍须具体授权。

| 2026-10-09 已执行的原生场景 | 构造及判据 | 证据边界 |
| --- | --- | --- |
| 发布失败后原请求重试 | 专用对象目录临时设为 `0500`，保留可写 staging；失败后原快照留在 worker，Inspect 不清理，关闭提示可取消；替换外部原件、恢复目录权限后，Retry 发布原 staging 密文，导出仍等于最初 bytes | 是受控文件系统权限故障，不是 publish 后 SQLite commit 失败或真实断电 |
| 重启后精确放弃 | 已提交版本的更新在发布时失败；确认关闭、恢复权限后重开，原快照不可伪造，Verify / Rebuild 不清理 pending；Cancel 保留 target，明确 Confirm 后只移除待处理 attempt，既有对象 / 版本不变 | 覆盖 staging-only attempt 和关闭 / 确认交互；其它文件状态仍按各自证据解释 |
| 原删除授权恢复 | 多版本 lineage 删除在 unlink 时失败，intent 已持久化；重启发现原 request，恢复权限后按该按钮继续；当前选择另一 lineage 时也不得重新规划闭包，原回执完成且对照来源 bytes 不变 | 不覆盖 evidence 写入前后中断；这些窗口继续保留原有合成回调证据 |

权限恢复必须核对同一专用目录、profile、inode 和原 mode，仅恢复本批撤销的权限；不修改用户资料库、父目录、ACL 或系统凭据。完整路径、唯一既有 slot、二进制摘要、逐步操作、预计时长及失败收尾只写在忽略的任务操作单。额外 SQL trigger 会被加密会话 schema 验真拒绝，不用于伪装提交阶段故障；提交成功但刷新失败仍保留既有合成测试边界。

本日三轮 GUI 正常退出，精确权限已恢复，本批来源经 GUI 删除并复核两份 Completed 回执、20 项 succeeded 组件结果及零活动资料。busy 时动作禁用已有原生可见证据；busy 与关闭竞争、SQLite commit / evidence 中断、真实系统密钥失败、多实例及其它平台仍未收口，见[实际记录](../implementation/phase1-source-vault-host.md#2026-10-09-原请求恢复的-macos-原生验收)。保留输入、4 份导出、证据及原 key，不将本地删除回执扩大到备份 / 导出。

2026-10-06 已按下表完成受损库重启后的两条边界，实际结果见[恢复扩展记录](../implementation/phase1-source-vault-host.md#2026-10-06-受损库重启与恢复扩展)。该批次复用已验收的合成 profile 和唯一 slot，不复制日常资料、不创建新 key。GUI 正常关闭后才注入状态；每次重开保留完整验收参数并核对窗口身份。具体路径、摘要、SQL、时长、备份及保留方式写入本批操作单，并对新增 GUI / 故障范围取得授权。

| 场景 | 构造及观察 | 通过条件 |
| --- | --- | --- |
| 派生损坏后显式恢复 | 经真实 GUI 导入一份合成资料并精确导出，关闭后仅删除 lineage tip 派生行并替换 FTS 内容；重启后依次打开、Verify、Rebuild、再打开 | 打开与 Verify 不隐式修复；显式 Rebuild 后目录 / 搜索恢复，导出 bytes 相同，canonical 表与密文对象摘要未变 |
| 密文损坏失败关闭 | 保存同一合成对象的原字节及摘要，关闭后翻转最后一个字节；重启后打开、Verify 与 Rebuild | 全部拒绝且不显示正文，不改写损坏对象或创建 key；关闭后精确还原本次备份，再打开和导出成功。任务恢复字节不计作产品具备自动密文修复 |

桌面 worker 另以 opt-in 合成 provider 覆盖对象缺失及字节损坏后的重启：旧入口、显式打开、Verify、Rebuild、Search 和 Export 失败关闭，外部原件更改也不回源；只恢复测试原字节后才可恢复原 source 与精确导出。该回归不访问系统 key store，不代替上表真实 GUI 证据。

缺 key、锁定、duplicate / ambiguity、真实中断需要额外对象库与精确系统操作方案；本批不通过删除唯一 key、锁定用户整个 keychain 或改系统配置制造故障。不能安全复现的项继续待验。

### 2026-10-09 Keychain 与多实例准备

代码复核确认 `LibraryReader` / 共用 vault session 持有真实 SQLite 排他锁，但桌面空闲窗口不长期持有该会话。因此分别记录双 GUI 交替操作、受控外部锁竞争与已有独立子进程协调器测试；两个窗口同时存在不等于已经观察到两次 GUI 命令的并发竞争。

新增桌面回归以合成 provider 和真实竞争 reader 验证：Export / Refresh 遇锁失败、key load 数量不增加、旧视图隐藏、导出文件不创建；失败的竞争连接不释放持锁 reader；释放后 Inspect 不恢复旧视图，明确 Refresh 才重新显示原来源，精确导出保持 BOM / CRLF 原 bytes。该测试不是 GUI 或系统 Keychain 证据。

准备时提出的原生范围如下，具体绝对路径、profile / 二进制摘要、输入和命令写入忽略的任务操作单；项目所有者随后明确批准，实际结果另列于本节末尾：

| 场景 | 具体操作与通过条件 | 授权和证据边界 |
| --- | --- | --- |
| 两个原生实例 | 同一默认构建、同一隔离 profile 启动两个进程；A 导入 v1，B 显式刷新、导出，A 更新 v2，B 刷新、导出历史 v1 和当前 v2；核对版本、精确 bytes 与各进程退出 | 两实例交替工作不冒充重叠提交；无法可靠区分窗口 / PID 时停止 |
| GUI 遇到真实数据库锁 | v1 导入后，固定目标 helper 开启不执行 DML 的 `BEGIN EXCLUSIVE`，最长 120 秒自动 rollback；GUI 的导出 / 刷新遇锁不得释放旧正文或生成成功文件；释放后显式刷新和精确导出恢复 | 外部持锁进程是验收夹具，不是第二个 GUI 的业务事务；临时 helper 已在独立合成 SQLite 上验证竞争拒绝、显式释放、超时释放与数据库 bytes 不变 |
| 唯一既有 Keychain 项的只读核对 | 仅按已记录 service / account 定位既有合成项，查看 metadata 与 Access Control，不显示 secret、不保存修改；若正常读取实际出现系统提示，由用户处理取消 / 拒绝并记录真实结果 | 现有读取未出现提示，不能保证可构造取消 / 拒绝；不改变 trusted applications、ACL、默认 keychain 或整库锁定状态。需要改变访问策略时，先按实际基线另行明确范围和恢复方式 |

操作单预计 30–45 分钟；要求正常退出本批窗口，精确释放 helper，保留输入 / 导出 / 证据及原 key，仅通过 GUI 删除本批新增 lineage 并检查真实回执，历史审计和旧备份继续保留。准备阶段没有启动 GUI、访问系统凭据或在既有原生库上持锁，原生执行发生在后续明确授权之后。macOS 的 `KeyStoreCancelled` / `KeyStoreDenied` 依真实 OSStatus 记录，不把某个按钮名称或通用 access failure 推断为 `KeyStoreLocked`。

本批实际执行：A 导入 v1 后，B 同参数启动；工具按相同 bundle / path 仍绑定 A，窗口清单查询超时，未能独立定位两个实例。按停止条件正常关闭 A、确认退出后绑定仍在运行的 B，后续为单 GUI 与外部 SQLite helper 的竞争，不是双 GUI 交替或重叠提交。B 的 Export / Refresh 在持锁时分别返回 `ExportSource` / `ListSources`、`SourceVaultFailure` 和 `SQLite Some(5)`；旧视图隐藏、失败导出不存在、busy 动作禁用。helper 提前释放且退出码 0；Inspect 不恢复旧视图，显式 Refresh 后同一来源恢复，两份 60 字节 BOM / CRLF 导出与 v1 完全一致，密文未变。

Keychain Access 仅核对匹配的唯一项：选择“允许访问之前确认”，未允许所有应用，常许应用列表显示四个同名验收 app 条目，名称不足以确认各条目的具体代码身份。本轮未显示密码、修改访问控制或保存变更，正常读取没有出现系统授权提示，所以实际取消 / 拒绝仍待验。一个坐标重试误打开 Prepare key 确认，随即 Cancel，未执行初始化；后续精确凭据元数据未变。最终 GUI 删除唯一 v1 来源得到 Completed 十组件回执、检索 `nebula` 为空、Verify 零对象 / 零 pending；两个 GUI 与 helper 均正常退出，Keychain Access 已退出。具体保留物、日志与证据限制见[宿主记录](../implementation/phase1-source-vault-host.md#2026-10-09-原生锁竞争与-keychain-只读核对)。

macOS 闭环完成后再按当前代码准备 Windows / Linux locked 构建、合成运行与真实 provider / 宿主验收，分别记录架构、文件系统、普通用户权限、系统 store 和诊断证据。`evidence_digest` 统一复算仍独立评审；PDF / 图片解析、默认加密切换、模型和同步不在此计划内。

### 2026-10-09 可区分实例与拒绝构造准备

上轮相同 bundle / path 的两个进程无法被工具分别绑定。本轮只在忽略的验收目录准备 A、B 和 Prompt 三个包装：分别设置 `CFBundleIdentifier` / 显示名，三份可执行文件与已验收默认构建逐字节一致。未增加产品参数或修改 production 代码；新 bundle ID 是否足以独立定位窗口 / PID 尚待原生验证，不能提前标记双实例通过。

签名静态复核发现，既有包装使用 linker ad-hoc 签名、`Info.plist` 未绑定；完整 `codesign --verify` 对原包装及三份副本均报告 `code has no resources but signature indicates they must be present`。这不是本次复制引入的 executable 差异，但限制了签名证据：仅确认相同构建字节和可检查的代码签名信息，不声明包装签名验证通过或可发行。

系统凭据方面，已编译固定唯一测试项的只读 metadata 工具，仅运行无参数说明分支。其查询只请求 item reference，再读取 ACL authorizations、prompt flags、应用路径信息摘要；关闭本进程认证交互，不请求 secret、不含修改 ACL 的操作，目标不唯一或默认 keychain 改变时失败关闭。该工具尚未访问 Keychain，不能记作本轮 metadata 验收。

[Apple 的公开实现](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_keychain/lib/SecTrustedApplication.cpp)显示 `SecTrustedApplicationCopyData` 返回路径字符串；路径摘要不是完整代码身份或可恢复的 ACL 备份。四个同名 / 同路径条目不能据此确认相同、去重、删除或重建。调用依据另以本机公开 Security SDK 的 `SecACL.h` / `SecAccess.h` / `SecTrustedApplication.h` 核对；上游源码仅阅读参考（APSL 2.0），未复制实现、引入依赖或采用私有接口。

因此下一原生批次采用保留既有 ACL 的方案：A/B 执行跨窗口导入、更新、显式刷新和历史 / 当前精确导出；另仅对任务专用 Prompt 副本以新 identifier 做无证书、无时间戳服务的 ad-hoc 重签名，记录变化后的 SHA-256 / cdhash 并要求该副本完整签名验证通过，再观察读取唯一测试 key 是否出现提示。真实提示由用户选择拒绝 / 取消；没有提示、没有相应按钮或发生 app 启动拦截均不能记作 provider 取消 / 拒绝。重签副本的构建来源可追溯，但不可继续称为原可执行文件的相同字节。

精确操作单已列出三个包装路径、唯一合成库 / slot、输入摘要、命令、停止条件、预计 30–45 分钟与收尾方式；原生启动和 Prompt 重签尚未执行，须就该具体范围取得授权。不修改 trusted applications、ACL、默认 keychain 或整个 keychain 锁定状态，不创建 / 删除 key，不选择“始终允许”。退出 Prompt 后由正常实例显式刷新 / 精确导出验证恢复，经 GUI 删除本批 lineage 并核对真实回执；输入、导出、包装、日志、原 key 及旧备份保留。不能把退出进程表述为抹去全部测试副本，也不能据此关闭其它 P1-S05 缺口。

### 2026-10-09 可区分实例的原生结果与许可偏差

项目所有者随后明确要求提交准备文档并继续，准备记录已提交 `9e184ec`，按上述精确操作单执行。A/B 的不同 bundle ID / 显示名解决了本轮工具绑定问题：两个独立 PID、相同固定合成 profile、各自可操作的 Synthetic acceptance 窗口均已核对；A 打开加密库时 B 仍保持未打开状态。A 导入 v1 后 B 显式打开 / 刷新；A 更新 v2 后 B 再刷新，历史 v1、当前 v2 和版本关系均可见。三份 B 导出分别为 60、60、54 字节，与对应输入完全一致，Verify 为两个对象、零 pending。该结果覆盖交替操作和显式刷新，不代表 GUI 提交事务重叠。

B 正常退出后，仅对 Prompt 副本执行已批准的 ad-hoc 重签名：新 identifier / cdhash / SHA-256 已记录，完整 bundle verification 通过，原包装及 A/B 字节未变。Prompt 显式打开后停留 busy、重复动作禁用；控制工具不能读取系统提示窗口，项目所有者随后报告选择了“始终允许”。助手在触发前未明确提醒具体按钮，实际手动操作偏离了预定拒绝场景。Prompt 最终成功打开 v2；没有取得拒绝 / 取消 error 或对应 OSStatus，不能记作系统失败验收通过。

只读 metadata 对比确认常许应用从四项增至五项，新增路径摘要匹配本批 Prompt bundle（含结尾 NUL 的路径字符串）；原四项路径摘要、ACL 数量 / authorizations / prompt flags 在该有限输出中未变。该证据仍不能证明全部存储代码身份相同。新增永久许可保留，未自动删除、重建 ACL 或声称已恢复；所以本轮实际结果不再满足计划中的“访问控制未变”。后续若撤销，须精确限定这条新增许可并另行取得授权，不得删除原 key 或据同名路径重建原四项。

Prompt 读取前后数据库、profile 和两个密文对象摘要一致；Prompt 正常退出后，A 显式 Refresh / Verify 成功，第四份 54 字节导出与 v2 一致。A 经 GUI 删除本轮两版本 lineage，获得 Completed、十项 succeeded 回执；`nebula` 检索为空，Verify 零对象 / 零待处理，磁盘只读复核零活动 references / bodies / fragments / tips / FTS、零对象 / staging，profile 和目录权限不变。A/B/Prompt 均正常退出，保留两份输入、四份导出、三个包装、日志、原 key、旧备份及新增 Prompt 常许许可。真实取消 / 拒绝和其它未覆盖平台 / 失败窗口继续待验，见[宿主记录](../implementation/phase1-source-vault-host.md#2026-10-09-双实例原生验收与-prompt-许可变化)。

### 2026-10-09 新增许可撤销与拒绝重验

首次误允许后，项目所有者另行批准只撤销新增 Prompt 常许项并重验。通过 Keychain Access 唯一定位该项，保留原四项和原 key；第一次保存认证被拒绝，没有记作撤销完成。第二次在“允许保存”交接后保存成功，列表恢复四项。按授权集合匹配的五组 ACL 元数据与原基线一致，仅组顺序改变；这不是完整代码身份备份。新增永久许可最终已撤销，原条目未重建。

关闭 Keychain Access 后重启同一 Prompt。收到用户已准备好点击拒绝，再触发读取；实际 UI 返回 `KeyStoreCancelled`，资料库不可用且新写入禁用，无回退打开。原始 OSStatus 未直接显示，不能额外宣称原生 `KeyStoreDenied` / locked 分支已通过。该重验发生在来源已清理的空合成库，覆盖显式打开失败，不扩张到已有正文视图或写操作中途。

拒绝后 ACL metadata、数据库 / profile 保持，零对象 / staging；Prompt 退出，原 A 仍能免提示打开并 Verify 零对象 / 零 pending，随后正常关闭，所有本轮进程已退出。最终状态、首次误允许偏差、精确撤销过程及证据限制见[宿主记录](../implementation/phase1-source-vault-host.md#2026-10-09-prompt-许可精确撤销与拒绝重验)。后续系统交互必须在触发前完成具体按钮交接，不同时混淆权限保存与拒绝测试。


## 2026-10-13 剩余宿主窗口核对与有限检查点

本批先核对 production UI、worker、controller 与既有合成 provider 回归。headless 测试运行真实界面渲染和 viewport 命令处理，仅以测试 channel 控制结果到达；不会创建原生窗口或访问系统密钥库。worker 退出另用真实线程、合成 provider 和临时 SQLite 验证。实现与验证结果见[宿主记录](../implementation/phase1-source-vault-host.md#2026-10-13-busy-关闭竞争修复与剩余窗口核对)。

| 剩余窗口 | 可控构造与持久状态 | UI / 恢复判据 | 当前证据及原生限制 |
| --- | --- | --- | --- |
| busy 与关闭竞争 | 测试 channel 分别保持执行中、在关闭帧前排入完成结果、断开结果通道；不写虚假数据库 checkpoint | 本帧处理结果前仍 busy，就取消该次关闭；结果 / 错误和刷新失败提示不能被覆盖。完成后不自动退出，用户再次关闭时再按是否持有原请求决定确认。多次布局不改变该判断 | 新增 headless UI 回归通过；原生 busy 动作禁用和原请求关闭确认有历史证据，但新构建的真实窗口关闭竞争仍待验 |
| 接受关闭与按钮事件同帧 | 无原请求、worker 空闲，关闭事件与 Refresh 按钮释放同时输入 | 接受关闭后不再派发操作；正常销毁 worker 等待已接受命令执行及 engine 释放 | 新增真实输入事件回归与真实线程 Drop / join 回归通过；不代表强制终止 / 系统关机或永久阻塞 provider 的退出保证 |
| capture 发布后、提交后中断 | 既有 `interrupt_next_capture_after_publish` / `interrupt_next_capture_after_commit`；分别留下已发布 pending 或已提交状态，由协调器真实写入 | 失败保持原请求和 provenance，拒绝替换；重试精确原 bytes，已提交请求不得重复创建来源。内存快照丢失时走认证发现与显式放弃 | `capture_retry_keeps_original_bytes_and_provenance_and_blocks_replacement` 已覆盖；回调返回错误不是进程崩溃、真实 commit 失败或断电。默认构建无稳定原生注入方式，保留该边界 |
| 写入成功后刷新 / recovery inspection 失败 | 既有合成 provider 在指定 key load 返回失败；正文已提交、原请求已释放 | 显示已提交与独立刷新失败，旧视图隐藏，不能提供重复 capture；显式 Refresh 恢复一个来源。关闭同帧到达也保留结果供查看 | `committed_capture_refresh_failure_reports_success_and_does_not_retain_retry`、`retry_commit_survives_recovery_inspection_failure_without_becoming_a_write_error` 和新增 UI 交付竞争测试；不是实际系统 key store 中途失败证据 |
| 删除 intent / execution / evidence 前后中断 | 既有删除和 evidence 回调覆盖四个边界；持久状态分别是原请求、执行结果或已提交 evidence | 错误不虚构 Completed；重启前三类继续同一 request ID，evidence 提交后的已完成请求不再列入待恢复；外部原件保留 | `restart_discovers_original_deletion_and_resumes_all_interruption_windows` 已覆盖；与此前权限故障原生恢复互补，不能记作真实 evidence commit 失败或掉电验收 |

本轮不新增生产故障 seam、SQL trigger 或持久格式，不用删除唯一 key、锁定整个 keychain、不受控结束进程来扩充原生覆盖。测试 channel 只验证 UI 交付时序，不证明存储中断语义；后者继续由对应引擎 / 协调器回归证明。`evidence_digest` 复算与历史兼容仍单独评审。

进入集中跨平台准备的有限条件为：上述窗口全部有明确判据、既有和新增合成回归通过、默认 desktop 构建通过、原生待验项及限制明列。达到这些条件即可准备 Windows / Linux 当前代码的 locked 编译与合成检查，无需无限追加难以安全构造的 macOS 故障；真实平台 provider / GUI 与 VM 操作仍分别取得具体授权。这个检查点不关闭 P1-S05，也不解除 PDF / 图片停止线。

下一原生批次只验证新构建的关闭行为：空闲正常关闭；可实际观察到 busy 时请求关闭并保留窗口；收到结果后再次关闭；失败保留原请求时 Cancel / Confirm 及最终进程退出。执行前须核对复用的隔离目录、profile、唯一既有 slot、构建摘要和操作方式，给出精确目标、时长和保留 / 收尾方式再请求授权。若操作太快不能稳定观察 busy，记录未覆盖；不能仅凭窗口正常退出判定竞争通过，也不以本节泛化计划作为系统操作授权。


### 2026-10-13 首轮原生结果与新增交互缺口

项目所有者批准具体操作单后，第一轮新构建在既有空合成库执行一次显式打开；用户按触发前交接选择本次允许，却报告反复出现系统提示，最终 GUI 打开成功。随后正常退出，数据库 / profile 不变，零活动资料 / pending，原权限保持。未进入导入、持锁、权限故障、原请求关闭确认或删除阶段，不能将本批记作 busy 关闭原生通过。详情见[现场记录及静态原因](../implementation/phase1-source-vault-host.md#2026-10-13-原生首轮的重复密钥提示与收尾)。

下一步先评审一次桌面动作包含多个独立 key 加载带来的交互成本和动作级认证边界，再决定如何继续原生验收。不得用永久许可、删除复验或未经审阅的跨动作缓存换取测试通过；已经通过的 headless 关闭回归继续成立。原有操作授权不扩张为 ACL 修改或新的密钥生命周期策略。

### 2026-10-13 只读动作会话的后续验收边界

后续评审与首批实现已获明确批准，`OpenEncrypted` 和显式 `Refresh` 按[动作会话契约](../implementation/phase1-source-vault-application.md#2026-10-13-只读动作认证会话)合并只读快照，保留首尾各一次双读和最终对象 / 数据库认证；其它动作和写后刷新保持原路径。实现与合成证据见[宿主记录](../implementation/phase1-source-vault-host.md#2026-10-13-只读动作认证会话实现与合成验证)。本批只运行合成测试与构建，未启动 GUI、访问真实系统 key store 或复用此前授权继续原生操作。

下一原生操作单须明确：正常路径预期四次 secret API 读取，系统提示次数需要实测，metadata 访问另计；每个提示只能按具体授权处理，不要求常许。需分别记录空库 / 有资料打开与显式刷新、首轮与末轮取消后不交付视图 / 恢复选择，以及原请求保留；key 替换继续先用合成测试，不为原生证据删除或替换唯一既有 key。busy 关闭及 pending 原请求 Cancel / Confirm 原生缺口仍保留，新增操作须重新列明范围、预计时长与收尾方式取得授权。

### 2026-10-13 新构建的局部原生结果

`89d53b0` 提交后，经新操作单明确授权启动一次 GUI。A01 空库打开的首个提示有真实调用栈与同构建 dSYM 证据，确认在 `snapshot.rs:43 → reader::open` 的入口认证；用户拒绝后 GUI 返回 `KeyStoreCancelled`，无可用视图、恢复未验证、未自动重试，数据库 / profile 未变。

A02 空库正常打开成功，但用户报告 9 次提示均点允许，超过操作单 8 次上限，因此停止新增测试并正常退出。该计数是用户汇总，不能从中推断每个提示对应的 API 或末轮阶段。空库刷新、有资料打开 / 刷新、末轮取消与 B/C 关闭场景未执行，完整验收仍未通过。最终零活动资料 / pending、原权限与摘要保持；无持锁 / 权限故障 / 删除，因此无本批删除回执。详见[实际阶段证据和收尾](../implementation/phase1-source-vault-host.md#2026-10-13-新构建原生入口取消与提示预算停止)。下一步先诊断提示来源并修订具体观测方案，不自行提高预算或通过永久许可绕过。

### 2026-10-13 单动作逐提示观测提案

状态：仅准备，尚未执行；本次诊断授权明确禁止 GUI 和真实 key 访问。调用预算、未确认原因及修复分支见[provider 静态诊断](../implementation/phase1-source-vault-key-provider.md#2026-10-13-合并会话后九次提示的静态诊断与修复提案)。新操作单单独使用任务专用忽略目录，不覆盖上一批计划 / 栈 / 台账。

拟授权范围缩为同一个既有默认二进制、同一个空合成资料根和唯一既有测试 slot，仅一次显式 OpenEncrypted；预计 20–35 分钟，另预留 5 分钟收尾。沿用已确认摘要的包装和同 UUID dSYM，未来执行前复核本地文件、数据库 / profile、零活动资料 / pending 和目录身份，不先运行会额外改变观察条件的 security metadata 探针。provider 本身继续校验 User domain、唯一性和身份。二进制 / profile / 数据漂移需重新准备，不能套用本单。

每个系统提示都先停留，由用户确认目标和非敏感文案类型；只对已核对 GUI PID 做一次 1 秒、20ms 间隔 sample，离线 atos，记录 snapshot 阶段、provider read 位置和 macOS API / metadata 子阶段。只有取得正向调用链且未达上限才逐次交接一次 Allow Once；下一提示重新停留，禁止连续点击。第九个提示只采样后交接拒绝，无论是否已找到原因都终止该动作；最多八次允许、九次采样、一次 GUI 启动 / 显式打开，不因没有看到完整成功路径而提高上限或重复测试。

阶段无法确认、信息不匹配、提示无变化且无法确认按钮结果、采样被拒、超过时限或取消不返回时停止新增操作，保留未覆盖结果；不通过缺少 reader::open 推断末轮。同一来源位置重复出现且完整调用链一致，是单个调用内重复交互的线索，只有调用实例可确认时才归因，否则标注待遥测。没有提示记实际零次并标本次无法观察；不改 ACL 制造提示。

本单不包含 Import / Verify / Refresh / Inspect / Export、持锁、chmod、放弃 / 删除、有资料场景或关闭竞争，不承接上一份 A/B/C 的后续执行授权。拒绝 / 失败或成功返回后只核对真实界面与只读基线，正常关闭并确认 exit；无业务写入时没有删除回执，保留 key、历史、包装和所有证据。不会自动重签、安装、改系统开发权限、强杀、转储内存或扩大进程采样范围。形成诊断结论后，先选择修复分支；再另行准备修复构建的完整首尾取消 / 关闭验收。
