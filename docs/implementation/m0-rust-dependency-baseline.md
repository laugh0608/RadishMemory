# RadishMemory Rust 依赖基线

日期：2026-08-28

范围：`M0-I02` canonical core 三个评审单元、`M0-I03 SQLite entry / source / memory / search / deletion storage`、`M0-I04 fixture runner`、`P1-I01 file snapshot contract`、`P1-I02 atomic source capture`、workspace 工具链与聚合检查入口。

## 当前解析结果

`Cargo.lock` 由 Cargo `1.96.0` 生成，lockfile format 为 `4`。当前依赖图包含四个第一方 workspace package，以及从 crates.io 解析并带 checksum 的 40 个第三方 package；没有 Git dependency。

| package | 直接依赖 | 来源 | 许可证 |
| --- | --- | --- | --- |
| `radishmemory-core 0.1.0` | `serde_json`、`sha2`、`time`、`unicode-normalization` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-file-entry 0.1.0` | `radishmemory-core =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-sqlite 0.1.0` | runtime：`radishmemory-core =0.1.0`、`rusqlite`；test-only：`radishmemory-file-entry =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-m0 0.1.0` | `radishmemory-core =0.1.0`、`radishmemory-sqlite =0.1.0`（`fixture-runner`）、`serde_json` | workspace path | 仓库 [LICENSE](../../LICENSE) |

## Core 直接依赖

四个第三方直接依赖都关闭 default features，再显式开启本单元需要的 feature：

| package | 解析版本 | 启用 feature | 用途 | 构建影响 |
| --- | --- | --- | --- | --- |
| `serde_json` | `1.0.151` | `arbitrary_precision`、`std` | 校验 JSON string / number 语法并保留 number 原始表示；不负责 canonical writer | 自带 Rust build script；无 native code |
| `sha2` | `0.11.0` | 无 | `SHA-256` | 纯 Rust；目标平台可通过 `cpufeatures` 选择实现 |
| `time` | `0.3.55` | `parsing`、`std` | RFC 3339 解析、UTC 比较和外部精度事实 | 引入 `time-macros` proc macro；不读取本地时区 |
| `unicode-normalization` | `0.1.25` | `std` | 仅用于 `utf8-nfc-text-v1` | 纯 Rust；Unicode 表由 crate 提供 |

这些版本均满足 workspace 的 Rust `1.96.0`，许可证均为 `MIT OR Apache-2.0`。manifest 使用兼容版本要求，首次受审阅解析结果由 `Cargo.lock` 精确固定；后续 lockfile 漂移必须重新审查并更新本页与检查器。

`radishmemory-m0` 直接复用同一 workspace `serde_json 1.0.151` 解析冻结 fixture 并构造最小证据 JSON；这只改变第一方 package 的直接依赖边，不新增第三方 package、feature、build script 或网络能力。fixture suite、ContextPack 和 DeletionEvidence 摘要仍调用 core 的 `radishmemory-canonical-json-v1` 实现，不依赖 serializer 的默认 key 顺序。

`radishmemory-file-entry` 只直接依赖第一方 `radishmemory-core`，复用 `exact-bytes-v1`、`SourceKind`、`MediaType`、稳定 Identifier / Version、`SourceCapture` 和敏感正文 Debug 边界。文件与路径操作全部使用 Rust 标准库；P1-I01 / P1-I02 没有新增 crates.io package、feature、build script、proc macro、native code 或网络能力，40 个第三方 package 的版本与 checksum 未变化。SQLite package 仅在 integration test 中引用第一方 file-entry，以合成临时文件贯通真实 snapshot → atomic store；production dependency 方向仍是 file-entry → core 与 sqlite → core，没有 file-entry / SQLite runtime 耦合。

## SQLite adapter 直接依赖与原生构建

`radishmemory-sqlite` 直接依赖 `rusqlite 0.40.2`，关闭 default features，只启用 `bundled`。这会同时启用 `modern_sqlite`、`libsqlite3-sys 0.38.2` 的 `bundled` 与预生成 `bundled_bindings`；`libsqlite3-sys` 自身的 default feature 还保留 `min_sqlite_version_3_34_1`、`pkg-config` 与 `vcpkg`，但构建由 bundled 分支选择内置源码。未启用 `cache`、`ffi-sqlite-wasm-rs`、`buildtime_bindgen`、SQLCipher 或 loadable-extension Rust API。

adapter 的第一方 `fixture-runner` feature 只由 `radishmemory-m0` 启用，用于建立场景隔离的内存数据库，并在冻结删除场景中显式注入一个稳定组件失败、持久化真实 failed attempt；默认 feature 为空，production `SqliteDatabase::open`、`DeletionStore` port、第三方 feature 图和运行时依赖不变。内存入口仍执行同一 capability probe、v1 → v6 migration、派生校验、`synchronous=FULL` 与真实 adapter 操作，但不把 Windows 文件系统逐事务同步成本混入 application-contract fixture；失败入口不能执行任意 SQL 或绕过删除计划，只能选择已冻结 component key、稳定 error code 与 retryable 状态。

| package / 源码 | 解析版本 | 来源与许可证 | 实际用途与构建影响 |
| --- | --- | --- | --- |
| `rusqlite` | `0.40.2` | crates.io，MIT | 参数化 SQL、事务、PRAGMA 和连接 API；本身无 build script |
| `libsqlite3-sys` | `0.38.2` | crates.io，MIT | `build.rs` 选择 bundled 分支、复制预生成 binding，并调用 `cc` 编译 SQLite amalgamation |
| SQLite | `3.53.2` | 随 `libsqlite3-sys` crate 固定的 upstream amalgamation；SQLite 为 [public domain](https://www.sqlite.org/copyright.html) | 编译并静态链接 C 源码；build script 明确传入 `SQLITE_ENABLE_FTS5`、foreign-key default、thread-safe 等开关 |

adapter 启动时同时核对运行时版本、`sqlite_compileoption_used('ENABLE_FTS5')` 与实际临时 FTS5 虚表创建；任一不符均失败关闭，不回退内存扫描。运行时版本实探只能证明所链接库报告 `3.53.2`，bundled 来源本身由 manifest feature、lockfile、crate checksum 与构建日志共同约束，不能把版本字符串单独当作供应链来源证明。

## 传递依赖与供应链面

40 个第三方 package 的精确解析清单为：

- 直接：`serde_json 1.0.151`、`sha2 0.11.0`、`time 0.3.55`、`unicode-normalization 0.1.25`；
- SHA-256：`block-buffer 0.12.1`、`cfg-if 1.0.4`、`cpufeatures 0.3.0`、`crypto-common 0.2.2`、`digest 0.11.3`、`hybrid-array 0.4.14`、`libc 0.2.189`、`typenum 1.20.1`；
- JSON / Serde：`itoa 1.0.18`、`memchr 2.8.3`、`proc-macro2 1.0.107`、`quote 1.0.47`、`serde 1.0.229`、`serde_core 1.0.229`、`serde_derive 1.0.229`、`syn 3.0.3`、`unicode-ident 1.0.24`、`zmij 1.0.23`；
- 时间：`deranged 0.5.8`、`num-conv 0.2.2`、`powerfmt 0.2.0`、`time-core 0.1.9`、`time-macros 0.2.32`；
- Unicode：`tinyvec 1.12.0`、`tinyvec_macros 0.1.1`；
- SQLite：`rusqlite 0.40.2`、`libsqlite3-sys 0.38.2`、`bitflags 2.13.1`、`fallible-iterator 0.3.0`、`fallible-streaming-iterator 0.1.9`、`smallvec 1.15.2`、`cc 1.4.4`、`find-msvc-tools 0.1.11`、`shlex 2.0.1`、`pkg-config 0.3.34`、`vcpkg 0.2.15`。

许可证例外为 `memchr` 的 `Unlicense OR MIT`、`tinyvec` / `tinyvec_macros` 的 Zlib / Apache-2.0 / MIT 组合、`unicode-ident` 的 Unicode-3.0 数据条款、`zmij` 与 `rusqlite` / `libsqlite3-sys` 的 MIT，以及 SQLite amalgamation 的 public-domain dedication；其余新增 SQLite 传递 package 为 MIT / Apache-2.0 组合。M0 当前没有发布产物；首次分发源码或二进制前仍须生成并人工复核 third-party notices，尤其不能遗漏 Unicode-3.0 数据归属，也不能把 SQLite 的 public-domain 状态误写成项目自身许可证。

`serde_derive` 与 `time-macros` 是实际解析的 proc macro。`libc`、`proc-macro2`、`quote`、`serde`、`serde_core`、`serde_json`、`zmij` 与 `libsqlite3-sys` 包含 Rust build script；其中只有 `libsqlite3-sys` 在当前 feature 图中通过 `cc` 编译并链接第三方 SQLite C 源码。`pkg-config` 与 `vcpkg` 随 `libsqlite3-sys` 锁定，但 bundled 分支不依赖宿主 SQLite 作为运行库。构建阶段会从 crates.io 获取已锁定源码并需要可用的目标平台 C 工具链；产品运行时仍没有 HTTP client、隐式联网、遥测或数据外发能力。

选择这些依赖而非本地平行实现，是因为 ADR 0005 已冻结 JSON 表示、SHA-256、Unicode NFC、RFC 3339 与 bundled SQLite / FTS5 基线；项目仍自行实现 `radishmemory-canonical-json-v1` writer，SQLite schema、migration 和查询也仍由本项目审阅。主要剩余供应链风险是 build script / proc macro 在编译时执行、SQLite C 编译器链与未来兼容版本更新；当前通过 crates.io checksum、精确 lockfile、直接依赖白名单、运行时 capability probe 和三平台 locked checks 约束。

## 工具链与验证证据

- workspace 使用 Rust 2024 edition，`rust-toolchain.toml` 精确固定 `1.96.0`，并要求 `rustfmt` 与 `clippy` component；
- 第一方 package 继承 `rust-version = "1.96.0"`、仓库许可证，以及 workspace `unsafe_code = "forbid"` 与 `unused_crate_dependencies = "deny"` lint；
- 本地 macOS 已使用 Rust / Cargo `1.96.0` 运行 workspace 格式、Clippy 与全部 target 测试；bundled SQLite `3.53.2`、FTS5 capability、新库与 v1 → v6 迁移、Source Vault、MemoryStore、atomic source capture、exact no-overwrite export、检索、删除、12 场景 / 86 操作 / 12 gate runner、确定性证据、未知操作失败关闭和错误脱敏均通过。本单元最终仍以正式仓库聚合入口结果为准；
- PR workflow 在 [PR #1](https://github.com/laugh0608/RadishMemory/pull/1) 对相同 locked 检查进行了真实执行：首轮 run `32976944213` 的 Linux / macOS 通过，Windows 因文件数据库逐事务同步放大重复 fixture suite 而在 `10m14s` 超时；提交 `918d045` 保留 production 文件入口与连接策略，仅把 runner-only 场景切换为独立内存连接，随后 run `32978669766` 的 Linux、macOS、Windows 与 `Candidate Quality` 已通过。最终文档 head `6df0891` 又在 run `32979128488` 全部通过，并由 merge commit `fe8186a` 合入 `master`、fast-forward 回流 `dev`。该历史证明已合并 M0 基线的三平台 CI，不外推为未来版本或生产环境保证。

本基线证明 canonical core primitive、九种顶层对象、字段级校验、跨对象不变量、SQLite 连接 / migration、Source Vault、MemoryStore、FTS5 派生索引、当前投影、本地删除执行与真实 fixture runner 的已合并依赖图，并记录 P1-I01 / P1-I02 / P1-I03 file snapshot、atomic capture 与 exact export 没有扩大第三方供应链。P1 本机验证不能替代后续 Linux / macOS / Windows CI，也不证明完整产品文件入口、lineage deletion、PDF / 图片采集、向量检索、模型问答、多设备同步、未来平台兼容或生产可用性。
