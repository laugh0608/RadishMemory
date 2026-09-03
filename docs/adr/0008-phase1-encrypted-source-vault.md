# ADR 0008：阶段 1 加密内容寻址 Source Vault

日期：2026-09-03

状态：Accepted

契约标识：`radishmemory.phase1-encrypted-source-vault/1`

## 背景

[ADR 0006](0006-phase1-text-markdown-file-entry.md) 已证明用户显式选择的文本 / Markdown 可以作为 exact bytes 进入受管 Source Vault，并保持来源版本、引用、导出、重建和删除边界。[ADR 0007](0007-phase1-local-library-host.md) 又建立 production application service、一次性系统文件授权、本地桌面 UI 与三平台宿主验收。

当前实现把小型 UTF-8 / Markdown 正文作为独立 SQLite BLOB 保存。这是 M0 和首个文本入口的可验证基线，不是长期大对象格式。[ADR 0005](0005-m0-implementation-stack.md) 已要求在 PDF、图片和其它大对象进入前，重新评审加密内容寻址对象存储、SQLite metadata 的事务协调和迁移方式。

如果把 PDF / 图片解析器、加密、密钥、文件存储、SQLite migration 与 UI 同时引入，会把密码边界、供应链、崩溃一致性和解析器风险混为一个不可评审单元。本文先冻结本地 Source Vault 原始对象的存储契约和可执行验收，不实现解析器，也不把“原始对象已加密”扩大为“整个资料库已加密”。

## 决策

### 范围与声明边界

首个加密 Source Vault 仍是单用户、单 namespace、单设备、本地进程内能力。它只保护由 Source Vault 管理的原始对象字节：

- 新写入的受管原始对象必须以经过认证的密文持久化到应用专用对象目录；
- SQLite 保存结构化 metadata、对象引用、来源事实、FTS5、投影、binding、audit 与 deletion evidence；首批不加密整个 SQLite 数据库；
- FTS、标题、media type、大小、时间、治理标签、内容摘要和其它 metadata 仍可能泄露内容或使用模式；
- 导入、解密、检索、导出和显式 verify 期间，明文会在受信本地进程内出现；本文不承诺防御已解锁设备上的恶意进程、调试器、内核、交换区、休眠镜像或用户主动导出的明文；
- 本契约不等于端到端加密、零知识同步、取证级擦除、备份清除或 production privacy approval。

因此产品和文档只能声明“受管原始对象采用本地认证加密”。在 SQLite、派生索引、平台临时状态和历史明文残留未分别验证前，不得使用“整个资料库已静态加密”或等价表述。

### Canonical identity、内容地址与物理对象

`SourceArtifact` 继续是来源事实，`source_id`、`lineage_id`、version、governance 和 provenance 继续按既有 canonical schema 解释。本文不新增 canonical 顶层对象，也不把文件路径、SQLite rowid、密钥 ID、ciphertext digest 或物理 locator 写入长期 canonical identity。

受管原始对象的逻辑地址由精确 `source_id` 与已验真的 `exact-bytes-v1` 内容摘要共同确定。物理目录名、文件名和布局属于 adapter-private、带版本的存储表示，可以迁移但不得改变 canonical fact。

首批采用一对一所有权：

- 一个不可变 `SourceArtifact` version 对应一个不可变密文对象；
- 同一 origin binding、同一 `source_id` 和同一 exact bytes 的幂等重试复用已提交对象；
- 不同 `source_id` 即使正文摘要相同，也建立不同密文对象，不进行跨 lineage 或跨 provenance 物理去重；
- 内容摘要用于完整性和幂等，不是来源身份、授权、retention 或删除范围。

该限制避免一个物理密文同时受不同 sensitivity、retention、删除请求或未来同步范围控制。未来若要跨来源去重，必须先扩展所有权、引用闭包和 deletion evidence，并通过新的 ADR；不得只增加可变 refcount 后静默改变删除保证。

### 加密 envelope 与密钥边界

每个对象使用独立随机数据加密密钥（DEK）和经过评审的 AEAD cipher suite。DEK 由设备本地 key-encryption key（KEK）包装；KEK 通过独立受信 capability 获取，不以明文写入 SQLite、对象目录、host profile、日志、诊断、fixture 或仓库。

对象 envelope 必须版本化，并至少绑定：

- contract / envelope version；
- 精确 cipher suite 与 key-wrap profile 标识；
- namespace、`source_id`、内容摘要 profile / value、原始字节长度和 media type；
- nonce 或等价的 suite-required unique input；
- wrapped DEK、ciphertext 与 authentication tag；
- 生成器版本和完成本地验证所需的最小非敏感 metadata。

namespace、`source_id`、摘要、长度、media type 与 envelope version 必须作为 AEAD associated data 或受等价认证保护，防止在不同来源或 metadata 之间交换合法密文。未知 version、cipher suite、key-wrap profile、缺失字段、重复字段、认证失败或 metadata 不匹配都必须失败关闭；不允许尝试其它算法、旧 key、明文 BLOB 或外部原件作为静默 fallback。

本文冻结密钥层级和必须满足的行为，不自行发明 cipher。[P1-S02 依赖与密码套件评审](../implementation/phase1-encrypted-source-vault-dependency-review.md)已将对象 profile 冻结为 `radishmemory.xchacha20poly1305-stream-be32/1`，将 DEK wrap profile 冻结为 `radishmemory.xchacha20poly1305-dek-wrap/1`，并选择系统随机、secret zeroization 与 macOS / Windows / Linux 精确 key provider。该评审只冻结精确版本、feature 和公开 test vector 门禁；manifest / lockfile 仍未落地，production encryption code 仍未开始。

未来零知识同步可以为同一对象 DEK 增加经过独立协议评审的设备或空间 wrapper，但不得要求服务端获得明文 DEK，也不得把本文的设备本地 KEK 直接升级为同步根密钥。同步密钥、恢复、撤销和轮换继续由 ADR 0003 及后续同步协议负责。

### 对象目录与文件系统边界

对象目录和 staging 目录必须位于平台 adapter 已解析的 RadishMemory 专用应用数据 capability 内，并与数据库、host profile 一样拒绝最终 symlink、路径逃逸和非普通文件。不得默认使用当前目录、home、仓库根、导入目录、导出目录或系统临时根作为永久对象目录。

最终对象不可变并以 create-new / no-overwrite 语义发布。staging 只能保存本任务生成的密文和最小 envelope，不得先写持久化明文临时文件。写入必须完成 flush、文件 sync、关闭、envelope / authentication / length 复验和父目录持久化，再进入可被 SQLite metadata 引用的 published 状态。

物理 locator 只允许是 adapter 生成和验证的相对 opaque ID；不得接受数据库、fixture、UI 或导入资料提供任意路径。普通错误、Debug、日志、receipt 和 deletion evidence 不输出对象目录、staging 路径、key reference、wrapped DEK、nonce、authentication tag、正文或可逆路径摘要。

### 文件系统与 SQLite 的提交协调

文件系统和 SQLite 不存在一个共同原子事务，因此成功条件必须通过显式状态机收口，不伪造跨资源原子性。首批顺序冻结为：

1. application service 分配 operation、`source_id`、lineage / version 与稳定时间，但不产生成功 receipt；
2. adapter 可以持久化不对普通读取可见的最小 capture attempt，用于精确恢复；该记录不是 canonical source；
3. 从一次性授权输入流式计算 exact digest，并直接加密到 create-new staging 对象；任何失败只留下可识别的本次 attempt；
4. 完成 sync、关闭和独立复验后，以 no-overwrite 方式发布不可变密文对象；
5. 在一个 SQLite `IMMEDIATE` transaction 内提交对象引用、SourceArtifact metadata、完整 fragment、FTS、binding、lineage tip、capture audit，并把 attempt 收口为 committed；
6. transaction 提交后重新按 SQLite reference 读取、解密并核对 envelope、AEAD、exact length / digest 与 canonical facts；只有该复验成功才返回 path-free receipt。

崩溃发生在第 5 步之前时，不得出现 active SourceArtifact；已发布但未被 committed metadata 引用的对象是 orphan candidate。崩溃发生在第 5 步之后、第 6 步之前时，canonical capture 已提交；同一 binding / bytes 的重试必须解析为幂等成功，而不是创建新 provenance。

恢复器只处理应用专用目录内、通过格式和 attempt 关系识别的对象。只有在确认对象没有任何 committed reference、没有仍可恢复的 active attempt，且删除目标身份未发生变化时，才能清理 orphan。未知文件、解析失败、重复 locator、引用分叉或 ambiguous state 必须让 library 失败关闭，不能把不认识的文件当垃圾删除。

任何 committed metadata 指向缺失、未认证、长度 / 摘要不符或错误 `source_id` 的对象时，`open_library`、读取、search、export、verify 和 rebuild 都失败关闭。rebuild 只能重建派生索引，不得重建、替换或重新加密 canonical 原始对象。

### 读取、导出与派生数据

Source Vault 按 namespace 与精确 `source_id` 解析对象 reference，验证 envelope 和 metadata，取得 KEK capability，解包 DEK 并认证解密。认证完成前不得把任何明文交给 parser、FTS、citation、UI 或导出路径。

首个实现继续支持现有文本 / Markdown exact export：解密结果必须重新核对 `exact-bytes-v1` 摘要和长度，再进入 ADR 0006 已冻结的同目录临时写入与 no-overwrite 发布。用户导出是用户控制的明文副本，不在 Source Vault 删除闭包内。

FTS5 和未来 PDF 文本、OCR、缩略图、Embedding 都是可重建派生数据，必须明确记录其生成器和原始对象引用。本文不加密这些派生物，也不授权创建它们；后续单元必须按各自敏感度和外发风险另行评审。

### SQLite v6 inline body 迁移

下一实现必须提供从 SQLite v6 inline source body 到密文对象 reference 的单调、可中断、可恢复迁移，不建立第二套长期来源真相。

- migration 在普通 library operation 暴露前运行；未完成、失败或 ambiguous 时资料库保持不可用，不混合返回 inline 和 object-backed source；
- 每个 body 先从 SQLite BLOB 读取并复验 canonical exact digest，再创建和验证密文对象；损坏 body 不迁移，也不建立空对象或新来源；
- 只有密文对象 durable publish、reference transaction 提交并完成 read-back 验证后，才能移除对应 active inline body；
- 中断后必须从已持久化 migration / attempt 状态恢复，已提交对象幂等复用，未引用 orphan 按冻结规则处理；
- migration 不修改 `source_id`、lineage、version、fragment、citation、governance、binding、audit 或 deletion state；
- migration 前的 plaintext 可能仍存在于 SQLite 空闲页、临时文件、文件系统快照或备份。迁移成功只证明 active Source Vault 读取改为认证密文对象，不证明历史明文已物理清除。

当前仓库和验收只使用合成资料，因此 migration fixture 也只使用合成 SQLite v6 数据库。未来若真实用户数据已存在，必须在迁移前另行冻结备份、空间需求、失败恢复、密钥丢失和用户可见提示。

### 删除与密钥失效

既有 `DeleteRequest` / `DeletionEvidence` 继续拥有删除真相。对象文件和 wrapped DEK 属于 `source_body` 组件的 adapter-private 物理执行范围，不新增平行删除协议。

首批不跨来源物理去重，因此一个 SourceArtifact body 的删除可以精确定位一个密文对象。执行成功必须验证 committed reference 已关闭、对象 locator 不可由普通 Source Vault 读取、对象文件和该对象的 wrapped DEK 已按计划处理，且 FTS / fragment / metadata 等其它组件仍分别报告真实结果。

删除密文文件或 wrapped DEK 不等于底层介质、快照和备份不可恢复。只有经过单独冻结的 key-destruction、备份到期和副本枚举证据成立时，才能使用对应更强声明；本阶段继续只报告本地应用可复验的组件结果。

### 密钥不可用与恢复

KEK 缺失、锁定、拒绝授权、wrapper 损坏或错误 key 都是显式失败状态。应用不得生成新 KEK 后认领既有对象，不得跳过无法解密的来源、建立空库或从外部原件静默重导入。

首批不实现用户口令恢复、恢复码、跨设备恢复、key escrow、自动 rotation 或远程解锁。P1-S02 已冻结 key provider 缺失、锁定、用户取消、ambiguity、bootstrap eligibility 与永久丢失语义；后续实现和宿主验收必须分别证明这些行为。在恢复路径真实成立前，文档必须明确本地 key 丢失会使对应对象不可恢复。

## 实施单元

1. `P1-S01 storage contract`：接受本文，冻结声明、identity、envelope、密钥、提交、迁移、删除和合成验收；不改 production code；
2. `P1-S02 dependency and cipher review`：已由[专项评审](../implementation/phase1-encrypted-source-vault-dependency-review.md)选择精确 AEAD / key-wrap / random / platform key provider，并冻结版本、test vector、许可证、native build、系统授权、维护和三平台影响；
3. `P1-S03 encrypted object adapter`：先以 `P1-S03a portable crypto dependency landing` 落地 portable cipher / wrap、AAD codec 与合成测试，再独立评审应用专用目录、streaming envelope、immutable publish、认证读取和稳定脱敏错误；
4. `P1-S04 SQLite coordination and migration`：实现 object reference、capture attempt、v6 migration、orphan reconciliation、verify / rebuild 与 deletion execution；
5. `P1-S05 application and host acceptance`：接入 application service / UI，完成合成迁移、重启、key failure、故障注入和三平台 locked / 真实宿主证据。

只有已经接受的 `P1-S02` 与后续 `P1-S03` 至 `P1-S05` 分别通过后，才评审 PDF / 图片的 media type、parser sandbox、页码 / 区域 citation、质量指标和派生数据治理。

## 合成验收

所有验收只使用任务临时目录、合成文本字节、合成 SQLite v6 数据库和测试专用密钥 provider。fixture、日志、截图和 CI artifact 不保存真实密钥、明文对象、完整对象路径或可复用 wrapped DEK。

| ID | 场景 | 必须观察到的结果 |
| --- | --- | --- |
| `P1-SF01` | 新 source capture | final object 只含版本化 envelope / 密文；SQLite reference、canonical facts、FTS、binding、tip 与 audit 成立后才返回 receipt |
| `P1-SF02` | 相同字节、不同 provenance | digest 相同但 source、lineage、governance、密文对象和删除范围独立 |
| `P1-SF03` | 同 binding / bytes 幂等重试 | 复用已提交 source 和对象，不新增 version、object、attempt 或 audit |
| `P1-SF04` | 关闭重开与读取 | 按 reference 认证解密并复验 source / digest / length；不依赖 origin file |
| `P1-SF05` | 精确导出 | 解密后 exact bytes 摘要相等；目标存在、symlink 或并发占用时不覆盖 |
| `P1-SF06` | ciphertext / tag / nonce 篡改 | open、read、search、export、verify 和 rebuild 失败关闭，不输出敏感值 |
| `P1-SF07` | envelope 与 SQLite metadata 交换 | associated data / reference 校验拒绝，不把合法密文认领给错误 source |
| `P1-SF08` | committed object 缺失或 locator 分叉 | library 不可用，不隐藏为 0 source，不从 origin 或 inline body fallback |
| `P1-SF09` | KEK 缺失、锁定、拒绝或错误 | 稳定、可区分的失败；不创建新 key、空库或成功 receipt |
| `P1-SF10` | staging 写入、sync、publish 失败 | 无 canonical source；只留下本次可识别状态，不覆盖其它对象 |
| `P1-SF11` | publish 后、metadata commit 前崩溃 | 没有 active source；重启精确识别 orphan / recoverable attempt，不删除 ambiguous object |
| `P1-SF12` | metadata commit 后、receipt 前崩溃 | 重启可完整读取；相同请求幂等返回既有事实，不创建新 provenance |
| `P1-SF13` | 磁盘满、权限撤销、目录 / object symlink | 失败关闭，不越过应用目录，不留下明文 staging 或部分成功 |
| `P1-SF14` | 健康 SQLite v6 migration | source identity、版本、citation、binding、audit 与 exact bytes 不变，active body 改由认证对象提供 |
| `P1-SF15` | migration 中断与重试 | 从精确状态继续，已发布对象幂等复用，未验证 body 不被移除 |
| `P1-SF16` | legacy BLOB、migration state 或 object 篡改 | migration / open 失败关闭，不把损坏数据升级到新 schema |
| `P1-SF17` | lineage deletion | 对象、wrapped DEK、reference 和既有十组件结果真实对应；不声称清除外部导出、快照或备份 |
| `P1-SF18` | 不可信内容与诊断 | 零网络、零模型、零工具授权；error / Debug / log / receipt 不含正文、路径、key material 或 envelope secret |

通过标准：十八项场景全部成立；所有成功读取 / 导出 exact digest 相等；认证失败、缺 key、崩溃与 migration 损坏不产生 false success；删除后普通召回与 rebuild 复活数为零；policy violation count、plaintext persistent staging count 和 unknown-object auto-delete count 均为零。实现阶段还必须通过 Linux、macOS、Windows locked build / test 与聚合 `Candidate Quality`；真实 platform key provider 和宿主授权行为必须单独留下实际证据，headless test 不能替代。

## 被拒绝的方案

### 直接把 PDF / 图片继续塞进 SQLite BLOB

这会把 M0 的小文本实施选择固化为长期大对象格式，放大数据库备份、migration、内存、删除与损坏影响，也绕过 ADR 0005 已冻结的重新评审门禁。

### 把内容摘要直接当 canonical source identity

相同字节可以来自不同用户操作、lineage、治理和 retention。按 digest 合并会丢失 provenance，并让一个删除请求影响另一个独立来源。

### 首批跨 lineage 物理去重

共享对象需要冻结多 owner、引用闭包、治理冲突、key wrapper、删除和备份证据。当前没有足够收益证明值得扩大首个加密切片。

### 只加密对象文件并宣传整个资料库加密

SQLite metadata、FTS、标题、摘要、时间和派生内容仍可能泄露语义。扩大声明会违背威胁模型，也掩盖后续数据库 / 索引保护仍需独立决策。

### 数据库先提交、随后尽力写对象

进程终止或磁盘错误会留下已提交 canonical source 指向缺失或部分对象。后台补偿不能把这种状态伪装为一次成功 capture。

### 认证失败时回退到旧 BLOB 或外部原件

fallback 会隐藏篡改、key 错误和 migration 漂移，并可能绕过用户一次性授权。无法认证的 canonical object 必须保持失败关闭。

### 自行实现密码算法或通用 crypto abstraction

本项目只组合经过评审的成熟 primitive 和精确 profile。没有已选依赖、test vector 与 threat review 时，不建立看似通用但无法验证的密码框架。

## 后果

收益：PDF / 图片进入前先获得稳定的原始对象真相、认证加密、崩溃恢复、迁移和删除边界；canonical identity 与物理布局解耦；未来 parser、同步和 key wrapper 可以复用明确接口，而不把 SQLite BLOB 或设备本地 key 固化为长期协议。

代价：文件系统与 SQLite 的协调需要显式 attempt / orphan 状态；每来源独立对象会放弃跨 provenance 存储去重；平台 key provider、密钥丢失、migration 空间和真实宿主验收增加实现成本；只加密原始对象不能隐藏 FTS 与 metadata。

兼容性：M0 Canonical Schema 已允许 SourceArtifact content 物理内联或外置，因此本文不修改九种 canonical 顶层对象、`radishmemory.m0/1`、citation 或现有 DeleteRequest / DeletionEvidence schema。具体 object reference、envelope、attempt 和 migration 表是 adapter-private、带版本的实现格式；若后续 PDF / 图片需要扩展 source kind、media type、片段坐标或 citation，必须通过新的 canonical 兼容性评审。

## 当前实施状态与停止线

`P1-S01 storage contract` 与 `P1-S02 dependency and cipher review` 已接受；精确 crypto / key-provider profile 已冻结，但 production dependency、encryption、object directory、key provider、SQLite migration 与 host integration 均未落地。当前代码仍使用 SQLite v6 inline plaintext body，不能因为两项评审已接受而宣称加密 Source Vault 已经可用。

- 未经 `P1-S03a` 独立授权，不新增 crypto 或其它 production dependency，不修改 manifest、`Cargo.lock`、notices 或第一方 package；
- 下一最小单元只允许落地 portable cipher / wrap dependency、AAD codec、合成 provider / random seam 和 known-answer / tamper tests；不得加入或访问 keychain / platform security provider；
- 未经独立实现授权，不修改 core port、SQLite schema、application service 或 UI；
- 未经独立平台授权，不启动 GUI / VM、不访问系统 key store、不修改权限或签名配置；
- 不使用真实个人资料、真实密钥或生产数据库；不进入 PDF / OCR、图片解析、Embedding、模型、网络、同步、发布或部署；
- 不 push、不创建 PR、不触发远程 CI、不 merge，也不把文档契约外推为实现证据。
