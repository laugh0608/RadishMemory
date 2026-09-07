# Phase 1 Windows 桌面宿主交互验收

日期：2026-09-02

状态：`Windows interactive evidence recorded — P1-H05 complete`

范围：只在获准修改的 `Windows11-ARM64` 测试副本中，使用纯合成 Markdown 和专用应用数据目录，实际构建并运行 `radishmemory-desktop`。本批覆盖可见窗口、Windows 原生 open / save dialog、取消、导入、关闭重开、路径 / 错误脱敏和 ACL 边界；没有启动或修改 `Windows11-ARM64-CleanBase`，没有使用真实个人资料，也没有进入 PDF、模型、网络或同步。

## 环境与构建

- Windows 11 Pro ARM64，4 vCPU、8 GB；测试账户桌面会话内运行。
- Git `2.55.0.windows.3`；Rust `1.96.0-aarch64-pc-windows-msvc`，含 Cargo、Clippy 和 rustfmt；Visual Studio Build Tools `17.14.39`，含 ARM64 C++ tools 与 Windows 11 SDK `26100`。
- 源码起点为 `57f4f446a1b14379d01d29db833c2eae893ebf85`；修复前 `cargo check --locked -p radishmemory-desktop` 与 `cargo build --locked -p radishmemory-desktop` 均在 ARM64 Developer Command Prompt 中通过。
- 首次实际启动在创建窗口前返回 `egui_glow requires opengl 2.0+`。根因是 desktop feature graph 只启用 `eframe/glow`，而该 Windows ARM64 虚拟显示设备不提供所需 OpenGL；没有增加 fallback 或吞掉错误。
- 最小修复把同版本 `eframe` 的 renderer feature 从 `glow` 改为 `wgpu`，并更新现有锁定依赖图；Windows 复验 `cargo build --locked -p radishmemory-desktop` 在 `1m26s` 内通过，随后出现真实 `RadishMemory` 窗口。
- 工具与 crates 下载只在安装 / 构建进程内使用获准代理。临时 Windows user / WinHTTP 系统代理在 Build Tools 安装后已恢复为 `ProxyEnable=0`、无 `ProxyServer` 且 WinHTTP direct access。

## 交互证据

应用数据基线检查确认 `%LOCALAPPDATA%\laugh0608\RadishMemory\data` 在本批首次启动前不存在；输入只包含任务生成的 UTF-8 `.md` 和一个无效 UTF-8 负向样本。

| 场景 | Windows 实际结果 |
| --- | --- |
| 首次启动 | `wgpu` 修复后的 ARM64 debug binary 出现可见窗口、0 active lineage；专用数据目录生成 `host-profile-v1.txt` 与 `library.sqlite3`。 |
| Open 取消 | `rfd` 打开 Windows 原生“打开”对话框；按 `Escape` 后 UI 显示 `Import cancelled. No library changes.`，仍为 0 active lineage。 |
| 合成导入 | 通过原生“打开”对话框选择 103 B 合成 `1.md`；UI 显示 `Source imported.`、1 active lineage、current version 1、managed bytes 103。 |
| Save 取消 | 选中 current version 后打开 Windows 原生“另存为”对话框；取消后 UI 显示 `Export cancelled. No file was written.`。 |
| 关闭重开 | 正常关闭窗口后从同一 binary 重开；`1.md`、version 1、103 B 和单一 lineage 保持。 |
| 错误脱敏 | 选择仅含无效 UTF-8 字节的合成 `2.md`；UI 只显示 `LocalLibrary / ApplicationFailed · ImportNewSource / FileEntry / FileEntryRejected`，不显示目录或绝对路径，active lineage 仍为 1。 |
| 路径脱敏 | 成功页只显示来源文件名 `1.md`、版本、字节数和时间；绝对选择路径只在 Windows 系统 picker 内可见，没有进入应用状态、错误或普通进程输出。 |

## Windows ACL 边界

对 `%LOCALAPPDATA%\laugh0608\RadishMemory\data`、`host-profile-v1.txt` 和 `library.sqlite3` 分别执行 `icacls`。目录只继承以下 full-control ACE：

- `NT AUTHORITY\SYSTEM:(I)(OI)(CI)(F)`；
- `BUILTIN\Administrators:(I)(OI)(CI)(F)`；
- 当前测试账户 `(I)(OI)(CI)(F)`。

两个文件只继承同三类主体的 `(I)(F)`；没有 `Everyone`、`BUILTIN\Users` 或其它宽泛主体。因为本批从 ARM64 Developer Command Prompt 启动，`dir /q` 显示目录和两文件 owner 为 `BUILTIN\Administrators`；这不是额外 owner-only ACL 实现，但当前继承边界没有向其它普通本机账户授予读取或写入。后续正式非提权安装入口仍应单独复核 owner 归属，不能把本次 elevated test owner 外推为发行态证据。

## 结论与 P1-H05 收口

本批证明 Windows ARM64 上的可见窗口、原生 open / save dialog、两类取消、合成导入、关闭重开、稳定脱敏错误和继承 ACL 边界真实成立，并暴露且修复了 OpenGL-only renderer 阻断。它不证明 Linux UI / XDG Portal、发行包、签名、真实个人资料或未来 Windows 显示驱动兼容；后续 [Linux 交互记录](phase1-linux-host-acceptance.md) 已单独补齐 Debian ARM64 / GNOME Wayland 的 XDG Portal / GTK 证据。

当前 `wgpu` head `c5dba35` 已在 [workflow run 33751048480](https://github.com/laugh0608/RadishMemory/actions/runs/33751048480) 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`。[第三方 notices 与条件平台依赖复核](phase1-third-party-notices.md)又完成可复现 target inventory、license option、完整文本、字体 / SQLite 归属和 Windows native 条件面，因此 `P1-H05` gate 已完成；正式非提权 installer 的 owner、签名和包内 notices 仍属于发布验证，不由本批外推。
