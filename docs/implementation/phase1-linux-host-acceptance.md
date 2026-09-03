# Phase 1 Linux 桌面宿主交互验收

日期：2026-09-03

状态：`Linux interactive evidence recorded — P1-H05 complete`

范围：只在获准修改的 `Debian13-ARM64` 测试副本中，使用纯合成 Markdown 和专用 XDG 应用数据目录，实际构建并运行 `radishmemory-desktop`。本批覆盖可见窗口、Linux XDG Desktop Portal / GTK 原生 open / save dialog、取消、导入、关闭重开、路径 / 错误脱敏和 Unix 访问控制边界；没有启动或修改 `Debian13-ARM64-CleanBase`，没有使用真实个人资料，也没有进入 PDF、模型、网络或同步。

## 环境、工具与构建

- Debian GNU/Linux 13.6 `trixie` ARM64，Linux `6.12.101+deb13-arm64`，GNOME Wayland 桌面，4 vCPU、4 GiB 内存；测试账户的可见桌面会话内运行。
- 系统已有 Git `2.47.3`、GCC `14.2`、`pkg-config 1.8.1`、CMake、Ninja、Wayland / X11 development metadata、`xdg-desktop-portal`、`xdg-desktop-portal-gtk` 和 Zenity。没有安装或升级 apt package。
- 经项目所有者明确授权后，把官方 `rustup-init` ARM64 binary 下载到任务缓存；SHA-256 `15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433` 与官方值一致。Rustup `1.29.1`、Rust / Cargo `1.96.0`、Clippy 与 rustfmt 只安装在 `/home/luobo/.rustup` 和 `/home/luobo/.cargo`，没有替换系统 Rust。
- 下载、clone 和 build 只在对应进程内使用 UTM NAT gateway `10.0.2.2:10808` 连接项目所有者已授权的 v2rayN LAN proxy；没有设置 GNOME、apt 或其它系统代理。
- 源码 clone 位于任务专用目录，`HEAD` 精确为 `c5dba35f933f72132611801e065218993bd2164f`。`cargo build --locked -p radishmemory-desktop` 使用 Rust `1.96.0` 与隔离 target directory，在 `57.46s` 内通过；没有修改 manifest、lockfile 或源码。
- 应用以 `XDG_DATA_HOME=/home/luobo/.local/share/radishmemory-p1-h05-xdg` 和任务专用 cache 启动；首次运行前该 XDG data root 不存在。输入仅为一个 83 B UTF-8 `.md` 和一个 3 B 无效 UTF-8 负向样本。

## 交互与 Portal 证据

GNOME 会话的 `xdg-desktop-portal.service` 与 `xdg-desktop-portal-gtk.service` 在 picker 交互期间均为 `active`。实际 open / save dialog 由 GNOME Files / GTK 界面呈现；同时检查没有 `zenity` 进程，因此本批没有进入 `rfd` 的 Zenity fallback。

| 场景 | Linux 实际结果 |
| --- | --- |
| 首次启动 | `wgpu` debug binary 在 GNOME Wayland 上出现可见 `RadishMemory` 窗口、0 active lineage；专用应用目录生成 `host-profile-v1.txt` 与 `library.sqlite3`。 |
| Open 取消 | `rfd` 打开 XDG Portal / GTK 原生 Files 对话框；按 `Escape` 后 UI 显示 `Import cancelled. No library changes.`，仍为 0 active lineage。 |
| 合成导入 | 通过原生 Files 对话框选择 83 B 合成 `radishmemory-p1-h05-linux-synthetic.md`；UI 显示 `Source imported.`、1 active lineage、current version 1、managed bytes 83。 |
| Save 取消 | 选中 current version 后打开 XDG Portal / GTK 原生保存对话框；取消后 UI 显示 `Export cancelled. No file was written.`，没有创建导出文件。 |
| 关闭重开 | 停止并从同一 binary、同一隔离 XDG data root 重开；合成文件名、version 1、83 B 和单一 lineage 保持。 |
| 错误脱敏 | 选择仅含 3 个无效 UTF-8 字节的合成 `.md`；UI 只显示 `LocalLibrary / ApplicationFailed · ImportNewSource / FileEntry / FileEntryRejected`，不显示目录或绝对路径，active lineage 仍为 1。 |
| 路径脱敏 | 成功页只显示来源文件名、版本、字节数和时间；绝对路径只在系统 picker 的当前授权交互内可见，没有进入应用状态、错误或普通应用日志。 |

## Unix mode 与 ACL 边界

在应用关闭前对专用目录、host profile 和数据库执行 `stat`、`find` 与 `getfacl -cp`：

- 测试账户 home、`.local` 与 `.local/share` 均为 `0700`；任务 XDG data root 为 `0755`，其下 RadishMemory 专用应用目录为 `0700`，owner / group 均为 `luobo:luobo`。
- `getfacl` 显示应用目录为 `user::rwx, group::---, other::---`；其它本机普通账户不能遍历该目录。
- `host-profile-v1.txt` 为 `0600`，ACL 为 `user::rw-, group::---, other::---`，符合 owner-only profile 边界。
- `library.sqlite3` 叶子文件为 `0644`，ACL 为 `user::rw-, group::r--, other::r--`；它本身不是 owner-only。当前保护边界来自外层 `0700` 应用目录，因此没有该目录的普通账户仍不能通过路径读取数据库。本文不把这个组合表述成数据库文件自身已加密或自身为 `0600`。
- 应用目录中没有发现额外 ACL entry；本批没有修改系统用户、组、mount、SELinux / AppArmor、全局 umask 或文件系统权限策略。

## 清理、可复验状态与边界

- `radishmemory-p1-h05.service` 已停止，`Debian13-ARM64` 已正常关机；`Debian13-ARM64-CleanBase` 始终保持关机且未修改。
- 为保留可复验证据，获准修改的测试副本中暂时保留 user-local Rust、源码 clone、隔离 target / log、专用 XDG app data 和两个合成 Desktop 输入。它们都位于上述精确任务路径；不包含真实个人资料。
- 回滚时只需在该测试副本内精确删除 `/home/luobo/.cargo`、`/home/luobo/.rustup`、`/home/luobo/radishmemory-p1-h05-linux-src`、`/home/luobo/.cache/radishmemory-p1-h05`、`/home/luobo/.local/share/radishmemory-p1-h05-xdg` 和两个 `radishmemory-p1-h05-linux-*.md` 合成输入，并移除 rustup 可能写入的用户 shell PATH marker；不需要回滚 apt、系统代理或 CleanBase。

本批证明 Linux ARM64 / GNOME Wayland 上的可见窗口、XDG Portal / GTK open / save dialog、两类取消、合成导入、关闭重开、稳定脱敏错误和 owner-only 应用目录 / host profile 边界真实成立。它不证明其它 Linux desktop / portal backend、X11-only session、Zenity fallback、发行包、签名、静态加密、真实个人资料或未来图形驱动兼容。

当前 `wgpu` head `c5dba35` 已在 [workflow run 33751048480](https://github.com/laugh0608/RadishMemory/actions/runs/33751048480) 通过 Repo Hygiene、Linux / macOS / Windows Rust Quality 与聚合 `Candidate Quality`。[第三方 notices 与条件平台依赖复核](phase1-third-party-notices.md)又完成可复现 target inventory、license option、完整文本、字体 / SQLite 归属以及 portal / Zenity、Wayland / X11、Vulkan / GLES 条件面，因此 `P1-H05` gate 已完成；其它 Linux desktop、fallback 和发行包仍须按目标独立验证。
