# P1-S05 隔离原生验收计划

日期：2026-10-06。状态：隔离入口已实现，macOS 最小正常链路、派生损坏重启恢复与密文损坏拒绝已获授权并完成；其余恢复 / 失败扩展与集中跨平台检查仍待执行。实际范围见[宿主证据](../implementation/phase1-source-vault-host.md#2026-10-06-macos-最小真实闭环)，本计划不等于全部场景已通过。

本计划承接[桌面宿主接入](../implementation/phase1-source-vault-host.md)与[ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)。先验收 macOS 当前代码，再进入 Windows / Linux 集中检查点；合成 provider、headless 测试和历史平台证据分别保留边界。

## 已实现的隔离入口

正常无参数启动继续使用默认目录与 SQLite v6。原生验收使用同一个默认构建产物与 production worker / provider，只显式选择另一目录和既有 host profile：

```bash
./target/debug/radishmemory-desktop \
  --acceptance-root "$ACCEPTANCE_DATA" \
  --acceptance-namespace "$ACCEPTANCE_NAMESPACE" \
  --acceptance-device "$ACCEPTANCE_DEVICE"
```

三个参数必须各出现一次，顺序不限；未知、重复、缺值、不完整组合和不合法身份均拒绝，不回退默认启动。namespace / device 沿用 `namespace-` / `device-` 加 32 位小写 hex，profile 沿用 `radishmemory.phase1-host-profile/1`，不增加 canonical schema、key slot 映射或依赖。

- `ACCEPTANCE_DATA` 是预建的绝对目录；拒绝父目录遍历、目录或祖先符号链接，以及默认资料库目录的自身、子目录和祖先。默认路径尚不存在时，也解析最近存在的祖先以排除别名重叠。
- Unix 验收根必须没有 group / other 权限，准备时使用 `0700`；入口不自动改权限。Windows ACL 与 reparse 行为仍须平台验证。
- `host-profile-v1.txt` 必须已经存在、格式合法且与两个显式身份完全相同；不自动生成、修复或替换。数据库文件的符号链接 / 非普通文件也拒绝。
- 参数解析、worker 创建及每条命令执行前复验目录和 profile；重开、原请求重试、failed-open 维护复用同一目标。profile 丢失或换身份时隐藏旧视图并失败关闭，恢复原 profile 后才可继续。
- 原生 UI 工具绑定后先检查窗口身份；正常关闭后通过原进程退出状态确认，不再查询已关闭应用的界面，以免工具重新启动无参数入口。发现默认窗口立即正常关闭并记录副作用。
- 窗口标题和界面标记 Synthetic acceptance；不显示路径、namespace、device 或 slot。无输出 logger / panic 策略提前到原生窗口创建之前安装，worker 内仍复验安装结果。

这提供显式目标绑定与运行前检查，不是抵抗同用户恶意并发替换、硬链接别名或所有 OS 文件别名的文件系统沙箱；不扩张已有 adapter 的保证。文件选择器仍沿用一次性选文件授权，验收操作只选择本任务的合成文件。不能复制日常库 profile 作为“合成”身份。

## 准备与授权材料

准备阶段只在本任务专用、已忽略的目录创建合成材料，不启动 GUI，不访问系统凭据。每批建立独立的 `data`、`input`、`export`、`evidence` 目录，创建随机合成 namespace / device 并写入既有 profile；`data` 中不预置数据库或 key。输入至少包含一个带 ASCII 检索词的 UTF-8 / 中文 Markdown 版本及其更正版本；中文分词质量不因 ASCII 找回通过而关闭。

运行前在仓库外或已忽略的任务操作单填写并核对：

| 项目 | 必须明确的内容 |
| --- | --- |
| 精确目标 | 实际绝对目录、输入 / 导出文件、namespace、device、二进制路径与摘要 |
| 系统 slot | macOS User domain 默认 keychain；generic password；service `io.github.laugh0608.RadishMemory.source-vault`；account `v1:<namespace>:<device>`；label `RadishMemory Source Vault key` |
| 执行动作 | 启动 / 关闭 / 重开本次 GUI；只查询该 slot 是否已存在；经 application 初始化协调器创建或复用、设置固定 label、读回验证；不直接注入 secret |
| 预期影响 | 写入隔离 SQLite、对象 / staging、合成导出和一个系统凭据；可能弹出 Keychain 授权提示，需要用户处理；无服务端、端口、网络、同步或真实资料 |
| 时长 | 最小闭环预计 30–45 分钟；系统提示、失败定位和恢复扩展另按实际评估 |
| 收尾 | 正常退出本次窗口并等待 worker join；保留隔离目录、证据和 slot 以便复验。失败同样保留，不自动删 key 或对象，不停止用户原有进程 |

随机身份降低碰撞概率但不证明 slot 空闲。授权后先精确检查；如出现非本批已记录的既有凭据，停止初始化，不复用或覆盖。不得输出 secret，系统工具不得使用显示密码的选项，不收集全 keychain dump。

当前基础授权范围不含删除 key、锁定整个 keychain、修改默认 keychain / ACL、系统设置、签名安装或远程操作。关闭进程能停止本批运行，不能撤销已发生的迁移或凭据创建；如需完全清理，先确认合成库不再需要，再单独明确授权精确删除 slot 与任务目录，不能将销毁 key 称为普通回滚。

## macOS 验收顺序与判据

| 顺序 | 操作 | 观察与通过条件 |
| --- | --- | --- |
| 1 | 精确 slot 预检后，带完整参数启动 | Synthetic acceptance 标识可见；隔离根创建 v6；profile 未被替换；无真实库访问 |
| 2 | 导入初始合成 Markdown，检索 ASCII 标记 | 可浏览、回源；记录初始来源 / 版本；不将该结果当作中文检索缺口已修复 |
| 3 | Prepare key → Migrate / resume bodies → Open encrypted objects | 每步按 UI 确认，系统提示由用户处理；创建 / 复用同一 slot，成功认证打开迁移资料；仍显示 FTS 完整明文边界 |
| 4 | 加密导入另一合成文件、更新既有来源、检索、导出历史版本 | 新版本关系正确；旧版导出与原始合成 bytes 精确一致；不重读外部原件冒充历史导出 |
| 5 | 关闭重开并显式 Open encrypted objects | 启动时旧入口拒绝已迁移库属预期；显式加密打开成功；原 profile、slot、版本与正文可复验，无新 key |
| 6 | Verify、显式删除本批来源、查看 evidence | 按真实组件结果记录 completed / partial / failed；删除后目录 / 搜索不再召回；原输入和此前导出仍在 |
| 7 | 正常退出、核对诊断与产物 | 本批 worker 退出；仅保留约定目标；诊断不含合成正文、身份或 secret；记录原生 stderr 与系统提示的实际观察范围 |

验收证据记录精确构建、场景、操作顺序、真实结果、合成文件摘要、是否产生系统提示及清理 / 保留状态；不记录 secret，不用“没有看到提示”替代取消 / 拒绝测试。日志无泄露检查不等于系统崩溃报告、交换区或第三方内存已清理。

## 后续恢复和失败扩展

按可安全构造的状态单独安排：busy 下重复动作、关闭时原快照提示、提交后刷新失败、原请求重试、重启精确放弃、原删除授权继续、failed-open 派生维护、多实例锁，以及真实提示的取消 / 拒绝。需要故障 seam 的场景必须标记为合成注入，不将默认 production 构建无法构造的状态记为通过。

2026-10-06 已按下表完成受损库重启后的两条边界，实际结果见[恢复扩展记录](../implementation/phase1-source-vault-host.md#2026-10-06-受损库重启与恢复扩展)。该批次复用已验收的合成 profile 和唯一 slot，不复制日常资料、不创建新 key。GUI 正常关闭后才注入状态；每次重开保留完整验收参数并核对窗口身份。具体路径、摘要、SQL、时长、备份及保留方式写入本批操作单，并对新增 GUI / 故障范围取得授权。

| 场景 | 构造及观察 | 通过条件 |
| --- | --- | --- |
| 派生损坏后显式恢复 | 经真实 GUI 导入一份合成资料并精确导出，关闭后仅删除 lineage tip 派生行并替换 FTS 内容；重启后依次打开、Verify、Rebuild、再打开 | 打开与 Verify 不隐式修复；显式 Rebuild 后目录 / 搜索恢复，导出 bytes 相同，canonical 表与密文对象摘要未变 |
| 密文损坏失败关闭 | 保存同一合成对象的原字节及摘要，关闭后翻转最后一个字节；重启后打开、Verify 与 Rebuild | 全部拒绝且不显示正文，不改写损坏对象或创建 key；关闭后精确还原本次备份，再打开和导出成功。任务恢复字节不计作产品具备自动密文修复 |

桌面 worker 另以 opt-in 合成 provider 覆盖对象缺失及字节损坏后的重启：旧入口、显式打开、Verify、Rebuild、Search 和 Export 失败关闭，外部原件更改也不回源；只恢复测试原字节后才可恢复原 source 与精确导出。该回归不访问系统 key store，不代替上表真实 GUI 证据。

缺 key、锁定、duplicate / ambiguity、真实中断需要额外对象库与精确系统操作方案；本批不通过删除唯一 key、锁定用户整个 keychain 或改系统配置制造故障。不能安全复现的项继续待验。

macOS 闭环完成后再按当前代码准备 Windows / Linux locked 构建、合成运行与真实 provider / 宿主验收，分别记录架构、文件系统、普通用户权限、系统 store 和诊断证据。`evidence_digest` 统一复算仍独立评审；PDF / 图片解析、默认加密切换、模型和同步不在此计划内。
