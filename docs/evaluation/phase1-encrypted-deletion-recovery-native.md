# macOS 加密库已持久化删除请求恢复操作单

2026-11-23 已用旧版应用在两个独立合成库生成删除检查点，并以新版分别重开、升级和恢复。原生准备程序写入检查点后，在重开回读原请求时退出，现场已封存，B1 至 B3 暂停。本单选择“执行已落盘、回执尚未写入”这一窗口，验证桌面能发现原授权、保留第一次执行并补齐唯一回执。下次从[修复后的续跑步骤](#native-resume-after-fix)进入。原批次的执行情况见[检查点记录](#native-checkpoint-result)。

## 哪一个请求可以恢复

11 月 8 日原生库内的 `delete-request-ccb92f39e6887637a0687d47ba4fb48d` 已完成，两个来源版本均为 `deleted`，原 v1 摘要为 `11eb8e7c6bfb31aeccec046d2fd3c94e479863e15dfa34b7423fd3d87ffbfa59`，计划摘要为 `972cf31517725d76874e6b978280ad56db172c03ce1ef13b4ceb3ee0b4f58bbf`。它是本单的历史对照。升级前备份内只有该终态请求。本次旧版应用在独立原生工作副本持久化了第二条请求，ID 和计划摘要见[检查点记录](#native-checkpoint-result)。

离线预演使用同批历史合成 provider 的库。旧版应用通过正常删除入口落盘，新进程按 ID 取回的 `DeleteRequest` 与最初的类型化请求相等，待办列表也只返回它。两个案例均指向该合成库的 `source-3e851f2d0cf69d50e683e5bf9739853c`。表中数量依次为**该请求的执行次数 / 组件结果数 / 回执数**。

| 已冻结检查点 | 升级前的磁盘事实 | 显式升级并恢复后 |
| --- | --- | --- |
| `B/intent/`，`delete-request-20261123-intent`；计划摘要 `c463c11cd091b4c91519f78e1baa2a448634f510560cd8aff9520e63b7d2cf78` | 提交意图后中断，状态 `deleting`，仍有 1 个密文对象。目标来源及其 1 条片段为 `pending`，活动对象引用、目录 tip 与 FTS 行均已撤下。计数 **0 / 0 / 0**。 | `deleted`，密文为零，计数 **1 / 10 / 1**。v2 指向第 1 次执行，摘要 `6f386c7c3984e2bf12b3ce094bef0aefe5fbfb57c14af22bbe04e9912d556992`。 |
| `B/executed/`，`delete-request-20261123-executed`；计划摘要 `edc9e31d48ded414402315b99f4049ce1c2b2e53d579092fabed17e9defe28f8` | 第一次执行完成，在取得回执结束时间时报错。状态 `deleted`，密文、活动引用、目录 tip、FTS 命中均为零。计数 **1 / 10 / 0**。 | 同一请求计数 **2 / 20 / 1**。第一次执行记录保留，v2 指向第 2 次执行，摘要 `3eb40deb25ba1ad7d3d17a5860d446adbc0379ecc9a6f4ac7fca61b536956904`。 |

两份旧库直接继续均得到 `EvidenceUpgradeRequired`，数据库及对象摘要不变。显式升级后重开，原请求、目标、组件、计划和已有执行行均保留。各自恢复后重开，待办清空，再执行同一请求会返回已有回执；原 v1 和新 v2 都通过生产回读与摘要复算。这里的 v1 属于另一请求，故新 v2 的 `previous_evidence_id` 为空。同一请求已有失败 v1 时，后续回执引用该 v1，由独立的[历史失败请求回归](../implementation/phase1-source-vault-deletion.md#deletion-evidence-v2-implementation)覆盖。

## 原生冻结件与权限

所有命令从本机仓库的 `.tmp/2026-11-23-encrypted-receipts/` 执行。`B/baseline/` 是原生旧库完整备份，`B/native-seed/` 是唯一可写库，两者独立于 A 单。B 单可从自己的备份及清单独立核验。两份合成 provider 检查点保留在各自目录；`B/work-intent/` 和 `B/work-executed/` 保存各自的终态，`evidence/rehearsal.txt` 记录实测计数和摘要。

| 项目 | 本单固定值 |
| --- | --- |
| B 清单 SHA-256 | `a1e33439458fd757a2a018696f5f6c201dd8e12858bf1983a875e3ef38adc5f4`，覆盖 B 备份、两个合成检查点、预演记录、两代工具、源码及属性探针 |
| 两个可执行文件 | 桌面默认构建 `b022807310db0128fce671a38789e592a90e746a`，SHA-256 `e7ebd2b5e8aab96cba08ecdaf2e51e54f145e1cafa8d756ec1784040f1d82ecc`。旧版准备程序使用 `0ce38b00e25f87bf7408bf221dce8743fcd0f436` 的 application 与默认平台 provider，SHA-256 `5df06701168f52204df9a53808f1d199c5d633abcd2e3144ab4b2dea3f409a4b` |
| 原生起点与唯一目标 | v11，`objects_ready`，尚无迁移 13，数据库 SHA-256 `b018c94df61ce953db5c3e6a1804d8728d2b4d6ffa67fa9441bb731fed674f16`。目标 `EncryptedControl.md` 属于 `lineage-c57344903abc5e202d0540729db1511f`，唯一活动版本 `source-f1843392cc7b5b11fb390ff2d3de9612`。准备程序先认证读取并逐字节比对冻结的 60 字节输入。 |
| 计划生成的原请求 | `delete-request-20261123-native-executed`，精确目标为上述一个版本，`local_purge`、当前设备、10 个组件。首次持久化后记录其真实 `plan_digest`，后续逐行对比冻结计划。两条离线计划摘要各属自己的请求。 |
| 既有系统测试项 | macOS 当前用户默认 Keychain，generic password；service `io.github.laugh0608.RadishMemory.source-vault`，account `v1:namespace-fd8a565292ea95da4c8b5327c012e8ac:device-91e620ecae3c041942722e24c5de8235`，label `RadishMemory Source Vault key`。SQLite 绑定 provider `radishmemory.platform-key-store/1`、`key_ready`。 |

执行授权包含精确条目的一次属性核对，旧程序与桌面通过现有 key 认证打开，以及对 `B/native-seed/` 中指定合成来源的受管删除、显式格式升级与原请求恢复。该副本中的目标会退出目录和索引，对象文件会退役。原库与备份仍保留该对象。执行前核对指定条目。遇系统提示即停，由所有者处理。创建或替换凭据、常许、修改 ACL、锁定 Keychain 或更改系统时钟均超出本单。

## 原批次执行顺序与退出判据

1. **核对起点。** 对照本页运行 `shasum -a 256 B/SHA256SUMS`、`shasum -a 256 -c B/SHA256SUMS`、`python3 tools/check.py B-seed`。确认无库句柄，原 v1 通过复算。单独授权后仅运行一次 `tools/key_slot_probe present`。属性唯一且匹配才继续。
2. **生成并封存原请求。** 运行一次 `tools/old-native-stage prepare-native-executed`。它对唯一目标准备并执行删除，在组件执行已提交后由测试 runtime 对回执结束时间返回 `ClockFailed`。程序随后重开，比较持久化请求与原请求并检查唯一待办。正常退出后运行 `python3 tools/check.py B-checkpoint`，把核验后的完整副本、文件摘要及实测计划摘要留在 `evidence/`。目标应为 `deleted`、无对象，原请求计数 **1 / 10 / 0**，历史 v1 仍可读。到不了这个状态就保留现场，停止开窗。该窗口验证应用错误边界，物理掉电另行验证。
3. **运行三个窗口。** 各轮按下表启动，核对 `Synthetic acceptance library`、加密格式和 FTS 明文提示，点击一次 `Open encrypted objects`。必要时可点一次 `Inspect recovery` 刷新待办。每轮正常退出 0、库句柄归零后再对快照执行检查。操作包将窗口绑定到 `B/native-seed/` 的绝对路径和冻结的 namespace / device。

| 窗口 | 可执行动作 | 通过条件 |
| --- | --- | --- |
| B1 旧格式与升级 | `python3 tools/window.py B1`。核对唯一 `Continue persisted deletion delete-request-20261123-native-executed` 可见且禁用。打开 `Upgrade deletion receipts…` 并确认一次，等待升级成功。 | 退出后 `python3 tools/check.py B-upgraded`。旧探针返回 `SchemaDrift`，迁移记录新增 13。逐行比对封存检查点，原请求、授权、目标、计划、第一次执行、10 项结果及历史 v1 均保留，仍无本请求回执。 |
| B2 重开后继续 | `python3 tools/window.py B2`。确认同一完整 ID 的继续按钮已启用，只点击一次。记录界面的 `Completed`、回执 ID 和组件数。 | `python3 tools/check.py B-done`。待办清空，原请求计数 **2 / 20 / 1**；仅新增第 2 次执行、其 10 项结果和一条 v2。其 ID 与界面相同，指向原请求及第 2 次执行，摘要可复算；历史 v1 可复算，计划、第一次执行与组件行仍相同。 |
| B3 终态重开 | `python3 tools/window.py B3`。核对继续入口消失，检索 `mockcontrol` 零命中。 | `python3 tools/check.py B-reopened`。与 B2 完成时的文件摘要相同。目标无对象、活动引用、片段、目录 tip 或 FTS 行，仍只有上述两条回执。 |

任一步身份、授权、计数、字段、系统状态或回执复算异常即停止并保留最后成功的快照。收尾复核 B 清单，保留工作副本、具名检查点与证据。需要重做时从 `B/baseline/` 另建起始副本，或从 `evidence/B-persisted/` 另建具有原请求的副本，并重新固定路径与清单。直接重跑准备程序可能产生冲突，已持久化的请求应按原 ID 恢复。仅在 worker 内存持有原件时才会出现的关闭确认、意图中断的真实平台时序、旧版 GUI 与其它平台另验。

<a id="native-checkpoint-result"></a>

## 2026-11-23 原生检查点与重复凭据提示

B 清单的 27 项冻结文件复核通过，一次 Keychain 属性查询匹配指定的唯一测试项。旧版准备程序对合成工作副本执行了一次。所有者报告多次输入密码后仍收到提示，选择拒绝后提示消失。程序以退出码 101 结束。最后的错误位于准备程序 `checkpoint.rs` 第 26 行：预期的结束时间错误已触发，库也已重开，随后 `get_delete_request` 返回 `DeleteSourceLineage / SourceVaultFailure`。程序输出只保留这层错误，无法据此判定 macOS 的底层状态码。原生的请求相等性断言及后续待办列表断言没有完成。

| 拒绝后核对的内容 | 关闭后的实际状态 |
| --- | --- |
| 删除目标与原请求 | `B/native-seed/` 中唯一目标为 `deleted`，对象文件、活动引用、片段、目录 tip 和 FTS 行均为零。请求 `delete-request-20261123-native-executed` 只指向冻结的合成来源，计划摘要为 `8c52d71f20cf9677293541bc7db6c27df5f70aa17b57492f5c6ca1c9ae9b218c`。 |
| 执行与回执 | 该请求有第 1 次执行及 10 项组件结果，回执为零。另一条历史请求的 v1 仍为 `Completed`，摘要复算通过。SQLite 仍为 v11，尚无迁移 13。 |
| 备份与现场 | 确认库句柄归零，运行 `python3 tools/check.py B-checkpoint` 通过。封存副本在 `evidence/B-persisted/`，数据库 SHA-256 为 `2a52f387a3b9e0bb68cad5db6c24120fd072d28182bf86619ba45fc7c25d7d37`。升级前的 `B/baseline/` 仍保留目标对象。 |

在封存件的另一个副本上，旧版 SQLite adapter 按 ID 读回了请求，待办列表恰好只有这条，且最新回执为空。核对前后数据库摘要一致。这个存储层复验没有访问 Keychain。原程序中需要真实凭据的 application 回读仍记录为失败。详见本机 `evidence/B-native-session/stored-request-h4mgr72y/result.txt`。

在独立合成副本上用 `SyntheticLibraryProvider::key_loads()` 按步骤计数。旧版准备程序走完共需 12 次逻辑密钥加载，至删除执行及预期的 `ClockFailed` 为 8 次，重开后为 9 次，回读原请求成功后应为第 10 次，末尾另有两次核验。旧 macOS provider 的 `load_existing` 在一次成功加载中执行 `Read1` 和 `Read2`，两次均查询密钥值以比较一致性。完整成功路径按源码推算涉及 24 次密钥值查询。本次系统弹窗次数未采集。

同一桌面构建的 worker 也在单独的合成检查点副本上逐动作计数。B1 显式打开及升级分别为 2 和 1 次加载，B2 显式打开及继续原请求分别为 2 和 6 次，B3 显式打开及搜索分别为 2 和 1 次。三次启动各为零次。这些是合成 provider 的调用计数，B1 至 B3 的真实窗口均未启动。两份计数日志保存在本机 `evidence/B-native-session/` 下的 `offline-reads-taoecxmu/counts.txt` 和 `worker-loads-pr07fpfl/counts.txt`。

本次原生验收停在准备程序的重开回读。保留该检查点，后续从它的副本继续。

<a id="offline-read-reduction"></a>

## 2026-11-23 重开回读与 B2 拒绝后的离线对照

**从现有检查点回读。** 在 `B/executed/` 的独立合成副本上，分别打开库、按 ID 读原请求、列待办、按预定 ID 查回执共需 4 次凭据加载。现有 `open_with_snapshot` 用 2 次加载返回唯一待办，再独立核对预定回执缺失，共 3 次。在另一副本上用 SQLite adapter 按 ID 取出完整类型化请求、查该请求的最新回执。两种 application 路径的请求均与此对照逐字段相等。对 3 个加载位置分别注入拒绝，以及在打开的第 2 次加载替换凭据，均在注入处返回相应错误。所有副本的文件摘要与起点相同。原生旧程序未完成的断言仍待在指定平台凭据下验证。

**桌面继续已持久化的删除。** 每种场景使用一份新的 `B/executed/` 副本，通过 worker 执行 B1 升级、B2 重开并继续同一请求。正常场景另用新的 worker 完成 B3。表中是合成 provider 的逻辑加载次数。B2 依次核对原请求及回执、执行删除、提交回执。两个试验均保留这三步，且在调用 backend 前撤回先前发现的待办。

| B2 写后处理 | B2 继续成功 | B1 至 B3 合计 | 第 4 次加载被拒绝后 |
| --- | ---: | ---: | --- |
| 修复前的 worker，分别刷新目录及检查恢复状态 | 6 | 14 | 总共仍读 6 次。资料视图撤回，后两次读取成功，worker 又把恢复状态标为已知。另在第 1 次加载拒绝时，返回状态仍带旧待办 ID。 |
| 保留独立读取，首次错误即返回 | 6 | 14 | 停在第 4 次。资料视图与旧待办均撤回，恢复状态为未知。 |
| 仅在 B2 写后用组合快照刷新 | 5 | 13 | 停在第 4 次。资料视图与旧待办均撤回，恢复状态为未知。成功交付前仍复验凭据。 |

**拒绝时磁盘实际写到了哪里。** 在两个试验各自的每个加载位置分别注入拒绝。下表仅统计原请求的执行次数、组件结果数和回执数。每个用例退出后都逐行核对原请求、计划、首次执行和历史 v1，用生产 adapter 回读并复算所有已存在的回执。

| 拒绝位置 | 磁盘计数 | worker 返回结果 |
| --- | --- | --- |
| 第 1、2 次，核对请求或执行删除 | 1 / 10 / 0 | 返回原错误，仍持有原请求，等待显式处理。 |
| 第 3 次，提交回执 | 2 / 20 / 0 | 第 2 次执行已落盘，仍持有原请求。没有报告完成。 |
| 写后刷新（独立读取第 4 至 6 次，组合读取第 4 至 5 次） | 2 / 20 / 1 | 保留本次真实的 `Completed` 回执，同时报告刷新错误。回执 ID 与磁盘一致，v2 指向第 2 次执行。 |

写后遇到取消时，两个试验均在对应加载处停止。组合读取还覆盖入口和交付前的已锁定、缺失，以及交付前密钥替换（`KeyReadbackMismatch`）。写后失败后再显式刷新，持续拒绝时仅加载一次。恢复合成凭据后加载两次，待办为空，磁盘仍只有一条该请求的回执。

选择保留独立认证的方案。组合方案在本检查点的正常三窗流程仅节省一次加载，超出[现行只读会话范围](../implementation/phase1-source-vault-application.md)。

本次未访问 Keychain 或启动原生窗口。测量限于无活动来源、未固定对照项的合成检查点，无法换算为系统提示次数。B 清单的 27 项以及 `B/native-seed/`、`evidence/B-persisted/` 的文件摘要在试验前后核对一致。本机 `evidence/B-native-session/coalesced-3x9HgOCu/` 保留回读计数、两份试验补丁、逐例 worker 输出与关闭后的存储核对结果。

<a id="native-resume-after-fix"></a>

## B2 修复与原生续跑步骤

正式 worker 的 `ResumeDelete` 已修复。提交回执前失败时，worker 撤回旧待办，并保留已交给 backend 的原请求供显式重试。提交成功后，首次刷新错误会结束本次读取，界面同时收到真实回执和刷新错误。仓库回归用合成库中已执行但无回执的原请求，逐一拒绝六个加载位置，另在写后首次读取时取消。回归核对错误位置、凭据读取次数和待办撤回，并在成功提交的场景验证重开后的回执与返回值相等。

另用正式 worker 源码复跑旧格式合成检查点的 10 个场景。每例关闭后核对原请求、计划、首次执行、全部组件结果及回执摘要。第 1、2 次加载被拒绝时为 **1 / 10 / 0**，第 3 次拒绝时为 **2 / 20 / 0**，三个写后读取位置拒绝或取消均为 **2 / 20 / 1**。正常流程的 B3 重开无待办，检索零命中。逐例证据位于新包的 `evidence/worker-confirmation/`。

**续跑起点。** 本机新包为 `.tmp/2026-11-23-b-resume.lIJs2pkk/`，下列命令均从该目录运行。`B/native-seed/` 是封存检查点的独立可写副本，数据库 SHA-256 为 `2a52f387a3b9e0bb68cad5db6c24120fd072d28182bf86619ba45fc7c25d7d37`。旧批次及新包对照副本均保留。新包 18 项清单 `B/SHA256SUMS` 的 SHA-256 为 `d9fa792c2337faf5f901daa94c2a63b07d30008b0dd730b8d5b84329bef6e15c`。清单覆盖修复后的默认桌面构建（SHA-256 `e95f604ec4cf05922d07e6cdadd5f0ed528f3fcb9a20493bf5e514871339b5a5`）、回读程序、核对工具及冻结输入。

1. **复核起点。** 核对 `shasum -a 256 B/SHA256SUMS` 后运行 `shasum -a 256 -c B/SHA256SUMS`。已执行的 `tools/readback storage-reference` 只核对关闭后的库存储。按旧准备程序的确定性输入重建完整 `DeleteRequest`，与对照副本按 ID 取出的请求和唯一待办逐字段相等，原计划、1 次执行、10 项结果及零回执已核实。历史 v1 摘要复算通过。
2. **先做一次原生只读回读。** 此步使用新包内的应用回读程序读取原单指定的现有 Keychain 测试项，预计数分钟，取决于系统提示。所有者确认新程序和副本后运行一次 `tools/readback native-readback`。它核对起点，通过显式打开的认证快照比较完整待办，再查预定回执缺失。合成预演实测 3 次逻辑加载。退出 0 后用 `python3 tools/check.py B-checkpoint` 确认数据库及对象摘要与封存件一致。旧程序当时的失败记录保留，此步单独记录新版的原生结果。
3. **再执行三个串行窗口。** 在新包窗口执行范围获确认后，按原表依次运行 `python3 tools/window.py B1`、`B2`、`B3`。每轮核对路径、加密格式和原请求 ID，上轮正常退出且库句柄归零后再进入下一轮。B1 只升级回执格式，退出后查 `B-upgraded`。B2 仅继续一次原 ID，退出后查 `B-done`，界面回执 ID、数据库唯一 v2 和 **2 / 20 / 1** 应一致。B3 重开后查无待办、搜索零命中，退出后查 `B-reopened`，与 B2 终态文件摘要相等。这三个窗口的正常合成路径仍为 14 次逻辑加载。预计 15–25 分钟，实际提示次数待测。

**提示与中断。** 遇到系统提示即停下交还所有者。重复提示时可直接拒绝，本轮在程序返回后核对最后落盘状态。若 B2 同时报告已提交回执与刷新错误，记录回执 ID 并在关闭后检查副本，避免再次执行删除。若有执行记录但仍无回执，保留现场并记录计数。只有新副本承接升级和恢复，重新开始时从封存件另建副本。本轮只完成了离线核对和合成回读，新包的原生回读及三个窗口均待执行。
