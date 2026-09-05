# RadishMemory 当前状态

更新时间：2026-09-05

## 当前阶段

`Phase 1 Source Vault portable crypto complete; immutable object adapter next`

M0、文本 / Markdown 文件入口和本地桌面宿主已建立；原始对象加密只完成独立 portable crypto，尚未接入产品数据流。当前 production code 仍是 SQLite v6 inline plaintext body。项目具备受约束的工程原型，但中文找回、完整目录访问、启动失败后的派生修复和生产验收仍有缺口，不能据历史合成验收宣称日常资料库已完整可用。

本轮只完善文档与检查路由，没有修复产品代码或改变 ADR。已确认问题、静态发现和待测风险见[2026-09-05 项目审阅](../implementation/2026-09-05-project-review.md)；截至 2026-09-03 的详细提交、三平台 CI、依赖数量与 M0 完成流水见[阶段基线归档](2026-09-03-baseline.md)。

## 能力与证据

| 能力 | 已成立 | 当前限制与真相源 |
| --- | --- | --- |
| M0 领域与存储 | canonical core、SQLite v6、来源 / 记忆 / 事件、FTS5、本地删除与真实 M0 runner 已经建立 | [ADR 0002](../adr/0002-m0-local-memory-loop.md)；runner 编排不等于 production 历史查询与上下文编译接口 |
| 字段与 fixture | M0 字段级 canonical schema 定义九种顶层对象；fixture 固定 12 个场景的 86 个有序操作和 12 个指标 gate | [M0 Canonical Schema](../schema/m0-canonical-schema.md)不绑定数据库、生产 ID 编码或语言类型；[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md)说明实际证据限制 |
| 文件入口 | `radishmemory-file-entry` 的 P1-I01 file snapshot contract、P1-I02 atomic source capture、P1-I03 exact export、P1-I04 lineage deletion 已落地 | [ADR 0006](../adr/0006-phase1-text-markdown-file-entry.md)；`SourceCaptureStore` 原子提交，`P1-F01` 至 `P1-F18` 已有三平台证据，故障 seam 仅 opt-in `acceptance-test-support`；不代表完整 importer / exporter 已实现 |
| 本地宿主 | P1-H02 application service、P1-H03 source catalog、P1-H04 desktop UI、P1-H05 host acceptance 已有实现及合成宿主证据 | [ADR 0007](../adr/0007-phase1-local-library-host.md)的 `P1-HF01` 至 `P1-HF12` 保留历史记录；目录第 201 条和重启后损坏修复等缺口尚未关闭 |
| 加密 Source Vault | P1-S01 storage contract、P1-S02 dependency and cipher review 与 P1-S03a portable crypto dependency landing 已完成对应范围 | [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)的 `P1-SF01` 至 `P1-SF18` 尚未全部实现；object adapter、真实 key provider、migration、宿主接入仍待后续批次 |
| 用户价值 | 已能通过本地入口导入、版本化、搜索、精确导出和删除合成文本 | 尚无完整记忆控制台、模型问答、PDF / 图片解析、向量、同步、恢复或签名发行包 |

## 当前顺位

1. 下一已定义实现单元仍为 `P1-S03b immutable object filesystem adapter`：范围限 versioned envelope、应用专用 object / staging capability、durable no-overwrite publish 与认证 read-back，须在具体实现任务授权后执行。
2. 近期修复计划应先明确审阅项 R01 至 R06 的范围与验收：中文检索、目录分页、派生损坏维护入口、读取性能、runner 证据和回源 / 结果刷新。场景见[本地资料库质量验收计划](../evaluation/phase1-local-library-quality.md)，目前全部待实现或待测，不以新增文档标记完成。
3. 后续按 ADR 0008 分别收口 platform provider、SQLite migration 与宿主验收。PDF / 图片解析继续等待 encrypted Source Vault 完整链路成立。
4. 产品验证聚焦“同一个长期项目的资料、关键事实、更正与受控上下文”；正式阶段依赖仍以[MVP 路线图](../mvp-roadmap.md)为准。提前跨阶段交付、整库加密、密钥恢复方案、许可证和分发授权均为待决策事项，文档更新不构成实现授权。

## 已接受的边界

- [ADR 0005](../adr/0005-m0-implementation-stack.md)冻结 Rust 2024 模块化单体，首个工具链固定为 Rust `1.96.0`。当前依赖、manifest / lockfile 和 notices 以[Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)及[P1-S03a 落地记录](../implementation/phase1-source-vault-portable-crypto.md)为准。
- P1-S02 已选择 `radishmemory.xchacha20poly1305-stream-be32/1` 与 `radishmemory.xchacha20poly1305-dek-wrap/1`；P1-S03a 已使 portable manifest / `Cargo.lock` / notices 和 cipher 实现落地，尚未访问真实系统 key store。
- 原始对象按 source version 独立认证加密，不跨 provenance 物理去重；publish → SQLite commit → read-back 必须完整成立。未知 profile、缺 key、认证失败或 ambiguous state 失败关闭，不回退旧 BLOB 或外部原件。
- 首批对象加密不覆盖 SQLite metadata、FTS、派生数据；当前整文件片段使 FTS 保存完整可读正文，不能简化为“只有少量元数据未加密”。历史明文、进程内明文、交换区、快照、备份和用户导出仍在该保证之外。
- [ADR 0003](../adr/0003-zero-knowledge-sync-first.md)选择零知识同步服务，可信计算节点后置为显式可选能力；不代表零知识同步已经实现。
- [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md)将首次接入放在完整 MVP 阶段 3，以显式可关闭的 Model Gateway 接入；首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回。本地能力不依赖 RadishMind。

## 当前停止线

- 不将本轮文档更新视为产品缺陷已修复，不扩大 P1-H05、M0 fixture 或 portable crypto 的证据范围。
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

2026-09-05 审阅时，`./scripts/check-repo.sh` 在解除合成测试 loopback 端口限制后通过，包含 140 个 Rust 测试；默认 features 的 `cargo check --workspace --all-targets --locked --offline` 通过。中文检索、FTS 完整正文副本和派生损坏后打开失败已用实际 application / SQLite 3.53.2 复现；没有重跑三平台 GUI、远程 CI、性能基准或独立密码学审计。该记录是审阅基线，本轮文档变更的验证以任务交接为准。
