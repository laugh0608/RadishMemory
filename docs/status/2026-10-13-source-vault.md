# 2026-10-13 Source Vault 日终记录与明日事项

日期：2026-10-13（Asia/Shanghai）。按本地所有 refs 的当天提交时间回顾，日终文档提交前共 8 个提交；本页随后单独提交。现行阶段、顺位和停止线以[当前状态](current.md)为准。今晚到文档提交为止，不安排继续开发或自动运行。

## 今天的全部提交

| 提交 | 实际变更 | 证据与边界 |
| --- | --- | --- |
| `f37078d` | 修复 busy 期间关闭、完成结果与关闭同帧到达，以及接受关闭后仍派发按钮动作的竞争 | 六项 headless UI 与一项真实 worker Drop / join 回归；全仓 Rust tests 增至 366。原生关闭竞争尚未验收 |
| `4ecd775` | 记录首次空合成库打开的重复密钥提示与收尾 | 用户仅允许本次访问仍反复提示，最终打开成功；正常退出、数据 / profile / 权限保持。没有进入原计划后续验收 |
| `89d53b0` | 合并 OpenEncrypted 与显式 Refresh 的目录、版本和恢复检查 | 首尾两次 key load、每次双读，保留 key 一致性与最终对象 / 数据库复验；无跨动作缓存，其它动作和写后刷新不变。新增 13 项回归，全仓 379 项 |
| `1cf3931` | 记录新会话入口取消与提示超限 | 调用栈确认入口阶段，拒绝后返回 `KeyStoreCancelled`；另一次打开用户报告 9 次提示，超过 8 次上限后停止，正常退出，基线保持。未覆盖末轮取消或关闭竞争 |
| `dffb40d` | 分析九次提示并限定逐提示观测 | 静态路径有四次 secret、十二次 metadata 查询，API 次数不等于系统提示数；诊断提案本身不构成运行证据 |
| `b07f861` | 记录旧 API 单次打开的八提示逐次定位 | 四个 secret 静态调用位置各对应两个提示，均停在 `SecKeychainFindGenericPassword`；未观察到 metadata 或第九次提示，旧九次记录不删除 |
| `bf8dddf` | macOS secret 读取切换 `SecItemCopyMatching` data-only 查询 | 精确 keychain / class / service / account、四次 secret 与全部 metadata / label 检查保留；合成测试验证错误分类、畸形结果拒绝及 Rust data buffer 清理，新增 7 项回归，全仓 386 项。没有修改依赖或密钥权限 |
| `8393cc0` | 记录 SecItem 候选单次原生打开仍八次提示 | 八次全部定位新 API，四个 secret 静态位置各两个；正常打开并退出 0，基线保持。API 变更未在本次观测中减少提示，系统内部原因仍未确定 |

按项目所有者要求先提交四份原生观测记录为 `8393cc0`，再进行本轮代码与文档复核。此时 `dev` 工作区干净，领先本地 `origin/dev` 9 个提交，其中包含先前日期的提交；当天计数不等于 ahead 计数。未刷新远程状态，tracking ref 不代表实时远程结果。本日终只作本地文档提交，没有 push、PR、tag、发布或部署。

## 代码与文档复核

| 核对代码 / 证据 | 结论与文档处理 |
| --- | --- |
| `ui.rs`、`ui/close_tests.rs` 与 worker 生命周期回归 | UI 在 poll 前保留 busy 状态，同帧多次 layout 仍取消关闭；真实完成结果先展示，接受关闭后不再派发动作。宿主与验收矩阵已区分 headless、真实 worker 和原生窗口，架构补充该区分 |
| controller / worker、application `encrypted.rs`、reader `snapshot.rs` 与 provider | 仅显式加密打开 / Refresh 使用固定 snapshot 入口，同一 store 身份观察下首尾双读；最终 key / 对象 / 数据库检查通过后才返回完整快照。失败撤回旧视图和恢复选择，保留原业务请求。application、reader 和隐私会话契约与实现一致，无需扩大到其它动作 |
| macOS provider 及合成 tests | secret 使用单项 data-only search，前后 attributes 检查保留；不把畸形成功响应当 missing，不回退旧读取，不重试，Rust data buffers 有清理保护。现有 provider 文档准确；不能将其扩大为 OS / CF 内部副本全部清除 |
| 八提示的新旧逐次定位及较早九次汇总 | 两轮八次均定位四个 secret 静态位置；新轮明确执行 SecItem，系统内部调用旧 Keychain 组件不等于应用 fallback。没有 OS 调用 begin/end 实例证据，不确定内部两次授权因果，也未解释更早第九次；保持原记录与限制 |
| 架构中的执行方式和维护范围 | “桌面同步执行”已过期，改为单一串行后台 worker，并保留全库认证成本 / 持锁限制。澄清显式加密 failed-open 维护已接入且有有限原生证据，默认 v6 仍有缺口；补只读会话边界与提示未改善状态 |
| 隐私模型、路线图、当前状态与索引 | 隐私模型“本批仅合成证据”更新为会话入口取消 / 空库正常打开的有限原生结果；不声称末轮取消、有正文或其它平台通过。路线图补当前检查点，当前状态压缩当天流水并写明后续顺位，索引加入本页 |

今天有三项生产行为修改；本日终不再改代码、测试或检查器。canonical schema、密码 profile、数据所有权、记忆语义、信任模式、依赖与许可证均未改变，产品范围、记忆模型、RadishMind 边界和 ADR 0008 的长期决定无需人为改写。本次是基于当天差异与对应文档的复核，不是独立密码学审计或完整产品复审。默认桌面仍为 v6；显式原始对象加密不覆盖 SQLite / FTS 完整明文，PDF / 图片保持原停止线。

## 原生证据与环境交接

- 各次原生动作均在对应精确授权范围执行；首次重复提示、入口取消、九次超限、旧 API 八次逐项定位及新 API 八次逐项定位分别保留，不能合并成所有场景通过。
- 新 API 观测只启动一个指定 GUI、点击一次 OpenEncrypted；八个提示由用户逐次确认目标并保持待选，对同一 GUI PID 作限定 sample，再交接 Allow Once。第八次后无新提示，打开成功、零活动来源；没有第九次拒绝或末轮取消证据。
- 新轮八个 sample 和 GUI 均正常退出，GUI 退出码 0、stdout / stderr 为空。数据库 / profile 摘要、五项活动表、pending capture / deletion、objects / staging、目录身份、0700 权限与根目录清单均等于预检；未修改 key / ACL，未强杀或进行其它业务操作。
- 34 份新轮本地证据已建立 SHA-256 索引，逐提示台账、操作单实际结果、包装与 dSYM 一并保留。原始栈、实际本机路径和测试 key 不进入 Git。保留忽略目录中的旧合成输入、导出、备份、包装和全部历史证据；前批误加的 Prompt 常许许可已撤销，本日没有以永久许可绕过提示。
- 本日 GUI 与采样进程已退出；日终不重新启动 GUI、访问系统凭据或清理保留产物。详尽过程见[宿主记录](../implementation/phase1-source-vault-host.md)，候选与局限见[provider 记录](../implementation/phase1-source-vault-key-provider.md)。P1-S05 及全部原生失败窗口仍未关闭。

## 验证

当日代码批次完整离线检查依次通过 366、379、386 项 Rust tests，数量为递进基线，不相加。SecItem 候选默认 desktop 构建通过；原生记录提交前的完整检查通过 264 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，以及 25 个 suites 共 386 项 Rust tests；11 个 ignored helper 由父测试调用。合成本地端口限制已按权限流程处理，不通过跳过检查规避。

日终文档修改后，`CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整退出 0：265 个仓库文件、notices、fmt、locked Clippy 和 25 个 suites 共 386 项 Rust tests 通过，11 个 ignored helper 由父测试调用。按已确认的合成本地端口限制在沙箱外离线运行，日志保留于任务临时文件；补记后再次执行文本 / 链接检查与 `git diff --check`。未重复未受影响的默认构建、Python 检查器回归、M0 fixture、doctest、GUI、系统密钥库、Windows / Linux、VM 或远程验收；不以文档更新扩大历史证据。

## 明日事项（2026-10-14）

1. **复核基线并限定重复提示诊断**：先看 Git 状态、本页和最新逐提示台账，确认保留的测试 slot、冻结二进制 / dSYM 与源代码对应。已有新 API 的八次结果足以说明本次未改善；先从既有证据明确下一项假设、需要区分的调用 / 授权阶段及可取得的最小证据，再形成有限方案，不直接继续换 API 或重复 GUI 试错。
2. **保持认证约束与授权范围**：四次 secret、metadata / label、首尾 key 一致性及最终对象 / 数据库复验不因提示多而删除；不提高八次预算、不跨动作缓存、不采用永久许可。若新方案需要运行态观测、临时诊断或生产实现，先说明精确影响、可观察判据、停止和收尾方式，按所需范围确认；旧操作单不自动延续。
3. **按可控切片补原生会话 / 关闭窗口**：入口取消已有有限证据，末轮取消、Refresh、有正文旧视图撤回及 busy 关闭仍待验。沿用[验收矩阵](../evaluation/phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)区分合成与原生；成功写入后刷新失败不重复写入，提交 / evidence 回调错误不冒充进程中断。无法安全构造的场景保留待验。
4. **形成有限跨平台检查点**：按已定义条件推进当前 Windows / Linux locked 编译与合成测试，再分别限定真实 provider / 宿主范围；不因不能穷尽 macOS 故障而无限扩大前置工作，也不把历史 filesystem 结果视为当前 provider 通过。
5. **保留独立产品质量与契约工作**：R01 至 R06、中文显示、目录分页、默认 v6 failed-open 仍待收口；字体引入须先说明来源 / 许可证，`evidence_digest` 复算与历史兼容先独立评审。已通过的权限故障恢复、外部锁失败恢复和双 GUI 交替不重复开发。

以上仅为后续建议，不创建提醒、定时任务或后台开发；新增系统操作、依赖、远程动作与彻底清理仍按精确范围授权。
