# RadishMemory

`RadishMemory` 是一个用户拥有、模型无关、隐私优先的个人长期记忆与上下文系统。

它允许用户持续保存短语、灵感、文件、对话、网页、图片、音频和其它个人资料，通过可追溯的记忆生命周期、混合检索与上下文编译，为 GPT、Gemini、Claude、Grok、DeepSeek、本地模型及未来模型提供长期记忆。

本项目只有一个产品名称：`RadishMemory`。个人伴侣、虚拟形象、聊天、写作和编程助手都是它之上的产品体验，不拆成独立项目。

## 核心定义

RadishMemory 不是“无限聊天记录”，也不只是向量数据库或传统个人知识库。它的核心职责是：

> 把无限、私密、持续变化的个人资料，编译成当前任务需要的有限、可靠、可追溯上下文。

模型上下文窗口无论增长到多大，都仍然是一次推理的工作内存；RadishMemory 承担跨会话、跨设备、跨模型的长期记忆真相、隐私策略和检索治理。

## 产品原则

- **用户拥有**：用户拥有数据、密钥、部署和迁移权，不依赖必须存在的托管云。
- **本地优先**：单机必须可用；多端同步通过用户自部署服务完成。
- **模型无关**：任何模型只能消费受控上下文并提出记忆候选，不能成为记忆真相源。
- **来源可追溯**：回答和记忆都应能回到原始资料、时间与处理记录。
- **写入可治理**：模型推断默认是候选；确认、更新、冲突、过期和撤回都有明确状态。
- **索引可重建**：全文、向量、图、摘要和缓存都是派生索引，不是唯一真相。
- **遗忘可验证**：删除必须覆盖正文、索引、缓存、同步副本和受控备份生命周期。
- **隐私是硬边界**：权限、敏感度和外发策略在检索前过滤，不能靠相关性分数软控制。

## 文档入口

- [协作入口](AGENTS.md)
- [文档索引](docs/README.md)
- [当前状态](docs/status/current.md)
- [产品范围](docs/product-scope.md)
- [系统架构](docs/architecture.md)
- [记忆模型](docs/memory-model.md)
- [M0 Canonical Schema](docs/schema/m0-canonical-schema.md)
- [M0 Fixture 与指标契约](docs/evaluation/m0-fixture-contract.md)
- [阶段 1 文本 / Markdown 文件入口 ADR](docs/adr/0006-phase1-text-markdown-file-entry.md)
- [阶段 1 本地资料库宿主与显式文件授权 ADR](docs/adr/0007-phase1-local-library-host.md)
- [阶段 1 加密内容寻址 Source Vault ADR](docs/adr/0008-phase1-encrypted-source-vault.md)
- [阶段 1 加密 Source Vault 依赖与密码套件评审](docs/implementation/phase1-encrypted-source-vault-dependency-review.md)
- [阶段 1 桌面宿主依赖评审](docs/implementation/phase1-desktop-dependency-review.md)
- [阶段 1 macOS 桌面宿主交互验收](docs/implementation/phase1-macos-host-acceptance.md)
- [隐私与威胁模型](docs/privacy-threat-model.md)
- [与 RadishMind 的边界](docs/radishmind-boundary.md)
- [MVP 路线图](docs/mvp-roadmap.md)
- [仓库治理](docs/governance/repository-governance.md)
- [分支、PR 与 Ruleset ADR](docs/adr/0001-branch-and-pr-governance.md)
- [参考系统与研究问题](docs/references.md)

## 当前状态

当前处于 `Phase 1 encrypted Source Vault cipher profile accepted; portable dependency landing next` 阶段；`Phase 1 host acceptance complete` 仍是已成立的前置事实。M0 本地记忆闭环和阶段 1 文本 / Markdown 文件入口均已通过 Linux、macOS、Windows locked CI 并合入稳定主线；[ADR 0007](docs/adr/0007-phase1-local-library-host.md)进一步冻结并完成本地桌面宿主、一次性文件选择授权、production application service、来源目录、UI 和十二项宿主验收。当前 `wgpu` head `c5dba35` 已在 [workflow run 33751048480](https://github.com/laugh0608/RadishMemory/actions/runs/33751048480) 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`，333 个目标可达 crate 的 [third-party notices](THIRD_PARTY_NOTICES.md)、license option、默认字体、bundled SQLite 与条件平台依赖也已收口。[ADR 0008](docs/adr/0008-phase1-encrypted-source-vault.md) 已冻结原始对象认证加密、一 source version 一密文对象、设备本地 KEK 包装、文件系统 / SQLite 提交协调、v6 migration、删除和十八项合成验收；[P1-S02 依赖与密码套件评审](docs/implementation/phase1-encrypted-source-vault-dependency-review.md)进一步冻结 XChaCha20-Poly1305 + STREAM-BE32、AEAD DEK wrap、系统随机、secret zeroization 与三平台精确 key provider。当前实现仍是 SQLite v6 inline plaintext body，下一步只允许在独立授权下落地 portable crypto dependency / known-answer tests，不自动授权 object adapter、系统 key store、PDF、向量、模型、网络或同步。阶段顺位、停止线和当前验证入口以[当前状态](docs/status/current.md)为准。

首个可执行切片 [M0 Local Memory Loop](docs/adr/0002-m0-local-memory-loop.md) 已使用合成文本 / Markdown、本地全文基线和确定性 proposal / decision 流程验证来源、引用、时间更正、失败关闭和单设备删除证据，不依赖模型、网络、RadishMind 或同步。

M0 的九种顶层对象已经在 [M0 Canonical Schema](docs/schema/m0-canonical-schema.md) 中冻结为实现中立的字段级契约；[M0 Fixture 与指标契约](docs/evaluation/m0-fixture-contract.md)进一步冻结 JSON mapping、稳定 ID、摘要向量、12 个场景的 86 个操作和指标 oracle。契约本身不证明实现，但真实 runner 已在同一 core 与 SQLite adapter 上执行全部步骤和门禁；证据边界以[当前状态](docs/status/current.md)为准。

首个多设备同步信任模式已由 [ADR 0003](docs/adr/0003-zero-knowledge-sync-first.md) 冻结为零知识同步服务：默认服务端只中继密文和最小元数据，解密、索引、检索和记忆计算留在受信设备；这仍是待实现、待密码协议评审和待验证的目标边界。

RadishMind 首次运行接入已由 [ADR 0004](docs/adr/0004-radishmind-optional-gateway-entry.md) 后置到完整 MVP 阶段 3，并且只作为可选 Model Gateway；M0、单机资料库和记忆生命周期不依赖它，首次不接 Workflow、Tooling 或共享业务数据库。

M0 的首个产品实现栈已由 [ADR 0005](docs/adr/0005-m0-implementation-stack.md) 冻结为 Rust 2024 模块化单体，使用独立 core、SQLite adapter 和 runner package；本地小文本与结构化事实使用 SQLite，全文基线使用 FTS5。该决定不冻结未来 UI 或服务端语言，也不代表加密存储已经实现。

阶段 1 的首个真实文件入口已由 [ADR 0006](docs/adr/0006-phase1-text-markdown-file-entry.md) 冻结为显式选择、允许根内、最大 8 MiB 的 UTF-8 `.txt` / `.md`。它定义来源版本、幂等、当前 lineage tip、精确导出、受管副本删除和 18 个合成场景；`P1-F01` 至 `P1-F18` 已实现并在 [PR #2](https://github.com/laugh0608/RadishMemory/pull/2) 的 [workflow run 33302423840](https://github.com/laugh0608/RadishMemory/actions/runs/33302423840) 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`，随后以 merge commit `c56f13f` 合入 `master` 并回流 `dev`。desktop host 代码、本机合成测试、macOS / Windows / Linux 真实窗口与原生 picker 证据、当前 `wgpu` 图的三平台 CI，以及可复现的目标依赖 / notices 已经建立；这完成 P1-H05，但不等于已有签名发行包，也不授权本任务使用真实个人资料。

阶段 1 的加密 Source Vault 已由 [ADR 0008](docs/adr/0008-phase1-encrypted-source-vault.md) 冻结为“只保护受管原始对象”的本地认证加密契约。SQLite metadata、FTS 与派生内容仍可能是明文；每个 SourceArtifact version 首批对应独立密文对象，不跨 provenance 去重；精确 cipher suite 与 key-provider profile 已完成评审但尚未进入依赖图，object adapter、migration 和宿主接入也尚未实现或授权，因此当前产品不能声明加密 Source Vault 已可用或整个资料库已静态加密。

`P1-S02` 已选定 `radishmemory.xchacha20poly1305-stream-be32/1` 与 `radishmemory.xchacha20poly1305-dek-wrap/1`，继续复用 `getrandom =0.4.3`，并按 target 选择 macOS Keychain、Windows Credential Manager 与 Linux Secret Service。该评审尚未修改 manifest / `Cargo.lock` 或访问真实 key store；精确 resolved graph、checksums、notices、known-answer tests、adapter、migration 与三平台宿主行为仍需后续独立授权和证据。

当前已完成精确 Rust `1.96.0` 工具链、三个 M0 package、canonical core、SQLite v6 connection / migration、Source Vault、MemoryStore、FTS5 业务索引、可重建当前投影、本地删除组件 / 证据链、真实 M0 runner、受审阅 lockfile 和 Linux / macOS / Windows locked CI；Phase 1 另有 `radishmemory-file-entry`、`radishmemory-application` 与 `radishmemory-desktop`。desktop host 通过 application service 提供应用数据目录、稳定 host profile、production random / UTC runtime、一次性 native picker，以及 import / update、目录 / 历史、search citation、exact export、完整 lineage deletion evidence、verify / rebuild UI。[Rust 依赖基线](docs/implementation/m0-rust-dependency-baseline.md)记录当前依赖图、原生构建和证据边界，[macOS 交互验收](docs/implementation/phase1-macos-host-acceptance.md)、[Windows 交互验收](docs/implementation/phase1-windows-host-acceptance.md)与 [Linux 交互验收](docs/implementation/phase1-linux-host-acceptance.md)记录单机证据，[第三方 notices 与条件平台依赖复核](docs/implementation/phase1-third-party-notices.md)完成其最后分发清单门禁。真实个人资料、签名包和发布仍须独立授权与验证。

当前不把以下内容声明为已实现：

- 真实个人文件导入、导出与可用产品入口；
- PDF / 图片解析、向量检索或带引用模型问答；
- 长期记忆算法；
- 加密多端同步；
- 生产可用部署；
- 多模型兼容；
- 虚拟形象或主动陪伴；
- 可证明删除；
- 零知识服务端。

## 仓库数据边界

本 Git 仓库只承载代码、规范、治理资产和合成 / 明确脱敏的测试材料，不是用户资料库或 Source Vault。真实个人文件、对话、记忆、ContextPack、Embedding 输入、密钥、本地数据库、同步状态和备份不得进入 Git、Issue、Pull Request 或 CI。

本地仓库检查入口：

```bash
./scripts/check-repo.sh
```

## 许可证

本仓库采用 [RadishMemory Source-Available License](LICENSE)，不是开放源码许可证。未经版权所有者书面许可，不授予复制、修改、再分发或商业使用权。

桌面目标的第三方 crates、选定 license option、checksum、默认字体与 bundled SQLite 归属见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 和 [第三方许可证文本](third_party/licenses/README.md)；这些第三方条款不改变 RadishMemory 自身许可证。
