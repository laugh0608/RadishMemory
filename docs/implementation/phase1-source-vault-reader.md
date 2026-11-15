# P1-S04 加密资料库读取切片

状态：`P1-S04 object-backed read slice implemented — synthetic acceptance`（2026-09-26）。

本批承接 `3dd312b` 的历史删除请求兼容，提供独立读取会话，复用既有来源、目录和检索规则。不改变 canonical schema、记忆状态或权限语义，不新增数据库迁移。普通 `SqliteDatabase::open` 仍只允许 v6；默认 `LocalLibrary` / UI 尚未切换加密数据流；2026-10-01 后续已完成[显式 application 读取接入](phase1-source-vault-application.md)，其证据与本批独立 reader 证据分开记录。

2026-10-28 后续：`4c9b0be` 的 [ADR 0009](../adr/0009-versioned-chinese-recall.md)派生 profile 已由共享 `LocalSearch` 读取，认证 `LibraryReadSnapshot` 同时返回实际检索格式，不增加一次系统 key 读取。查询、精确正文 / 引用与升级维护仍分属既有边界；新字段仍为明文，全库认证成本没有消除。双入口合成结果与有限成本见[中文检索记录](phase1-chinese-recall.md)，下文历史“下一步”保留为 9 月 26 日交接。

2026-11-02 后续：`2156759` 为既有 reader / snapshot / 对象认证路径增加 opt-in 的固定阶段计量；默认 span 无时钟、collector 或输出，不改变 key 复验、锁、对象 / canonical / FTS 检查与错误顺序。`e0b9926` 只减少冻结字符范围内的派生字段编码分配 / 格式化，编码字节保持。32 份与 `3f05a13` 的 100 来源 / 160 版本[规模基线](../evaluation/phase1-local-library-scale.md)均为独立合成 release 输入；5 命中仍有 13 轮全库对象认证，不推导生产容量或跨操作会话复用。`3cde127` 的[原生局部结果](../evaluation/phase1-recall-native-acceptance.md#2026-11-02-macos-原生局部结果)补充两来源、三版本库的读取 / 格式切换 / 精确导出，不替代本页全部故障与平台边界。

## 读取边界

- `PlatformKeyProvider::open_library_reader` 只接受已完成迁移的维护 v8 至 v12。先验证数据库 schema、namespace / device / provider，再加载既有 key；缺库、未完成迁移或身份不匹配不加载 key，不创建空库、新 key 或迁移 checkpoint。
- `LibraryReader` 持有数据库 exclusive session、目录与数据库身份观察及 key。每次查询先认证全部登记对象、校验 canonical / 派生事实，再解析结果；返回前重复认证和数据库身份检查。损坏对象、缺对象、未知文件、派生漂移或晚到损坏均失败关闭，不回退旧 BLOB 或外部原件，不在读取中重建索引。
- SQLite `ObjectReadView` 是调用方已认证正文集合上的临时查询视图，不是独立的认证 capability。SQLite 重新核对正文长度、摘要、canonical 字段、fragment / memory provenance、FTS 和 lineage tip；Source Vault 负责真实对象认证。视图不暴露写入接口，也不为读取实现占位写方法。
- 来源和 fragment 按 namespace 与精确 source ID 读取；不存在或已关闭删除召回的来源返回 `None`，错误 namespace 返回失败。pending / abandoned capture 不进入来源目录或搜索；即使目标不可见，库中的未知或损坏对象仍导致失败。
- `SourceCatalog` 复用相同 SQL、版本连续性检查、captured-time / lineage 稳定排序及 offset / limit 切页逻辑。`LocalSearch` 复用相同 FTS 查询和候选校验，namespace、敏感度、可用时间、retention 和记忆 valid-time 在 top-k 前过滤；未确认 proposal 不成为搜索记忆。
- 会话关闭释放锁和 key；会话期间其他数据库写入失败。读取结果是调用方持有的明文 canonical 值，会话不跨查询缓存解密正文；不承诺撤回先前返回的副本或进程明文取证清除。

2026-10-13 的[只读动作认证会话](phase1-source-vault-application.md#2026-10-13-只读动作认证会话)在上述 reader 上增加固定目录 / 版本 / 恢复快照入口：首尾使用同一 store 连接分别双读 key，比较首尾值并在交付前再次认证对象和数据库。它只服务桌面显式打开与刷新，不改变其它 reader / 写用例的生命周期，也不跨动作保留 key 或锁。

## 本批发现并修复的 Unix 锁问题

独立子进程回归在修复前复现：`LibraryReader` 仍存活时，第二进程能够通过数据库打开检查并进入 key loader。同进程 SQLite 连接检查此前仍返回锁冲突，不能据此证明跨进程互斥。根因是共用 `Observation::verify_identity` 打开并关闭数据库的额外文件描述符；Unix 的 POSIX 文件锁会因同进程关闭该 inode 的任意描述符而释放。这与 [SQLite 官方锁风险说明](https://www.sqlite.org/howtocorrupt.html) 一致。

数据库身份检查改用独立 `DatabaseObservation`：Unix 使用无跟随路径元数据核对 device / inode，不额外打开或关闭数据库描述符；活动会话由 SQLite 自身连接持有 inode。Windows 继续使用原有 native file identity 检查。此修复覆盖共用 bootstrap、migration、capture、reconciliation、abandonment、verify / rebuild、deletion 与 reader；普通不可变对象仍保留原先的句柄观察机制。主机会话期间也不得绕过 SQLite 自行打开 / 关闭数据库文件。

回归除“锁定时无法打开 reader、释放后可重开”外，还让独立子进程直接尝试 `BEGIN IMMEDIATE`，验证初始化 key 操作、capture 的 publish / commit / read-back 和 deletion 的意图 / 退役 / 执行 checkpoint 持续拒绝并发写入。数据库路径替换拒绝检查保留；旧同进程证据不替代本次跨进程结果。

数据库文件还曾复用单个对象 envelope 的大小限制，导致包含多个对象、索引或历史空闲页的合法 SQLite 文件被拒绝。本批将数据库的 regular-file / symlink 检查与单对象大小检查分开，并用超过 envelope 上限的合法 SQLite 空闲页库验证初始化、迁移和读取。密文对象本身的大小上限保持原契约。

## 精确导出衔接

读取返回既有 `SourceArtifact`，其正文已经完成对象认证、canonical exact digest 与长度校验，可交给既有 `radishmemory-file-entry::export_managed_source`。导出继续使用用户明确目标与 allowed roots、同目录临时写入和 no-overwrite 发布；调用方应在导出期间持有读取会话，阻止并发 canonical 删除。用户已导出的明文副本不在 Source Vault 删除闭包内。

经项目所有者确认，仅增加第一方 `radishmemory-file-entry` dev-dependency；Cargo 离线更新 lockfile 只增加这一条依赖边，453 个 package 的 name / version / source / checksum 集合不变。组合验收实际调用既有导出器，覆盖迁移来源与新版本、关闭重开后的导出、BOM / CRLF / 组合 Unicode 的原字节和摘要、拒绝覆盖已有文件及越界目标且无临时文件残留。独立子进程验证导出期间数据库写入仍被阻止；释放读取会话后执行真实删除，再次读取无来源，先前明确导出的副本保持原字节。未新增 production 导出 API 或 runtime 依赖。

## 验证与限制

合成测试覆盖迁移来源和新 capture、历史版本、namespace / device 隔离、缺 key / 错 key、目录分页第 201 条及越界 offset、迁移前后目录 / 搜索一致、已确认记忆回源、敏感度 / retention / 时间过滤、FTS / tip 漂移、缺失 / 损坏 / 未知对象、返回前晚到损坏、数据库替换、删除请求登记后与执行后不可见、重建不复活、pending / abandoned capture 不暴露、独占会话释放与独立子进程重开。数据库替换注入仅在 Unix 执行；不外推 Windows 文件共享行为。

本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：220 个仓库文件、notices、fmt、all-targets / all-features locked Clippy 与 309 个 Rust tests。新增 14 项 reader 测试、3 项导出组合验收、3 项共用协调器锁回归；9 个 ignored helper 由父测试实际执行，不另计入通过数。35 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest、BOM / CRLF / 组合 Unicode 原字节回读定向测试及 `git diff --check` 通过。既有 P1-F17 临时端口测试在沙箱内遭 PermissionDenied，按权限流程在沙箱外完整重跑后通过，未放宽检查。测试仅使用隔离合成库和测试 key；未调用真实 Keychain / OS key store。现有 plaintext catalog / search 回归继续覆盖共用查询实现。

每次读取仍进行全库认证，分页没有降低全库读取成本；201 条合成数据通过不构成大库性能验收，也没有修复宿主目录第 201 条不可访问的 R02 UI 缺口。FTS 仍含完整可读正文。当前 Windows / Linux 新代码、真实 key store 和宿主生命周期未验收；不声明完整加密产品可用。

<a id="q04-single-search-review"></a>

## 2026-11-15 Q04 单次加密检索的认证边界评审

状态：2026-11-15 所有者接受单次检索中间观察点减少的取舍。单次应用检索的来源筛选、排序和同批命中回源已合并为一个固定只读动作，有效范围为显式加密 `EncryptedLibrary::search_sources_filtered`。沿用 [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md#读取导出与派生数据)、[Source Vault 信任边界](../privacy-threat-model.md#阶段-1-加密-source-vault-信任边界)及 [SourceArtifact / SourceFragment](../schema/m0-canonical-schema.md#sourceartifact)。部署与持久格式沿既有契约。

### 代码边界和候选接口

1. `EncryptedLibrary` 继续通过 `with_reader` 持有一条 SQLite exclusive session。schema、namespace、device、provider 和数据库身份通过后才加载既有 key。保留 reader 构造时的完整认证。每个新的检索动作独立打开 reader，锁与 key 在返回、错误或 unwind 时释放。该动作沿用现有一次 provider load，平台内部的 secret 双读和属性校验保持原合同。显式加密打开和刷新原有首尾 key 复验由[快照契约](phase1-source-vault-application.md#生命周期与交付边界)约束。
2. Application 使用共用逻辑建立 `LocalSearchRequest::for_sources`，保留采集入口、媒体类型、保存时间、敏感度和 `top_k`。同一个 `as_of` 采样用于整个请求。保留时钟、存储打开与请求校验的现有先后顺序。新增的固定入口位于 `LibraryReader`，接受已校验并限制为来源的请求，拒绝包含记忆检索的请求。Source Vault 仅返回可独立持有的最终结果，借用视图与回调留在内部。
3. 固定入口复用一次 `LibraryReader::read`。进入时由 `authenticate_all` 和 `EncryptedCaptureDatabase::read_view` 校验对象 inventory、canonical、绑定和派生事实。在同一个 `ObjectReadView` 内走既有 `LocalSearch` 实现先筛选再排序，以排序后的 fragment `source_id` 做精确回源。复用现有 `SourceSearchResult::from_resolved`，避免第二套排序、过滤或 lineage 选版。结果顺序与字节范围保持。将回源缺失作为内部结果保留到交付复验后，再返回 `SourceNotFound`，保持原有复验错误优先级。进入或交付复验错误继续保留底层 Source Vault 或 SQLite 分类。合法零命中同样经过交付复验。
4. 构造完整候选后，在原锁与 key 生命周期内再次认证全库对象和数据库身份及其 canonical / 派生事实。发生晚到损坏、数据库替换、未知对象或失配时丢弃整批结果。Desktop 沿既有错误处理撤下旧视图。明文搜索、其它读入口、导出、快照和写入沿各自现有路径。

### 审阅判断

当前五命中调用包含构造 reader 一轮、初次检索两轮及五次回源各两轮。按上述边界预计变为 **3 轮**，与命中数无关。零命中和权限拒绝后返回空结果仍为 3 轮。减少的是同一动作中间的对象和文件身份观察点。SQLite 会话能排斥遵循锁的写入者。交付复验发现对象损坏、数据库路径身份不符或 inventory 漂移时继续失败关闭。对于在两次检查之间发生且还原的外部变化，观察覆盖随检查点减少。现有实现同样无法证明这种变化一定被捕获。本次所有者确认按该边界继续实施。

`ObjectReadView` 本身只代表已经认证的正文集合和 SQLite 上的查询。把视图或通用回调暴露给 application 会允许在最终复验前观察中间结果，扩大 API 能力。推荐固定来源检索入口，保持业务 API、错误和授权语义。SQLite 锁解决的是遵循锁的并发写入，最终复验也不能撤回已返回给调用方的副本。

### 实施后验证

| 目标 | 复用的回归与新增判据 |
| --- | --- |
| 结果和筛选 | Q01 双入口、旧格式与升级、桌面组合筛选、Q04 多版本语料。核对来源次序、source / fragment ID、字节和范围。验证无答案、敏感度拒绝、过期、删除关闭、旧版本、pending capture 和错误 namespace |
| 交付失败关闭 | 复用 reader 现有 `read_with_check` 测试入口。在同批命中构造后、交付前注入合成对象持久损坏与 Unix 数据库路径替换，断言整批无结果。测试亦覆盖零命中的末次复验、拒绝超出来源范围的请求和密钥失败原分类 |
| 锁及生命周期 | 复用跨进程 writer probe，在最后一道复验前确认另一进程无法写入，动作退出后可重新读写。每个新搜索动作重新加载 key，故障动作结束后不会复用结果 |
| 性能 | 复跑两轮 [Q04 冻结预算](../evaluation/phase1-local-library-scale.md#macos-finite-q04-budget)，分别核对首次与组内 p95 均在 1 秒内，认证轮数与各阶段计数可解释，资源及其它操作仍满足既定预算 |

### 实现与定向验证

`LibraryReader::search_source_hits` 将排序后的片段及其精确来源作为固定结果交付。应用层的两种正文模式共用请求构造，加密路径在原独占会话中调用此入口，复验通过后才构造对外的 `SourceSearchResult`。在 Source Vault 内部复用现有末次复验，回源缺失在通过该复验后映射为 `SourceNotFound`。请求、key 和借用视图均在当前调用内结束。

macOS ARM64 的定向回归通过 47 项。多命中的次序、原文和来源版本与既有查询一致，跨进程 writer 在交付前仍被锁阻止。已构造结果后的密文损坏或 Unix 数据库路径替换会拒绝整批内容。零命中也会拒绝迟到密文损坏。旧版本、过期、权限排除、pending 和已删除来源不进入结果，外来 namespace 和记忆检索范围被拒绝。应用入口的多命中、零命中和权限排除分别记录 3 轮全库对象认证和 1 次 provider load，动作完成后可从另一进程获取写锁。

[Q04 双轮复测](../evaluation/phase1-local-library-scale.md#macos-q04-single-search-retest)确认本批离线预算达标。以上故障与成本均使用隔离合成资料，实际窗口、真实 Keychain 和其他平台继续按各自验收范围核对。

## 工作区与下一步

本批已在 `dev` 提交为 `8d2c651`，未 push，未运行远程 CI 或启动 GUI / VM / 长期服务。下一步将读取与既有 capture、删除、verify / rebuild 能力接入现有 application service 的显式库打开及操作生命周期；继续沿 ADR 0008 推进 macOS 宿主、真实 Keychain 和集中跨平台验收。
