# Phase 1 桌面中文显示切片

更新时间：2026-10-28

状态：`Bundled font integrated; headless checks and macOS visible text passed; minimum-size and Windows/Linux native acceptance pending`

## 目标与现有证据

承接[当前顺位](../status/current.md)与[Q01 中文质量验收](../evaluation/phase1-local-library-quality.md)。macOS、Windows ARM64、Linux ARM64 的历史 GUI 验收均观察到中文缺字方框。本次只解决桌面文字可读显示，中文分词、命中 / 误召回、分页、回源与加密验收各自保持原关闭条件。

从干净的 `dev` / `5ac55e6` 开始检查：`eframe 0.36.1` 启用 `default_fonts`，`apps/radishmemory-desktop/src/lib.rs` 的创建回调未使用 creation context；desktop 没有显式 `set_fonts`。候选检查使用现有 locked 构建产物与纯合成文字，不启动 GUI、不访问资料库或系统凭据。

## 固定候选与来源

| 项目 | 核实值 |
| --- | --- |
| 字体 | Noto Sans SC Regular，静态 OTF，使用上游已发布的 SC 区域子集原文件 |
| 版本 | `2.004`，上游 tag `Sans2.004` |
| 上游 commit | `523d033d6cb47f4a80c58a35753646f5c3608a78` |
| 原始路径 | `Sans/SubsetOTF/SC/NotoSansSC-Regular.otf` |
| 文件大小 | `8,331,336` bytes，约 `7.95 MiB` |
| 字体 SHA-256 | `faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9` |
| 上游 Git blob | `fc0fda9394367c16c1af7555a2ae4d5e2e6a6f02`，固定 commit 的目录 API 与本地 `git hash-object` 一致 |
| 上游 LICENSE SHA-256 | `6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2` |
| 内嵌版权 | `© 2014-2021 Adobe (http://www.adobe.com/).` |
| 许可证 | SIL Open Font License 1.1；随分发保留原许可证和版权归属，字体继续适用 OFL，项目自身许可证不变 |

原始来源：[固定字体文件](https://github.com/notofonts/noto-cjk/blob/523d033d6cb47f4a80c58a35753646f5c3608a78/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf)、[固定许可证](https://github.com/notofonts/noto-cjk/blob/523d033d6cb47f4a80c58a35753646f5c3608a78/LICENSE)、[上游格式说明](https://github.com/notofonts/noto-cjk/blob/523d033d6cb47f4a80c58a35753646f5c3608a78/Sans/README.md)。不自行裁字、转码或修改字体，不把上游区域子集表述为覆盖所有 Unicode / CJK 字符。

## 最小接入范围与影响

项目所有者已在本任务确认上述固定版本、7.95 MiB 字体、OFL-1.1、随包内嵌和 notices / 回归范围。原字节已放到 `third_party/fonts/noto-sans-sc/`，并保留该字体专用许可证、版权与来源说明。沿现有 notices 生成器记录非 Cargo 分发资产及摘要，继续使用既有仓库检查入口，不新建另一套依赖清单。

桌面增加私有字体初始化模块，在既有 `eframe` 创建回调中使用 `FontData::from_static` 和 `Context::set_fonts`；将固定中文字体追加到 `Proportional` 与 `Monospace` 字体族，保留默认拉丁 / emoji 字体顺序。这里是显式声明的随包字形选择，不是隐式探测系统字体或运行时下载；中文补充字体不承诺严格等宽代码排版。

影响为仓库新增约 7.95 MiB 字体原件、随包内嵌相应字节与运行时字体解析 / 字形图集开销。可执行文件和内存实际增量须实测，不能直接以原件大小充当峰值内存。没有新 crate、版本、feature 或 lockfile 变更，没有系统字体安装、网络 runtime、数据迁移、provider 或存储语义变化。回滚仅涉及本批字体资产、初始化代码、测试和 notices，资料库不受影响。

不选择整套多地区 / 多字重字体作为首批依赖；不以系统专有字体拷贝或少量 fixture 专用字形解决用户任意中文输入。

## 验收与证据分层

1. 字体原件与许可证摘要固定，notices 生成 / 检查覆盖其存在与字节漂移。
2. headless 渲染使用与 production 同一个字体初始化函数，检查简繁中文、全角标点、中英混排及默认拉丁字形；同时覆盖 proportional / monospace。验证实际字符映射和排版产物，不能只断言配置中存在字体名称。
3. 覆盖来源标题、搜索输入、结果 snippet 与现有正文预览区域；换行和混排不能造成重叠或裁切。生产 UI 帧证据与单独字体探针分别记录。
4. 运行针对性回归、默认 desktop 构建与 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh`。不为本切片调整依赖或放宽检查。
5. 真实 GUI 可见验收单独列出平台、合成资料、窗口、时长和退出方式并取得具体授权；没有运行的平台保留待验。UTF-8 精确导出一致和 ASCII 找回不替代中文可读显示或中文检索质量。

### 2026-10-28 候选探针

确认接入范围之前，字体和上游 LICENSE 仅下载到本批忽略目录作检查；确认后才纳入产品资产。使用本机已存在的 `egui 0.36.1` 构建产物执行 headless 排版；默认两字体族在合成中文样本缺字，加入固定候选后同一样本缺字数为 0。两字体族均可映射基本 CJK 区间 `U+4E00..U+9FFF` 中的 `20,976 / 20,992` 个字符；这不是全 Unicode 覆盖保证，也不是检索验收。

探针初版使用 `has_glyph`，在默认 monospace 的 ASCII 上遇到假阴性：当前 epaint 实现以解析出的 face 是否等于 replacement face 判断，不能作为本次完整字符覆盖 oracle。改用公开 `Font::characters()` 的实际映射后重跑通过，不修改上游、不隐藏首轮失败。该探针证明现有渲染栈可读候选，不替代产品接入、可见字形检查或跨平台 GUI。

### 接入与本机验证

- `fonts::install` 在 production `eframe` 创建回调执行，headless UI harness 复用同一函数。字符映射 / 字形纹理、默认拉丁排版尺寸、中文 / 混排换行及 1.0 / 1.5 / 2.0 缩放的行间不重叠回归通过。
- production 加密 engine 的合成中文文件经过真实导入 / 检索后，中文文件标题、输入框与结果预览进入实际 UI 绘制帧，字形映射检查通过；ASCII 检索 marker 只隔离显示问题。
- 使用同一初始化函数生成纯 headless egui 排版图，人工查看 12 / 14 / 16 / 20 pt 简繁中文、标点、输入框、标题和混排换行，无缺字方框或可见行重叠。图像是 egui 网格 / 字形图集的软件渲染，不是 macOS 原生窗口截图。
- notices 定向测试 11 项通过，包含字体 / 许可证 / 归属缺失或漂移，以及字体 symlink 的拒绝。第一次脚本调用使用 PATH 中的 Python 3.9，因缺 `tomllib` 失败；改用本机已有 Python 3.12.14 后通过，没有安装包或修改系统环境。
- 默认 desktop 的 locked / offline 构建通过。完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：仓库基线 282 个文件、notices / 检查器回归、fmt、locked Clippy 和 25 个 Rust suites 通过，合计 403 passed、0 failed、11 个由父测试显式调用的 ignored helper。新增 5 项 Rust 回归；该实现检查阶段没有真实 GUI / key store、Windows / Linux 新字体构建或发行验证；后续独立授权的 macOS 结果见下文。
- 当前默认 debug binary 为 `72,360,776` bytes；10 月 24 日保留的默认验收副本为 `63,939,096` bytes，相差 `8,421,680` bytes（约 `8.03 MiB`）。这是两个本地 debug 构建的实测比较，不是 release 压缩包或峰值内存承诺；运行时峰值内存未测量。

测试接线期间曾因共享 harness 辅助函数可见性与 epaint `ByteIndex` 类型编译失败，修正测试代码后通过。另一次旧视图测试误把失败 OpenEncrypted 的恢复动作写成 Refresh，得到真实 `RecoveryRequired`；已按现有 UI 的 `Retry encrypted open` 修正验收操作，未增加 production fallback。

准备阶段的[下一原生切片](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-28-有资料视图撤回的有限验收准备)已生成全新隔离 profile、合成 Markdown、当前默认 binary 包装及 168 份源码 / 字体文件的冻结摘要。当时准备目录没有业务数据库，evidence / export 为空；准备阶段没有创建真实 key、访问系统凭据或启动 GUI / VM。源码和产物摘要冻结不授予原生操作权限。

## 2026-10-28 macOS 可见显示结果

实现提交 `eab2b0f` 后，项目所有者明确授权冻结操作单，单次原生窗口完成新合成库 / 测试 key 初始化与迁移。实际来源标题、搜索框的 `中文` 与 ASCII marker 混排、结果中的简繁中文 / 全角标点 / API 均可见，无观察到的缺字方框或重叠。故障前后两次有结果的搜索使用 ASCII marker；中文输入只检查显示，未执行中文查询，不关闭 Q01 检索质量。

截图窗口为 2160×1504 像素；多次窗口边缘拖拽未改变捕获尺寸，未建立计划的 760×520 最小布局证据。因此只记实际窗口下的可读显示通过，最小窗口布局、Windows / Linux 新字体构建及 GUI、其它输入法与缩放仍待验证，不把 headless 多缩放结果替代原生覆盖。

同一窗口同时完成有资料 Refresh 认证失败、旧视图撤回、密文精确还原与显式恢复；两份 166 字节导出与带 BOM / CRLF 的输入逐字节一致，两个 Verify 均无 pending，应用层删除后正常退出 0。详细过程、保留测试 key / 副本和未覆盖项见[本批原生结果](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-28-macos-有资料-refresh-原生结果)。
