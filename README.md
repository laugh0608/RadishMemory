# RadishMemory

`RadishMemory` 是一个用户拥有、模型无关、隐私优先的个人长期记忆与上下文系统。

长期目标是允许用户持续保存短语、灵感、文件、对话、网页、图片、音频和其它个人资料，通过可追溯的记忆生命周期、混合检索与上下文编译，为 GPT、Gemini、Claude、Grok、DeepSeek、本地模型及未来模型提供长期记忆。

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
- [阶段 1 Source Vault portable crypto 落地记录](docs/implementation/phase1-source-vault-portable-crypto.md)
- [阶段 1 桌面宿主依赖评审](docs/implementation/phase1-desktop-dependency-review.md)
- [阶段 1 macOS 桌面宿主交互验收](docs/implementation/phase1-macos-host-acceptance.md)
- [隐私与威胁模型](docs/privacy-threat-model.md)
- [与 RadishMind 的边界](docs/radishmind-boundary.md)
- [MVP 路线图](docs/mvp-roadmap.md)
- [仓库治理](docs/governance/repository-governance.md)
- [分支、PR 与 Ruleset ADR](docs/adr/0001-branch-and-pr-governance.md)
- [参考系统与研究问题](docs/references.md)

## 当前状态

当前处于 `Phase 1 Source Vault portable crypto complete; immutable object adapter next`；`Phase 1 host acceptance complete` 是已有合成宿主验收事实，具体能力与近期缺口见[当前状态](docs/status/current.md)。

- 已有 canonical core、SQLite v6 connection / migration、来源与记忆事件、FTS5、本地删除证据和真实 M0 runner；runner 的词项扩展、历史投影与部分断言存在[证据限制](docs/evaluation/m0-fixture-contract.md#当前实现的证据边界)。
- [ADR 0006](docs/adr/0006-phase1-text-markdown-file-entry.md)的文本入口与 [ADR 0007](docs/adr/0007-phase1-local-library-host.md)的 application service / 桌面宿主已落地，支持合成 UTF-8 `.txt` / `.md` 导入、更新、搜索、版本导出与本地删除。已有系统 picker 和合成测试证据不授权本任务使用真实个人资料，也不等于日常资料库或签名发行包已经完整可用。
- [ADR 0008](docs/adr/0008-phase1-encrypted-source-vault.md)冻结一 source version 一密文对象；当前产品仍使用 SQLite v6 inline plaintext body，不能声明加密 Source Vault 已可用或整个资料库已静态加密。FTS 当前保存整文件片段的完整可读正文，未来仅加密原始对象仍不保护这份正文副本。
- P1-S02 选择 XChaCha20-Poly1305 + STREAM-BE32；P1-S03a 已完成 portable manifest / `Cargo.lock`、cipher / wrap / AAD 与合成测试。[P1-S03a 落地记录](docs/implementation/phase1-source-vault-portable-crypto.md)记录扩大到 344 项的 notices；三个 platform provider、object filesystem、SQLite migration 与宿主加密数据流尚未实现。

2026-09-05 审阅发现中文词语搜索不命中、桌面目录只取前 200 条、启动失败后无法进入派生重建等问题，详见[审阅记录](docs/implementation/2026-09-05-project-review.md)和[质量验收计划](docs/evaluation/phase1-local-library-quality.md)。这些问题尚未因文档更新而修复。

首个重点验证场景是围绕一个长期项目保存资料、找回依据、确认和更正事实，再提供受控上下文。它是产品验证方向；完整记忆控制台、模型问答、PDF / 图片、向量、多模型、同步、恢复和个人伴侣均不在当前已实现能力中。阶段依赖以[MVP 路线图](docs/mvp-roadmap.md)为准，历史 CI / 批次证据见[阶段基线归档](docs/status/2026-09-03-baseline.md)。

## 仓库数据边界

本 Git 仓库只承载代码、规范、治理资产和合成 / 明确脱敏的测试材料，不是用户资料库或 Source Vault。真实个人文件、对话、记忆、ContextPack、Embedding 输入、密钥、本地数据库、同步状态和备份不得进入 Git、Issue、Pull Request 或 CI。

本地仓库检查入口：

```bash
./scripts/check-repo.sh
```

## 许可证

本仓库采用 [RadishMemory Source-Available License](LICENSE)，不是开放源码许可证。未经版权所有者书面许可，不授予复制、修改、再分发或商业使用权。

用户自部署是产品目标，不等于本仓库已授予安装、复制或修改等使用授权。外部试用与发行前需明确授权渠道、分发形式和维护范围；本轮文档不改变 `LICENSE`。

桌面目标的第三方 crates、选定 license option、checksum、默认字体与 bundled SQLite 归属见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 和 [第三方许可证文本](third_party/licenses/README.md)；这些第三方条款不改变 RadishMemory 自身许可证。
