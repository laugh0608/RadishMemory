# macOS 加密库已持久化删除请求恢复操作单

2026-11-23 已用旧版应用在两个独立合成库生成删除检查点，并以新版分别重开、升级和恢复。原生执行另待本单授权。本单选择“执行已落盘、回执尚未写入”这一窗口，验证即使对象已经退役，桌面仍能发现原授权、保留第一次执行并补齐唯一回执。预计 25–35 分钟，至多一次旧版准备程序、三个串行桌面窗口。

## 哪一个请求可以恢复

11 月 8 日原生库内的 `delete-request-ccb92f39e6887637a0687d47ba4fb48d` 已完成，两个来源版本均为 `deleted`，原 v1 摘要为 `11eb8e7c6bfb31aeccec046d2fd3c94e479863e15dfa34b7423fd3d87ffbfa59`，计划摘要为 `972cf31517725d76874e6b978280ad56db172c03ce1ef13b4ceb3ee0b4f58bbf`。它是本单的历史对照。原生工作副本目前只有该终态请求，待恢复请求须在授权后由旧版应用真实持久化。

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

执行授权包含精确条目的一次属性核对，旧程序与桌面通过现有 key 认证打开，以及对 `B/native-seed/` 中指定合成来源的受管删除、显式格式升级与原请求恢复。该副本中的目标会退出目录和索引，对象文件会退役。原库与备份仍保留该对象。系统条目的实时存在性和可访问性待测；遇系统提示即停，由所有者处理。创建或替换凭据、常许、修改 ACL、锁定 Keychain 或更改系统时钟均超出本单。

## 执行顺序与退出判据

1. **核对起点。** 对照本页运行 `shasum -a 256 B/SHA256SUMS`、`shasum -a 256 -c B/SHA256SUMS`、`python3 tools/check.py B-seed`。确认无库句柄，原 v1 通过复算。单独授权后仅运行一次 `tools/key_slot_probe present`。属性唯一且匹配才继续。
2. **生成并封存原请求。** 运行一次 `tools/old-native-stage prepare-native-executed`。它对唯一目标准备并执行删除，在组件执行已提交后由测试 runtime 对回执结束时间返回 `ClockFailed`。程序随后重开，比较持久化请求与原请求并检查唯一待办。正常退出后运行 `python3 tools/check.py B-checkpoint`，把核验后的完整副本、文件摘要及实测计划摘要留在 `evidence/`。目标应为 `deleted`、无对象，原请求计数 **1 / 10 / 0**，历史 v1 仍可读。到不了这个状态就保留现场，停止开窗。该窗口验证应用错误边界，物理掉电另行验证。
3. **运行三个窗口。** 各轮按下表启动，核对 `Synthetic acceptance library`、加密格式和 FTS 明文提示，点击一次 `Open encrypted objects`。必要时可点一次 `Inspect recovery` 刷新待办。每轮正常退出 0、库句柄归零后再对快照执行检查。操作包将窗口绑定到 `B/native-seed/` 的绝对路径和冻结的 namespace / device。

| 窗口 | 可执行动作 | 通过条件 |
| --- | --- | --- |
| B1 旧格式与升级 | `python3 tools/window.py B1`。核对唯一 `Continue persisted deletion delete-request-20261123-native-executed` 可见且禁用。打开 `Upgrade deletion receipts…` 并确认一次，等待升级成功。 | 退出后 `python3 tools/check.py B-upgraded`。旧探针返回 `SchemaDrift`，迁移记录新增 13。逐行比对封存检查点，原请求、授权、目标、计划、第一次执行、10 项结果及历史 v1 均保留，仍无本请求回执。 |
| B2 重开后继续 | `python3 tools/window.py B2`。确认同一完整 ID 的继续按钮已启用，只点击一次。记录界面的 `Completed`、回执 ID 和组件数。 | `python3 tools/check.py B-done`。待办清空，原请求计数 **2 / 20 / 1**；仅新增第 2 次执行、其 10 项结果和一条 v2。其 ID 与界面相同，指向原请求及第 2 次执行，摘要可复算；历史 v1 可复算，计划、第一次执行与组件行仍相同。 |
| B3 终态重开 | `python3 tools/window.py B3`。核对继续入口消失，检索 `mockcontrol` 零命中。 | `python3 tools/check.py B-reopened`。与 B2 完成时的文件摘要相同。目标无对象、活动引用、片段、目录 tip 或 FTS 行，仍只有上述两条回执。 |

任一步身份、授权、计数、字段、系统状态或回执复算异常即停止并保留最后成功的快照。收尾复核 B 清单，保留工作副本、具名检查点与证据。需要重做时从 `B/baseline/` 另建起始副本，或从 `evidence/B-persisted/` 另建具有原请求的副本，并重新固定路径与清单。直接重跑准备程序可能产生冲突，已持久化的请求应按原 ID 恢复。仅在 worker 内存持有原件时才会出现的关闭确认、意图中断的真实平台时序、旧版 GUI 与其它平台另验。
