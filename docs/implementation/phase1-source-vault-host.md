# P1-S05 macOS 宿主接入准备

状态：`P1-S05 host recovery preparation — dependency approval pending`（2026-10-01）。

项目所有者已授权继续 macOS 宿主接入。本批先完成现有依赖范围内的恢复接口；桌面依赖连线、实际 UI 接入与真实平台验收分别记录，不把准备工作视为宿主已经切换。

## 当前实现

application 已复用既有 Source Vault 精确放弃能力，提供 `inspect_capture_abandonment` 与 `abandon_capture`。前者只认证并选择不透明 target，不授权删除；后者必须在宿主获得针对该 target 的明确确认后调用，继续复验原 request / attempt / locator 与文件身份。不清理未知文件、不删除已提交来源、不修改 canonical 删除语义。location 和打开后的 library 均可调用；未完成的派生修复仍须先明确执行。

`unfinished_delete_requests` 在认证 reader 内查询同 namespace 尚无 Completed 最新 evidence 的既有请求，包括已经执行完但尚未保存 evidence 的窗口。查询先通过既有 loader 复验最新 evidence 的身份、时间、计划及真实组件结果，再排除 Completed，不能只相信状态字段。它返回原 canonical `DeleteRequest`，只发现状态，不生成、扩大或自动执行请求。恢复不依赖 host 另存 request ID；后续执行仍校验原 device、授权和冻结计划。

两项能力均不新增数据库 schema、依赖、明文 journal 或长期格式。`SourceCapture`、`DeleteRequest` 作为已有 application 方法的 canonical 输入 / 输出类型由 application 公开导出，宿主不需要另建请求结构。

## 待接入的宿主流程

1. 现有默认 v6 入口保留；提供显式加密打开、首次准备与迁移恢复动作。每个动作保留既有 namespace / device。迁移前关闭旧连接；打开失败不自动初始化 key、降级读取或新建空库。
2. 存储、全库认证、迁移与系统 provider 操作移入单一串行 worker，界面只发送用户动作并接收状态。系统文件选择器保留一次性文件授权；路径消费后不写入 host state 或持久诊断。
3. worker 在执行 capture 前持有完整原请求。提交返回失败时保留它并显示“重试原请求”；目录刷新失败须与提交失败区分，不能因刷新错误再次导入。新变更和重复点击在待恢复操作结束前被阻止。
4. 进程重启后，已提交 capture 通过目录读取恢复可见；未提交 capture 若原快照丢失，不能从 fingerprint 或外部文件伪造精确重试。界面呈现真实 pending 状态，允许用户明确确认放弃当前精确 target，完成后才允许新导入。本批方案不添加磁盘快照 journal，不声称跨进程保存了原明文请求。
5. 删除失败保留原请求，重启后通过数据库发现原授权，用户显式继续；操作完成但 evidence 未保存时沿原请求补全真实执行 / evidence，不根据当前选择重新规划。Failed / Partial / Completed 仍分别展示。
6. 普通打开失败后保留原位置 / profile 的维护入口；verify 不修复，rebuild 仅修复已认证 canonical 的派生行。未知文件、对象缺失、错 key 与 canonical 损坏继续失败关闭。
7. 显示稳定有界的 key missing、locked、denied、cancelled、authentication 与 database 原因。访问真实 provider 前安装经验证的日志抑制策略；不得输出路径、slot identity、原请求正文、密钥或上游诊断。

关闭期间不能静默丢弃待重试的内存快照；界面应提示关闭会失去当前快照及其后续恢复限制。worker 的 busy、错误和停止状态必须真实，不能通过空列表或成功通知掩盖任务尚在执行、线程失败或读取失败。

## 待批准的依赖连线

按照 `AGENTS.md` 的依赖 / lockfile 授权规则，已请求以下精确范围，尚未修改 manifest 或 lockfile：

| 位置 | 声明 | 用途与影响 |
| --- | --- | --- |
| desktop runtime | `radishmemory-source-vault.workspace = true`，workspace `=0.1.0` | 直接引用已构建可达的 sealed provider 和有界错误，不新增第三方 package |
| desktop runtime / workspace | `log = "=0.4.34"`，沿用 lockfile 版本 | 在系统凭据调用前建立可测试的日志抑制策略；现有 MIT / Apache-2.0 依赖，不初始化文件、网络或 telemetry sink |
| desktop dev-dependency | 既有 Source Vault `acceptance-test-support` | 使用固定合成 key 驱动同一 worker / controller 流程；默认生产构建不启用 |

批准后通过离线 Cargo 解析维护 lockfile 依赖边，逐项复核第三方 name / version / source / checksum 集合不变；同步依赖基线、notices 文件摘要和精确 manifest 检查。预计为分钟级本地解析与构建，不访问真实 Keychain，不启动 GUI，不改系统或远程状态。撤回本批声明及调用可恢复旧构建关系；不引入新许可或上游版本升级。

诊断检查器当前禁止所有未审阅日志入口。日志策略落地时须为精确抑制实现建立可复验的检查和回归，保留其余源文件的禁止规则，不能通过关闭或泛化豁免绕过正文 / 凭据日志门禁。

## 验收与未完成范围

application 合成测试覆盖：删除 intent 或 execution 后重启发现同一原请求；Completed 后不再列为未完成；丢失 capture 请求后只观察状态，不自动清理；显式放弃后保留旧来源并拒绝原请求复活；其它库 target 和未知文件拒绝且保持原文件；Completed evidence 的 device 与请求不符时返回错误，不能隐藏成无待恢复任务。

复核另发现既有 `DeletionEvidence` loader 只检查 `evidence_digest` profile / 格式，不重算该摘要；历史 fixture 使用的摘要内容也不统一。本批不改变既有摘要语义或兼容性，不把该字段当作数据库未被恶意改写的密码学证明。摘要契约、历史兼容和统一复算需要独立收口；本批恢复判断依据经复验的请求、计划与持久化组件结果。

最终 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 在沙箱外完整通过：245 个仓库文件、notices、fmt、locked all-targets / all-features Clippy、25 个 Rust test suites 共 334 项通过，10 个 ignored helper 由父测试调用。本批新增 4 项 application 回归；42 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 与 `git diff --check` 通过。首次完整检查仅在既有 P1-F17 临时端口处被沙箱拒绝，经授权重跑；摘要篡改负例暴露上述既有摘要复算缺口，最终身份不符负例验证现有 canonical 契约，未扩大摘要保证。

测试只使用合成资料和固定测试 key，并清理自身隔离目录、子进程和临时端口；没有新增后台服务。验证日志保留在仓库外任务临时文件中。当前尚未完成 worker、controller / UI 接线、关闭提示或日志策略；默认桌面仍使用 SQLite v6 inline plaintext body。真实 Keychain、GUI、用户授权提示、取消 / 锁定、多实例和端到端验收未执行；Windows / Linux 保持后置集中检查点。

原始正文对象加密不等于整个资料库静态加密，FTS 仍保存完整可读正文。真实系统访问、桌面启动、slot 范围及测试后的清理另行说明并取得授权。
