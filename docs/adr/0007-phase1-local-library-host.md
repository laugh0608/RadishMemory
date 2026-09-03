# ADR 0007：阶段 1 本地资料库宿主与显式文件授权

日期：2026-08-30

状态：Accepted

契约标识：`radishmemory.phase1-local-library-host/1`

## 背景

[ADR 0006](0006-phase1-text-markdown-file-entry.md) 已冻结并实现文本 / Markdown 文件的快照、原子 capture、精确导出、来源 lineage 删除和 `P1-F01` 至 `P1-F18` 合成验收；[PR #2](https://github.com/laugh0608/RadishMemory/pull/2) 的最终 head `9bd0af5` 已在 [workflow run 33302423840](https://github.com/laugh0608/RadishMemory/actions/runs/33302423840) 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`，随后以 merge commit `c56f13f` 合入 `master` 并回流 `dev`。

这些能力目前仍由测试或 fixture 调用。`radishmemory-file-entry` 不拥有 UI 文件选择、production Capture Gateway、ID / 时间分配、来源浏览或应用生命周期；`radishmemory-m0` 只运行冻结 fixture，不是产品宿主。继续进入 PDF、图片、向量或模型会让新的解析、供应链和外发风险建立在尚未成立的用户授权面之上。

本文冻结阶段 1 首个本地资料库宿主的职责、一次性文件授权、应用操作、读取模型、UI 行为和验收。它不新增 canonical 顶层对象，不冻结桌面 UI 工具包、FFI / IPC 表示、安装包、自动更新或长期 platform bookmark 格式。

## 决策

### 运行与信任边界

首个宿主是单用户、单 namespace、单设备的本地桌面应用。领域、文件入口与 SQLite adapter 在同一受信本地进程内运行：

```text
desktop UI
  -> explicit platform selection capability
  -> production application service
  -> radishmemory-file-entry + radishmemory-core ports
  -> radishmemory-sqlite
```

首批不启动本地 HTTP 服务、后台 daemon、文件监视器、插件宿主、Provider、RadishMind、同步或其它网络监听。UI 工具包可以在后续实现评审中选择，但不得改变本文的数据所有权、失败关闭、日志和授权语义；新增依赖必须单独记录版本、许可证、native build、隐式联网和分发影响。

SQLite 文件位于平台应用数据目录下的 RadishMemory 专用目录。平台 adapter 必须给应用服务一个已经解析的专用目录 capability；不得默认使用当前目录、用户目录、文件系统根、仓库根或用户任意导入目录作为数据库位置。数据库路径、应用数据根和系统用户目录不进入普通日志、UI 遥测、canonical 对象或错误文本。

首批仍是本地明文数据库，不声明静态加密、零知识、取证级擦除、备份清除或生产隐私保证。开发、测试、CI 和截图只使用合成资料。

### 显式文件授权与 platform bookmark

每次导入、更新和导出都由当前可见 UI 中的用户操作触发系统文件选择器。选择器返回的一次性 capability 只在本次操作存活：

- `Import New Source` 为一次新来源入口分配新的 opaque `origin_binding_id`、lineage 和 source ID；
- `Update Existing Source` 先由用户选择一个现有 lineage，再重新选择本地 `.txt` / `.md`，并显式复用该 lineage 的 opaque binding；
- `Export Source Version` 先选择精确 source version，再由用户选择一个不存在的目标；
- 用户取消、选择器失败或权限撤销时，不创建 ID、不打开数据库写事务、不留下成功 receipt；
- 选定路径只在本次进程内调用链存在，操作完成后不进入 host state、普通日志或持久诊断。

首个宿主不持久化 platform bookmark、security-scoped bookmark、文件访问 token 或原始路径，也不自动判断“这个路径以前是否导入过”。再次通过 `Import New Source` 选择相同文件仍是新的 provenance；只有用户通过 `Update Existing Source` 明确选择已有 lineage 时才复用 binding。该决策避免在尚无静态加密和平台凭据生命周期设计时保存敏感 capability。

如果 UI 工具包只返回路径，平台 adapter 只能把用户选中的文件及其直接 parent 作为本次 `FileReadRequest` / `FileExportRequest` 的允许范围；不得扩大为 home、volume 或文件系统根。`radishmemory-file-entry` 继续执行 canonical root、symlink、普通文件、TOCTOU、类型、大小、UTF-8 和目标不覆盖检查。

未来若需要后台重导入、文件监视或跨启动直接访问原件，必须以新 ADR 冻结 bookmark 加密 / 保护、存储、失效、迁移、撤销和删除语义；不得静默扩大本契约。

### Production application service

新增一个 production application service 作为 UI 与现有领域 / adapter 的唯一组合入口。它可以位于新的第一方 workspace package，但必须保持以下依赖方向：

- UI 只调用 application operation，不读取 SQLite 表、rowid、FTS 分数或 adapter-private binding；
- application service 复用 `SourceCaptureStore`、`SourceVault`、`LocalSearch` 与 `DeletionStore`，只为真实宿主缺失的来源目录、lineage resolution 和应用事务增加精确 port；
- `radishmemory-core` 不依赖 UI、SQLite、路径、platform bookmark、随机数实现或系统时钟；
- `radishmemory-file-entry` 不依赖 SQLite，也不分配 canonical ID、持久化 binding 或决定删除闭包；
- SQLite 仍是 canonical facts、managed exact bytes、FTS、projection、binding、audit 与 deletion evidence 的唯一当前 adapter，不建立 UI 专用数据库或第二套来源表。

应用服务至少提供以下操作：

1. `open_library`：打开文件数据库，执行 capability、migration、canonical / derived integrity 检查；
2. `import_new_source`：消费一次性文件选择，分配 opaque ID 与时间，提交 version 1 并返回 path-free receipt；
3. `update_source`：解析现有 lineage tip，分配下一 source / fragment ID，精确 supersede 当前 tip；
4. `list_sources` / `get_source`：浏览当前来源及显式历史版本；
5. `search_sources`：调用现有全文检索并返回可解析 citation 所需的稳定引用；
6. `export_source`：按 namespace 与精确 source ID 读取已验真 managed body，再调用 file-entry 原子不覆盖导出；
7. `delete_source_lineage`：展开完整来源 lineage 与 active memory 依赖，持久化删除计划、执行并读取真实 evidence；
8. `verify_library` / `rebuild_recall`：显式报告完整性或派生漂移，不扫描外部原件作为 fallback。

ID 与时钟是 application runtime capability。测试使用确定性实现；production 实现必须使用受审阅的随机 ID 来源和 UTC wall clock，并保证同一 operation 内时间事实一致。不得从文件路径、正文摘要、inode、SQLite rowid 或当前时间单独推导 canonical identity。

### 来源目录与 UI 读取模型

宿主需要只读、可分页、稳定排序的来源目录。读取模型不是 canonical object，不作为同步格式，也不复制正文；至少包含：

- namespace、lineage、当前 source ID、version；
- 标题是否存在及其受保护显示值；
- source kind、media type、content length、content digest profile；
- observed / captured time、sensitivity、retention 与 deletion state；
- 当前 / 历史标记与历史版本计数。

目录和详情必须从已验真的 SourceArtifact、lineage tip 和治理事实产生。缺失 tip、多个 tip、版本断裂、binding / audit 漂移或 canonical body 损坏时整体失败关闭；不得把损坏行隐藏成空资料库。列表不返回完整路径、allowed root、bookmark、SQLite schema、rowid 或正文预览。

### 首批 UI 行为

首批 UI 至少呈现：

- 资料库启动、迁移、完整性和不可用状态；
- 新建导入、更新已有来源、成功 / 幂等 / 新版本 receipt；
- 当前来源列表、来源详情和版本历史；
- 全文搜索、结果来源与 citation byte range；
- 当前或历史版本的精确导出；
- lineage 删除确认、执行状态、逐组件结果和 evidence 范围；
- 用户取消、权限失败、文件变化、目标存在、完整性损坏和重试建议。

UI 可以在当前本地交互中显示用户刚刚选择的文件名和系统选择器返回的可见路径，但不得把路径复制到持久应用错误、普通日志、诊断包、截图 fixture 或 canonical metadata 之外的 host state。删除 UI 必须明确区分 RadishMemory 受管副本与外部原件 / 用户导出，不能把本地 evidence 显示为外部副本或备份已删除。

首批 UI 不包含聊天、模型回答、PDF / OCR、图片、Embedding、网页抓取、目录递归、剪贴板监听、后台导入、同步、插件或通用 workflow engine。

### 错误、日志与恢复

application error 只公开 operation、稳定 category / reason、retryable、必要稳定 ID 和可安全展示的阶段。它可以保留 core、file-entry 或 SQLite error 作为本地 source chain，但 `Display` / `Debug`、普通日志和 UI telemetry 不得输出正文、路径、allowed root、bookmark、数据库路径、被拒绝字节或可逆摘要。

启动时数据库 capability、migration、canonical integrity、FTS / projection 或 binding 验证失败时，宿主显示不可用状态并停止普通操作。只有派生数据损坏且 canonical facts 已通过完整复验时，用户才能显式执行 rebuild；canonical 损坏不允许重建、重新读取外部文件或创建空数据库来掩盖。

首批不自动恢复正在进行的 UI operation。capture 与计划提交继续依赖现有 SQLite transaction；export 继续依赖任务临时文件和原子不覆盖发布；删除继续报告真实 pending / failed / completed evidence。进程终止后重新打开必须从持久事实恢复状态，不从 UI cache 推断成功。

## 实施单元

1. `P1-H01 host contract`：同步 Phase 1 远程证据，接受本文并冻结 host operation、一次性授权和合成验收；
2. `P1-H02 application service`：增加 host application package、ID / clock runtime capability、错误与 open / import / update orchestration；
3. `P1-H03 source catalog`：增加来源目录、lineage / version resolution、search citation、export 与 deletion application operation；
4. `P1-H04 desktop UI`：评审并引入桌面依赖，实现平台选择 adapter 与首批资料库界面；
5. `P1-H05 host acceptance`：覆盖关闭重开、失败关闭、脱敏、无副作用、平台选择和三平台 locked CI。

这些单元按真实依赖顺序推进，可以拆成多个 Conventional Commit；不要求用一个巨大提交同时引入文档、应用服务、UI 依赖和平台代码。

## 合成验收

所有自动验收只使用任务临时目录、合成 `.txt` / `.md` 和合成数据库路径；完整路径和正文不得进入失败输出或 CI artifact。

| ID | 场景 | 必须观察到的结果 |
| --- | --- | --- |
| `P1-HF01` | 首次启动空资料库 | 文件数据库成功建立并通过 capability / migration / integrity 检查；无导入副作用 |
| `P1-HF02` | 用户选择并导入 `.txt` / `.md` | application service 分配 opaque IDs，返回 path-free receipt，目录与搜索出现 current source |
| `P1-HF03` | 关闭并重新打开 | 来源、版本、search citation 与 managed body 保持可复验，不依赖 origin file |
| `P1-HF04` | 明确更新已有来源 | 复用 binding / lineage，创建严格下一版本；重复字节幂等，变化字节只召回新 tip |
| `P1-HF05` | 浏览目录和版本历史 | 稳定排序、分页与版本计数正确，不返回正文、路径、bookmark 或 adapter metadata |
| `P1-HF06` | 搜索并解析 citation | 只返回 active tip，citation 精确解析到 managed source / fragment / byte range / digest |
| `P1-HF07` | 导出当前与历史版本 | 精确字节恢复；目标存在、symlink 或并发占用时不覆盖，不改变资料库 |
| `P1-HF08` | 删除完整 lineage | plan 即停止召回和导出，执行与 evidence 真实覆盖受管闭包；外部原件保持不变 |
| `P1-HF09` | 取消选择或授权失败 | 不分配可观察 ID、不写数据库、不产生成功 receipt 或残留 staging |
| `P1-HF10` | 重启前后显式 verify / rebuild | 派生漂移可显式修复；canonical 或 binding 损坏持续失败关闭 |
| `P1-HF11` | 不可信 Markdown | 只作为来源正文与 FTS 输入；网络、模型、工具授权和记忆写入为零 |
| `P1-HF12` | UI / error / Debug / log 脱敏 | 不包含正文、完整路径、allowed root、bookmark、数据库路径或可逆路径摘要 |

通过标准：十二项 host 场景全部成立，所有成功 citation 可解析，所有成功导出摘要相等，删除后普通召回与 rebuild 复活数为零，policy violation count 为零；Linux、macOS、Windows locked build / test 和聚合 `Candidate Quality` 通过。真实系统选择器与人工可见 UI 的平台行为必须单独留下实际证据，headless test 不能替代。

## 被拒绝的方案

### 把 fixture runner 变成产品宿主

fixture runner 使用固定 logical key、确定性 ID、冻结时间和测试入口，不拥有用户授权、应用目录、平台选择、真实错误呈现或交互取消。扩展它会把评测 mapping 误当成 production API。

### 让 UI 直接调用 SQLite 或文件入口

UI 无法独立正确分配 lineage / version、展开删除依赖、保持错误脱敏和协调 capture / export。直接调用会把应用规则散落到界面事件，并形成 adapter schema 依赖。

### 首批保存原始路径或 platform bookmark

路径和 bookmark 是敏感 capability；当前没有静态加密、平台凭据生命周期、撤销或迁移契约。一次性选择已能支持显式导入、更新和导出，首批不持久化。

### 在宿主任务中加入 PDF、向量或模型

这些能力分别引入解析器、大对象、native dependency、质量评测和外发策略。先建立 production application boundary、授权和用户可见治理，后续单元才能复用同一来源与删除语义。

### 通过本地 HTTP 服务连接 UI

首个单进程宿主不需要监听端口。HTTP 会新增认证、CSRF / origin、端口冲突、日志和网络暴露面，当前没有收益证据。

## 实施状态（2026-08-30）

P1-H02 / P1-H03 已由 `radishmemory-application`、core `SourceCatalog` 与 SQLite v6 adapter 实现。经项目所有者明确授权后，P1-H04 已新增 `radishmemory-desktop`，实现平台应用目录、原子 host profile、production random / UTC runtime、一次性 native picker、资料库 / 来源 / 版本 / 搜索 / citation / 导出 / lineage 删除 evidence / verify / rebuild UI；desktop package 在第一方业务 package 中只直接依赖 application service，不绕过本文依赖方向，平台与 UI crate 则以单独依赖评审为准。

P1-H05 已取得纯合成数据的 macOS AppKit、Windows ARM64 native dialog 与 Debian ARM64 / GNOME Wayland XDG Portal / GTK open / save picker 实际证据，覆盖可见窗口、取消、导入、关闭重开、路径 / 错误脱敏和对应应用数据保护边界；Windows 实机暴露的 OpenGL-only renderer 阻断已最小切换为 `wgpu`，当前 head `c5dba35` 又在 workflow run `33751048480` 通过 Linux / macOS / Windows locked build、Clippy、test 与聚合 `Candidate Quality`。三个目标可达的 333 个 crates.io package、license option、完整文本、默认字体、bundled SQLite 与条件平台依赖又由 [专门复核](../implementation/phase1-third-party-notices.md)收口，因此 P1-H05 gate 完成。这些证据仍不外推为其它 Linux desktop / portal backend、Zenity fallback、签名发行包或真实个人资料授权。

后续阶段评审已通过 [ADR 0008](0008-phase1-encrypted-source-vault.md) 冻结受管原始对象认证加密、内容地址、设备本地 KEK 包装、文件系统 / SQLite 协调、v6 migration 和删除边界，并由 [P1-S02](../implementation/phase1-encrypted-source-vault-dependency-review.md)选定精确 cipher / key-provider profile；这不改变本文的一次性文件授权，也不代表依赖已落地或加密 Source Vault 已实现。

## 后果

收益：已经通过三平台验证的文本 / Markdown 能力获得正式应用入口；UI、平台授权、ID / 时间、目录读取和错误恢复有单一职责；路径与 bookmark 不进入持久状态；PDF、图片、向量和模型以后可以复用同一应用边界。

代价：用户更新来源时必须重新选择文件；首批没有后台监视或跨启动原件访问；桌面工具包与平台依赖已增加显著跨目标供应链和 native surface；本地数据库仍是明文；真实 UI 和系统选择器证据需要平台运行环境，不能只靠 Rust 单测。

## 停止线

- 在 `P1-H02` / `P1-H03` 完成前，不让 UI 直接拼装 `FileCapturePlan`、读取 SQLite 表或自行展开删除闭包；
- 桌面依赖评审与授权门禁已经满足；后续新增 / 升级依赖仍须重新评审，且未经单独授权不安装平台工具链、不写入系统权限或签名配置；
- `P1-HF01` 至 `P1-HF12`、真实平台选择和 notices 已成立，但签名发行包、真实个人资料与后续能力仍须独立授权和验证；
- 不因宿主存在而进入 PDF / OCR、Embedding、模型、网络、同步、自动发布或通用 workflow engine；
- 不把本地明文、一次性文件选择或平台沙箱描述为加密存储、零知识或完整删除。
