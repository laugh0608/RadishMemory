# RadishMemory MVP 路线图

能力归属、原实施单元与验收读取链见[专题地图](topics.md)；实际能力与近期顺位只以[current](status/current.md)为入口。本文保留阶段目标、依赖与出口，历史实现流水见[整理前快照](status/planning-before-alignment-2026-11-01.md#radishmemory-mvp-路线图)。

## MVP 要证明的事情

MVP 不以功能数量为目标，而要证明一个可信闭环：

> 用户随手保存资料，经过大量历史后仍能可靠找回并引用来源；用户能纠正或忘记记忆；未授权资料不会被发送给云端；资料能通过用户自部署服务安全地到达第二台设备。

## 阶段 0：边界与评测基线

目标：实现前先确定能否客观判断记忆系统变好或变坏。

交付：

- 产品范围、架构、记忆模型和威胁模型评审；
- [M0 本地记忆闭环](adr/0002-m0-local-memory-loop.md)及其[合成验收边界](evaluation/m0-local-memory-loop.md)；
- 已冻结的 [M0 Canonical Schema](schema/m0-canonical-schema.md)；
- 已冻结的 [M0 Fixture 与指标契约](evaluation/m0-fixture-contract.md)；
- 至少覆盖事实、偏好、时间变化、冲突、敏感度和删除的本地评测集；
- 全历史输入、全文检索、向量检索和混合检索的基线方案；
- 已通过 [ADR 0003](adr/0003-zero-knowledge-sync-first.md) 冻结的首个零知识同步信任模式；
- 已通过 [ADR 0004](adr/0004-radishmind-optional-gateway-entry.md) 冻结的 RadishMind 可选 Gateway 首次接入阶段；
- 已通过 [ADR 0005](adr/0005-m0-implementation-stack.md) 冻结的 Rust 模块化单体、SQLite / FTS5、依赖和验证基线。

停止线：不先选择复杂图数据库、不先制作虚拟形象、不声明新记忆算法。

阶段 0 的首个可执行验证是 M0：只用合成文本与 Markdown、本地全文基线和确定性 proposal / decision 流程证明来源、引用、时间更正、失败关闭和删除证据。它不要求向量、模型、RadishMind 或同步先存在。

## 阶段 1：单机资料库与可追溯问答

目标：完成单用户、单设备、本地优先的资料采集、检索、引用、导出与删除能力。

范围：

- [在桌面直接保存短语和 Markdown](adr/0010-phase1-desktop-text-entry.md)，支持原文查看和更正历史；
- 导入文本、PDF 和常见图片；
- 原始对象与派生文本分离保存；
- 全文检索和一个可替换向量索引；
- 按来源、时间、类型和敏感度过滤；
- 使用 mock 或一个直接模型 adapter 生成带引用回答，不以 RadishMind 为启动依赖；
- 显示每个回答使用的来源；
- 基础导出和本地删除。

验收：

- 原件可恢复、派生索引可删除并重建；
- 问答引用能定位到具体片段、页码或时间码；
- `local_only` 内容在云端调用前失败关闭；
- 无模型时仍能采集、浏览和搜索。

阶段 1 不把上述范围一次性展开为大批次。首个评审单元已通过 [ADR 0006](adr/0006-phase1-text-markdown-file-entry.md) 冻结用户显式选择的文本 / Markdown 文件入口、身份 / 版本、导出与删除边界及 18 个合成验收场景；[ADR 0007](adr/0007-phase1-local-library-host.md) 冻结 application service、来源目录、桌面 UI 与十二项宿主验收。[ADR 0008](adr/0008-phase1-encrypted-source-vault.md) 冻结一 source version 一密文对象、认证加密、密钥包装、提交协调、迁移和删除边界；P1-S02 的密码套件和精确 provider 沿用[专项评审](implementation/phase1-encrypted-source-vault-dependency-review.md)。这些是正式合同与进入条件，实际完成范围不在路线图重复维护。

[P1-S03a portable crypto dependency landing](implementation/phase1-source-vault-portable-crypto.md)负责 manifest / lockfile / notices 变化、portable dependency graph、AAD 与密码向量；[P1-S03b immutable object filesystem adapter](implementation/phase1-source-vault-filesystem.md)负责 versioned envelope、对象 / staging capability、durable no-overwrite publish 与认证 read-back。provider、P1-S04a / S04b、capture、维护、删除、读取、application 和 host 的原切片由[专题地图](topics.md#子专题主要归属)串联；平台和完整 P1-S05 验收以原记录与 current 为准，不从独立 adapter 通过推导产品可用。

PDF / 图片解析只能在 encrypted Source Vault 的 adapter、migration 和 host acceptance 完成后另行评审；向量实现和模型 adapter 继续分别审查隐私失败关闭和质量指标。默认桌面产品仍以 SQLite v6 inline plaintext body 运行；显式维护 / 加密入口与默认入口的版本和保护范围见 current，不因组织整理改变。M01 身份绑定与暂缓条件、真实系统调用权限、原生失败窗口和跨平台差异继续沿[current 后续事项](status/current.md#后续事项)处理，不能以旧合成、正常退出或历史授权替代。

### 阶段 1 的用户路径与质量补齐

阶段 1 不以格式数量或模型数量证明完成。当前文本能力应先能通过 production 入口完成保存、中文找回、完整目录访问、回源、版本更新、导出与删除，并能在派生损坏后进入受限修复。具体计划见[本地资料库质量验收](evaluation/phase1-local-library-quality.md)，这些场景尚未因文档加入而通过。

[ADR 0009](adr/0009-versioned-chinese-recall.md)定义版本化中文短语检索与显式升级 / 降级边界，实施与局部结果见[生产切片](implementation/phase1-chinese-recall.md)。人工标签、原生切换、跨平台和规模成本按 Q01 / Q04 原标准分别验证；实际接续顺序见 current，不从固定样本通过关闭完整质量场景。

扩大资料规模前先建立启动、搜索、导入、重建和内存基线；原始对象加密接入后分别报告对象文件、SQLite / FTS 正文与恢复能力。既有文件 / 宿主合成证据仍按各自实际场景解释，不替代新增质量验收。

第一次真实云端模型调用就必须满足[隐私模型](privacy-threat-model.md#模型外发控制)要求的权限与外发过滤、最小 ContextPack、OutboundContextManifest、接收方 / 用途和实际 attempts 记录。阶段 3 扩展多模型及完整 Context Compiler；不能把最小外发清单推迟到阶段 3。此处澄清既有安全依赖，不授权当前接入模型。

围绕长期项目的窄文本价值路径可以作为阶段 1 至 3 的连续验证主线。提前实现阶段 2 / 3 能力或改变 PDF / 图片的既有依赖属于待决策事项，需在修改 ADR 0004、ADR 0008 或阶段顺位前确认范围。

## 阶段 2：长期记忆生命周期

目标：从“资料搜索”进入“长期记忆”。

范围：

- Observation、Claim、Episode、Preference 和 Procedure；
- MemoryProposal 审查收件箱；
- confirmed、rejected、superseded、contradicted、retracted、expired 状态；
- 时间有效性和 point-in-time 查询；
- 用户显式“记住”“更正”“忘记”；
- 背景去重与整理候选；
- 记忆污染和错误个人化评测。

验收：

- “以前喜欢 A、现在喜欢 B”不会被静默覆盖；
- 被拒绝候选不会用相同来源反复出现；
- 模型不能绕过 MemoryProposal 直接修改确认记忆；
- 当前回答能说明使用了事实、偏好还是推断。

## 阶段 3：多模型与 Context Compiler

目标：证明记忆与具体模型解耦。

范围：

- canonical ModelRequest / ModelResponse；
- Memory Compiler 与可解释 ContextPack；
- OutboundContextManifest；
- 至少两个云端 Provider 和一个本地模型；
- Provider 级外发策略、用量和错误记录；
- 在直接 adapter 基线成立后可选接入 RadishMind Gateway；
- 首次 RadishMind 接入不包含 Workflow、Tooling、RAG 数据 owner 或业务写回。

验收：

- 更换模型不修改记忆 canonical schema；
- 同一任务可以比较不同模型使用同一 ContextPack 的结果；
- 每次云端调用能列出实际使用的来源；
- Gateway、允许 Provider 集合和实际 Provider attempts 均进入外发清单；
- Provider 或 Gateway 失败不会隐式回退到不符合隐私策略的模型或另一条 adapter 路径；
- RadishMind 不可用时，本地能力与直接 adapter 基线不受影响。

## 阶段 4：用户自部署同步

目标：完成多端使用和用户数据主权的关键闭环。

2026-10-12 已确认[客户端连接规划](client-server-connection-plan.md)：同一官方安装包运行时选择官方端点或用户自部署后端，无需重新编译。该目标纳入阶段 4，不提前进入当前本地桌面 / Source Vault 批次，也不改变首个零知识模式。

范围：

- 可自部署的零知识服务端，以及遵守相同兼容协议的官方同步端点交付；
- 设备身份、注册、撤销和恢复；
- 加密对象与操作日志同步；
- 冲突、离线写入和幂等；
- 第二台设备恢复资料、记忆状态和删除状态；
- 零知识同步服务及其可见元数据、客户端密钥和语义计算边界；
- 可信计算节点后置，不作为默认同步服务或首个同步批次的一部分。

验收：

- 同一未改动官方安装包完成官方 / 自部署选择、兼容检查、授权、同步、停止、重启与切换；两类路径分别按实际开放范围取得真实目标证据，官方未开放不额外阻塞兼容自部署验收；
- 实例 / 同步域的凭据、设备授权、密钥封装、cursor、删除与恢复证据不会串用，改地址不自动迁移或上传资料；
- 服务端关闭期间本地仍可采集和搜索，官方端点不可用不阻断兼容自部署路径；
- 两台设备并发修改不会静默丢失确认记忆；
- 被撤销设备不能获取后续数据；
- 删除状态可以跨设备传播并准确显示未完成节点；
- 备份恢复不会造成对象和记忆版本回退；
- 服务端存储、日志和传输载荷不包含内容明文、未包装内容密钥、语义索引或 ContextPack；
- 篡改、重放、缺失和未知协议版本不会被静默接受。

## 阶段 5：个人伴侣体验

目标：让同一记忆核心形成有连续感但不过度自作主张的个人伴侣。

范围：

- 连续对话和会话恢复；
- 可编辑 persona 与表达风格；
- 虚拟形象、语音和状态表现；
- 用户授权的主动提醒和回顾；
- “为什么这样回答”和“为什么记得”的可见解释；
- 所有产品面共用同一个 Memory Engine。

停止线：虚拟形象不能创建独立画像；主动行为不能绕过通知、权限和时间策略。

## 长期研究与连续性验证

[长期记忆研究专题](long-term-memory-research.md)将跨会话找回、更正与历史解释、跨模型使用和受控删除映射到阶段 1 至 3，并将设备 / 恢复连续性归入阶段 4。参数个性化与推理时神经记忆是后置可选研究，不是 MVP 的新增依赖；只有现有基线与评测揭示明确缺口后，才另行评审实验范围。该专题不修改当前阶段顺位、schema 或权限契约。

## 首批评测指标

| 指标 | 说明 |
| --- | --- |
| Retrieval Recall@k | 正确来源是否进入候选集合 |
| Answer correctness | 答案是否被正确资料支持 |
| Citation accuracy | 引用是否指向真正支持答案的片段 |
| Temporal accuracy | 时间变化和历史状态是否正确 |
| Conflict handling | 冲突证据是否被识别而非静默覆盖 |
| Memory pollution rate | 错误、重复、无来源候选进入长期层的比例 |
| False personalization rate | 未经证据错误推断用户偏好的比例 |
| Context compression ratio | ContextPack 相对全历史的 Token 缩减 |
| Policy violation count | 未授权检索或外发次数，目标为零 |
| Delete completeness | 删除传播到各存储、索引和设备的比例 |
| Retrieval latency | 不同数据量下的本地与服务端延迟 |
| Provider cost | 不同编译策略的模型调用成本 |

## 可持续使用与交付验证

记忆控制台除污染、错误个人化和时间一致性外，记录候选接受 / 拒绝率、重复候选率、确认耗时及持续维护时间，避免用候选数量代替用户收益。检索同时覆盖中文 / 混合语言、高相似干扰和无答案场景；性能阈值须在代表性数据和环境下确定。

原件精确导出、完整资产迁移、备份恢复分别验收。迁移契约应明确来源与记忆关系、版本、决定、策略和必要证据；恢复方案应明确密钥保管、设备 / 系统重装和损坏后的真实结果。具体格式、恢复方式和所在交付批次仍待独立决策，不自动扩展阶段 1 加密范围。

外部试用与发行前须明确可用功能、已知限制、分发 / 自部署授权及维护范围；可公开阅读源码和允许用户部署是不同问题，现行许可证保持不变。

## 完整 MVP 首个可演示场景

以下场景属于完整 MVP，不是 M0 的首批代码范围。建议完整 MVP 的首个真实演示只覆盖一条高价值路径：

1. 用户保存一句灵感和一个相关 PDF；
2. 系统保留原件并生成可审查片段；
3. 在大量无关资料中询问此前想法；
4. 系统找回短语和 PDF，生成带引用回答；
5. 系统提出一个偏好或项目事实候选；
6. 用户确认、随后更正，再查询当前与历史状态；
7. 用户要求忘记，系统展示删除传播证据；
8. 第二台设备同步后不再召回该内容。

这条路径可靠之前，不以插件数量、模型数量、UI 页面数量或虚拟形象精度作为主要进度。
