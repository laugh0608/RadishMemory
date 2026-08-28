# ADR 0006：阶段 1 文本 / Markdown 文件入口

日期：2026-08-28

状态：Accepted

契约标识：`radishmemory.phase1-file-entry/1`

## 背景

M0 已经证明合成文本 / Markdown 可以进入 `SourceArtifact` 与 `SourceFragment`，通过本地 FTS5 召回并形成 citation，最终进入可复验的本地删除闭环。它没有定义真实文件如何被用户授权、怎样安全读取、如何识别重复与版本、由谁拥有原件、怎样精确导出，也没有把 fixture operation 升级为 production API。

阶段 1 的长期范围还包含 PDF、图片、向量检索、模型问答和 UI，但同时展开会把文件系统、解析器、依赖、模型和产品交互的风险混成一个不可评审批次。首个真实入口应先复用已经成立的来源、治理、全文、citation 与删除语义，只增加能够独立验证的本地文本文件边界。

本文冻结该入口的行为和验收，不冻结 JSON / FFI / CLI 表示、UI 文件选择器、随机 ID 编码或长期大对象存储布局。`radishmemory.phase1-file-entry/1` 是 application contract 标识，不是第二套 canonical object schema；持久化真相继续使用 [M0 Canonical Schema](../schema/m0-canonical-schema.md) 中的对象和不变量。

## 决策

### 运行与能力边界

首个阶段 1 文件入口只运行在单用户、单 namespace、单个受信本地设备，且只使用合成临时文件验收。

| 项目 | 首个入口决策 |
| --- | --- |
| 输入 | 用户显式选择的单个普通文件 |
| 类型 | `.txt` 与 `.md`，扩展名按 ASCII 大小写不敏感匹配 |
| 内容 | 非空 UTF-8，最大 `8_388_608` bytes（8 MiB） |
| 原始真相 | 成功导入后由 Source Vault 受管副本承担 |
| 派生能力 | 现有确定性分段、FTS5、citation 与重建 |
| 治理 | `local_only`，沿用 canonical sensitivity、retention 与 deletion state |
| 导出 | 用户显式选择目标，恢复受管副本的精确原始字节 |
| 删除 | 本地受管副本、来源闭包和派生数据；不操作外部原件或用户导出 |
| 网络与模型 | 不联网、不解析远程引用、不调用模型或 RadishMind |

不接收目录、glob、递归扫描、标准输入、URI、网页、剪贴板、PDF、图片、压缩包、富文本、`.markdown` 或其它扩展名。Markdown 在本切片中是不可信 UTF-8 资料，不执行 HTML、脚本、front matter、include、插件或代码块，也不获取图片和链接。

### 显式选择、允许根与路径解析

导入请求必须同时携带一次显式文件选择授权和非空的允许根集合。调用方不得隐式使用当前目录、用户目录、文件系统根或仓库根作为默认允许根。实现表示可以是路径、文件选择器 capability 或平台 bookmark，但授权范围必须等价于一个已解析的本地文件及其允许根。

路径入口必须按以下顺序失败关闭：

1. 先把每个允许根解析为现存普通目录的 canonical absolute location；平台固定别名可以在用户授权根时解析，但未解析根、文件根和文件系统根不被接受。
2. 解析用户选择，拒绝 NUL、空路径、`.` / `..` 逃逸、缺失目标、目录、设备文件、FIFO、socket 和其它非普通文件。
3. 解析后的目标必须位于一个已授权 canonical root 内；只比较字符串前缀不构成边界证明。
4. 从已授权 root 到目标的任何后续 path component 都不得是 symlink；最终目标是 symlink 时同样拒绝。入口不跟随 symlink 后再把结果当作已授权普通文件。
5. 以只读 handle 打开后再次核对普通文件身份、大小和允许根约束；读取前后可观察的文件身份、长度或修改事实发生变化时，返回 `source_changed_during_capture`，不提交部分导入。

hardlink 可以作为普通文件读取，但 inode、file ID、link count 或 canonical path 都不是 `SourceArtifact` identity。两个显式选择的 hardlink alias 默认是两个来源入口；实现可以按内容寻址去重受管物理字节，但不得因此合并 provenance、授权、retention 或删除范围。RadishMemory 永不通过某个 hardlink alias 修改或删除外部文件。

允许根、完整路径、平台 bookmark 和文件身份属于本地敏感入口状态，不进入 canonical 摘要、普通日志、删除证据或导出 metadata。需要支持再次导入时，adapter 可以保存受保护的本地 origin binding；该 binding 必须有独立删除范围，不能把真实路径写入 `origin_ref`。

### 字节、类型与大小

扩展名只决定本切片的 `source_kind` 与 `media_type`：

| 扩展名 | `source_kind` | `media_type` |
| --- | --- | --- |
| `.txt` | `text` | `text/plain` |
| `.md` | `markdown` | `text/markdown` |

内容必须满足：

- 实际读取字节数在闭区间 `[1, 8_388_608]`；metadata 的预检和读取后的实际计数都必须通过，上限后一字节即拒绝；
- 整体是严格 UTF-8；UTF-8 BOM 只允许出现在开头并作为原始字节保留，非法或截断编码拒绝；
- 不允许 NUL byte；其它合法 Unicode、TAB、LF 与 CRLF 可以存在；
- 不做 Unicode normalization、换行转换、尾换行补齐、空白修剪或 Markdown 重写；
- `content_length` 和 `exact-bytes-v1` 摘要覆盖用户选择文件的精确原始字节。

扩展名与有效 UTF-8 必须同时成立；不得仅靠扩展名跳过内容检查，也不得通过内容 sniffing 把其它扩展名静默接受为文本。文件权限、IO 或完整性失败不能回退为部分读取。

### 来源身份、版本与幂等

文件路径和内容摘要分别只能证明“从哪里选择”和“字节是否相同”，都不能单独作为来源身份。首次成功导入必须由应用分配不透明 `origin_binding_id`、`lineage_id` 与首个 `source_id`：

- `origin_binding_id` 绑定同一 namespace 内的一次受控文件入口，不由完整路径、inode 或内容摘要直接充当；映射细节是 adapter-private state；
- `SourceArtifact.origin_kind = explicit_user_input`，`origin_ref` 只允许保存不透明 binding，不保存真实路径；
- `title` 可以保存最终文件名，但它是受保护的来源 metadata，不能进入普通错误和日志；
- `observed_at` 表示 importer 从受信 handle 观察到这份字节快照的时间，`captured_at` 表示受管事实提交完成时间；文件 mtime 不替代二者，也不决定版本顺序。

在同一 namespace、同一 origin binding 和相同不可变治理快照下：

1. exact bytes 摘要和长度均相同的再次导入必须幂等返回已有 `source_id`、`lineage_id` 和 `version`，不得新增 SourceArtifact、fragment 或 FTS 条目；返回前仍须复验 canonical body、完整 fragment 集合、FTS 与 lineage tip 一致性，发现漂移时失败关闭并要求显式修复。
2. 字节变化必须创建同一 lineage 的新 `SourceArtifact`，`version` 单调加一，`supersedes_source_ids` 精确包含前一个 lineage tip；不得原地覆盖旧正文。
3. 首个入口不允许 lineage 分叉或一次合并多个来源版本。版本事实、fragment、FTS 与 lineage tip 必须原子更新；任一失败时旧 tip 继续生效，新版本不可见。
4. 普通 search、citation 与 ContextPack 只接受 active 的唯一 lineage tip。旧版本保留为显式历史来源，可以按精确 `source_id` 读取或导出，但不能与当前版本一起污染普通召回。

不同 origin binding 即使 exact bytes 相同，也保留不同 `source_id`、lineage、治理和删除范围。物理存储可以在不改变这些事实的前提下按摘要去重；摘要相同不是自动合并来源的授权。

### Canonical 映射与提交原子性

文件入口必须映射到现有 `SourceArtifact` 与 `SourceFragment`，不得建立 `FileArtifact`、路径主表或第二套正文真相：

```text
explicit file selection + allowed roots
  -> validated byte snapshot
  -> SourceArtifact + managed exact body
  -> deterministic SourceFragment set
  -> FTS5 + lineage-tip projection
  -> capture receipt
```

应用层 capture receipt 至少能够返回 contract 标识、namespace、`source_id`、`lineage_id`、version、内容摘要、字节数、media type、幂等 / 新版本结果和稳定状态；它不得返回数据库 rowid、真实路径、SQLite schema 或未经授权正文。精确字段和序列化表示在实现评审单元中冻结，不直接复用 M0 fixture operation mapping。

在 receipt 报告成功前，受管正文、SourceArtifact metadata、完整 fragment 集合、FTS、lineage tip、origin binding 与最小审计必须形成一个应用级原子结果。实现可以用单事务或可复验 staging + commit 达到该结果，但不得让普通读取观察到半个导入，也不得用后台“最终会补齐”冒充成功。

导入失败时不得留下 active canonical 对象、可召回 FTS 条目、成功 receipt 或包含正文的诊断；本次创建的 staging 产物必须清理，清理失败以稳定状态报告。canonical body、fragment 或引用完整性损坏时失败关闭；不得把扫描原文件或重建索引作为静默 fallback。

### 分段、检索与 citation

`.txt` 与 `.md` 复用版本化、确定性的本地 segmenter。fragment 必须是原始正文的非空连续 UTF-8 byte range，其正文和摘要可由受管 SourceArtifact 复验；Markdown `heading_path` 可以由确定性标题解析产生，但不得改变 byte range 或正文。

普通检索按 namespace、governance、deletion state、lineage tip 与时间资格失败关闭后，再使用现有 FTS5 选择候选。citation 必须包含当前 `source_id`、`fragment_id`、byte range 与摘要，并能回到受管原始字节。缺失 tip、多个 tip、悬空 fragment、摘要漂移或 FTS 漂移均不能降级成无 citation 结果。

显式 rebuild 只从已验真的 active `SourceArtifact` 与 `SourceFragment` canonical facts 重建 FTS 和 lineage-tip projection。缺失或损坏的 SourceArtifact / SourceFragment 是 canonical integrity failure，不能由派生 rebuild 静默补造。重建不得读取外部 origin file，不依赖路径仍存在，也不得恢复 pending、failed、deleted 或非 tip 版本到普通召回。

### 精确导出

导出必须由用户显式选择一个 active 或历史可读的 `source_id` 和本地目标位置。导出前执行 namespace、授权、deletion state、受管 body 摘要与目标允许根检查：

- pending、failed 或 deleted 来源不得导出；
- 目标 parent 必须位于显式 export allowed root，parent 下的 symlink component 拒绝；
- 目标必须不存在；首个入口不覆盖、追加或跟随已有 symlink，即使内容相同也不例外；
- 在目标目录创建任务专用临时文件，写入受管 body 的精确原始字节，flush / close 后复验长度与 `exact-bytes-v1` 摘要，再以不覆盖语义原子发布；
- 任一步失败都不报告成功；任务临时文件必须清理，清理失败显式报告但不得删除其它文件。

导出 round-trip 的含义是导出文件与对应 SourceArtifact body 字节级相等，包括 BOM、CRLF、Unicode normalization 形态和尾换行。它不承诺恢复原始文件权限、owner、inode、mtime、extended attributes、resource fork 或完整路径。

导出文件由用户拥有，不再属于 Source Vault；后续本地删除不会追踪或删除它。再次从新位置导入该文件默认产生新的 origin binding，除非调用方通过受控交互显式选择已有 binding。

### 删除、撤回与外部副本

首个产品删除动作以来源 lineage 为默认语义目标，不提供只删除一个历史版本后仍保留同 lineage 其它版本的入口。执行前必须冻结该 lineage 的全部 SourceArtifact 版本、fragment、引用它们的 proposal / decision / memory / state event、FTS、lineage-tip projection、context cache、origin binding 和最小审计闭包；存在无法展开的 active 依赖时失败关闭。

DeleteRequest 持久化时，整个 lineage 及其依赖至少进入 pending，并立即停止普通读取、搜索、citation、ContextPack 与导出。实际 local purge 和 DeletionEvidence 继续复用 canonical 删除语义；任一计划组件失败时保持 failed / partial，不恢复 active，也不声明完全删除。

删除范围只包含 RadishMemory 当前受信本地设备控制的受管副本与派生数据。用户最初选择的外部原件、hardlink alias、手工副本、用户导出、文件系统快照、SQLite 空闲页、备份和其它设备不在本切片的可删除控制面；系统不得修改它们，也不得从本地 evidence 推断它们已经删除。界面和文档只能声明“已停止本地召回”或“已完成已枚举的本地受管组件处理”。

删除后的 rebuild 必须保持来源不可召回、不可引用且不可导出。`not_found` 只有在 canonical 删除证据允许且派生数据不能重建该内容时才算成功。

### 错误、日志与不可信资料

公共错误只携带稳定 category / reason、阶段、可重试性和必要稳定 ID。首个入口至少区分：

- `path_not_allowed`
- `symlink_not_allowed`
- `not_regular_file`
- `unsupported_file_type`
- `empty_file`
- `file_too_large`
- `invalid_utf8`
- `nul_byte_not_allowed`
- `source_changed_during_capture`
- `destination_not_allowed`
- `destination_exists`
- `integrity_mismatch`
- `io_failure`
- `canonical_conflict`

错误、Debug、普通日志、fixture evidence 和 CI 输出不得包含正文、完整路径、允许根、platform bookmark、导出目标、被拒绝字节或可逆路径摘要。调用方可以在本地交互层重新显示用户刚刚选择的文件名，但不能把它复制进持久诊断。

文件内容始终是不可信数据。Markdown 中的命令、链接、HTML、代码或 front matter 不能改变系统指令、allowed roots、governance、网络权限、工具授权、记忆状态或删除范围。首个入口没有网络依赖和模型调用；观察到远程获取、Provider 记录或内容外发即为策略违规。

本切片不实现静态加密，不能因为文件在本地、正文进入 SQLite BLOB 或路径被最小化，就宣称加密存储、零知识、取证级擦除或生产隐私能力。仓库和 CI 只使用合成临时文件。

### 首个实现 package 边界

`P1-I01 file snapshot contract` 新增且仅新增第一方 `crates/radishmemory-file-entry/` package。它依赖 `radishmemory-core`，负责本 ADR 的 contract ID、允许根与 symlink 检查、`.txt` / `.md` 分类、8 MiB / UTF-8 / NUL 校验、读取期间可观察变化复核、path-free `ValidatedFileSnapshot`、稳定 `FileEntryError` 和最小 `FileCaptureReceipt` 类型。

该 package 不依赖 SQLite，不读取或写入数据库业务表，不拥有 canonical persistence、origin binding、lineage-tip projection、export、deletion、UI 或通用 Capture Gateway。文件快照成功只证明一次本地读取通过当前校验，不代表 SourceArtifact 已持久化或 importer 已完成。首个 package 没有新增第三方依赖；workspace 的 40 个第三方 package、feature、native build 与网络能力保持不变。

`P1-I02 atomic source capture` 在 `radishmemory-core` 增加完整 fragment-set 校验、`SourceCapture`、path-free `SourceCaptureResult` 与最小 `SourceCaptureStore` port；`radishmemory-file-entry` 通过 `FileCapturePlan` 把 validated snapshot 映射为确定性的整文件单片段 candidate，仍不依赖 SQLite。首个 opaque binding ID 必须使用 `origin-binding-` 前缀，整体不超过 128 个 ASCII 字节，后缀只允许字母、数字、`-`、`_`、`.`，因此路径分隔符不能进入 `origin_ref`。SQLite schema v6 新增 lineage-tip、opaque origin-binding 和最小 capture-audit 三张 STRICT 表，adapter 用一个 `IMMEDIATE` transaction 提交受管 body、SourceArtifact metadata、完整 fragment、FTS、binding、tip 与 audit。

首次 capture 只接受 version 1 / 空 supersedes；内容变化只接受同 lineage 的下一 version 和精确前 tip；同 binding、相同 exact bytes 与治理快照返回已有 source facts，不增加 canonical / fragment / FTS / audit。事务开始和提交前都复验 canonical / derived / binding 状态，fragment 冲突等中途失败会恢复旧 tip 与旧 FTS。普通召回与 rebuild 从 canonical facts复算唯一 active tip，历史 source / fragment 保留精确读取但不进入普通 FTS。旧 `SourceVault` 写入口拒绝 `explicit_user_input`，防止绕过原子边界。

## 合成验收

验收只在任务专用临时目录创建合成 `.txt` / `.md`，正文、文件名与路径均不得来自真实个人资料。每个场景使用独立 namespace / store / allowed root，固定 contract 标识、时间与稳定测试 ID；失败输出必须通过敏感内容缺席检查。

| ID | 场景 | 必须观察到的结果 |
| --- | --- | --- |
| `P1-F01` | 首次导入普通 `.txt` | 创建 version 1、精确 body、fragment、FTS、tip 与新建 receipt |
| `P1-F02` | 导入含 BOM、CRLF、组合 Unicode 与尾换行的 `.md` | 原始字节、长度、摘要和 fragment byte range 可复验，不发生规范化 |
| `P1-F03` | 同 origin binding、同字节重复导入 | 返回相同 `source_id` / version，canonical、fragment 与 FTS 计数不增长 |
| `P1-F04` | 同 origin binding 内容变化 | 原子创建 version 2，精确 supersede version 1；普通召回只出现新 tip，旧版可显式读取 |
| `P1-F05` | 不同 origin binding 导入相同字节或 hardlink alias | provenance 与删除范围保持独立；不得按 digest 或 inode 合并 canonical 来源 |
| `P1-F06` | 全文查询当前文本 / Markdown | 命中 active tip，citation 精确解析到受管 source、fragment、byte range 与摘要 |
| `P1-F07` | 导出当前和历史可读版本 | 导出字节与所选 SourceArtifact 完全相等；已存在目标和 symlink 目标失败且不覆盖 |
| `P1-F08` | 显式 rebuild | 从已验真的 SourceArtifact / SourceFragment 恢复 FTS / tip，结果与重建前一致且不读取 origin file；canonical fragment 缺失时失败关闭 |
| `P1-F09` | 删除来源 lineage | 计划创建即停止读取、搜索、citation、ContextPack 与导出；完整执行后 evidence 覆盖受管闭包 |
| `P1-F10` | 删除后 rebuild | 已删除 lineage 不复活；外部原件和先前用户导出保持未修改且不被声明已删除 |
| `P1-F11` | 允许根外、`..` 逃逸、目录或非普通文件 | 使用稳定错误拒绝，store / FTS / receipt 无变化 |
| `P1-F12` | root 以下任一 symlink、symlink leaf | 使用 `symlink_not_allowed` 拒绝；不跟随目标，不泄露解析路径 |
| `P1-F13` | 空文件、不支持扩展名、非法 UTF-8 或 NUL | 分别使用稳定错误拒绝，不产生部分事实 |
| `P1-F14` | 文件恰为 8 MiB 与 8 MiB + 1 byte | 前者在其它约束成立时接受，后者以 `file_too_large` 拒绝 |
| `P1-F15` | 读取期间文件身份、长度或修改事实变化 | 以 `source_changed_during_capture` 失败，旧 lineage tip 和索引保持不变 |
| `P1-F16` | 导入或导出各提交点故障注入 | 不可观察到半个成功；只清理本任务 staging，真实原因和 retryable 状态保留 |
| `P1-F17` | Markdown 包含链接、HTML、伪指令和远程图片 | 只作为正文召回；网络请求、模型调用、工具授权和记忆写入均为零 |
| `P1-F18` | 失败消息、Debug、日志与最小 evidence 检查 | 不包含合成正文、完整临时路径、允许根、导出目标或可逆路径摘要 |

通过标准：18 个场景全部成立，policy violation count 为零，所有成功 citation 可解析，所有成功导出摘要相等，删除后普通召回与 rebuild 复活数为零。静态文档检查不等于这些运行行为已经实现；只有未来真实 importer / exporter 执行该套验收后才能声明入口能力完成。

## 被拒绝的方案

### 把 M0 fixture operation 公开为 production API

fixture mapping 为确定性评测服务，不包含真实文件授权、TOCTOU、路径脱敏、原件所有权或导出事务。直接公开会把测试逻辑变成产品协议，并绕过新的失败面。

### 只保存外部路径，查询时重新读取原文件

外部文件可能移动、修改、离线或被另一个程序替换，导致 citation 和历史版本无法复验。成功导入后必须以受管 exact bytes 为原始真相，路径只保留为最小入口 binding。

### 按路径、inode 或内容摘要自动合并来源

路径会重命名，inode / file ID 不可迁移，hardlink 共享身份，而相同字节可以来自不同授权和保留范围。它们都不能替代不透明来源 lineage。

### 允许 symlink 后只检查最终 canonical path

这会掩盖用户选择与实际读取目标的差异，并扩大路径替换与授权混淆风险。首个入口拒绝 root 以下 symlink；需要平台 capability 的体验以后通过独立评审引入。

### 在首个入口同时加入 PDF、对象存储、向量或 UI

它们分别带来解析器、native dependency、加密大对象、质量评测和交互授权问题，不能提高文本入口契约的证明力。先完成可复验的最小纵向切片。

## 后果

收益：真实文件入口不依赖外部路径长期存在；重复导入、版本、当前召回、精确导出和本地删除具有单一语义；文件系统攻击面、隐私日志和外部副本边界可用合成数据验证；现有 canonical truth 不被复制。

代价：首个入口不递归扫描目录，不跟随 symlink，不覆盖导出目标，也不保留完整文件 metadata；8 MiB 上限、整文件单片段和 SQLite BLOB 只适合窄文本切片；精确 export、lineage 删除与平台 bookmark 仍需后续最小实现评审；本地明文仍依赖受信设备保护。

## 后续实施顺序与停止线

1. `P1-I01` / `P1-I02` 已先固定文件快照、receipt、atomic capture、origin binding 与 lineage-tip 的最小 package / port 边界，不修改 M0 fixture schema。
2. 下一单元实现精确 export 与不覆盖发布，再扩展 lineage 删除闭包；只增加 `P1-F07` 至 `P1-F10` 真实需要的职责，不预建通用 workflow、插件或后台队列。
3. 继续复用现有 Source Vault、LocalSearch 和 DeletionStore；文件 adapter 不直接写 SQLite 业务表，export 不回读外部 origin file。
4. 在 macOS、Linux 与 Windows 运行 locked 检查和合成验收；静态检查、单平台结果或内存 fixture 不能替代真实临时文件行为。
5. 只有该入口通过后，才分别评审 PDF / 图片解析、加密内容寻址大对象存储、向量、模型 adapter 与 UI。

在真实验收通过前，不导入真实个人资料，不声明产品文件入口、加密存储、完整删除或生产可用；未经独立授权，不 push、不创建 PR、不修改远端状态。
