# RadishMemory 当前状态

更新时间：2026-10-01

## 当前阶段

`Phase 1 application encrypted use cases implemented; host integration next`

M0、文本 / Markdown 文件入口和本地桌面宿主已建立；原始对象加密已完成独立 portable crypto 与 filesystem adapter 实现，并具备 macOS 本机、Windows ARM64 / NTFS、Linux ARM64 / ext4 的合成运行证据，现已接入显式 application 加密用例，默认桌面宿主尚未切换。默认桌面产品入口仍是 SQLite v6 inline plaintext body；这里指普通产品正文路径，P1-S04a 的 v7 仅用于显式维护入口的密钥准备 checkpoint；P1-S04b 的 v8 承载正文迁移状态与已迁移对象库，后续 v9 供显式加密 capture 协调，v10 增加显式放弃与终态记录，v11 增加既有 DeleteRequest 下的对象删除执行 checkpoint，v12 保存历史正文缺失的原执行凭据引用；默认 `LocalLibrary` 入口拒绝 v7 / v8 / v9 / v10 / v11 / v12，显式加密入口只接受迁移完成的 v8 至 v12。项目具备受约束的工程原型，但中文找回、完整目录访问、启动失败后的派生修复和生产验收仍有缺口，不能据历史合成验收宣称日常资料库已完整可用。

P1-S03c-2 的独立 provider、macOS 构建与合成验证见[落地记录](../implementation/phase1-source-vault-key-provider.md)，三平台 locked CI 由下述 PR #4 复验；真实平台密钥库仍待验收。P1-S03b 实现与证据见[filesystem adapter 落地记录](../implementation/phase1-source-vault-filesystem.md)；实现范围为独立 Source Vault package 与 Windows 文件身份 adapter，没有修复既有文本产品质量缺口。已确认问题、静态发现和待测风险见[2026-09-05 项目审阅](../implementation/2026-09-05-project-review.md)；截至 2026-09-03 的详细提交、三平台 CI、依赖数量与 M0 完成流水见[阶段基线归档](2026-09-03-baseline.md)。

## 能力与证据

| 能力 | 已成立 | 当前限制与真相源 |
| --- | --- | --- |
| M0 领域与存储 | canonical core、SQLite v6、来源 / 记忆 / 事件、FTS5、本地删除与真实 M0 runner 已经建立 | [ADR 0002](../adr/0002-m0-local-memory-loop.md)；runner 编排不等于 production 历史查询与上下文编译接口 |
| 字段与 fixture | M0 字段级 canonical schema 定义九种顶层对象；fixture 固定 12 个场景的 86 个有序操作和 12 个指标 gate | [M0 Canonical Schema](../schema/m0-canonical-schema.md)不绑定数据库、生产 ID 编码或语言类型；[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md)说明实际证据限制 |
| 文件入口 | `radishmemory-file-entry` 的 P1-I01 file snapshot contract、P1-I02 atomic source capture、P1-I03 exact export、P1-I04 lineage deletion 已落地 | [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md)；`SourceCaptureStore` 原子提交，`P1-F01` 至 `P1-F18` 已有三平台证据，故障 seam 仅 opt-in `acceptance-test-support`；不代表完整 importer / exporter 已实现 |
| 本地宿主 | P1-H02 application service、P1-H03 source catalog、P1-H04 desktop UI、P1-H05 host acceptance 已有实现及合成宿主证据 | [ADR 0007](../adr/0007-phase1-local-library-host.md)的 `P1-HF01` 至 `P1-HF12` 保留历史记录；目录第 201 条和重启后损坏修复等缺口尚未关闭 |
| 加密 Source Vault | P1-S01 storage contract、P1-S02 dependency and cipher review、P1-S03a portable crypto dependency landing 已完成对应范围；P1-S03b immutable object filesystem adapter 已通过 macOS 合成验证、Windows ARM64 / NTFS 提升权限与普通用户验收、Linux ARM64 / ext4 普通用户验收 | [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)的 `P1-SF01` 至 `P1-SF18` 尚未全部实现；object adapter 的 Windows 文件身份替换缺陷已修复并通过普通用户 / ACL 回归；P1-S03c-2 独立 key provider 已实现并有 macOS 构建与合成测试证据；P1-S04a 已建立密钥初始化 checkpoint；正文迁移 / 恢复、加密 capture、inventory reconciliation 及显式放弃 / 精确 orphan retirement 合成切片已落地；独立 object-backed verify / rebuild、删除执行及历史请求接管已实现；认证读取、目录 / 检索及精确导出组合验收已通过；显式 application 打开 / 恢复、读取、原请求 capture / 删除及派生维护已接入；真实密钥库与宿主接入仍待后续批次 |
| 用户价值 | 已能通过本地入口导入、版本化、搜索、精确导出和删除合成文本 | 尚无完整记忆控制台、模型问答、PDF / 图片解析、向量、同步、恢复或签名发行包 |

## 当前顺位

1. `P1-S04a SQLite key bootstrap coordination` 已实现：在真实 SQLite `IMMEDIATE` transaction 内验真初始化资格，复验对象目录、创建 / 复用密钥并提交 maintenance-only v7 `key_ready` checkpoint。合成测试覆盖并发连接、独立子进程、真实 commit 失败、缺 key、历史正文损坏和 schema 漂移，见[落地记录](../implementation/phase1-source-vault-key-bootstrap.md)。普通产品入口继续使用 v6；该初始化步骤不迁移正文；后续迁移由独立 v8 维护入口承担。
2. `P1-S04` 已完成独立的正文迁移 / 恢复、加密 capture、inventory reconciliation、显式放弃、verify / rebuild、对象删除与历史请求接管，以及认证读取 / 目录 / 检索和精确导出组合验收；各批实现与代码复核见[9 月 26 日日终记录](2026-09-26-source-vault.md)。[application 加密用例](../implementation/phase1-source-vault-application.md) 已完成显式旧连接关闭、密钥准备 / 迁移恢复、已迁移库打开及完整读取 / 导出会话，具备合成验收。本批已接入 application 的 import / update、lineage 删除及 verify / rebuild，保留原请求精确恢复。macOS 宿主接入已开始准备：application 已补齐重启后的原删除请求发现与精确 capture 放弃接口，[宿主记录](../implementation/phase1-source-vault-host.md)保存 worker / UI 方案和待批准的依赖连线。下一步落实原请求持有 / 恢复及显式维护交互，再进入真实 Keychain 和端到端验收。具体真实系统操作仍单独授权。
3. 按项目所有者 2026-09-15 的安排，Windows / Linux 编译与运行验证后置到上述链路形成阶段切片后的集中检查点；待验收状态保留，不能以 macOS 结果代替。真实系统 key store 与上游日志过滤仍须按具体测试范围授权和验证。PDF / 图片解析继续等待完整加密链路通过。
4. R01 至 R06 的产品质量缺口继续跟踪：[本地资料库质量验收计划](../evaluation/phase1-local-library-quality.md)中的中文检索、目录分页、派生损坏维护入口、读取性能、runner 证据和回源 / 结果刷新仍未收口。本批基础存储实现不等于这些产品问题已经修复。
5. 产品验证继续聚焦“同一个长期项目的资料、关键事实、更正与受控上下文”，完整阶段依赖以[MVP 路线图](../mvp-roadmap.md)为准。

## 已接受的边界

- [ADR 0005](../adr/0005-m0-implementation-stack.md)冻结 Rust 2024 模块化单体，首个工具链固定为 Rust `1.96.0`。当前依赖、manifest / lockfile 和 notices 以[Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)及[第三方 notices 记录](../implementation/phase1-third-party-notices.md)为准。
- P1-S02 已选择 `radishmemory.xchacha20poly1305-stream-be32/1` 与 `radishmemory.xchacha20poly1305-dek-wrap/1`；P1-S03a 已使 portable manifest / `Cargo.lock` / notices 和 cipher 实现落地；P1-S03b 复用该依赖与 AAD，新增严格 envelope、不可覆盖发布及精确 attempt 检查；Windows 使用单独审阅的最小文件身份 adapter，P1-S03c-2 另落地独立 provider；尚未访问真实系统 key store。
- 原始对象按 source version 独立认证加密，不跨 provenance 物理去重；publish → SQLite commit → read-back 必须完整成立。未知 profile、缺 key、认证失败或 ambiguous state 失败关闭，不回退旧 BLOB 或外部原件。
- 首批对象加密不覆盖 SQLite metadata、FTS、派生数据；当前整文件片段使 FTS 保存完整可读正文，不能简化为“只有少量元数据未加密”。历史明文、进程内明文、交换区、快照、备份和用户导出仍在该保证之外。
- [ADR 0003](../adr/0003-zero-knowledge-sync-first.md)选择零知识同步服务，可信计算节点后置为显式可选能力；不代表零知识同步已经实现。
- [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md)将首次接入放在完整 MVP 阶段 3，以显式可关闭的 Model Gateway 接入；首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回。本地能力不依赖 RadishMind。

## 当前停止线

- 不将独立 adapter 或文档更新视为产品缺陷已修复，不扩大 P1-H05、M0 fixture 或 portable crypto 的证据范围；P1-S03b 已有平台证据不替代其它 Linux / Windows 文件系统、架构、真实断电、SQLite / host 验收。
- 不在 `P1-S03b` 至 `P1-S05` 完成前声明加密 Source Vault 可用，不进入 PDF / 图片解析；真实 key store、migration 与宿主接入须分别授权和验证。
- 不将原始对象加密表述为整个资料库静态加密；不声明零知识同步、取证级永久删除、备份可恢复或生产可用。
- 不使用真实个人资料进行仓库 / CI 验收；不自动接入模型、网络、同步或兄弟项目，不增加虚拟形象、图数据库或微服务。
- 不改变数据所有权、canonical schema、记忆确认、权限失败关闭、许可证或远程治理；待决策建议须先明确影响再取得范围确认。

## 当前验证入口

macOS / Linux：

```bash
./scripts/check-repo.sh
./scripts/check-m0-fixtures.py
```

Windows：

```powershell
pwsh ./scripts/check-repo.ps1
```

2026-10-01 宿主恢复准备：补齐认证的原删除请求发现与精确 capture 放弃入口，新增 4 项 application 回归；本机完整检查、fmt / Clippy、334 项 Rust tests、42 项检查器回归、M0 fixture 和 compile-fail doctest 通过。P1-F17 沙箱端口限制经授权重跑解决。桌面依赖连线待批准，worker / UI、真实 Keychain / GUI 与跨平台验收未执行；删除证据摘要复算的既有缺口及范围见[宿主记录](../implementation/phase1-source-vault-host.md)。

2026-10-01 application 加密用例切片：本机完整仓库检查、notices、fmt、locked Clippy 与 330 项 Rust tests（写用例批次新增 9 项、此前读取批次新增 12 项）通过；42 项检查器回归、M0 fixture 和 compile-fail doctest 通过。P1-F17 合成本地端口的沙箱限制经授权在沙箱外完整重跑解决，全部使用合成资料和测试 key；范围、原请求恢复、派生修复与未验收边界见[application 记录](../implementation/phase1-source-vault-application.md)。默认桌面及真实系统密钥库仍未切换。

2026-09-26 实现基线 `8d2c651`：本机完整仓库检查、notices、fmt、locked Clippy / Rust tests、检查器回归、M0 fixture 和 compile-fail doctest 通过；批次数量、独立子进程锁回归、沙箱限制与验证日志范围见[日终记录](2026-09-26-source-vault.md#验证与环境收尾)。全部使用合成库与测试 key，未调用真实系统密钥库。

Linux ARM64 / ext4 filesystem 证据来自 `9d88319`；Windows ARM64 / NTFS 继续引用 9 月 10 日基线。它们不覆盖后续 provider / SQLite 协调和读取代码；当前候选三平台编译与合成测试由下述 PR #4 检查，真实平台 key store、ARM64 当前代码集中运行和宿主生命周期仍待验收。历史范围与清理见[filesystem 记录](../implementation/phase1-source-vault-filesystem.md#linux-arm64--ext4-普通用户验收2026-09-15)、[9 月 10 日记录](2026-09-10-source-vault.md)和[9 月 15 日记录](2026-09-15-source-vault.md)。

当前维护与读取入口接受已完成迁移的 v8 至 v12，正文迁移入口仍仅接受 v7 / v8，普通产品入口仍为 v6。删除中的对象与历史正文缺失凭据须复验，verify / rebuild / reconciliation 不自动推进删除；详见[历史请求兼容](../implementation/phase1-source-vault-legacy-deletion.md)。数据库身份检查已修复 Unix 额外文件描述符关闭导致的 POSIX 锁释放，并分离单对象与数据库大小限制；[读取记录](../implementation/phase1-source-vault-reader.md)保存跨进程证据。每次读取的全库认证成本、持锁期间的操作串行化及产品 R01 至 R06 缺口尚需后续处理。

2026-10-01 阶段晋级 [PR #4](https://github.com/laugh0608/RadishMemory/pull/4) 的首轮 CI 已通过仓库卫生和 Linux / macOS locked Rust Quality；Windows 暴露 provider 引用私有 `Cred` 的编译阻断。经项目所有者批准，以 [同版本最小补丁](../../third_party/vendor/README.md)保留冻结 target/account 和既有依赖图，修复源码可见性并增加无系统访问的构造回归。本地历史已包含该 PR 的合并提交 `689c166`；本批未重新读取远程 head checks，不以补丁、合并或历史 filesystem 记录代替各平台通过证据；真实平台 key store / logger、宿主交互和产品加密接入仍待后续验收。修复范围与来源见 [provider 记录](../implementation/phase1-source-vault-key-provider.md#windows-编译阻断与最小补丁2026-10-01)。

## 后续事项

当前首项是 macOS 宿主接入与原请求持有 / 恢复，再进入 P1-S05 真实密钥库与端到端验收；不能将显式 application 加密用例视为默认桌面产品已切换加密。具体完成范围和下一步见[application 记录](../implementation/phase1-source-vault-application.md)。9 月 15 日和 9 月 27 日建议保留为历史安排；R01 至 R06、宿主接入及集中跨平台验证继续遵循上述顺位与停止线。
