# macOS 加密旧库回执升级操作单

2026-11-23 已完成离线准备，原生 A1 至 A3 待授权。本单验证三件事：取消后旧库可原样使用，确认后历史回执仍能按原 ID 复验，重开后仍能认证读取原始对象。预计 15–25 分钟，至多三个串行窗口。恢复待处理请求另有[独立操作单](phase1-encrypted-deletion-recovery-native.md)。

## 库、构建和凭据绑定

操作包在本机仓库的 `.tmp/2026-11-23-encrypted-receipts/`。下文命令均从此目录运行。`A/baseline/` 保存完整升级前备份，`A/native/` 是唯一窗口操作库，各自独立保存数据库、profile、对象和空 staging 目录。来源为 [11 月 8 日 macOS 加密问答验收](phase1-local-library-quality.md#macos-加密正文问答与跨窗口刷新)关闭后的合成库。离线预演使用当日合成 provider 自己的密文副本 `A/offline/`。

| 冻结项 | 核对值 |
| --- | --- |
| A 清单 SHA-256 | `0125096ec3df60f700fbc5883f00fe4de2ae81a7b1d5a8d042c285d177ced3b4`，清单含工具、属性探针、预演记录及备份文件 |
| 桌面程序 | macOS 27.0.1 ARM64，源码 `b022807310db0128fce671a38789e592a90e746a`，`cargo build --offline --locked -p radishmemory-desktop` 默认构建，SHA-256 `e7ebd2b5e8aab96cba08ecdaf2e51e54f145e1cafa8d756ec1784040f1d82ecc` |
| 旧库磁盘状态 | SQLite `user_version=11`，迁移记录 1 至 11，`key_ready`、`objects_ready`。数据库 SHA-256 `b018c94df61ce953db5c3e6a1804d8728d2b4d6ffa67fa9441bb731fed674f16`，host profile SHA-256 `547f0aea5a96644d9b173765a30cec9509dd88e80f850b630513d6cb8333446f` |
| 历史请求和回执 | `delete-request-ccb92f39e6887637a0687d47ba4fb48d` 已删除两个旧版本。唯一回执 `deletion-evidence-00f2da82f4507fe54600fe251f6e6544`，`deletion-evidence-v1`，`Completed`，10 项结果，摘要 `11eb8e7c6bfb31aeccec046d2fd3c94e479863e15dfa34b7423fd3d87ffbfa59` |
| 活动对照 | `EncryptedControl.md`，`source-f1843392cc7b5b11fb390ff2d3de9612`，原文 60 字节，SHA-256 `f9c3733279e7d78a657791acf813f59fcfcdf6231ec5f5e30c45f6543b43cd61`。一个活动对象，密文 SHA-256 `6e599d85bf337715e12f904bbc128486f2cd94400bc7ecf15308aa1d274645a0`，行内原文为零 |

已逐项核对 host profile、SQLite key profile 和历史 `target.json`。provider 为 `radishmemory.platform-key-store/1`。本单只涉及 macOS 当前用户默认 Keychain 的一个 generic password，service 为 `io.github.laugh0608.RadishMemory.source-vault`，account 为 `v1:namespace-fd8a565292ea95da4c8b5327c012e8ac:device-91e620ecae3c041942722e24c5de8235`，label 为 `RadishMemory Source Vault key`。历史窗口曾创建并保留这个合成库专用条目，今天的条目存在性、访问结果和提示均待实测。

## 已复验的离线边界

- 两版分别检查关闭后的原生起始库快照。回执升级前的 SQLite adapter（`0ce38b00e25f87bf7408bf221dce8743fcd0f436`）返回 `EncryptedObjects`，新版按原 ID 复算 v1 成功。数据库完整性与外键检查通过。
- 在独立的合成 provider 库，匹配绑定能认证读取完整 60 字节。缺失凭据、错误设备绑定、相同绑定下的错密钥分别返回 `KeyMissing`、`KeyMetadataMismatch`、`AuthenticationFailed`。拒绝前后文件摘要一致，合成 provider 创建密钥次数为零。相同的 namespace 和 device 不使两套密文或密钥可以互换。
- 在该预演库调用显式升级后，`user_version` 仍为 11，只新增迁移记录 `0013_deletion_evidence_v2`。原有业务行、FTS 行、profile 与密文一致，重开可复算原 v1 并读回原文。旧格式探针在升级后返回 `SchemaDrift`。旧版 GUI 兼容性尚未实测。合成应用的逐项结果保存在本机包的 `evidence/rehearsal.txt`。

## 原生执行与判定

执行前单独确认 A 的三个窗口、只读属性检查和现有测试 key 的应用内读取。窗口可写范围限于 `A/native/` 的一次显式回执升级；快照与观测记录保存在 `evidence/`。本单的目标来源应在三轮后仍可回源。

1. 从操作包根运行 `shasum -a 256 A/SHA256SUMS` 对照本页，再运行 `shasum -a 256 -c A/SHA256SUMS` 和 `python3 tools/check.py A-seed`。获本单授权后，仅运行一次 `tools/key_slot_probe present`。它只查指定条目的属性、唯一性及标签。遇到缺项、重复、目标不符或系统权限 / 凭据提示就停下，由所有者处理。应用只读既有 key；创建、替换、删除 key、常许授权与修改 ACL 均超出本单。
2. 每轮使用下表入口，以冻结的绝对库路径、namespace 和 device 启动一个常规窗口。看到 `Synthetic acceptance library`、已迁移的加密库及 FTS 保存完整可读文本的说明后，点击一次 `Open encrypted objects`。通过本轮动作后正常关闭，确认该 PID 退出 0，再做退出后检查。核对工具拒绝仍有句柄的库，复制独立快照并从快照读回。

| 窗口 | 界面操作 | 退出后通过条件 |
| --- | --- | --- |
| A1 取消 | `python3 tools/window.py A1`。选中 `EncryptedControl.md`，打开 `Upgrade deletion receipts…`，核对备份、旧版兼容及现有 key 提示，点击 `Cancel`。升级入口仍在，所选来源的受管删除禁用。 | `python3 tools/check.py A-seed`。数据库、profile 和密文逐字节等于备份，仍只有原 v1。 |
| A2 确认 | `python3 tools/window.py A2`。再次打开升级提示，点击一次 `Confirm`。看到 `Deletion receipts upgraded. Existing receipts and requests were preserved.`，升级入口消失。 | `python3 tools/check.py A-upgraded`。只新增迁移记录 13，正文格式仍为 11；请求、计划、执行、结果、v1、来源与索引逐行一致。对象和 profile 摘要一致。旧探针报 `SchemaDrift`，新版复验同一 v1。 |
| A3 重开 | `python3 tools/window.py A3`。升级入口应已消失。搜索 `mockcontrol`，打开唯一活动来源的原文，核对标题、可见内容及字节范围 `0..60`。 | `python3 tools/check.py A-upgraded` 再次通过。记录可见字节范围及内容，保留唯一活动对象和原回执。 |

任一摘要、库身份、窗口或真实结果偏离预期，记录最后一步并保留该副本。正常收尾再核验 A 清单。若需从头复验，从 `A/baseline/` 另建隔离副本，重新固定路径及清单；这会回到升级前的格式；读取密文仍依赖已绑定的系统测试项。保留系统测试项、历史原库、本批备份及现场。
