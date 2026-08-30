# RadishMemory MVP 路线图

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

- 快速保存短语和 Markdown；
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

阶段 1 不把上述范围一次性展开为大批次。首个评审单元已通过 [ADR 0006](adr/0006-phase1-text-markdown-file-entry.md) 冻结用户显式选择的 UTF-8 文本 / Markdown 文件入口、来源身份 / 版本、幂等导入、精确导出、派生重建、删除边界和 18 个合成验收场景；该入口已经通过 Linux、macOS、Windows locked CI 并合入稳定主线。下一评审单元由 [ADR 0007](adr/0007-phase1-local-library-host.md) 冻结本地桌面宿主、一次性文件授权、application service、来源目录、基础 UI 和十二项宿主验收；P1-H02 / P1-H03 与经独立依赖授权的 P1-H04 desktop host 已完成本机实现，P1-H05 已记录纯合成数据的 macOS 可见窗口、AppKit open / save picker 与 canonical / binding 损坏失败关闭交互，当前继续补齐 Linux / macOS / Windows desktop CI 和可分发依赖清单。PDF / 图片解析、向量实现和模型 adapter 继续分别审查依赖、许可证、native build、隐私失败关闭和质量指标。

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

范围：

- 自部署服务端；
- 设备身份、注册、撤销和恢复；
- 加密对象与操作日志同步；
- 冲突、离线写入和幂等；
- 第二台设备恢复资料、记忆状态和删除状态；
- 零知识同步服务及其可见元数据、客户端密钥和语义计算边界；
- 可信计算节点后置，不作为默认同步服务或首个同步批次的一部分。

验收：

- 服务端关闭期间本地仍可采集和搜索；
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
