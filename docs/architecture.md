# RadishMemory 系统架构

## 架构目标

RadishMemory 必须同时满足：

- 单机离线可用；
- 用户自部署多端同步；
- 原始资料、结构化记忆和派生索引相互分离；
- 模型、Embedding、数据库和 Provider 可替换；
- 权限、敏感度和外发策略在检索前执行；
- 所有回答、记忆写入和删除动作可追溯；
- 虚拟形象和聊天体验不拥有独立记忆真相。

## 总体结构

```text
采集入口
  短语 / 文件 / 网页 / 图片 / 音频 / 对话 / 应用事件
        │
        ▼
Source Vault ── 原始对象、元数据、内容摘要、版本和来源
        │
        ▼
Ingestion Pipeline ── 解析、OCR、转写、分段、分类、风险扫描
        │
        ├──────────────► Derived Indexes
        │                 全文 / 向量 / 实体 / 时间 / 图 / 摘要
        ▼
Memory Engine ── Observation / Claim / Episode / Preference / Procedure
        │
        ▼
Policy-aware Retrieval ── 权限硬过滤 + 混合召回 + rerank
        │
        ▼
Memory Compiler ── ContextPack + Outbound Context Manifest
        │
        ▼
Model Adapter / RadishMind ── GPT / Gemini / Claude / Grok / DeepSeek / Local
        │
        ▼
回答、引用、Trace、MemoryProposal
        │
        └──────────────► 规则校验 / 用户确认 / 记忆状态变更
```

多端同步独立于模型调用：

```text
Device A Local Store ── encrypted operation/object sync ── Self-hosted Server
                                                           │
Device B Local Store ◄── encrypted operation/object sync ──┘
```

## M0 本地架构边界

首个可执行切片只启用总体架构的本地最小子集：Capture Gateway 接收合成短文本与 Markdown；Source Vault 保存正文和摘要；Ingestion Pipeline 做确定性分段；Derived Indexes 只包含全文基线；Policy Engine 执行 namespace、敏感度、状态和删除过滤；Memory Engine 管理 proposal、decision、record、时间更正和删除事件；Memory Compiler 生成带 citation map 的本地 ContextPack。

M0 不调用 Model Adapter 或 RadishMind，不产生已发送的外发 manifest、Provider trace 或模型用量，也不实现同步。核心验收期间任何网络请求都视为失败。这个边界不删除长期组件，只避免在来源、状态、权限和删除契约尚未成立时引入模型与分布式复杂度。完整决策见 [ADR 0002](adr/0002-m0-local-memory-loop.md)，字段与跨对象不变量见 [M0 Canonical Schema](schema/m0-canonical-schema.md)，可执行操作与指标 oracle 见 [M0 Fixture 与指标契约](evaluation/m0-fixture-contract.md)。

## 阶段 1 文本 / Markdown 文件入口边界

阶段 1 的首个真实入口已通过 [ADR 0006](adr/0006-phase1-text-markdown-file-entry.md) 冻结为用户显式选择的单个本地 `.txt` / `.md` 普通文件。入口在显式允许根内读取非空、最大 8 MiB 的 UTF-8 原始字节，拒绝 root 以下 symlink，不把 hardlink、路径、inode 或内容摘要当作 canonical identity；成功后由 Source Vault 受管 exact bytes 承担原始真相，外部原件不成为检索、citation、重建或导出的运行依赖。

该入口继续使用现有 `SourceArtifact`、`SourceFragment`、FTS5、citation、DeleteRequest 与 DeletionEvidence，不增加文件专用 canonical object。相同 origin binding 与 exact bytes 的导入幂等，内容变化创建不可变新版本，普通召回只使用 active lineage tip；导出恢复受管副本的精确字节，删除只处理已枚举的本地受管闭包，不修改或声称删除外部原件、hardlink alias 或用户导出。

ADR 0006 冻结 application behavior 与合成验收；`P1-F01` 至 `P1-F18` 已通过 Linux、macOS、Windows locked CI 并合入稳定主线。该证据证明当前 file-entry / SQLite application contract，不证明后续 desktop host 的真实 UI、系统选择器或个人资料授权面。

`P1-I01` 使用独立第一方 `radishmemory-file-entry` package 隔离本地文件系统读取，并且只依赖 `radishmemory-core`。它返回 path-free validated snapshot；`P1-I02` 在 core 增加完整 `SourceCapture` / `SourceCaptureResult` 与最小 `SourceCaptureStore` port，由 file-entry 把快照映射为整文件单片段 capture candidate，由 SQLite v6 adapter 在一个 `IMMEDIATE` transaction 内提交 SourceArtifact body / metadata、完整 fragment、FTS、origin binding、lineage tip 与最小 audit。相同 binding / exact bytes 返回已存事实，内容变化严格推进一个版本并移除旧 tip 的普通召回，任一写入或派生校验失败均回滚到旧 tip。

`P1-I03 exact export` 继续让 file-entry package 拥有目标允许根、symlink 拒绝、任务临时文件、字节复验与不覆盖发布；调用方必须先通过 namespace 和精确 `source_id` 从 `SourceVault` 取得 active 或历史可读的已验真 `SourceArtifact`。file-entry 复验 deletion state、长度、正文与 `exact-bytes-v1` 摘要后，在目标 parent 创建并同步任务临时文件，关闭后重新逐字节复验，以同目录 `hard_link` 原子建立不存在的目标目录项，再复验发布结果并只清理自身临时文件。目标存在、目标或 parent 为 symlink、临时写入或并发发布失败均不覆盖现有目标，也不修改 Source Vault。

`P1-I04 lineage deletion` 不增加 schema 或平行删除协议，继续使用 canonical `DeleteRequest` / `DeletionEvidence` 和 SQLite `DeletionStore`。一个请求只要包含某个文件来源版本，就必须精确包含同 namespace、同 lineage 的全部 active SourceArtifact 版本及所有已展开 active memory 依赖；缺一版本或依赖时整笔拒绝。计划提交原子地把全部来源、fragment、proposal 与显式 memory 置为 pending，删除 FTS、当前投影和 lineage tip；执行阶段处理 body、fragment、metadata、origin binding 与 capture audit，并由既有最小 audit / evidence 保留真实结果。verify 与 rebuild 都复验每个 active 文件来源的 body、完整 fragment 集、capture audit 与 binding，rebuild 只在这些 canonical 与入口事实通过后才改写派生表；pending / failed / deleted lineage 不会被恢复为 active tip。

`P1-F02` / `P1-F05` 的跨层验收继续走上述同一数据流：exact UTF-8 bytes 从 snapshot 进入 canonical source、SQLite BLOB 与 fragment 后，在 reopen / rebuild 中保持摘要、长度和 byte range；不同 opaque binding 即使来自同一 hardlink inode 且摘要相同，也建立独立 source lineage、tip、audit 与删除闭包，adapter 不按路径、inode 或 digest 合并 provenance。

`P1-F11` 至 `P1-F14` 复用相同串行提交边界并证明失败发生在 canonical / SQLite 写入之前：路径、symlink、类型、内容和超限错误不会产生 receipt，也不改变 source、body、fragment、tip、binding、audit 或 FTS；恰为 8 MiB 的合法 UTF-8 文件则必须完成同一原子 capture。该证据不引入后台补偿或第二套 staging 状态。

`P1-F15` 至 `P1-F18` 继续保持相同 production 数据流。file-entry 默认 build 不包含测试操作，只有 SQLite integration test 通过第一方 `acceptance-test-support` feature 调用 private read seam，在初始文件观察后确定性替换、截短或扩展路径；失败发生在 snapshot / canonical candidate 之前，旧 tip、binding、audit 与 FTS 行投影逐项不变。SQLite capture 的最终 commit 故障通过 adapter-private callback 注入真实 SQL cause，transaction Drop 整体回滚；export 复用临时写入和 `hard_link` 发布 seam，不增加后台补偿或通用 fault framework。不可信 Markdown 仍只进入 exact body、whole-file fragment 与 FTS，loopback observer 证明当前场景没有网络连接，memory facts 保持为零；公开诊断与最小 receipt 不携带正文、路径、allowed root、导出目标或路径摘要。

file-entry package 继续不知道 SQLite；SQLite adapter 也不读取或写入外部路径。旧 `SourceVault` 两步写入口只保留 M0 synthetic source，显式用户输入必须走原子 capture port，不能通过顺序调用两个旧方法冒充完成。lineage tip 是可重建派生投影，origin binding 只保存 namespace、opaque binding ID 与 lineage，不保存路径、inode 或正文。

## 阶段 1 本地资料库宿主边界

[ADR 0007](adr/0007-phase1-local-library-host.md) 冻结首个 production host 为单用户、单 namespace、单设备的本地桌面进程。UI 只调用 production application service；application service 组合 file-entry、core port 与 SQLite adapter，负责应用目录、opaque ID / UTC time、首次导入 / 更新、来源目录、search citation、精确导出、lineage 删除、verify 与 rebuild。UI 不读取 SQLite 表、rowid、FTS 分数或 adapter-private binding，也不自行拼装删除闭包。

每次导入、更新和导出只消费当前 UI 操作产生的一次性系统文件选择 capability。首批不持久化路径、allowed root、platform bookmark 或文件访问 token，不后台监视或自动重导入；更新已有来源时，用户先选择现有 lineage，再重新选择本地文件并显式复用 opaque binding。系统选择器路径只在本次调用链存在，file-entry 继续执行允许根、symlink、普通文件、TOCTOU、内容与目标不覆盖检查。

宿主在平台应用数据目录的专用位置打开文件 SQLite，启动时验证 capability、migration、canonical facts、派生索引和 binding。派生漂移只有在 canonical facts 通过完整复验后才能由用户显式 rebuild；canonical 损坏不能通过扫描外部原件、建立空库或 UI cache 静默恢复。首批仍不启动本地 HTTP 服务、daemon、网络、模型、RadishMind 或同步，也不声明本地明文数据库已经加密。

`P1-H02` / `P1-H03` 已以第一方 `radishmemory-application` package 落地上述组合边界。core 的 `SourceCatalog` 只定义当前 lineage、版本历史和 body-free summary，SQLite adapter 从已验真的 active source、opaque binding 与单一 lineage tip 生成读取模型；`LocalLibrary` 组合 open、import / update、list / get、search citation、exact export、canonical lineage deletion evidence、verify / rebuild，并通过 `ApplicationRuntime` 隔离 production ID / clock。

`P1-H04` 已以第一方 `radishmemory-desktop` package 落地平台壳层：`directories::ProjectDirs` 只解析专用 local data directory，host profile 原子保存 namespace / device identity，`getrandom` 与 UTC clock 实现 `ApplicationRuntime`，`rfd` 把一次选择缩为精确路径及直接 parent capability，`eframe` UI 只持有 `LibraryController` 读取状态并调用 application operation。该层不依赖 `radishmemory-sqlite` 或 `radishmemory-file-entry`，不构造 canonical object / 删除闭包，不初始化普通日志 sink，也不保存路径、bookmark、picker token 或第二份 UI 数据库。当前已取得 macOS AppKit、Windows ARM64 native dialog 与 Debian ARM64 / GNOME Wayland XDG Portal / GTK picker 的可见 GUI 正向 / 失败关闭证据；Windows 实机暴露的 OpenGL-only 阻断已通过把同版本 `eframe` renderer feature 切换为 `wgpu` 最小修复，当前图也已通过 Linux / macOS / Windows locked CI。333 个目标可达 crate、license option、默认字体、bundled SQLite 与条件平台依赖已由 [third-party notices 复核](implementation/phase1-third-party-notices.md)收口，P1-H05 gate 完成。

## 阶段 1 加密内容寻址 Source Vault 边界

[ADR 0008](adr/0008-phase1-encrypted-source-vault.md) 已冻结下一 Source Vault 存储契约；[P1-S03a](implementation/phase1-source-vault-portable-crypto.md) 已落地独立 portable crypto package，但 filesystem / SQLite production data flow 尚未实现。受管原始对象将从 SQLite inline body 外置为应用专用目录中的版本化认证密文；SQLite 继续保存结构化 metadata、对象 reference、FTS、投影、binding、audit 与 deletion evidence，因此首批只能声明原始对象加密，不能声明整个资料库或所有派生数据已经静态加密。

一个不可变 SourceArtifact version 首批对应一个不可变密文对象。逻辑 lookup 使用精确 `source_id` 与 `exact-bytes-v1` digest，物理 locator 保持 adapter-private；不同 `source_id` 即使摘要相同也不跨 lineage / provenance 物理去重。该选择保留独立 governance、retention 和 deletion scope，不把内容摘要升级为 canonical identity。

每个对象使用独立随机 DEK，并由设备本地 KEK capability 包装。version、cipher suite、key-wrap profile、namespace、source、digest、length 和 media type 必须受 envelope authentication 约束；未知 profile、认证失败、metadata 交换、缺 key 或对象缺失均失败关闭，不回退到旧 BLOB 或外部原件。[P1-S02 依赖与密码套件评审](implementation/phase1-encrypted-source-vault-dependency-review.md)已将精确 profile 冻结为 XChaCha20-Poly1305 + STREAM-BE32 与独立 XChaCha20-Poly1305 DEK wrap，随机源复用 `getrandom =0.4.3`，设备 KEK 按 target 使用 macOS Keychain、Windows Credential Manager 或 Linux Secret Service。P1-S03a 已使 portable crypto 依赖进入 manifest / lockfile 并实现 deterministic AAD、seal / open 与合成验证；三个 platform provider 尚未进入依赖图，filesystem adapter 也未接入 production。

对象提交遵循“密文 publish → SQLite commit → read-back”三段状态：先在应用专用 staging 中直接生成密文，经 sync、关闭、认证与 no-overwrite publish 后，才能在一个 SQLite `IMMEDIATE` transaction 内提交 object reference、canonical facts、FTS、binding、tip 与 audit；commit 后 read-back 复验成功才返回 receipt。publish 后、metadata commit 前的对象只是可识别 orphan candidate；恢复器只能清理无 committed reference、无可恢复 attempt 且身份明确的对象，ambiguous state 使 library 失败关闭。

SQLite v6 migration 在普通操作暴露前逐对象复验 inline body、发布密文、提交 reference 并 read-back；未完成或损坏时不混合返回 inline / object-backed source。迁移不改变 canonical identity、citation、governance 或 deletion state，也不证明 SQLite 空闲页、快照和备份中的历史明文已物理清除。`P1-S03b` 至 `P1-S05` 完成 filesystem adapter、platform provider、migration 与宿主验收前，PDF / 图片解析保持停止。

## 当前模块与读取维护边界

| Package | 职责 | 依赖约束 |
| --- | --- | --- |
| `radishmemory-core` | canonical 类型、领域校验与 ports | 不持有 SQLite、UI、模型或文件路径 |
| `radishmemory-sqlite` | 事实持久化、迁移、事件与派生索引 | 事务和 adapter-private schema 留在本层 |
| `radishmemory-file-entry` | 显式文件 snapshot 与 exact export | 只依赖 core，不读取 SQLite |
| `radishmemory-application` | 组合本地资料库用例 | production 业务入口，不承担桌面 toolkit 或 fixture mapping |
| `radishmemory-desktop` | 平台目录、profile、runtime、picker 与 UI | 第一方业务依赖只到 application |
| `radishmemory-m0` | 合成 suite 映射与证据编排 | 不将 runner 专用逻辑表述为 production API |
| `radishmemory-source-vault` | 当前独立 portable crypto | object filesystem、key provider 与 application 数据流尚未接入 |

当前搜索与目录实现包含全量正文读取、事实复验和内存排序 / 分页，桌面同步执行相关操作。后续优化应先取得数据量、正文大小和版本分布对应的性能证据，再决定增量校验、SQL 分页、top-k 或 UI 执行方式；不能通过省略权限、时间、删除或完整性检查降低成本。

ADR 0007 要求派生损坏时可在 canonical 完整的前提下显式 rebuild。维护入口必须在普通启动失败时仍可安全到达，并限制允许操作；canonical / binding 损坏不能被重建、空库或外部原件 fallback 掩盖。当前实现存在启动后重建不可达等缺口，观察见[审阅记录](implementation/2026-09-05-project-review.md)，修复验收见[质量计划](evaluation/phase1-local-library-quality.md)。本文记录架构要求，不表示维护模式或性能优化已经实现。

## 核心组件

### 1. Capture Gateway

提供统一采集协议，接收文本、文件、URI、媒体和应用事件。采集必须快速完成原始资料持久化，耗时的 OCR、转写、Embedding 和记忆提取可以异步执行。

每次采集生成稳定 `source_id`、内容摘要、来源类型、采集时间、设备、命名空间和敏感度初值。

### 2. Source Vault

Source Vault 是用户原始资料的真相源，采用内容寻址对象与结构化元数据分离的方式。它负责：

- 保存原始字节或用户明确保存的正文；
- 内容摘要、MIME、大小、来源、版本和完整性检查；
- 加密、保留、归档、删除和同步状态；
- 派生文本、缩略图、转写和分段到原件的引用关系。

大对象不要求进入人类可读仓库或 Git 历史。

### 3. Ingestion Pipeline

导入流水线负责把不同来源转换为统一的可检索材料：

1. 类型识别与安全检查；
2. 文本提取、OCR 或音频转写；
3. 结构化分段与页码、时间码保留；
4. 语言、人物、项目、时间和主题候选提取；
5. Prompt Injection 与不可信指令标记；
6. 派生索引更新；
7. `MemoryProposal` 生成。

导入材料默认是“不可信资料”，其中的命令不得升级为系统指令或程序性记忆。

### 4. Memory Engine

Memory Engine 管理记忆类型、状态机、来源、时间有效性、冲突、确认、撤回和过期。它不依赖单一模型生成结果，所有写入都经过 canonical schema 和确定性校验。

详细语义见 [记忆模型](memory-model.md)。

### 5. Derived Indexes

派生索引可以包含：

- 全文倒排索引；
- 一个或多个版本化向量索引；
- 实体与别名索引；
- 时间、来源、项目、人物和敏感度索引；
- 关系边和时间图投影；
- 分层摘要和查询缓存。

所有索引必须记录生成器、模型、版本、维度、输入摘要和生成时间，并能从 Source Vault 与 Memory Engine 重建。Embedding 不进入长期 canonical 格式。

### 6. Policy Engine

Policy Engine 在检索、同步、导出和模型调用前提供硬边界：

- namespace、用户、设备和应用授权；
- `local_only`、`trusted_server_only`、`cloud_allowed` 等外发级别；
- 来源级、文件级、记忆级和字段级敏感度；
- 模型 Provider、地区、用途和保留策略限制；
- 临时授权、到期、撤销和审计。

拒绝结果不得回退到更宽权限，也不得通过摘要、缓存或 Embedding 绕过限制。

### 7. Retrieval Orchestrator

检索按以下顺序执行：

1. 解析任务、主体、时间范围和期望输出；
2. 执行权限、敏感度和外发硬过滤；
3. 并行进行全文、向量、实体、时间和关系召回；
4. 归并、去重和来源扩展；
5. 使用相关性、时效、重要度、来源可信度和任务覆盖进行 rerank；
6. 显式处理冲突、已撤回、已过期和不确定记忆；
7. 把候选交给 Memory Compiler。

安全策略不是打分项；未获授权的内容不能作为低分候选进入后续步骤。

### 8. Memory Compiler

Memory Compiler 根据任务和 Token 预算生成模型无关的 `ContextPack`。它类似编译器：长期资料是源代码，派生索引是中间表示，ContextPack 是本次模型调用的构建产物。

ContextPack 至少包含：

- task instruction；
- 已选择的资料片段与稳定引用；
- 已确认事实、偏好、相关经历和程序性规则；
- 时间与冲突说明；
- 不确定性和禁止推断边界；
- Token 预算、截断和覆盖说明；
- 可供回答引用的 citation map。

同时生成 `OutboundContextManifest`，记录接收 Gateway、调用前允许的 Provider / Profile 集合、调用后实际 Provider attempts、资料引用、敏感度、授权依据、摘要和时间，但审计日志不得复制完整私密正文。没有 Gateway 时仍直接记录目标 Provider；每个中间方和最终接收方都必须满足外发策略。

### 9. Model Adapter Layer

模型适配层把 canonical 请求翻译为不同 Provider 协议。它负责能力发现、结构化输出、流式响应、错误分类、用量记录和超时取消，不负责决定哪些记忆是真实的。

RadishMind 可以实现这一层或作为其上游服务。根据 [ADR 0004](adr/0004-radishmind-optional-gateway-entry.md)，它首次在完整 MVP 阶段 3 以可选 Model Gateway 接入，且必须晚于 mock 或直接 adapter 基线；首次不接 Workflow、Tooling 或业务写回。RadishMemory 保留直接接入和本地模型能力，M0、单机资料库和记忆生命周期不以 RadishMind 可用为前提。

### 10. Companion Experience

个人伴侣、虚拟形象、语音和主动提醒属于表现与交互层。它们只能通过公共记忆和上下文接口读取或提出修改，不能创建第二套画像、会话或长期记忆数据库。

## 三层数据真相

### 原始真相层

用户明确保存的原件、正文、对话和应用事件。系统不得用模型摘要替换原件。

### 结构化记忆层

经过状态管理的 Observation、Claim、Episode、Preference、Procedure、Entity 和 Relation。它们是可查询、可冲突、可撤回的长期语义。

### 派生投影层

Markdown 投影、用户画像、时间线、摘要、向量和知识图谱视图。它们服务阅读和检索，但可以被重建。

仓库式用户视图可以呈现为：

```text
AGENTS.md
PROFILE.md
MEMORY_POLICY.md
inbox/
knowledge/
timeline/
skills/
sources/
.memory/
```

这是一种可读交互投影，不要求底层直接用 Git 保存全部私密历史。

## 同步架构

长期产品可以支持两种明确的信任模式：

1. **可信私有服务器**：用户信任自己的服务器解密、索引和运行后台任务。
2. **零知识同步服务**：服务器只保存密文对象与加密操作日志，解密和语义索引在受信设备执行。

首个多设备同步已经通过 [ADR 0003](adr/0003-zero-knowledge-sync-first.md) 冻结为零知识同步服务；可信私有服务器不进入首个同步批次。未来支持两种模式时，必须在配置、UI、密钥和部署声明中明确区分，不能静默降级，也不能把“自托管”自动宣传为“零知识”。

首个模式中，根密钥、内容明文、语义索引、检索、ContextPack 和记忆状态计算只存在于受信设备。服务端只中继客户端加密的不可变对象、追加操作日志、密钥封装和已枚举最小元数据；服务端数据库不是记忆 canonical truth，也不运行服务端语义查询。结构化记忆的用户修改保留版本与冲突，不简单依赖最后写入覆盖。设备撤销后必须阻止新的同步读取，并通过未来密钥分发和必要轮换控制后续数据访问。

客户端必须验证对象完整性、协议版本、幂等和可检测的重放 / 回滚异常。可信计算节点后置为独立、显式授权且可撤销的受信设备能力，不得作为零知识同步服务的默认组成部分。密码算法、线协议、设备授权、恢复、冲突和删除证据仍须在同步实现前另行冻结。

## Canonical 接口边界

M0 已冻结 `SourceArtifact`、`SourceFragment`、`MemoryProposal`、`MemoryDecision`、`MemoryRecord`、`MemoryStateEvent`、`ContextPack`、`DeleteRequest` 和 `DeletionEvidence` 的逻辑字段。精确契约以 [M0 Canonical Schema](schema/m0-canonical-schema.md) 为准。

M0 fixture 已冻结这些动作在评测中的输入与预期，但不把测试 operation 当作 production API。以下运行接口仍需在对应阶段冻结，而不是提前绑定某个 Provider SDK：

- `CaptureRequest / CaptureReceipt`
- `SearchRequest / SearchCandidate`
- `OutboundContextManifest`
- `ModelRequest / ModelResponse / UsageRecord`
- `SyncOperation / DeviceIdentity`

## M0 实现与存储基线

[ADR 0005](adr/0005-m0-implementation-stack.md) 已冻结 M0 为 Rust 2024 模块化单体：`radishmemory-core` 承载领域与应用边界，`radishmemory-sqlite` 实现本地持久化和 FTS5，`radishmemory-m0` 执行冻结 fixture。三个 package 在单进程内运行，不引入网络、异步运行时、Provider SDK 或服务拆分。

production adapter 入口 `SqliteDatabase::open(path)` 当前使用文件数据库：SourceArtifact 正文作为独立 BLOB 保存，source / fragment / proposal / decision / record / state event / delete request / deletion evidence 由结构化表承载；FTS5、当前状态与 source lineage tip 是可重建派生数据，origin binding 和 capture audit 是 path-free 本地入口状态，ContextPack / query cache 在当前阶段不持久化。仅 opt-in `fixture-runner` feature 为每个合成场景建立独立内存连接；它仍执行同一 capability probe、v1 → v6 migration、连接策略、派生校验与真实 adapter 操作，但避免把文件系统逐事务同步成本混入 application-contract fixture。数据库 rowid、SQL schema、FTS 分数和 SQLite JSON 不进入长期 canonical 格式。P1-S03a 的独立 package 尚无 production dependency edge；当前数据流仍未静态加密，不能被描述为已经实现 encrypted Source Vault 或覆盖 SQLite / FTS。

加密内容寻址 Source Vault 与 SQLite metadata 协调已由 ADR 0008 接受为阶段 1 后续方向；以下仍是后续阶段候选，不是 ADR 0005 / ADR 0008 的已接受决定：

- 服务端结构化数据评估 PostgreSQL，但零知识同步服务不得因此获得内容明文或语义索引；
- 向量实现保持可替换，不把模型、维度或数据库扩展写入 canonical 格式；
- 实体与时间关系先以关系投影验证，不默认引入独立图数据库；
- 后台任务先评估数据库任务表或单进程 worker，不默认引入消息队列。

这些选择必须由对应阶段的数据规模、延迟、离线、加密和迁移证据另行决定。
