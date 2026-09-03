# Phase 1 第三方 notices 与条件平台依赖复核

日期：2026-09-03

状态：`Accepted — P1-H05 distribution inventory gate complete`

范围：复核 `radishmemory-desktop` 在三个已验收 ARM64 目标上的 locked normal / build 依赖、可分发许可证文本、默认字体、bundled SQLite 和操作系统条件依赖。本文不授权签名、打包、发布、真实个人资料、PDF / OCR、向量、模型、网络或同步，也不把单个测试环境外推为所有平台版本和驱动组合。

## 可复现清单

根目录 [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md) 由 `scripts/generate-third-party-notices.py` 从以下三个 `cargo metadata --locked --filter-platform` 图生成：

| 目标 | 清单条目 | 已有运行证据 |
| --- | ---: | --- |
| `aarch64-apple-darwin` | 204 | macOS AppKit 可见窗口与 open / save panel |
| `aarch64-unknown-linux-gnu` | 274 | Debian ARM64 / GNOME Wayland 的 XDG Portal / GTK picker |
| `aarch64-pc-windows-msvc` | 198 | Windows 11 ARM64 的 native dialog、重开与 ACL |

三个图合并后为 333 个唯一 crates.io package；全部有 `Cargo.lock` checksum 和上游声明许可证，没有 Git dependency、缺失许可证、未审查 source 或需要另行合并的 top-level `NOTICE` 文件。清单记录完整 checksum、平台成员关系、upstream / author attribution、原始 license expression 与选定 distribution basis；当前 inventory SHA-256 为 `17d86e4f32f4b8d4691b54a977bc9b87354db1a4c734a09b8f1dc7768622ebb3`。

生成器只从第一方 `radishmemory-desktop` 根沿 normal / build edge 遍历，排除纯 dev dependency 和不可达的其它 lockfile 条目。`--check` 会重新解析三个目标图并逐字节比较生成物；新增 source、缺失 checksum / license、未知 license expression 或清单漂移均失败关闭。完整 lockfile 仍是 412 个第三方 package 的供应链上限，不应与 333 个当前桌面目标并集混写。

## License option 与人工复核

- 多数 `MIT OR Apache-2.0` 及等价顺序 / 分隔写法选择 MIT；`self_cell` 的 `Apache-2.0 OR GPL-2.0-only` 选择 Apache-2.0，`moxcms` / `pxfm` 的 BSD / Apache 选项选择 Apache-2.0。因此当前 distribution basis 没有选择 GPL / LGPL 分支，也没有用 OR 表达式掩盖实际选择。
- `dpi` 明确声明 `Apache-2.0 AND MIT`，两份文本均保留；`option-ext` 保持 MPL-2.0，未来 binary package 还必须告知接收者如何按 MPL-2.0 以合理方式取得该 Covered Software 的对应 Source Code Form；`unicode-ident` 使用 `MIT AND Unicode-3.0`；`clipboard-win` / `error-code` 保持 BSL-1.0；`libloading` 保持 ISC；`foldhash` / `zlib-rs` 保持 Zlib。
- `epaint_default_fonts` 使用 `MIT AND OFL-1.1 AND Ubuntu-font-1.0`。除通用文本外，[字体专用 notices](../../third_party/licenses/epaint-default-fonts-notices.txt)保留 Hack / Source Foundry、DejaVu public-domain、Bitstream Vera reserved font names 和 John Slegers emoji font notice；Noto Emoji 与 Ubuntu Light 分别对应 OFL-1.1 和 Ubuntu Font Licence 1.0。
- `libsqlite3-sys 0.38.2` Rust wrapper 依清单使用 MIT；启用 `bundled` feature 编译的 SQLite `3.53.2` 是独立 public-domain material，[dedication](../../third_party/licenses/SQLite-public-domain.txt)没有被误写成 RadishMemory 自身许可证。
- [许可证文本目录](../../third_party/licenses/README.md)包含实际选中的 Apache-2.0、MIT、MPL-2.0、OFL-1.1、Ubuntu-font-1.0、Unicode-3.0、BSL-1.0、ISC 与 Zlib 全文。表中 author / upstream attribution 与专用 notices 一起承载对应版权来源；选项选择不重新许可上游代码，也不改变根 `LICENSE` 的 source-available 条款。

## 条件平台依赖清单

以下组件由操作系统、桌面会话或图形驱动提供，不是 Cargo 打包的 Rust crate。发行说明必须按目标保留这些条件，不能把三平台任一实测结果写成无条件保证。

| 条件面 | macOS | Windows | Linux |
| --- | --- | --- | --- |
| 窗口与事件 | AppKit / Cocoa；系统窗口、事件、剪贴板和 accessibility API | Win32 windowing、剪贴板与 UI Automation / AccessKit | Wayland 与 X11 client 面均编译；运行时取决于会话和 compositor |
| 文件选择 | `rfd` 调用 AppKit open / save panel | `rfd` 调用 COM / Windows Shell native open / save dialog | 首选 XDG Desktop Portal over session D-Bus，依赖可用 portal backend；失败路径可能调用外部 Zenity |
| 应用数据目录 | Standard Directory 规则 | Known Folder API 与目录继承 ACL | XDG base directory 规则与 Unix mode / ACL |
| GPU | `wgpu` Metal backend | `wgpu` 的 DX12 / Vulkan / GLES 条件 backend，最终可用性由驱动决定 | `wgpu` 的 Vulkan / GLES 条件 backend，最终可用性由驱动和 display stack 决定 |
| 辅助功能 / IPC | 系统 accessibility API | UI Automation / COM | AT-SPI / D-Bus 条件面 |

Linux 发行环境必须提供可工作的 XDG Desktop Portal 及与桌面匹配的 backend；如果承诺 Zenity fallback，还必须把 Zenity 作为外部运行依赖明确声明并测试。当前 Debian / GNOME Wayland 实测 portal active 且没有 Zenity 进程，这只排除了该次运行进入 fallback，不能删除 fallback 的分发说明。Wayland / X11、Vulkan / GLES 同样是编译与运行条件，不代表每条 backend 都在本批逐一执行。

Windows 当前证据来自 elevated ARM64 Developer Prompt，确认测试数据目录没有 `Everyone` / `BUILTIN\\Users` ACE，但 owner 是 `Administrators`；正式非提权安装仍须单独复核 owner 和安装器是否随包携带 notices。macOS 当前是未签名本机构建；签名、notarization、sandbox entitlement 和发行包内容不在本门禁内。

## 结论与持续门禁

P1-H05 所需的三平台真实宿主交互、当前 `wgpu` 图三平台 CI、可复现 target-specific crate inventory、license option、完整文本、字体 / SQLite notices 与系统条件依赖已全部形成可审查证据，因此 P1-H05 gate 完成。

这不等于已有发行包或 production deployment：任何 installer / DMG / archive 必须实际携带 `THIRD_PARTY_NOTICES.md` 与 `third_party/licenses/`，并在发布前验证包内容、目标架构、签名链、非提权数据 owner、平台最低版本和对应 native backend。后续依赖、feature、target 或 `Cargo.lock` 发生变化时，必须重新生成、人工复核并更新本页；检查器不会把未知表达式自动归为宽松许可证。
