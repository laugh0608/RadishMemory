# M0 Rust 依赖基线

日期：2026-08-22

范围：`M0-I01` workspace、工具链与聚合检查入口。

## 当前解析结果

`Cargo.lock` 由 Cargo `1.96.0` 生成，lockfile format 为 `4`。当前依赖图只包含三个第一方 workspace package：

| package | 直接依赖 | 来源 | 许可证 |
| --- | --- | --- | --- |
| `radishmemory-core 0.1.0` | 无 | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-sqlite 0.1.0` | `radishmemory-core =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |
| `radishmemory-m0 0.1.0` | `radishmemory-core =0.1.0`、`radishmemory-sqlite =0.1.0` | workspace path | 仓库 [LICENSE](../../LICENSE) |

当前没有 registry 或 Git dependency，没有传递第三方依赖、default feature、build script、proc macro 或 native code，也没有产品网络能力和数据外发面。生成 lockfile 不需要下载项目依赖。

`rusqlite`、bundled SQLite 和 FTS5 尚未进入本切片，因此 SQLite 版本、启用 feature、原生构建和第三方许可证当前均为不适用；它们必须在实现 SQLite adapter 的后续切片中随实际依赖、lockfile 和三平台证据一起补充，不能提前加入白名单依赖占位。

## 工具链与验证证据

- workspace 使用 Rust 2024 edition，`rust-toolchain.toml` 精确固定 `1.96.0`，并要求 `rustfmt` 与 `clippy` component；
- 第一方 package 继承 `rust-version = "1.96.0"`、仓库许可证，以及 workspace `unsafe_code = "forbid"` 与 `unused_crate_dependencies = "deny"` lint；
- 本地 macOS 已使用 Rust / Cargo `1.96.0` 运行格式、Clippy 和全部 target 测试；
- PR workflow 已配置 Linux、macOS、Windows 三平台真实运行相同 locked 检查，并由 `Candidate Quality` 聚合；仓库内配置不等于 GitHub 执行结果，在实际 workflow run 产生前不得宣称三平台已经通过。

本基线只证明 workspace、依赖方向和空实现可编译，不证明 canonical 领域类型、SQLite、FTS5、fixture runner 或任何产品能力已经实现。
