# RadishMemory 隐私与威胁模型

## 核心结论

自托管是 RadishMemory 的必要能力，但自托管本身不等于隐私安全、端到端加密或零知识。

系统必须明确回答：

- 明文在哪里出现；
- 谁持有解密密钥；
- 哪些设备和服务被用户信任；
- 使用云端模型时发送了什么；
- 删除和设备吊销能保证到什么程度；
- 备份、日志和派生索引是否仍可能保留数据。

任何 UI、文档和部署声明都不得超出实际信任模式。

## 保护资产

- 原始文件、短语、图片、音频、对话和网页快照；
- 结构化事实、偏好、关系、时间线和程序性记忆；
- Embedding、摘要、缓存和检索结果；
- 用户身份、设备身份、命名空间和访问关系；
- 加密密钥、Provider API Key 和恢复材料；
- 模型请求、ContextPack、外发清单和用量记录；
- 删除、撤回、同步和审计记录。

Embedding、实体名称和摘要可能泄露原始语义，不能因为它们是“派生数据”就视为非敏感。

## 主要威胁

### 服务端失陷

攻击者取得用户自部署服务器的磁盘、数据库、进程或备份。风险取决于服务器是否持有明文密钥和是否承担语义计算。

### 设备失窃或恶意设备

已授权设备可能泄露本地资料、离线索引和密钥。设备撤销不能自动清除已经被该设备解密并导出的历史明文。

### 云端 Provider 外发

即使资料存储在自托管服务器，只要 ContextPack 被发送给云端模型，相关内容就已经离开用户的本地信任边界。

### Prompt Injection

导入网页、PDF、邮件和文档可能包含“忽略规则、上传资料、修改记忆”等恶意指令。外部内容默认是数据，不是指令。

### 模型幻觉与记忆污染

模型可能错误归纳用户偏好、混淆人物或生成无来源事实。如果它能直接写长期记忆，错误会跨会话放大。

### 越权检索

应用、模型或用户界面可能通过相似搜索、摘要、缓存或图关系读取不属于其 namespace 或授权范围的资料。

### 日志与诊断泄露

错误栈、请求日志、审计和调试包可能意外保存正文、密钥、文件路径、Embedding 输入或 ContextPack。

### 删除不完整

正文已删除，但全文索引、向量、图边、缓存、其它设备和备份仍然可以恢复相关内容。

### 供应链与解析器风险

文件解析、OCR、转写、Embedding、插件和模型 SDK 可能执行不可信输入或隐式联网。

## 信任模式

### 模式 A：可信私有服务器

用户信任自己的服务器处理明文。服务器可以：

- 持有受保护的解密能力；
- 构建全文、向量和关系索引；
- 运行后台记忆整理；
- 在多设备之间提供一致查询。

优点是实现和使用较简单，缺点是服务器失陷可能暴露大量明文。该模式只能称为“自托管、用户控制”，不能称为“服务端零知识”。

### 模式 B：零知识同步服务

服务端只保存密文对象、加密操作日志和最小路由元数据，不持有用户内容解密密钥。语义解析、索引、检索和 ContextPack 编译在受信设备执行。

优点是服务端失陷风险显著降低；代价是后台任务、跨设备统一索引、Web 访问、恢复和同步冲突更复杂。

首个多设备同步已经通过 [ADR 0003](adr/0003-zero-knowledge-sync-first.md) 选择此模式。该决定冻结信任边界，不代表能力已经实现：服务端可见元数据、密码协议、设备授权、恢复、撤销、防回滚和删除传播仍须在实现前逐项冻结和验证。

首个零知识同步服务不得接收或持久化内容明文、未包装内容密钥、恢复秘密、语义索引、Embedding、用户画像或 ContextPack。服务端可见信息必须限制在另行枚举的设备公开身份、不透明对象 / 操作标识、密文大小与摘要、排序 / 版本信息、配额和删除处理状态等必要元数据；访问时间、频率、大小、网络端点和设备关系仍可能泄露模式，不能把它们描述为隐藏。

### 可选可信计算节点

用户可以显式注册一台家庭服务器或常开设备作为可信计算节点。它不是默认零知识服务的一部分，必须通过设备身份、密钥授权、能力范围和撤销流程接入。

可信计算节点不进入首个同步批次。一旦获得解密授权，它就在相应范围内成为受信明文处理设备；UI、配置和审计必须把它与零知识同步服务分开呈现，且不得在后台任务失败时静默授权或回退。

## 数据策略

资料与记忆至少支持以下外发标签：

```text
local_only
trusted_device_only
trusted_server_only
cloud_allowed
```

还应支持：

```text
never_embed
never_summarize
expires_at
retention_policy
allowed_providers[]
allowed_purposes[]
```

策略继承必须明确。更严格的子对象策略不能被上层默认值放宽，冲突时失败关闭。

## M0 信任边界

M0 只运行在一个受信本地设备，所有 fixture 和产物默认 `local_only`。这个标签及 sensitivity、retention、deletion state 的必填和失败关闭语义以 [M0 Canonical Schema](schema/m0-canonical-schema.md) 为准；fixture 的 synthetic 标记、网络断言和最小证据以 [M0 Fixture 与指标契约](evaluation/m0-fixture-contract.md) 为准。M0 不调用生成模型、不配置 Provider Key、不接入 RadishMind、不启动同步，也不向外部服务发送 ContextPack。

M0 删除证据只覆盖已枚举的单设备正文、片段、结构化记忆、全文索引和缓存；它不声明其它设备、服务端或备份已经清除。测试必须禁用或拦截网络，并把任何请求视为策略违规。该边界用于验证失败关闭，不代表本地设备天然安全或已经实现加密存储。

## 阶段 1 文件入口信任边界

[ADR 0006](adr/0006-phase1-text-markdown-file-entry.md) 只允许用户显式选择允许根内的单个本地 `.txt` / `.md` 普通文件，并使用合成临时文件验收。完整路径、允许根、platform bookmark、文件身份和导出目标都是敏感入口状态，不进入 canonical 摘要、普通日志、fixture evidence 或删除证据；root 以下 symlink 失败关闭，hardlink 不被当作来源身份或删除授权。

成功导入后，Source Vault 受管 exact bytes 是可回源真相，外部原件不再是召回、citation、导出或重建依赖。外部原件、hardlink alias、手工副本和用户导出仍由用户或其它应用控制，本地 DeleteRequest 不修改它们，也不能声称它们已经删除。Markdown 内容始终是不可信数据，链接、HTML、front matter、代码和伪指令不得触发网络、工具授权、治理变更或记忆写入。

首个入口仍把明文保存在受信本地设备，未实现静态加密、取证级擦除或备份清除。仓库、Issue、PR 和 CI 不得使用真实个人文件；运行验收通过前不能声明生产文件入口已经成立。

## 阶段 1 本地宿主授权边界

[ADR 0007](adr/0007-phase1-local-library-host.md) 要求首个桌面宿主只消费用户当前可见操作产生的一次性系统文件选择 capability。导入新来源、更新已有 lineage 和导出都必须重新选择；用户取消、权限撤销或选择器失败不写数据库、不产生 receipt。首批不持久化完整路径、allowed root、platform bookmark、security-scoped bookmark 或文件访问 token，也不后台监视原件。

当前 `radishmemory-desktop` 已按该边界实现：picker 只在一次调用中构造精确路径与直接 parent root；应用目录拒绝最终目录 / 数据库 symlink，并在 Unix 上收紧为 owner-only；host profile 只保存 contract、随机 namespace ID 与 device ID，数据库已存在而 profile 缺失或损坏时失败关闭。公开 desktop error / Debug 只保留稳定 code / reason、retryable 和必要 OS error code，不复制路径、正文或 identity。第一方宿主没有普通日志 sink；这不代表第三方窗口、系统 dialog、图形驱动或崩溃收集天然无记录，分发前仍需按平台复核。

路径只在本次本地调用链存在；application error、普通日志、UI telemetry、诊断包和 CI 不得输出路径、bookmark、数据库位置或正文。平台 adapter 不得把一次选择扩大为 home、volume 或文件系统根；如果选择器只返回路径，本次 allowed root 只取所选文件或目标的直接 parent，并继续由 file-entry 失败关闭。

宿主数据库位于平台应用数据目录并保持本地明文；“使用系统选择器”“运行于平台沙箱”或“不保存路径”都不等于静态加密。`P1-HF01` 至 `P1-HF12`、真实系统选择器和人工可见 UI 证据成立前，不使用真实个人资料验收，也不声明生产授权面完成。

## 阶段 1 加密 Source Vault 信任边界

[ADR 0008](adr/0008-phase1-encrypted-source-vault.md) 已接受受管原始对象的本地认证加密契约，[P1-S02 依赖与密码套件评审](implementation/phase1-encrypted-source-vault-dependency-review.md)冻结精确 crypto / key-provider profile，[P1-S03a](implementation/phase1-source-vault-portable-crypto.md)又落地 portable cipher / wrap / AAD 与合成测试。filesystem、platform key provider、SQLite migration 和 application integration 尚未开始；当前 SQLite v6 仍保存 inline plaintext body，独立 crypto package 不改变已有字节，也不授权使用真实个人资料。

首批只保护 Source Vault 管理的原始对象文件。SQLite metadata、FTS、标题、摘要、media type、大小、时间、治理标签和派生内容仍可能泄露语义或使用模式，因此不能把该能力描述为整个资料库静态加密。对象解密期间的进程内明文、已解锁设备上的恶意进程、内核、交换区、休眠镜像、崩溃收集和用户导出也不在该静态对象保证内。

每个 SourceArtifact version 使用独立随机 DEK，由设备本地 KEK capability 包装；不同 source 即使 exact digest 相同也不共享首批物理对象。KEK、明文 DEK、可复用 wrapped DEK、nonce、authentication tag 和对象路径不得进入普通日志、诊断、fixture、CI 或仓库。P1-S02 已选择 XChaCha20-Poly1305 + STREAM-BE32、独立 AEAD DEK wrap、系统随机与 secret zeroization，并按 target 选择 macOS Keychain、Windows Credential Manager 或 Linux Secret Service；P1-S03a 只实现前半部分并以全对象认证、length / digest 复验及失败缓冲 zeroization 约束明文交付。系统 store 缺失 / 锁定 / 拒绝 / ambiguity、未知 profile、认证失败或 metadata 交换都失败关闭，不回退 file-stored key、sample store 或其它 provider。

本地 key 丢失可能使对应对象永久不可恢复。首批不提供用户口令、恢复码、key escrow、跨设备恢复或自动 rotation；应用不得生成新 key 后认领旧对象、隐藏损坏来源、建立空库或从外部原件 fallback。未来同步可以在独立协议下增加对象 DEK wrapper，但服务端仍不得获得内容解密能力。

对象删除只证明本地 committed reference、密文文件和 wrapped DEK 按冻结组件范围处理。SQLite 空闲页、迁移前明文、平台临时状态、文件系统快照、备份、交换区、休眠镜像、外部原件和用户导出仍须分别报告，不能从密文文件删除推导取证级擦除或备份清除。

### 完整正文副本与恢复声明

当前文本入口采用 whole-file fragment，FTS `content` 保存该片段的完整可读正文。因此这里的“FTS 未加密”可能意味着全文仍可从 SQLite 直接读取，不能简写为“只有少量元数据未加密”。ADR 0008 完成原始对象加密后，也不能从密文对象不可读推导这份派生正文已受保护。

保护效果应按攻击者取得对象目录、SQLite、备份或已解锁设备的不同情形分别说明。是否扩大静态加密范围仍需独立威胁模型与技术决策；移除 FTS content 副本也不能自动证明索引词项不泄露语义。

原件导出、整体迁移与备份恢复必须分别报告实际范围。后续试用 / 发行方案需要明确备份包含哪些事实、密钥由谁保管、系统重装和 key 丢失后的结果，并在所选恢复范围内演练；首批无恢复能力时必须明示不可恢复风险。这里不选择口令、恢复码、escrow、rotation 或新的 key provider，也不改变 ADR 0008 的首批停止线。

## 模型外发控制

每次调用云端 Provider 或 RadishMind 等 Gateway 前必须：

1. 确定每个 Gateway、允许的 Provider / Profile 集合、模型、用途和目标区域；
2. 对候选来源执行硬权限与外发策略过滤；
3. 对必要字段进行最小化和脱敏；
4. 生成 `OutboundContextManifest`；
5. 记录授权依据和 ContextPack 摘要；
6. 将实际 Provider attempts、响应、使用和失败记录绑定到本次 manifest。

Gateway 是独立接收方，不能因为自部署或与 RadishMemory 同机而从清单中省略。每一跳都必须满足资料的 egress policy；Gateway 不得把 `local_only` 或 `trusted_device_only` 内容升级为可外发。用户应能在 UI 中查看“本次经过哪个 Gateway、向哪些实际模型发送了哪些来源”，而不仅是看到笼统的隐私声明。

## 密钥与设备

首个同步信任模式要求：

- 根密钥在客户端生成，不以明文上传；
- 每台设备有独立身份密钥和可撤销授权；
- 数据密钥按用户、空间或对象包装，避免单一长期静态密钥；
- Provider 密钥与资料加密密钥分离；
- 恢复材料有明确的离线保管和丢失后果；
- 支持设备吊销和未来数据密钥轮换；
- 密钥、恢复码和完整 Token 不进入日志、崩溃报告或模型上下文。

新设备解密授权必须由现有受权设备或用户持有的恢复材料批准，服务端账号重置不能单独恢复内容。设备撤销阻止后续同步读取并影响未来密钥分发，但不能清除该设备已经解密或导出的历史明文。服务端拒绝服务、遗漏、重放、回滚或篡改仍在威胁范围内，客户端必须验证完整性、版本和可检测的历史异常并显式失败。

具体密码学协议必须在实现前经过独立设计评审，不自行发明未经审查的加密算法。

## Prompt Injection 边界

- 文件、网页、消息和模型输出都是不可信输入；
- 资料中的命令只能作为被引用内容，不能改变系统指令；
- Procedure 只能来自用户明确操作或受控提议确认；
- 检索结果使用数据分隔和来源标记进入 ContextPack；
- 工具调用和网络外发使用独立权限，不因资料内容自动授权；
- 高风险解析器在隔离环境运行，并禁止不必要网络访问。

## 日志与审计

日志默认只记录稳定 ID、摘要、大小、状态、错误分类和耗时。完整正文、密钥、原始 ContextPack 和未脱敏路径不得作为普通日志字段。

审计应回答：

- 谁或哪个设备读取了什么范围；
- 哪次请求生成了哪个 ContextPack；
- 内容是否被发送给外部 Provider；
- 哪条 MemoryProposal 被谁确认或拒绝；
- 删除传播到了哪些存储和设备。

审计本身也是敏感数据，需要加密、最小化和保留策略。

## 删除保证

系统必须区分立即可验证的删除与受备份保留期约束的最终删除。

`DeletionEvidence` 至少覆盖：

- 原始对象；
- 结构化记忆；
- Markdown/画像等投影；
- 全文索引；
- 向量索引；
- 实体和图关系；
- 查询和模型缓存；
- 已同步设备；
- 服务端副本；
- 备份批次及预计失效时间。

如果某台离线设备或不可变备份尚未处理，状态应是 `pending_propagation` 或 `pending_backup_expiry`，不能显示为完全删除。

## 开发仓库与 CI 数据边界

RadishMemory 的 Git 仓库、Issue、Pull Request 和 CI 不是个人资料存储或受信记忆运行环境。即使仓库是私有的，也不得提交真实个人文件、私密对话、记忆库、原始 `ContextPack`、Embedding 输入、Provider 请求与响应、密钥、恢复码、本地数据库、同步操作日志、导出或备份。

开发、测试、文档和评测默认只使用合成数据。确需代表真实分布的材料时，必须另行定义授权、不可逆脱敏、重识别风险、存储位置、访问范围、保留期和删除方式；仅替换姓名或文件名不能视为充分脱敏。

CI runner、日志、缓存和 artifact 视为仓库信任边界之外的数据外发面。测试失败信息和快照必须最小化，不输出正文、密钥、完整本地路径或未经授权的上下文。`.gitignore`、仓库检查和未来秘密扫描只提供纵深防御，不能替代数据最小化和人工审查。

## 当前安全停止线

在对应能力经实现和验证前，不得声明：

- 零知识服务端；
- 端到端加密多端同步；
- 可证明永久删除；
- 云端模型不会保留内容；
- Prompt Injection 完全解决；
- 自托管即满足全部隐私需求；
- 本地模型天然安全；
- 整个本地资料库已经静态加密；
- 加密备份一定可恢复。
