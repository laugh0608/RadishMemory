# RadishMemory Rust 依赖基线

日期：2026-09-03

范围：`M0-I02` canonical core 三个评审单元、`M0-I03 SQLite entry / source / memory / search / deletion storage`、`M0-I04 fixture runner`、`P1-I01` 至 `P1-I04` 文件入口、`P1-H02 application service`、`P1-H03 source catalog`、`P1-H04 desktop UI`、`P1-S03a portable crypto dependency landing`、阶段 1 本机合成验收、workspace 工具链与聚合检查入口。

## 当前解析结果

`Cargo.lock` 由 Cargo `1.96.0` 生成，lockfile format 为 `4`。当前依赖图包含七个第一方 workspace package，以及从 crates.io 解析并带 checksum 的 423 个第三方 package；没有 Git dependency。数量包含 Linux、macOS、Windows、Android、WASM 和可选 renderer 的条件解析全集，不等于单个产物会编译或链接全部 package。

| package | 直接依赖 | 来源 | 许可证 |
| --- | --- | --- | --- |
| `radishmemory-core 0.1.0` | `serde_json`、`sha2`、`time`、`unicode-normalization` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-file-entry 0.1.0` | `radishmemory-core =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-sqlite 0.1.0` | runtime：`radishmemory-core =0.1.0`、`rusqlite`；test-only：`radishmemory-file-entry =0.1.0`（`acceptance-test-support`） | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-application 0.1.0` | `radishmemory-core =0.1.0`、`radishmemory-file-entry =0.1.0`、`radishmemory-sqlite =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-desktop 0.1.0` | `radishmemory-application =0.1.0`、`eframe`、`rfd`、`directories`、`getrandom`、`time` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-source-vault 0.1.0` | `aead-stream`、`chacha20poly1305`、`getrandom`、`sha2`、`zeroize` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-m0 0.1.0` | `radishmemory-core =0.1.0`、`radishmemory-sqlite =0.1.0`（`fixture-runner`）、`serde_json` | workspace path | 仓库 [LICENSE](../../LICENSE) |

## Core 直接依赖

四个第三方直接依赖都关闭 default features，再显式开启本单元需要的 feature：

| package | 解析版本 | 启用 feature | 用途 | 构建影响 |
| --- | --- | --- | --- | --- |
| `serde_json` | `1.0.151` | `arbitrary_precision`、`std` | 校验 JSON string / number 语法并保留 number 原始表示；不负责 canonical writer | 自带 Rust build script；无 native code |
| `sha2` | `0.11.0` | 无 | `SHA-256` | 纯 Rust；目标平台可通过 `cpufeatures` 选择实现 |
| `time` | `0.3.55` | `formatting`、`parsing`、`std` | RFC 3339 解析、UTC 比较和 production UTC clock 格式化 | 引入 `time-macros` proc macro；不读取本地时区 |
| `unicode-normalization` | `0.1.25` | `std` | 仅用于 `utf8-nfc-text-v1` | 纯 Rust；Unicode 表由 crate 提供 |

这些版本均满足 workspace 的 Rust `1.96.0`，许可证均为 `MIT OR Apache-2.0`。manifest 使用兼容版本要求，首次受审阅解析结果由 `Cargo.lock` 精确固定；后续 lockfile 漂移必须重新审查并更新本页、[third-party notices](../../THIRD_PARTY_NOTICES.md)与检查器。

`radishmemory-m0` 直接复用同一 workspace `serde_json 1.0.151` 解析冻结 fixture 并构造最小证据 JSON；这只改变第一方 package 的直接依赖边，不新增第三方 package、feature、build script 或网络能力。fixture suite、ContextPack 和 DeletionEvidence 摘要仍调用 core 的 `radishmemory-canonical-json-v1` 实现，不依赖 serializer 的默认 key 顺序。

`radishmemory-file-entry` 只直接依赖第一方 `radishmemory-core`，复用 `exact-bytes-v1`、`SourceKind`、`MediaType`、稳定 Identifier / Version、`SourceCapture` 和敏感正文 Debug 边界。文件与路径操作全部使用 Rust 标准库；P1-I01 file snapshot、P1-I02 atomic source capture、P1-I03 exact export、P1-I04 lineage deletion 及 `P1-F01` 至 `P1-F18` 当时没有扩大 40 个第三方 package 的 headless 基础子图。SQLite package 仅在 integration test 中引用第一方 file-entry，以合成临时文件贯通真实 snapshot、atomic store、exact export、lineage deletion、拒绝、TOCTOU、无副作用与诊断边界；production dependency 方向仍是 file-entry → core 与 sqlite → core，没有 file-entry / SQLite runtime 耦合。

`radishmemory-application` 只组合上述三个第一方 library package。它新增 file-backed `LocalLibrary`、`ApplicationRuntime`、脱敏 application error 和 body-free `SourceCatalog` 读取模型，复用既有 capture、search、export、deletion、verify / rebuild 语义；自身没有新增 crates.io package、feature、build script、proc macro、native code、平台权限或网络能力。UI 工具包、系统文件选择、应用数据目录与 production runtime 只位于下述 desktop package。

## Source Vault portable crypto 直接依赖

`P1-S03a` 新增独立第一方 `radishmemory-source-vault` package，只落地 [P1-S02 依赖与密码套件评审](phase1-encrypted-source-vault-dependency-review.md)冻结的 portable cipher / wrap、AAD、系统随机和 secret-memory profile。其精确直接依赖为 `aead-stream =0.6.0`、`chacha20poly1305 =0.11.0`、既有 `getrandom =0.4.3`、既有 `sha2 0.11.0` 与 `zeroize =1.9.0`；三个 platform key-store provider 仍未进入 manifest 或 lockfile。

本次 lockfile 精确新增 `aead 0.6.1`、`aead-stream 0.6.0`、`chacha20 0.10.2`、`chacha20poly1305 0.11.0`、`cipher 0.5.2`、`cmov 0.5.4`、`ctutils 0.4.2`、`inout 0.2.2`、`poly1305 0.9.1`、`universal-hash 0.6.1` 与 `zeroize 1.9.0` 共 11 个 crates.io package。它们没有 build script、proc macro、native `links`、OpenSSL、FFI、网络 client 或 async runtime；三项目标的 portable crypto / digest 子图一致，只有既有 `getrandom` / `sha2` 解析目标条件。公开向量、项目固定向量、负向测试、许可证、checksum、notices 和 RustSec 复核详见 [P1-S03a 落地记录](phase1-source-vault-portable-crypto.md)。

## Desktop 直接依赖、平台面与当前目标

`radishmemory-desktop` 的精确直接依赖与选择理由见 [Phase 1 桌面宿主依赖评审](phase1-desktop-dependency-review.md)：`eframe =0.36.1` 关闭 default features 并只启用 `accesskit`、`default_fonts`、`wayland`、`wgpu`、`x11`；`rfd =0.17.2` 只启用 `xdg-portal`、`wayland`；`directories =6.0.0`、`getrandom =0.4.3` 与 `time =0.3.55` 提供应用目录、系统随机与 UTC RFC 3339。Windows ARM64 真实运行证明 `glow` 无法在该虚拟显示宿主取得 OpenGL 2.0，当前以唯一 `wgpu` renderer 修复，不保留静默 fallback。

当前 `aarch64-apple-darwin` desktop 根可达 180 个唯一 package ID，其中 5 个第一方；真实编译使用 AppKit、AccessKit、`wgpu` / Metal binding、剪贴板、系统随机与 bundled SQLite。P1-H05 当时的 lockfile 为 418 个 package；P1-S03a 加入独立 portable crypto 根后，当前全集为 430 个 package。全集仍保存其它 target 和可选依赖，因此出现 `glow` / Glutin、Linux XDG Portal / D-Bus、Wayland / X11、Windows、Android 与 WASM package，不代表当前 macOS artifact 启用了这些路径。整份 lockfile 与当前目标树都没有常见 HTTP / TLS client 或 `tokio`；Linux portal 的本地 D-Bus / async 条件面、窗口系统、GPU backend、剪贴板和 accessibility 仍是必须承认的平台能力。

desktop 新增四个第三方直接依赖的声明许可证是：`eframe`、`directories`、`getrandom` 为 `MIT OR Apache-2.0`，`rfd` 为 `MIT`；`time` 保持既有 `MIT OR Apache-2.0`。完整 locked metadata 没有缺失 license 字段，跨目标全集有 75 个 build-script package ID、27 个 proc-macro package ID 和 3 个 native `links` 声明；P1-S03a 的 11 个新增 package 不增加这三类执行 / 原生面。`radishmemory-desktop` 与 `radishmemory-source-vault` 两个分发根在三目标可达的 344 个 crate、`epaint_default_fonts` 的 OFL / Ubuntu Font License、`option-ext` 的 MPL-2.0、`unicode-ident` 的 Unicode-3.0 与每个 OR expression 的实际 distribution basis 已由 [third-party notices 与条件平台依赖复核](phase1-third-party-notices.md)逐项收口。不得把本页摘要替代完整 notices。

file-entry 的第一方 `acceptance-test-support` feature 默认关闭，只由 SQLite dev-dependency 启用。它把替换、截短和扩展三种冻结操作映射到 production `read_file_snapshot` 复用的 private 初始观察 seam；默认 build 不导出测试类型或函数，不允许任意 callback、网络、数据库或模型操作。SQLite capture commit 故障 seam 保持 adapter private 且只在 crate unit test 中调用，不是 Cargo feature 或 public port。两者均不改变 `Cargo.lock`、production feature 图、第三方编译面或运行时数据流。

## SQLite adapter 直接依赖与原生构建

`radishmemory-sqlite` 直接依赖 `rusqlite 0.40.2`，关闭 default features，只启用 `bundled`。这会同时启用 `modern_sqlite`、`libsqlite3-sys 0.38.2` 的 `bundled` 与预生成 `bundled_bindings`；`libsqlite3-sys` 自身的 default feature 还保留 `min_sqlite_version_3_34_1`、`pkg-config` 与 `vcpkg`，但构建由 bundled 分支选择内置源码。未启用 `cache`、`ffi-sqlite-wasm-rs`、`buildtime_bindgen`、SQLCipher 或 loadable-extension Rust API。

adapter 的第一方 `fixture-runner` feature 只由 `radishmemory-m0` 启用，用于建立场景隔离的内存数据库，并在冻结删除场景中显式注入一个稳定组件失败、持久化真实 failed attempt；默认 feature 为空，production `SqliteDatabase::open`、`DeletionStore` port、第三方 feature 图和运行时依赖不变。内存入口仍执行同一 capability probe、v1 → v6 migration、派生校验、`synchronous=FULL` 与真实 adapter 操作，但不把 Windows 文件系统逐事务同步成本混入 application-contract fixture；失败入口不能执行任意 SQL 或绕过删除计划，只能选择已冻结 component key、稳定 error code 与 retryable 状态。

| package / 源码 | 解析版本 | 来源与许可证 | 实际用途与构建影响 |
| --- | --- | --- | --- |
| `rusqlite` | `0.40.2` | crates.io，MIT | 参数化 SQL、事务、PRAGMA 和连接 API；本身无 build script |
| `libsqlite3-sys` | `0.38.2` | crates.io，MIT | `build.rs` 选择 bundled 分支、复制预生成 binding，并调用 `cc` 编译 SQLite amalgamation |
| SQLite | `3.53.2` | 随 `libsqlite3-sys` crate 固定的 upstream amalgamation；SQLite 为 [public domain](https://www.sqlite.org/copyright.html) | 编译并静态链接 C 源码；build script 明确传入 `SQLITE_ENABLE_FTS5`、foreign-key default、thread-safe 等开关 |

adapter 启动时同时核对运行时版本、`sqlite_compileoption_used('ENABLE_FTS5')` 与实际临时 FTS5 虚表创建；任一不符均失败关闭，不回退内存扫描。运行时版本实探只能证明所链接库报告 `3.53.2`，bundled 来源本身由 manifest feature、lockfile、crate checksum 与构建日志共同约束，不能把版本字符串单独当作供应链来源证明。

## Headless 基础子图与供应链面

不含 desktop host 且不含后来独立落地的 Source Vault crypto package 时，M0 / file-entry / application 的 40 个第三方 headless 基础 package 精确解析清单为：

- 直接：`serde_json 1.0.151`、`sha2 0.11.0`、`time 0.3.55`、`unicode-normalization 0.1.25`；
- SHA-256：`block-buffer 0.12.1`、`cfg-if 1.0.4`、`cpufeatures 0.3.0`、`crypto-common 0.2.2`、`digest 0.11.3`、`hybrid-array 0.4.14`、`libc 0.2.189`、`typenum 1.20.1`；
- JSON / Serde：`itoa 1.0.18`、`memchr 2.8.3`、`proc-macro2 1.0.107`、`quote 1.0.47`、`serde 1.0.229`、`serde_core 1.0.229`、`serde_derive 1.0.229`、`syn 3.0.3`、`unicode-ident 1.0.24`、`zmij 1.0.23`；
- 时间：`deranged 0.5.8`、`num-conv 0.2.2`、`powerfmt 0.2.0`、`time-core 0.1.9`、`time-macros 0.2.32`；
- Unicode：`tinyvec 1.12.0`、`tinyvec_macros 0.1.1`；
- SQLite：`rusqlite 0.40.2`、`libsqlite3-sys 0.38.2`、`bitflags 2.13.1`、`fallible-iterator 0.3.0`、`fallible-streaming-iterator 0.1.9`、`smallvec 1.15.2`、`cc 1.4.4`、`find-msvc-tools 0.1.11`、`shlex 2.0.1`、`pkg-config 0.3.34`、`vcpkg 0.2.15`。

许可证例外为 `memchr` 的 `Unlicense OR MIT`、`tinyvec` / `tinyvec_macros` 的 Zlib / Apache-2.0 / MIT 组合、`unicode-ident` 的 Unicode-3.0 数据条款、`zmij` 与 `rusqlite` / `libsqlite3-sys` 的 MIT，以及 SQLite amalgamation 的 public-domain dedication；其余新增 SQLite 传递 package 为 MIT / Apache-2.0 组合。目标依赖 notices 已生成并人工复核，明确保留 Unicode-3.0 数据归属和 SQLite public-domain dedication；任何发行包仍须实际携带这些文件，不能把 SQLite 的状态误写成项目自身许可证。

`serde_derive` 与 `time-macros` 是 headless 基础子图实际解析的 proc macro。`libc`、`proc-macro2`、`quote`、`serde`、`serde_core`、`serde_json`、`zmij` 与 `libsqlite3-sys` 包含 Rust build script；其中 `libsqlite3-sys` 在当前 feature 图中通过 `cc` 编译并链接第三方 SQLite C 源码。desktop 图另包含 UI / windowing 的 proc macro、platform binding 与 native build 元数据，必须按目标平台复验，不能沿用这里的 40-package 结论。`pkg-config` 与 `vcpkg` 随 `libsqlite3-sys` 锁定，但 bundled 分支不依赖宿主 SQLite 作为运行库。

选择这些依赖而非本地平行实现，是因为 ADR 0005 已冻结 JSON 表示、SHA-256、Unicode NFC、RFC 3339 与 bundled SQLite / FTS5 基线；项目仍自行实现 `radishmemory-canonical-json-v1` writer，SQLite schema、migration 和查询也仍由本项目审阅。主要剩余供应链风险是 build script / proc macro 在编译时执行、SQLite C 编译器链与未来兼容版本更新；当前通过 crates.io checksum、精确 lockfile、直接依赖白名单、运行时 capability probe 和三平台 locked checks 约束。

## 工具链与验证证据

- workspace 使用 Rust 2024 edition，`rust-toolchain.toml` 精确固定 `1.96.0`，并要求 `rustfmt` 与 `clippy` component；
- 第一方 package 继承 `rust-version = "1.96.0"`、仓库许可证，以及 workspace `unsafe_code = "forbid"` 与 `unused_crate_dependencies = "deny"` lint；
- 本地 macOS 已使用 Rust / Cargo `1.96.0` 运行 workspace 格式、Clippy 与全部 target 测试；bundled SQLite `3.53.2`、FTS5 capability、新库与 v1 → v6 迁移、Source Vault、MemoryStore、atomic source capture、exact no-overwrite export、source lineage deletion、body-free catalog、application import / update / search / export / delete、文件数据库重启、BOM / CRLF / Unicode exact-byte reload、hardlink provenance 独立删除、路径 / symlink / 内容拒绝原子性、8 MiB capture 边界、确定性 TOCTOU、capture commit rollback、export write / publish failure、不可信 Markdown 零网络 / 零 memory side effect、诊断脱敏、检索、删除、12 场景 / 86 操作 / 12 gate runner、确定性证据和未知操作失败关闭均通过。本单元最终仍以正式仓库聚合入口结果为准；
- PR workflow 在 [PR #1](https://github.com/laugh0608/RadishMemory/pull/1) 对 M0 locked 检查进行了真实执行：首轮 run `32976944213` 的 Linux / macOS 通过，Windows 因文件数据库逐事务同步放大重复 fixture suite 而在 `10m14s` 超时；提交 `918d045` 保留 production 文件入口与连接策略，仅把 runner-only 场景切换为独立内存连接，随后 run `32978669766` 的 Linux、macOS、Windows 与 `Candidate Quality` 已通过。最终文档 head `6df0891` 又在 run `32979128488` 全部通过，并由 merge commit `fe8186a` 合入 `master`、fast-forward 回流 `dev`。
- Phase 1 [PR #2](https://github.com/laugh0608/RadishMemory/pull/2) 的最终 head `9bd0af5` 在 run `33302423840` 真实运行 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality` 并全部通过，随后由 merge commit `c56f13f` 合入 `master`、fast-forward 回流 `dev`。该结果覆盖 file snapshot、atomic capture、exact export、lineage deletion、TOCTOU、故障回滚、不可信 Markdown 无副作用和诊断脱敏的当前 locked feature graph，不外推为 production host / UI、真实个人资料或未来平台兼容保证。

本基线证明 canonical core、SQLite / FTS5、file-entry 与 application service 的已合并依赖图，也记录了 desktop UI、一次性平台选择、应用目录、host profile、production runtime 与 P1-S03a portable crypto package 的当前依赖和构建证据。`P1-F01` 至 `P1-F18` 已通过 Linux / macOS / Windows locked CI；desktop head `57f4f44` 的旧 `glow` 图也通过 run `33394918896`，但 Windows ARM64 实际启动随后暴露 OpenGL-only 阻断。当前 `wgpu` 图已在 Windows ARM64 与 Debian ARM64 / GNOME Wayland 完成 build 和真实 GUI / picker 复验，并由 head `c5dba35` 的 run `33751048480` 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`；两个分发根的三目标可达依赖 notices 已再生成并人工复核。P1-S03a 只有本地 macOS locked test / Clippy 和静态三目标 graph 证据，尚未触发包含该 package 的远程三平台 CI，也不证明 object filesystem、平台 key store、SQLite migration、签名发行包、真实个人资料授权、PDF / 图片、向量、模型、同步、未来平台兼容或 production deployment。
