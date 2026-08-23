# M0 Rust 依赖基线

日期：2026-08-23

范围：`M0-I02` canonical core 三个评审单元、workspace 工具链与聚合检查入口。

## 当前解析结果

`Cargo.lock` 由 Cargo `1.96.0` 生成，lockfile format 为 `4`。当前依赖图包含三个第一方 workspace package，以及从 crates.io 解析并带 checksum 的 29 个第三方 package；没有 Git dependency。

| package | 直接依赖 | 来源 | 许可证 |
| --- | --- | --- | --- |
| `radishmemory-core 0.1.0` | `serde_json`、`sha2`、`time`、`unicode-normalization` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-sqlite 0.1.0` | `radishmemory-core =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-m0 0.1.0` | `radishmemory-core =0.1.0`、`radishmemory-sqlite =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |

## Core 直接依赖

四个第三方直接依赖都关闭 default features，再显式开启本单元需要的 feature：

| package | 解析版本 | 启用 feature | 用途 | 构建影响 |
| --- | --- | --- | --- | --- |
| `serde_json` | `1.0.151` | `arbitrary_precision`、`std` | 校验 JSON string / number 语法并保留 number 原始表示；不负责 canonical writer | 自带 Rust build script；无 native code |
| `sha2` | `0.11.0` | 无 | `SHA-256` | 纯 Rust；目标平台可通过 `cpufeatures` 选择实现 |
| `time` | `0.3.55` | `parsing`、`std` | RFC 3339 解析、UTC 比较和外部精度事实 | 引入 `time-macros` proc macro；不读取本地时区 |
| `unicode-normalization` | `0.1.25` | `std` | 仅用于 `utf8-nfc-text-v1` | 纯 Rust；Unicode 表由 crate 提供 |

这些版本均满足 workspace 的 Rust `1.96.0`，许可证均为 `MIT OR Apache-2.0`。manifest 使用兼容版本要求，首次受审阅解析结果由 `Cargo.lock` 精确固定；后续 lockfile 漂移必须重新审查并更新本页与检查器。

## 传递依赖与供应链面

29 个第三方 package 的精确解析清单为：

- 直接：`serde_json 1.0.151`、`sha2 0.11.0`、`time 0.3.55`、`unicode-normalization 0.1.25`；
- SHA-256：`block-buffer 0.12.1`、`cfg-if 1.0.4`、`cpufeatures 0.3.0`、`crypto-common 0.2.2`、`digest 0.11.3`、`hybrid-array 0.4.14`、`libc 0.2.189`、`typenum 1.20.1`；
- JSON / Serde：`itoa 1.0.18`、`memchr 2.8.3`、`proc-macro2 1.0.107`、`quote 1.0.47`、`serde 1.0.229`、`serde_core 1.0.229`、`serde_derive 1.0.229`、`syn 3.0.3`、`unicode-ident 1.0.24`、`zmij 1.0.23`；
- 时间：`deranged 0.5.8`、`num-conv 0.2.2`、`powerfmt 0.2.0`、`time-core 0.1.9`、`time-macros 0.2.32`；
- Unicode：`tinyvec 1.12.0`、`tinyvec_macros 0.1.1`。

许可证例外为 `memchr` 的 `Unlicense OR MIT`、`tinyvec` / `tinyvec_macros` 的 Zlib / Apache-2.0 / MIT 组合、`unicode-ident` 的 Unicode-3.0 数据条款，以及 `zmij` 的 MIT；其余传递 package 为 `MIT OR Apache-2.0`。M0 当前没有发布产物；首次分发源码或二进制前仍须生成并人工复核 third-party notices，尤其不能遗漏 Unicode-3.0 数据归属。

`serde_derive` 与 `time-macros` 是实际解析的 proc macro。`libc`、`proc-macro2`、`quote`、`serde`、`serde_core`、`serde_json` 与 `zmij` 包含 Rust build script；它们不编译或链接第三方 C / C++ 源码。`libc` 在此依赖图中为平台绑定与 cfg 支持，不代表新增产品 FFI。构建阶段会从 crates.io 获取已锁定源码，产品运行时没有 HTTP client、隐式联网、遥测或数据外发能力。

选择这些依赖而非本地平行实现，是因为 ADR 0005 已冻结 JSON 表示、SHA-256、Unicode NFC 与 RFC 3339 基线；项目仍自行实现 `radishmemory-canonical-json-v1` writer，不依赖 map 默认顺序、浮点格式或 `serde_json` 默认 serializer。主要剩余供应链风险是 build script / proc macro 在编译时执行，以及未来兼容版本更新；当前通过 crates.io checksum、精确 lockfile、直接依赖白名单和三平台 locked checks 约束。

`rusqlite`、bundled SQLite 和 FTS5 尚未进入本切片，因此 SQLite 版本、启用 feature、原生构建和第三方许可证当前均为不适用；它们必须在实现 SQLite adapter 的后续切片中随实际依赖、lockfile 和三平台证据一起补充，不能提前加入白名单依赖占位。

## 工具链与验证证据

- workspace 使用 Rust 2024 edition，`rust-toolchain.toml` 精确固定 `1.96.0`，并要求 `rustfmt` 与 `clippy` component；
- 第一方 package 继承 `rust-version = "1.96.0"`、仓库许可证，以及 workspace `unsafe_code = "forbid"` 与 `unused_crate_dependencies = "deny"` lint；
- 本地 macOS 已使用 Rust / Cargo `1.96.0` 运行全 workspace 的格式、Clippy 和全部 target 测试；core 的冻结 digest vectors、完整 suite digest、九种对象正例、跨对象闭环与补充的时间、NFC、JSON、字段条件和隐私错误负例均通过；
- PR workflow 已配置 Linux、macOS、Windows 三平台真实运行相同 locked 检查，并由 `Candidate Quality` 聚合；仓库内配置不等于 GitHub 执行结果，在实际 workflow run 产生前不得宣称三平台已经通过。

本基线证明 canonical core primitive、九种顶层对象、字段级校验、跨对象不变量与当前依赖图在本机成立，不证明 SQLite、FTS5、fixture runner 或任何完整产品闭环已经实现。
