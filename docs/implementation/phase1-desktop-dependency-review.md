# Phase 1 桌面宿主依赖评审

日期：2026-09-02

状态：`Accepted — manifest / lockfile 已按 Windows 真实宿主证据修正并重新冻结`

范围：`P1-H04 desktop UI` 的本地窗口、一次性系统文件选择、平台应用数据目录、production opaque ID 与 UTC clock。本文不授权安装平台工具链、签名、发布、网络、同步、模型或真实个人资料导入。

## 结论

已新增第六个第一方 workspace package `apps/radishmemory-desktop`，只通过 `radishmemory-application` 使用资料库能力。直接依赖固定为：

```toml
eframe = { version = "=0.36.1", default-features = false, features = ["accesskit", "default_fonts", "wayland", "wgpu", "x11"] }
rfd = { version = "=0.17.2", default-features = false, features = ["xdg-portal", "wayland"] }
directories = "=6.0.0"
getrandom = { version = "=0.4.3", default-features = false }
time = { version = "=0.3.55", default-features = false, features = ["formatting", "parsing", "std"] }
```

`time 0.3.55` 原已存在；P1-H04 把 workspace feature 从 `parsing, std` 扩为 `formatting, parsing, std`，用于把 production UTC clock 格式化为 core 接受的 RFC 3339。其余四项是当时新增的第三方直接依赖。2026-09-02 的 Windows ARM64 真实启动证明 `glow` 在该宿主缺少 OpenGL 2.0，当前不增加 renderer fallback，而是保留同一 `eframe` 版本并把唯一 renderer feature 改为 `wgpu`。

## 已解析依赖图与证据边界

- Cargo `1.96.0` 生成的 format 4 lockfile 当前包含 418 个 package：6 个第一方 workspace package 与 412 个带 crates.io source / checksum 的第三方 package，没有 Git dependency；仓库检查器以 package 数、name / version / source / checksum 的排序 SHA-256 摘要共同拒绝漂移，不在检查器里维护一份容易漏项的手抄长列表。
- 在当前 `aarch64-apple-darwin`、已选 feature 图下，`cargo tree -p radishmemory-desktop` 可达 180 个唯一 package ID，其中 5 个是第一方；实际编译使用 `wgpu`、Metal binding、AppKit、AccessKit、剪贴板与 bundled SQLite，不启用 `glow` renderer。
- format 4 lockfile 仍会收录依赖 manifest 中可选或其它 target 的解析项，因此其中可见 `glow` / Glutin、Linux XDG Portal / D-Bus、Wayland / X11、Windows、Android 与 WASM 条件 package；这不等于它们进入当前 macOS artifact。Windows ARM64 与 Debian ARM64 / GNOME Wayland 已取得 `wgpu` build 与实际窗口证据，head `c5dba35` 的 workflow run `33751048480` 已通过当前 feature graph 的三平台 locked CI；Linux 运行还实际确认 XDG Portal / GTK picker active，且没有进入 Zenity fallback。
- 整份 lockfile 与当前目标树都没有 `tokio`、`reqwest`、`hyper`、`rustls`、`openssl`、`ureq`、`curl`、`isahc` 或 `surf` package。Linux XDG Portal 所需的 `zbus` / async executor 是本地 D-Bus / IPC 条件面，不得据此宣称绝对“零系统通信”，但它不是产品 HTTP / TLS 外发能力。
- `cargo metadata --locked` 已读取 412 个第三方 package 的声明许可证，没有缺失 license metadata；跨目标全集包含 75 个带 build script 的 package ID、27 个 proc-macro package ID，以及 `sqlite3`、Objective-C runtime、WASM binding 三种 `links` 声明。它们是供应链上限，不是当前 macOS artifact 的执行清单。
- [third-party notices 与条件平台依赖复核](phase1-third-party-notices.md)现已从三个目标的 locked metadata 生成 333 个目标可达 crate，单列 `epaint_default_fonts` 的 OFL-1.1 / Ubuntu Font License 字体条款、`option-ext` 的 MPL-2.0、`unicode-ident` 的 Unicode-3.0，并为全部 OR expression 记录 distribution basis；`self_cell` 选择 Apache-2.0，不形成强制 GPL 选择。完整 lockfile 中不可达目标项仍是供应链上限，不混入当前分发清单。
- 原始 `glow` 图曾通过 desktop package 的 locked check、15 个单测、all-targets / all-features Clippy `-D warnings` 和仓库聚合门禁；这些自动化证据覆盖 production random / UTC runtime、应用数据目录、host profile、picker request 映射、关闭重开、损坏失败关闭和 UI 状态逻辑。随后 [macOS 验收](phase1-macos-host-acceptance.md) 使用纯合成数据实际运行窗口、AppKit open / save panel 和负向状态，[Windows 验收](phase1-windows-host-acceptance.md) 暴露 OpenGL-only 阻断并在 `wgpu` 修复后完成可见窗口、native dialog、重开、脱敏与 ACL 复验，[Linux 验收](phase1-linux-host-acceptance.md) 又在 Debian ARM64 / GNOME Wayland 完成 XDG Portal / GTK dialog、取消、导入、重开、脱敏与 Unix ACL / mode 复验。当前 `wgpu` 图已由本机聚合门禁和 run `33751048480` 的三平台 CI 重新收口。

## 选择与限制

### `eframe 0.36.1`

- upstream 为 Rust 2024、MSRV `1.95`、`MIT OR Apache-2.0`，满足 workspace Rust `1.96.0`；
- 选择 immediate-mode 本地 UI，是因为现有 application service 已隔离领域与 adapter，首批界面可以保持单进程、同步、无前端构建链；
- 关闭 default features，显式选择唯一 `wgpu` renderer；保留 `accesskit`、内置默认字体以及 Linux 的 Wayland / X11 编译面，不同时启用 `glow` 或增加运行时 fallback；
- 不启用 `links`、`persistence`、`web_screen_reader` 或 `inspection`：首批 UI 不能打开外部链接、建立第二份持久 UI 状态、启用 web surface 或打开 inspection TCP 端口；`wgpu` 扩大的平台 GPU backend / native binding 已计入 lockfile、metadata 与本页供应链边界；
- native 运行仍会使用窗口、平台图形 backend、系统事件、剪贴板和可访问性 API。剪贴板只服务用户主动复制 / 粘贴，不自动读取或持久化；第一方代码不初始化普通日志 sink。

官方依据：[eframe 0.36.1 crate](https://docs.rs/crate/eframe/0.36.1)、[0.36.1 workspace 元数据](https://github.com/emilk/egui/blob/0.36.1/Cargo.toml)、[eframe feature 定义](https://github.com/emilk/egui/blob/0.36.1/crates/eframe/Cargo.toml)。

### `rfd 0.17.2`

- 许可证为 `MIT`，MSRV `1.88`；提供 Windows、macOS 与 Linux / BSD 的 native open / save dialog；
- 关闭隐式 default features，再显式保留 Linux `xdg-portal` 与 `wayland`；不选择需要 GTK3 development headers 的 `gtk3` backend；
- macOS 使用 AppKit open / save panel，Windows 使用 COM / Shell API；Linux 使用 XDG Desktop Portal 的 D-Bus API，依赖运行环境提供受支持的 portal backend，并在 portal 失败时可能调用 Zenity；这些是本地 IPC / 系统进程能力，不是产品网络能力；
- picker 只返回本次调用的路径。首批不保存最近路径、bookmark、portal token 或授权句柄；取消选择返回普通取消状态，不触发 import / export；
- Linux 分发物必须声明 portal backend 与 Zenity 运行依赖，三平台真实 picker 证据不能由 macOS 单机测试替代。

官方依据：[rfd 0.17.2 文档与平台后端](https://docs.rs/rfd/0.17.2/rfd/)、[rfd 0.17.2 manifest](https://docs.rs/crate/rfd/0.17.2/source/Cargo.toml)、[rfd 0.17.2 changelog](https://docs.rs/crate/rfd/0.17.2/source/CHANGELOG.md)。

### `directories 6.0.0`

- 许可证为 `MIT OR Apache-2.0`，直接依赖 `dirs-sys 0.5.0`；通过 Linux XDG、Windows Known Folder 与 macOS Standard Directory 规则计算项目目录；
- 只使用 `ProjectDirs`，宿主标识冻结为 qualifier `io.github`、organization `laugh0608`、application `RadishMemory`；资料库数据库、host profile 与任务临时状态只位于 `data_local_dir`，不使用用户 Documents 或任意 home-relative 拼接；
- crate 只计算路径，不创建目录。第一方宿主负责精确创建应用目录、检查它不是 symlink / 普通文件冲突，并在 Unix 上收紧为 owner-only；目录解析或创建失败时不回退当前目录或临时目录。

官方依据：[directories 6.0.0](https://docs.rs/crate/directories/6.0.0)。

### `getrandom 0.4.3` 与 `time 0.3.55`

- `getrandom` 为 `MIT OR Apache-2.0`、MSRV `1.85`，默认 feature 为空；只调用操作系统首选随机源填充固定 128-bit bytes，再由宿主编码为带类型前缀的 lowercase hex opaque ID；随机源失败时整个 operation 失败，不使用时间、路径、计数器或弱随机 fallback；
- 不启用 `getrandom` 的 `sys_rng`、`wasm_js` 或自定义 backend；首批只构建 native Linux / macOS / Windows；
- `time` 保持固定 `0.3.55`，只新增 `formatting` feature；production clock 使用 `SystemTime` / UTC 并格式化 RFC 3339，不读取本地时区。系统时钟不可表示或格式化失败时 operation 失败，不生成伪时间。

官方依据：[getrandom 0.4.3](https://docs.rs/getrandom/0.4.3/getrandom/)、[getrandom feature](https://docs.rs/crate/getrandom/0.4.3/features)、[time 0.3.55 feature](https://docs.rs/crate/time/0.3.55/features)。

## 宿主 profile 与应用目录

production host 不能在每次启动重新分配 namespace / device ID，也不能从数据库路径、用户名、机器名或原始文件路径推导 canonical identity。`apps/radishmemory-desktop` 因此需要在首次启动使用同一 OS random capability 生成 host profile，至少保存 profile contract、namespace ID 与 device ID，并与 `library.sqlite3` 放在同一应用数据目录。

profile 必须任务临时文件写入、sync、关闭、逐字节复验并原子无覆盖发布；公开错误不包含应用目录或 ID。状态组合按以下规则失败关闭：

| profile | database | 行为 |
| --- | --- | --- |
| 不存在 | 不存在 | 创建并原子发布 profile，再创建资料库 |
| 存在且合法 | 不存在 | 使用既有 identity 创建空资料库 |
| 存在且合法 | 存在 | 使用既有 identity 打开并复验资料库 |
| 不存在或损坏 | 已存在 | 拒绝打开；不得生成新 identity 认领旧事实 |

profile 不是原始资料、记忆正文、路径 bookmark 或 UI cache，但它是 canonical namespace 的 identity root；删除、覆盖、恢复和未来同步迁移都必须把它当作受治理宿主状态。首批不增加自动恢复 UI operation，也不把 profile 当作备份或跨设备身份。

## 数据流与权限面

一次 import / update 只允许如下路径：

`user gesture → native picker → selected absolute path → FileReadRequest(exact path + selected parent) → LocalLibrary → managed exact bytes`

一次 export 只允许如下路径：

`user gesture → native save picker → selected absolute target → FileExportRequest(exact target + selected parent) → LocalLibrary → no-overwrite publish`

路径仅存在于 picker adapter、同一调用栈和文件系统 syscall；不进入 profile、SQLite、receipt、普通日志或错误。UI 不持有 SQLite handle，不构造 canonical object / deletion component，不绕过 `radishmemory-application`。

所选 feature 图不包含 HTTP client、TLS、自动更新、遥测、模型 SDK、同步、inspection listener 或浏览器打开能力。XDG portal 的 D-Bus、本地窗口系统、图形驱动、剪贴板和辅助功能属于必须记录的平台 IPC / native surface，但不能被表述为网络外发。

## 已执行与后续停止线

已按授权完成：

1. 修改 workspace manifest，新增 desktop package 和上述精确直接依赖，生成并冻结 lockfile；
2. 实现应用目录、原子 host profile、random / UTC runtime 与 one-shot picker adapter；
3. 实现资料库启动 / 不可用、导入 / 更新、来源 / 版本、搜索 / citation、导出、删除确认与逐组件 evidence、verify / rebuild UI；
4. 用合成文件数据库验证 profile 与 managed bytes 关闭重开、picker 取消 / 脱敏和 host operation，并运行本机 locked build / test / Clippy；
5. 同步正式依赖基线与仓库检查器，使 manifest、第一方 package、lock count / digest、source / checksum 和诊断 sink 漂移失败。

后续已按单独 GUI 授权，用隔离应用数据目录和合成 Markdown 完成 macOS 可见窗口、AppKit open / save picker、取消、导入、更新、search citation、历史导出、verify / rebuild、关闭重开和经即时确认的 lineage 删除交互验收；还用三个独立合成状态验证了 picker 后读取失败无副作用，canonical body 篡改和 origin binding 缺失在 Verify / Rebuild / 重启 / Retry 上持续失败关闭且不自愈。应用、缓存、篡改器与合成数据临时根已清理。该证据不改变本评审的依赖版本或授权范围。

后续停止线保持：未取得独立授权不再次启动真实 GUI、不修改系统权限、不安装额外平台工具链、不签名或发布；当前 desktop 变更的 Linux / macOS / Windows locked CI、三平台可见 UI / native picker 实际证据、完整可分发依赖清单与 third-party notices 已成立，P1-H05 gate 已完成。签名发行包仍须实际携带 notices 并复验非提权 owner 与平台 backend；本任务不使用真实个人资料，也不自动进入 PDF / OCR、Embedding、模型、网络、同步或通用 workflow engine。
