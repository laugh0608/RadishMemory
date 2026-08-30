# Phase 1 macOS 桌面宿主交互验收

日期：2026-08-30

状态：`macOS interactive evidence recorded — P1-H05 remains open`

范围：使用纯合成 Markdown 数据，在 `aarch64-apple-darwin` 上实际启动 `radishmemory-desktop`，操作人工可见窗口与 AppKit open / save panel，复验首次启动、取消、导入、更新、搜索、历史导出、关闭重开、verify / rebuild 和完整 lineage 删除。本文只记录 P1-H05 的 macOS 交互子证据，不代表 Linux / Windows picker 运行通过，不授权真实个人资料、签名、发布、网络、同步或模型。

## 隔离方式

- 使用 `cargo build --locked -p radishmemory-desktop` 构建当前 workspace；依赖下载位于任务专用临时 Cargo home，未修改 manifest 或 lockfile。
- 将进程 `HOME` 定向到任务专用临时目录，使 `ProjectDirs` 解析出的 `library.sqlite3` 和 `host-profile-v1.txt` 不接触用户现有应用数据。
- 只创建一个 107 B 的合成 Markdown 初始版本，随后把同一合成文件改为 165 B 的更新版本；没有选择、读取或导入真实个人文件。
- 为使裸调试二进制可被 macOS 辅助功能接口定位，在临时目录创建未签名、未安装的 `.app` 外壳；它不进入仓库，也不是分发物证据。
- 系统选择器只在当次授权期间看到所选绝对路径和内容预览。RadishMemory 窗口、状态消息、citation、删除 evidence 与进程输出没有显示或记录该路径；应用数据库和 host profile 只存在于隔离目录。

## 实际结果

| 操作 | 观察结果 |
| --- | --- |
| 首次启动 | 窗口显示 `0 active lineage(s)`、`No managed sources`，文件数据库与 host profile 在隔离应用数据目录建立。 |
| open picker 取消 | 状态为 `Import cancelled. No library changes.`，仍为 0 条 active lineage。 |
| picker 后读取权限失败 | AppKit open panel 实际选择权限位为 `000` 的合成 Markdown；应用显示 path-free `ImportNewSource / FileEntry / FileEntryRejected`，仍为 0 条 active lineage。随后以 bundled SQLite 临时工具复核 source artifact、body、fragment、origin binding、capture audit 与 FTS 六类计数全部为 0。 |
| 导入合成 Markdown | 状态为 `Source imported.`；目录出现 `p1-h05-note.md`，当前版本 `v1`、107 B、版本数 1。 |
| 搜索初始版本 | `alpha` 命中 `v1 · bytes 0..107`，并显示 opaque source ID 与 fragment ID。 |
| 明确更新同一来源 | 再次经 open picker 选择同一合成文件后状态为 `Source version updated.`；当前版本变为 `v2`、165 B、版本数 2，`v1` 仍在历史中。 |
| 搜索当前版本 | `beta` 只返回当前 `v2 · bytes 0..165`，citation 使用新的 opaque source / fragment ID。 |
| 导出历史版本 | 选择 `v1` 后经 save panel 写入不存在的目标；状态为 `Managed bytes exported without overwrite.`。导出文件恰为 107 B，SHA-256 为 `0ce413b836a68202e627dfdfc0c3f88433b24964017545ed113fa1f4564211e7`，内容与初始版本逐字节一致。 |
| verify / rebuild | verify 报告 `Canonical facts and derived recall are consistent.`；rebuild 报告 `Derived recall was rebuilt from verified facts.`，之后 `beta` 仍命中同一 `v2` citation。 |
| canonical body 损坏 | 在独立合成库中只替换一行受管 body，不改摘要、fragment 或 FTS。UI Verify 与 Rebuild 分别返回 path-free `VerifyLibrary / Storage / StorageFailure` 和 `RebuildRecall / Storage / StorageFailure`；六类计数保持各 1，证明失败重建没有修改 canonical facts 或派生索引。关闭重开后只显示 `Local library unavailable` / `OpenLibrary / Storage / StorageFailure`，Retry 仍拒绝。 |
| origin binding 缺失 | 在另一独立合成库中删除唯一 origin binding，并由外键级联删除 capture audit。UI Verify 与 Rebuild 均返回稳定 `StorageFailure`；artifact、body、fragment、FTS 仍各 1，binding / audit 保持 0，未补造 provenance。关闭重开和 Retry 均在暴露资料库操作前失败关闭。 |
| 关闭重开 | 正常退出并使用同一隔离 `HOME` 重启后，仍显示 1 条 lineage、当前 `v2`、两个版本；`beta` 仍命中 `v2`。 |
| 删除确认 | UI 在最终动作前明确说明会 purge 全部 managed versions 和 active memory dependencies，也明确说明不会删除原始选择文件或既有导出。最终删除由项目所有者即时确认后执行。 |
| 删除与 evidence | 状态为 `Local managed lineage deletion completed with persisted evidence.`；0 条 active lineage、搜索为空。evidence 为 local device scope、`Completed`、10 个组件结果；source body / fragment 为 `Deleted 2/2`，source metadata / minimal audit 为 `RetainedMinimal 2/2`，其余未存在组件为真实 `NotFound 2/2`。 |
| 外部边界 | 删除后原始合成文件仍为 165 B，历史导出仍为 107 B；二者 SHA-256 分别为 `0c82d9495dfcb8ccfa09adf052dba2d311b0bf360773e274a7bebd4feac87aa5` 与 `0ce413b836a68202e627dfdfc0c3f88433b24964017545ed113fa1f4564211e7`。 |
| 清理 | 应用正常退出；任务专用 app、Cargo cache、应用数据、输入和导出所在临时根已精确删除并复验不存在。未修改系统权限、系统设置、签名或安装状态。 |

系统自带 `sqlite3` 未包含 FTS5，因而不能作为该 bundled SQLite 数据库的独立解析器；没有据此推导数据库结果。canonical / derived 一致性、重建、重启和删除结果均由使用 production bundled SQLite 的应用路径观察，并继续由 workspace 自动化测试约束。

## ADR 0007 场景覆盖

| 场景 | 本次 macOS 交互证据 | 仍需完成 |
| --- | --- | --- |
| `P1-HF01` | 首次启动空库、0 source、数据库 / profile 建立 | 三平台 locked CI |
| `P1-HF02` | AppKit open panel 导入 `.md`，目录与搜索出现 current source | Linux / Windows picker 平台证据 |
| `P1-HF03` | 正常关闭重开后来源、版本和 citation 保持 | 三平台 locked CI |
| `P1-HF04` | 明确选择 lineage 并重新选择文件，建立严格 `v2`；搜索命中 current tip | 重复字节幂等继续由自动化测试证明 |
| `P1-HF05` | UI 显示稳定来源、当前版本和 `v1` / `v2` 历史 | 分页边界继续由自动化测试证明 |
| `P1-HF06` | `v1` / `v2` 搜索显示 source、fragment 和 byte range citation | 三平台 locked CI |
| `P1-HF07` | save panel 精确导出历史 `v1`，摘要与字节数相符且不覆盖 | 当前版本和拒绝覆盖边界继续由自动化测试证明 |
| `P1-HF08` | 经即时确认删除两版本完整 lineage，0 召回，10 组件 evidence，外部文件不变 | 三平台 locked CI |
| `P1-HF09` | picker 取消明确无资料库变化；选择不可读合成文件后稳定拒绝，六类事实 / 派生计数仍为 0 | Linux / Windows picker 平台差异继续由对应平台证据约束 |
| `P1-HF10` | 健康库 verify / rebuild 与重启成立；canonical body 和 binding 独立损坏后，Verify、Rebuild、重启与 Retry 均持续失败关闭且不自愈 | 三平台 locked CI |
| `P1-HF11` | 不可信 Markdown 只进入受管正文与本地搜索；未触发工具、模型或记忆写入 | 网络能力仍由依赖图、代码检查和 CI 共同约束，不把单次观察当作网络取证 |
| `P1-HF12` | 应用 UI、状态、citation、evidence 和进程输出未出现路径；系统 picker 仅在授权调用内暂时显示合成路径 | Linux / Windows UI 与失败输出复核 |

## P1-H05 剩余门禁

1. 让当前未提交 desktop workspace 在既有 Linux、macOS、Windows Rust Quality matrix 上完成 locked format、Clippy 和 test，并取得聚合 `Candidate Quality` 证据；当前只确认 workflow 已覆盖三个平台，没有创建远程 run。
2. 生成可分发的 third-party notices 与 Linux portal / Zenity、Wayland / X11 和 Windows native dialog 依赖清单；在此之前不进入签名、打包或发布。

在上述门禁完成前，P1-H05 保持开放，RadishMemory 仍不是获准导入真实个人资料的生产文件入口。
