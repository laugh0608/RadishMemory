# P1-S04 application 显式打开与读取生命周期

状态：`P1-S04 application read lifecycle implemented — synthetic acceptance`（2026-10-01）。

本批承接独立 `LibraryReader`，将显式密钥准备、正文迁移 / 恢复、已迁移库打开及读取用例接入 application。项目所有者已批准 application → Source Vault 的第一方 runtime 连线、现有 `rusqlite` 测试依赖和 opt-in 合成验收入口。不改变 canonical schema、数据库版本、密码套件、数据所有权或记忆确认语义。

## 应用入口与生命周期

- `LocalLibrary::close` 消费旧 v6 句柄，先关闭其 SQLite 连接，再交还原 runtime 与 profile。host 仍须暂停同库其它操作并关闭其它旧句柄；本方法不声称接管任意外部连接。
- `EncryptedLibraryLocation` 绑定显式专用目录和既有 namespace / device profile；数据库固定为该目录的 `library.sqlite3`。构造时验证 key slot 和目录 capability，仅准备 object / staging 子目录，不加载 key、创建数据库或执行迁移。
- `initialize_key` 显式复用 SQLite 初始化资格与 v7 checkpoint；已有 checkpoint 只加载原 key，缺 key 不重新生成。初始化成功后立即丢弃本次 key capability。不得以打开失败为理由自动调用初始化。
- `migrate_bodies` 单独复用 v7 / v8 正文迁移与精确恢复；只加载原 key，发布后中断可复用已登记对象。v9 至 v12 已迁移库直接打开，不重新执行旧迁移入口。
- `open` 只对已完成迁移的 v8 至 v12 执行认证读取验证，成功后释放该会话，再返回 `EncryptedLibrary`。缺库、未完成迁移、profile 错配、缺 key、坏对象及派生漂移均失败关闭；不回退旧 BLOB 或外部原件，不自动修复、删除 pending 或放弃 capture。
- `EncryptedLibrary` 当前是显式只读应用句柄，提供目录、历史版本、来源读取、搜索回源和精确导出。它与原 `LocalLibrary` 共用私有读取用例和已有 core / file-entry 契约，不建立第二套来源、查询或导出协议。
- 每次用例重新取得认证 `LibraryReader`，一个 exclusive session 覆盖完整搜索及来源解析或完整导出发布；正常返回、错误和 unwind 都释放锁与 key。句柄空闲时没有数据库连接、缓存 key 或解密正文，已有 capture / 删除 / maintenance 协调器可取得同库会话。返回值和用户导出是调用方持有的明文副本，关闭句柄不撤回这些副本。

宿主接入时的显式调用顺序为：消费并关闭旧 `LocalLibrary` → 用原 profile 构造 location → 用户明确选择首次准备或既有迁移恢复 → `open` → 单次读取用例。已迁移库只需构造 location 后 `open`。迁移失败保留真实 checkpoint，重试沿原 profile / 原 key；不重新生成身份。

`ApplicationError` 新增 Source Vault 分类和两个维护 operation，并保留有界 `VaultMaintenanceError` 原因，可区分 key failure、认证失败及 SQLite 原因。不会保留任意 SQL、路径、key material 或上游凭据诊断；不把维护失败统一标为可自动重试。

## 实现与依赖边界

`LibraryProvider` 是 Source Vault 内 sealed 的应用协调能力，production 实现仍为 `PlatformKeyProvider`。测试只显式启用 `acceptance-test-support`，使用公开固定测试 key 与共享合成状态；同一真实 bootstrap / migration / capture / deletion / reader coordinator 仍执行存储检查。合成 provider 不访问 Keychain、Credential Manager 或 Secret Service，不能用于真实资料。

Cargo 离线解析仅给 application 增加 `radishmemory-source-vault` 与测试 `rusqlite` 两条 lockfile 依赖边。453 个 package 的 name / version / source / checksum 集合逐项保持一致；没有新增或升级第三方。Source Vault 同为 workspace `=0.1.0`，由本项目维护并使用同一许可证；`rusqlite` 沿用既有冻结版本。application 的 dev-dependency 单独启用合成 feature，默认 production 不启用它。

desktop 经 application 新增对既有 crypto / platform provider 的传递构建可达性，但运行时只有显式 provider 调用才访问系统凭据。两个分发根的 notices 并集与各目标条目保持不变，只有 lockfile 文件摘要更新，详见[依赖基线](m0-rust-dependency-baseline.md)与[notices 复核](phase1-third-party-notices.md)。撤回本批连线和调用可恢复旧构建图；没有系统或远程状态需要回滚。

## 合成验收

新增 application 测试覆盖以下真实组合路径：

- v6 导入与版本更新 → 显式 close → key checkpoint → migration → application 目录 / 历史版本 / 搜索回源 / 导出 → 关闭重开；原字节含 BOM、CRLF、中文与组合 Unicode，移除外部原件后仍可读取和导出。
- 缺库不建库、不建 key；未完成迁移不普通打开；既有 checkpoint 丢 key 不 bootstrap；发布后迁移中断保留对象并显式恢复。
- 每次操作重新加载原 key；缺失、锁定、拒绝、取消和错 key 保留可区分有界原因；profile 错配在 key access 前拒绝。
- 旧连接写锁阻止准备和迁移，释放并关闭后成功；独立子进程验证完整导出前后拒绝写入，第二 application 进程在锁期间打不开、锁释放后可重开；file-entry 失败、无效请求、clock 失败和 unwind 后无残留锁。
- 操作间缺对象、坏密文或未知文件阻止 open / list / get / search / export；错误链脱敏、无导出文件，无外部原件 fallback。
- FTS 漂移不被 open / search 自动修复；显式既有 maintenance 重建后同一应用句柄可读。
- 既有 capture 的发布后中断保留原 pending 状态和对象，普通读取不暴露候选、不取消；原请求精确重试后下一次应用读取可见。
- 空闲应用与真实删除协调器交接；十组件成功后目录 / 历史 / 回源 / 搜索 / 导出均不可见，重建不复活，先前用户导出保留。

本机 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：240 个仓库文件、notices、fmt、all-targets / all-features locked Clippy、25 个 Rust test suites 共 321 项通过。其中本批新增 12 项 application 验收，原有 4 项 application 集成测试仍通过；10 个 ignored helper 由父测试实际作为子进程调用，不另计通过数。42 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 和 `git diff --check` 通过。

首次全仓运行中，既有 P1-F17 合成本地网络观察器在沙箱内绑定端口遭 `PermissionDenied`；按权限流程在沙箱外完整重跑后通过，未跳过或放宽检查。所有运行只使用隔离合成库和测试 key；测试清理自身临时资料、子进程和端口。任务专用验证日志保留在仓库外临时目录。

`cargo tree -p radishmemory-desktop --edges normal,build,features --locked --offline` 复核确认默认桌面图包含 Source Vault，但不含 `acceptance-test-support`。该图证据不代替 Windows / Linux 本批编译或运行；真实 key store、logger、GUI / host、多实例交互、断电、磁盘满与大库性能未验收。

## 限制与下一步

默认桌面产品入口仍是 SQLite v6 inline plaintext body，`LocalLibrary` 和 UI 尚未切换到此显式加密入口。加密 application 尚无 import / update、lineage deletion、verify / rebuild 用例；本批通过的是与现有独立协调器的交接，不代表上述写用例已经接入，也不代表 P1-S05 完成。

下一批按原请求 / 持久化 checkpoint 接入 capture、删除和维护，保留 pending 精确重试及显式放弃边界；再进入 macOS host、真实 Keychain / logger 与多实例验收，随后集中 Windows / Linux 编译和运行。真实系统操作另行授权。

FTS 仍含完整可读正文，原始对象加密不等于整库静态加密。每次读取仍全库认证，性能与 R01 至 R06 尚未收口；不扩大 PDF / 图片、模型、同步、备份恢复、生产可用或取证级删除声明。本批未访问真实密钥库、启动 GUI / VM / 长期服务或修改远程状态。
