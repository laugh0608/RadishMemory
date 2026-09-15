# RadishMemory 当前状态

更新时间：2026-09-15

## 当前阶段

`Phase 1 Source Vault key bootstrap coordination implemented; object migration next`

M0、文本 / Markdown 文件入口和本地桌面宿主已建立；原始对象加密已完成独立 portable crypto 与 filesystem adapter 实现，并具备 macOS 本机、Windows ARM64 / NTFS、Linux ARM64 / ext4 的合成运行证据，尚未接入产品数据流。当前 production code 仍是 SQLite v6 inline plaintext body；这里指普通产品正文路径，P1-S04a 的 v7 仅用于显式维护入口的密钥准备 checkpoint。项目具备受约束的工程原型，但中文找回、完整目录访问、启动失败后的派生修复和生产验收仍有缺口，不能据历史合成验收宣称日常资料库已完整可用。

P1-S03c-2 的独立 provider、macOS 构建与合成验证见[落地记录](../implementation/phase1-source-vault-key-provider.md)，Windows / Linux provider 编译和真实密钥库尚待验收。P1-S03b 实现与证据见[filesystem adapter 落地记录](../implementation/phase1-source-vault-filesystem.md)；实现范围为独立 Source Vault package 与 Windows 文件身份 adapter，没有修复既有文本产品质量缺口。已确认问题、静态发现和待测风险见[2026-09-05 项目审阅](../implementation/2026-09-05-project-review.md)；截至 2026-09-03 的详细提交、三平台 CI、依赖数量与 M0 完成流水见[阶段基线归档](2026-09-03-baseline.md)。

## 能力与证据

| 能力 | 已成立 | 当前限制与真相源 |
| --- | --- | --- |
| M0 领域与存储 | canonical core、SQLite v6、来源 / 记忆 / 事件、FTS5、本地删除与真实 M0 runner 已经建立 | [ADR 0002](../adr/0002-m0-local-memory-loop.md)；runner 编排不等于 production 历史查询与上下文编译接口 |
| 字段与 fixture | M0 字段级 canonical schema 定义九种顶层对象；fixture 固定 12 个场景的 86 个有序操作和 12 个指标 gate | [M0 Canonical Schema](../schema/m0-canonical-schema.md)不绑定数据库、生产 ID 编码或语言类型；[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md)说明实际证据限制 |
| 文件入口 | `radishmemory-file-entry` 的 P1-I01 file snapshot contract、P1-I02 atomic source capture、P1-I03 exact export、P1-I04 lineage deletion 已落地 | [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md)；`SourceCaptureStore` 原子提交，`P1-F01` 至 `P1-F18` 已有三平台证据，故障 seam 仅 opt-in `acceptance-test-support`；不代表完整 importer / exporter 已实现 |
| 本地宿主 | P1-H02 application service、P1-H03 source catalog、P1-H04 desktop UI、P1-H05 host acceptance 已有实现及合成宿主证据 | [ADR 0007](../adr/0007-phase1-local-library-host.md)的 `P1-HF01` 至 `P1-HF12` 保留历史记录；目录第 201 条和重启后损坏修复等缺口尚未关闭 |
| 加密 Source Vault | P1-S01 storage contract、P1-S02 dependency and cipher review、P1-S03a portable crypto dependency landing 已完成对应范围；P1-S03b immutable object filesystem adapter 已通过 macOS 合成验证、Windows ARM64 / NTFS 提升权限与普通用户验收、Linux ARM64 / ext4 普通用户验收 | [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)的 `P1-SF01` 至 `P1-SF18` 尚未全部实现；object adapter 的 Windows 文件身份替换缺陷已修复并通过普通用户 / ACL 回归；P1-S03c-2 独立 key provider 已实现并有 macOS 构建与合成测试证据；P1-S04a 已建立密钥初始化 checkpoint；真实密钥库、正文 migration、宿主接入仍待后续批次 |
| 用户价值 | 已能通过本地入口导入、版本化、搜索、精确导出和删除合成文本 | 尚无完整记忆控制台、模型问答、PDF / 图片解析、向量、同步、恢复或签名发行包 |

## 当前顺位

1. `P1-S04a SQLite key bootstrap coordination` 已实现：在真实 SQLite `IMMEDIATE` transaction 内验真初始化资格，复验对象目录、创建 / 复用密钥并提交 maintenance-only v7 `key_ready` checkpoint。合成测试覆盖并发连接、独立子进程、真实 commit 失败、缺 key、历史正文损坏和 schema 漂移，见[落地记录](../implementation/phase1-source-vault-key-bootstrap.md)。普通产品入口继续使用 v6；正文尚未迁移。
2. 下一批推进 `P1-S04b`：密文对象引用、capture / migration attempt、v6 正文逐项迁移和中断恢复，再收口 orphan reconciliation、verify / rebuild 与删除执行。随后接入 application / macOS 宿主，完成真实 Keychain 与端到端验收。
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

2026-09-15 日终基线 `b484656`：本机完整仓库检查、191 个 Rust tests、35 个检查器回归和 1 个 compile-fail doctest 通过；跨进程 helper 由父测试实际执行。初始化、回滚和并发均使用合成数据与测试 provider。各批次数量、代码与文档复核见[日终记录](2026-09-15-source-vault.md)，初始化机制见[落地记录](../implementation/phase1-source-vault-key-bootstrap.md)。

Linux ARM64 / ext4 filesystem 证据来自 `9d88319`；Windows ARM64 / NTFS 继续引用 9 月 10 日基线。它们不覆盖后续新增的 provider / SQLite 协调代码。平台范围及清理分别见[filesystem 记录](../implementation/phase1-source-vault-filesystem.md#linux-arm64--ext4-普通用户验收2026-09-15)和[9 月 10 日记录](2026-09-10-source-vault.md)。真实密钥库、当前代码的 Windows / Linux 编译和运行、远程 CI 均尚未验证。

## 后续事项

`P1-S03c-2` 已完成独立 provider、六项精确直接依赖、22-package 增量及 366 项 notices 落地，见[实现与验收记录](../implementation/phase1-source-vault-key-provider.md)。macOS locked Clippy / 合成测试通过；Windows / Linux provider 编译、真实系统密钥库与上游 logger 过滤仍待分平台验证。P1-S04a 新增受事务约束的 library initializer，slot-only 创建 / setter 继续不公开；资格来自实际数据库与对象目录，不接受调用方自报。初始化使用合成 provider 验证，尚未接入宿主。

明日首项为 P1-S04b 正文迁移与恢复，具体切片和验收见[明日事项（2026-09-16）](2026-09-15-source-vault.md#明日事项2026-09-16)；R01 至 R06、宿主接入及集中跨平台验证遵循上述顺位与停止线。9 月 10 日[明日验收清单](2026-09-10-source-vault.md#明日事项2026-09-11)保留为历史安排，不再作为未完成状态。下一批真实 key store、依赖或 VM / 系统变更须按具体范围授权。
