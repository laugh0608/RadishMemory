# 阶段 1 旧检索格式与最小窗口验收

## 2026-11-02 旧检索格式与最小窗口验收准备

准备时状态：操作包完成，真实 GUI / key store 尚待授权；随后项目所有者按本单授权执行，实际结果见[本批原生结果](#2026-11-02-macos-原生局部结果)，最小窗口仍未覆盖。本批承接[当前状态](../status/current.md#后续事项)第 2 项；默认新库已使用 HanPhraseV1，因此不能沿用 10 月 28 日仅有 profile 的空库作为旧格式输入。历史包装、库、key、摘要与 M01 暂缓状态均保持，新增材料使用独立目录。

### 输入、构建与授权边界

从 `dev / 3f05a13` 创建独立合成库，业务事实全部由 production `LocalLibrary` 导入 / 更新，再显式切换为 `LegacyUnicode61`：2 lineage、3 source versions，A 的 v1 是红色主题，v2 是蓝色主题与 API / 繁體混排，B 是不含红 / 蓝词语的资料治理干扰项。A 两版本均带 UTF-8 BOM / CRLF，当前版本为 103 bytes；B 为 78 bytes。初始正文 schema 为 v6，3 inline bodies、2 当前 FTS rows，profile 表不存在且 FTS 为旧五列结构，无 vault、pending 或删除请求。此输入不同于“新库默认旧格式”的过期假设，不改 production 默认值。

本地忽略目录 `.tmp/2026-11-02-recall-native/` 保存生成器、原始身份、预演库与导出、默认构建记录、只读预检和精确操作单；其 `package/` 保存唯一新 app 包装、data / input / export / native-evidence。包装仅复制当前默认 debug binary，不重签、不安装、不修改全局字体；没有新增 feature、依赖或 lockfile 变化。最终 `freeze.json` 绑定构建输入、binary、副本、profile、输入、操作单与检查器；初始数据库另存摘要，运行后不得用旧摘要覆盖实际状态。

拟请求的执行范围为同一包装的最多三次串行 GUI 启动，约 40–60 分钟及 5 分钟正常收尾；只操作该合成库，一次 Prepare 创建唯一新系统 slot、一次迁移三份正文，以及下表有限动作。完整 namespace / device、key service / account、可执行文件、启动参数、预算和导出目标见本地精确操作单，授权只覆盖该冻结包。不得把本节准备记录视为已取得启动或 key 访问授权。

### 原生判据与顺序

| 阶段 | 动作与通过条件 | 状态边界 |
| --- | --- | --- |
| N01 明文旧库 | 旧模式 ASCII 当前词 1 命中、中文蓝色与旧版本词 0；升级 Cancel 无变更、Confirm 后旧结果清空；中文蓝色仅 A v2、主题命中 A v2 / B、红色与旧 ASCII 词 0；导出 A v2、Verify、正常关闭 | v6 保持；关闭后应为 Han，canonical 与精确正文不变 |
| N02 重开与加密 | 明文重开保持 Han；降级 Cancel / Confirm，提示短词遗漏且不恢复备份 / body compatibility，旧结果清空；中文 0、ASCII 当前词 1，精确导出、Verify；一次 Prepare / Migrate 后显式打开 encrypted Legacy，再执行升级 Cancel / Confirm、中文检索与精确导出、Verify、正常关闭 | v8 objects_ready、3 references、0 inline bodies，Han；迁移与格式切换是两个独立动作 |
| N03 加密重开与降级 | 默认明文入口对 v8 失败关闭后显式 OpenEncrypted，确认 Han 与中文命中保持；降级 Cancel / Confirm 后核对限制文案、中文 0 / ASCII 当前词 1、精确导出、Verify、正常关闭 | 最终 v8 / Legacy，降级不恢复 v6 / 旧备份或改变加密正文；source IDs / canonical 与对象摘要保持 |

四份原生导出必须各使用事先不存在的目标，并与 A v2 原件逐字节及 SHA-256 比对，保留 BOM / CRLF。合成库预先导入不算原生 Import / Update 验收。FTS 仍有完整可读正文，迁移不代表整个资料库静态加密。旧客户端拒绝继续引用 ADR 0009 对应真实旧 adapter 回归；本单不另编译或启动旧客户端。

最小窗口要求 **egui inner logical viewport 760×520**。必须记录实际 content rect / scale、原生 outer frame 与截图尺寸的对应关系，不能将截图像素或标题栏外框直接当内容尺寸。调整后在明文 / 加密入口分别观察中文标题、简繁混排正文、Search、Export、升级 / 降级按钮和 modal 文案、Cancel / Confirm，无裁切或重叠并可点击。工具无法证实实际 content rect 或不能调整到目标时，明确记未覆盖；不以 headless 或较大窗口截图补成通过，不增加系统权限或产品诊断入口。

动作预算：GUI ≤3 次、Prepare / Migrate 各 1、OpenEncrypted ≤2、升级 / 降级 Confirm 各 2、相应 Cancel 共 4、Search ≤20、Export 4、Verify ≤4。只允许本新 slot 的预期提示逐次交接用户选择 Allow Once，每动作最多 8 次、整单最多 64 次；达到上限、目标 / 阶段不明、意外状态或摘要变化即停止新增动作，不提高预算或选择永久允许。没有提示不证明系统调用为零，正常路径不替代真实首尾取消证据。

正常收尾等待当前动作完成、关闭窗口、确认退出码与无遗留本批进程 / 句柄，再只读复验实际格式、canonical、对象及导出。保留新库、输入、导出、证据、包装和唯一新 slot；不复制预演库盖回、不删除唯一 key、不伪造正文迁移回滚。没有损坏 / 持锁 / ACL、Delete、Rebuild、Refresh、VM 或远程动作，完整 P1-S05、Q01 / Q04 和 PDF / 图片停止线保持。

### 授权前验证

新增 headless 回归在真实 production worker 和合成 provider 上，以 760×520 viewport 检查两来源、多版本快照下的 Search / Export / 更新 / 升级入口与升级 / 降级 Cancel / Confirm：文本边界完整落在 viewport / clip rect 内；Cancel 不派发命令或增加 key load，Confirm 只派发一次真实格式切换，成功后清空旧结果。该测试未建立原生窗口或访问真实系统 key。

生成器另在独立副本上走 production 明文升级 → 中文命中 / 干扰与旧版排除 → 精确导出 → 重开 → 降级 → 精确导出 / Verify；实际待执行 data / profile 摘要保持。只读预检拒绝文件摘要漂移、符号链接、非空输出、profile 身份变化、非旧格式和正文变化，六项负向副本验证通过。脚本初版把 FTS fragment ID 误当 source ID，已修正关联查询，未修改业务输入或放宽条件；新建 modal 的 headless 测试也先等待 egui 稳定帧，再测量可见边界。

完整检查、构建、冻结与未执行面见[本批准备记录](../status/2026-11-02-recall-native-preparation.md)。以上均为合成与准备证据，原生结果须在新授权后另行登记。

## 2026-11-02 macOS 原生局部结果

项目所有者在当前任务授权冻结操作单，执行前重新通过 181 份构建输入 / 13 份不可变文件及初始库预检。基线为 `dev / 3f05a13` 加本批两个 test-only 源文件修改，默认 binary SHA-256 为 `f7835c42073894e3d031284a373442eec95d338a5a19c6d41a0debcb116db884`；包装与默认构建逐字节一致，无重签、安装或依赖 / lockfile 变化。冻结 manifest SHA-256 为 `82ac24790ba0cc04936990d35db52ba11e0f93931636a58d884e157276f2d364`，未因运行状态变化而改写。

### 实际入口与结果

| 范围 | 实际结果 | 限制 |
| --- | --- | --- |
| N01 明文旧库升级 | Legacy 下 ASCII 当前词仅 A v2、中文蓝色与旧 ASCII 词均 0；升级 Cancel 保持，Confirm 后 Han / 清空旧结果；蓝色仅 A v2、红色 0、主题为 A v2 / B v1；A v2 精确导出、Verify、正常退出 0 | v6、3 inline bodies 与全部既有 canonical / audit rows 保持；升级后未在 GUI 重跑旧 ASCII 词，不以升级前的零命中替代 |
| N02 重开与加密 | 明文重开保持 Han / 蓝色命中；降级 Cancel 保持，Confirm 后明确提示中文短词限制、正文 / body-version 不变、不恢复备份，清空旧结果，蓝色 0 / ASCII 当前词 1；精确导出、Verify；一次 Prepare / Migrate，显式 OpenEncrypted 保持 Legacy；加密升级 Cancel / Confirm、蓝色 / 红色 / 主题、第三份导出和 Verify 通过，正常退出 0 | 已到 v8 objects_ready，3 references、3 retired migration attempts、0 inline bodies；canonical / source IDs 保持；FTS 仍有完整明文 |
| N03 加密重开与降级 | 默认明文入口真实失败关闭且无来源 / 正文快照；显式 OpenEncrypted 保持 Han，蓝色命中 A v2；降级 Cancel 保持，Confirm 后清空旧结果并提示限制，蓝色 0 / ASCII 当前词 1；第四份精确导出和 Verify，正常退出 0 | 默认错误实际为 `LocalLibrary / ApplicationFailed · OpenLibrary / Storage / StorageFailure`，并非专门的迁移引导；拒绝 v8 是既有入口边界，不代表格式丢失，也不记作错误体验问题已修复 |
| 中文显示 | 三个实际窗口中的中文文件标题、蓝色 / 红色 / 主题输入、简繁中文 / API / 标点正文可读，结果 source ID / v2 / bytes 0..103 可核对 | 仅 macOS 已观察尺寸和合成两来源；独立人工标签、完整 corpus 和其它平台仍待验 |
| 最小窗口 | 两次角 / 右边缘拖动后捕获仍为 2160×1504 像素，未取得原生 content rect / scale，也未实际缩小 | **760×520 原生验收未覆盖**；不以截图像素、AX 控件存在或 headless 通过替代 |

四份导出各使用唯一、事先不存在的目标，全部与 A v2 原件逐字节一致：103 bytes，SHA-256 `9bc5a0a5d99e8923df9e4a16f16def6210d05ec60f1192d8e664a1bcb22b4509`，保留 UTF-8 BOM / CRLF。最后两次加密 Verify 均报告 3 objects、0 pending captures、abandonment pending=false、0 deletion pending objects、derivations rebuilt=false；两个明文 Verify 均报告 canonical facts / recall verified。

### 启动、交互与预算记录

首次在工作区沙箱读取进程清单失败，后续启动仍被错误执行，进程约 0.1 秒后退出 `-6`，stdout / stderr 为空；未作业务点击，初始数据库字节不变。这是执行顺序错误，不记作产品 GUI 失败通过。随后在已授权目标范围内以系统访问复验零进程 / 句柄后执行 N01 / N02。失败尝试计入三次启动预算，两轮退出后停止新增启动；项目所有者再明确追加一次、只完成既定 N03，才执行第三个原生窗口。

三轮可见窗口为 UTC 13:41:42–13:48:00、13:50:08–13:54:22、13:57:03–13:58:40，均串行且退出 0，实际累计约 12 分 11 秒。总启动尝试 4 次（1 次失败 + 3 次可见），已由原授权及追加一次授权覆盖。Prepare / Migrate 各 1，OpenEncrypted 2，升级 / 降级 Confirm 各 2，格式 Cancel 4，Export / Verify 各 4。Search 按保守口径记 18 次，包含一次 AX 输入未生效的尝试；未从该尝试推断通过，后续用截图坐标完成实际输入并读回。没有观察到范围外业务动作。

全程未出现可见系统凭据提示，没有选择永久允许或代输凭据；无系统调用 trace，不能从无提示推出 key API 为零或历史重复提示已解决。新 slot 由唯一 Prepare 建立并供后续读取，未另行查询 secret / 修改 ACL / 删除 key。保存对话框首次目录粘贴未生效，执行 Save 前改用原生字段 setValue 并读回唯一 export 目录及文件名；四份实际产物均在授权目录，无覆盖或目录外导出。

### 只读收尾与未验证面

最终库为 v8 / Legacy / objects_ready、3 references、3 retired attempts、0 inline bodies；全部初始 canonical / audit rows 与 source IDs 不变，三个对象文件相对 N02 的 SHA-256 不变。SQLite integrity / foreign-key check 通过，profile / 三输入 / app / 源码冻结摘要保持，四份导出逐字节通过；没有拿初始 v6 或预演库盖回。确认本批 GUI / 数据库句柄均为零，所有合成库、输入、导出、包装、证据和唯一新 key 保留，不声明迁移回滚、清除或整个库已加密。

运行证据保留在本批忽略目录的 native-evidence，包括原始退出记录、AX / 截图、三个阶段快照、导出摘要和最终进程核对。50 份窗口截图实际为 JPEG；早期 16 份沿用了 `.png` 文件名，原字节保留，尺寸以 `sips` 解码的 2160×1504 为准。初轮收尾脚本曾假定 PNG header 和迁移中间态 referenced，分别经实际图像解码与正式迁移状态机核对后修正为 JPEG / retired；没有改动库、对象或判据来通过检查。

明文 / 加密的有限格式切换、真实中文命中、重开与四份精确导出已有本批证据。760×520 内容尺寸、Han 升级后旧 ASCII 词的 GUI 重跑、Windows / Linux 新字体 / 切换、真实 key 首尾取消及其它失败窗口仍待验；独立人工标签、Q04 后续规模预算、Q02 / Q03 / Q06 和完整 Q01 / P1-S05 不因本批关闭。实际检查、Git 与保留范围见[执行记录](../status/2026-11-02-recall-native.md)。
