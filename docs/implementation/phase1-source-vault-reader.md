# P1-S04 加密资料库读取切片

状态：`P1-S04 object-backed read slice implemented — synthetic acceptance`（2026-09-26）。

本批承接 `3dd312b` 的历史删除请求兼容，提供独立读取会话，复用既有来源、目录和检索规则。不改变 canonical schema、记忆状态或权限语义，不新增数据库迁移。普通 `SqliteDatabase::open` 仍只允许 v6；`LocalLibrary` / UI 尚未切换加密数据流。

## 读取边界

- `PlatformKeyProvider::open_library_reader` 只接受已完成迁移的维护 v8 至 v12。先验证数据库 schema、namespace / device / provider，再加载既有 key；缺库、未完成迁移或身份不匹配不加载 key，不创建空库、新 key 或迁移 checkpoint。
- `LibraryReader` 持有数据库 exclusive session、目录与数据库身份观察及 key。每次查询先认证全部登记对象、校验 canonical / 派生事实，再解析结果；返回前重复认证和数据库身份检查。损坏对象、缺对象、未知文件、派生漂移或晚到损坏均失败关闭，不回退旧 BLOB 或外部原件，不在读取中重建索引。
- SQLite `ObjectReadView` 是调用方已认证正文集合上的临时查询视图，不是独立的认证 capability。SQLite 重新核对正文长度、摘要、canonical 字段、fragment / memory provenance、FTS 和 lineage tip；Source Vault 负责真实对象认证。视图不暴露写入接口，也不为读取实现占位写方法。
- 来源和 fragment 按 namespace 与精确 source ID 读取；不存在或已关闭删除召回的来源返回 `None`，错误 namespace 返回失败。pending / abandoned capture 不进入来源目录或搜索；即使目标不可见，库中的未知或损坏对象仍导致失败。
- `SourceCatalog` 复用相同 SQL、版本连续性检查、captured-time / lineage 稳定排序及 offset / limit 切页逻辑。`LocalSearch` 复用相同 FTS 查询和候选校验，namespace、敏感度、可用时间、retention 和记忆 valid-time 在 top-k 前过滤；未确认 proposal 不成为搜索记忆。
- 会话关闭释放锁和 key；会话期间其他数据库写入失败。读取结果是调用方持有的明文 canonical 值，会话不跨查询缓存解密正文；不承诺撤回先前返回的副本或进程明文取证清除。

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

## 工作区与下一步

本批为 `dev` 上的新工作区差异，未提交、未 push，未运行远程 CI 或启动 GUI / VM / 长期服务。下一步将读取与既有 capture、删除、verify / rebuild 能力接入现有 application service 的显式库打开及操作生命周期；继续沿 ADR 0008 推进 macOS 宿主、真实 Keychain 和集中跨平台验收。
