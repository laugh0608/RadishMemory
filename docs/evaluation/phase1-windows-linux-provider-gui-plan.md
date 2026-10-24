# P1-S05 Windows / Linux 真实 provider 与 GUI 验收方案

日期：2026-10-24。状态：**Windows / Linux ARM64 均完成两轮真实 provider / GUI 正常链路、重登、精确导出与应用层删除；完整 P1-S05 仍待收口**。静态准备已提交为 `41663d7`，随后两平台分别取得当前任务的精确授权；实际结果及异常见文末。本文保留原执行方案，不改变 provider、profile、记忆或删除契约；先前构建和临时 ACL 账户授权不沿用。

## 已有证据与本轮产物

Linux / Windows ARM64 默认构建与合成测试已分别通过；Windows 另有 reparse 与普通用户 ACL 证据。Linux 构建来自 `c30b310`，Windows 来自 `da4cd4f`，与准备提交 `6d164a8` 的全部非文档输入一致。候选产物仍在各自客体的上一批目录，本轮不启动 VM 取回或重新声称已经核对客体当前字节；执行时必须先复算长度与 SHA-256，再将精确产物复制到新任务 `bin`。

| 平台 | 已记录候选默认产物 | SHA-256 |
| --- | --- | --- |
| Windows ARM64 PE | 36,832,768 bytes，debug | `9b74a671bca32de4c57acc10e8a42486a398ba8a743833bbf3add04d40883bec` |
| Linux AArch64 ELF | 358,504,336 bytes，debug | `3a03ca92d4ec0fc268589cb82a59f7307ead6cc7a2599f9bfcc835a7cc823a2c` |

本机忽略目录已准备两份不同的随机合成 namespace / device、合法 `host-profile-v1.txt`、每平台三份合成 Markdown 和五项导出比对目标，并冻结摘要。没有预置 SQLite、KEK 或账户密码，没有传输至客体。精确 VM UUID / bundle、账户名、绝对任务根、slot、启动 argv 与文件摘要只在本机操作单保存；这份清单是批次材料，不是新增产品 schema。

## 执行目标与会话

拟先 Windows，正常关机并核对完整清单全停后再 Linux。全机所有项目合计最多运行一台 VM；每次启动前、启动后、阶段切换、运行监控和关机后核对完整清单。未知 / 过渡状态、其它 VM 运行、查询或正常关机失败均停止后续启动；不擅自停止其它 VM，不强制断电。

| 项目 | Windows | Linux |
| --- | --- | --- |
| 目标 VM | 已核对的日常 Windows11-ARM64 | 已核对的日常 Debian13-ARM64；不使用 DependencyFrozen 或 CleanBase |
| 拟新建系统账户 | `rm_p1s05_gui`，本地普通账户，仅 Users | `rm_p1s05_gui`，新普通 UID / GID，无 sudo、无附加特权组 |
| 身份门禁 | 创建前确认同名账户不存在；记录新 SID；实际 GUI token 非管理员、非 SYSTEM、medium integrity | 创建前确认同名账户和拟用 home 不存在；记录新 UID / GID，非 root、`CapEff=0` |
| 必需会话 | 用户交互式登录，加载该账户 profile，GUI 与凭据操作位于同一用户会话 | 该账户通过现有登录管理器进入桌面，GUI、用户 D-Bus、Secret Service、系统 picker 同属此 UID |
| 不复用 | 已删除的 ACL 测试账户、SYSTEM session 0、S4U / 无 profile 的合成测试进程、现有管理员账户 | 旧普通用户的资料 / keyring、root 会话、临时 `dbus-run-session` 或从其它用户拼接的 bus 地址 |
| 创建影响 | 新账户、profile、用户级 DPAPI / Credential Manager 状态及任务目录 ACL | 新账户 / home、桌面配置、用户 runtime 与可能的 login keyring / default collection |

同名账户或目标目录已存在即停止，不接管、删除或自动换名。账户创建和首次登录本身涉及系统凭据 / profile，必须纳入执行前确认。测试账户密码由用户在客体安全输入界面设置和输入，不在聊天、命令行、操作单、截图或日志中交接；不请求既有账户密码，不设置空密码、自动登录或持久化提权。

本批普通账户方案不改变 Windows UAC、全局登录权限、系统 key store、默认真实用户 keyring 或全局工具。需要新增依赖、安装 GUI / picker、改变 PAM / 会话配置时停止并单列范围。桌面会话目前未验证成立，不能因系统已装 GNOME / keyring 或已有 ARM64 二进制而跳过这一门禁。

## 测试库、文件与启动

每台采用独立任务根，内含 `bin`、`data`、`input`、`export`、`evidence`；只复制上述冻结 binary 和本平台冻结输入。`data` 初始只有本平台 profile，数据库与 key 均由批准后的真实流程创建；不复制旧 v6 / 加密库、历史 profile 或旧凭据。Linux 私有目录为 `0700`；Windows 任务 ACL 只赋予该测试 SID 业务目录所需权限及 SYSTEM / Administrators 管理权限，不改旧构建根或其它用户目录 ACL。

启动严格使用已存在的三参数入口：`--acceptance-root <data> --acceptance-namespace <namespace> --acceptance-device <device>`。每轮复验路径 / 祖先无链接或 reparse、与默认资料库不重叠、profile 字节与显式身份一致；拒绝时不无参数重启。不使用 macOS 专用 `--acceptance-key-read-trace`。先核对 Synthetic acceptance 窗口和实际 PID / 身份，再执行动作。

应用目录通过显式参数隔离，**不再套用离线构建的空 HOME / USERPROFILE 环境**。Windows 保留该登录账户真实 profile / APPDATA；Linux 保留同一会话真实 HOME、XDG runtime / display 和用户 bus。只按清单隔离本批输出与临时文件，不伪造会话来获得 provider 成功。默认无输出 logger / panic 策略保持；不得为排障打开 keyring debug / trace、抓取进程内存或打印完整环境。

每个平台的输入为：带 BOM / CRLF 的初始中文 Markdown、保留相同 ASCII 搜索标记的 LF 更正版、独立对照来源。固定五项导出分别对照初始版本、更正版、重登后的两个版本，以及主来源删除后的对照来源。按原 bytes / SHA-256 比较，不规范化换行；ASCII 找回不关闭中文检索质量缺口。系统文件选择器只选择本批 `input` / `export` 中的精确文件。

## 凭据范围与执行前确认

系统登录账户与凭据字段 `username` / account 不是同一概念。两平台各有一个独立随机 slot；两台不共用 KEK 或 profile。实际调用前须让项目所有者确认本平台操作单中的完整身份及下列影响，不把“只读预检”排除在凭据授权之外。

| 项目 | Windows Credential Manager | Linux Secret Service |
| --- | --- | --- |
| 唯一身份 | `CRED_TYPE_GENERIC`；target 为 `io.github.laugh0608.RadishMemory/source-vault/v1/<namespace>/<device>`；username 为 `v1:<namespace>:<device>` | 精确 attributes：`service=io.github.laugh0608.RadishMemory.source-vault`、`username=v1:<namespace>:<device>`；无额外 target |
| 元数据要求 | persistence 为 `Local`；comment 为 `RadishMemory Source Vault key` | 本会话默认 collection 存在且已解锁；label 为 `RadishMemory Source Vault key`；跨 collections 搜索保留唯一性检查 |
| 首次预检 | 只读精确 target / type；仅明确 `ERROR_NOT_FOUND` 才视为空闲 | 只查询新账户本会话 default alias / locked 属性及精确 service / username 的 locked + unlocked 匹配；总数为 0 才视为空闲 |
| 允许申请的业务动作 | 经现有 application 初始化协调器创建一个 KEK、按契约写 label / comment、读回与后续认证读取 | 相同；创建发生在新测试账户的 default collection，不替换 provider 或存储语义 |
| 不纳入首批 | 直接注入 / 导出 secret、覆盖未知既有项、批量枚举 / dump、rotation、删除 key、改 persistence | secret 明文查询工具、跨用户 store、覆盖未知项、批量 unlock、删除 collection、替换已有 default alias |

Windows 的 `CredReadW` 按当前登录 token 访问用户凭据集；即使预检只记录存在性和元数据，返回结构仍可能携带 credential blob，须按凭据读取授权并限制在进程内处理，不显示或落盘。API 和精确删除语义见 [CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw) 与 [CredDeleteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-creddeletew)。

Linux 以当前冻结 `secret-service 5.2.0` / `zbus-secret-service-keyring-store 1.0.1` 和仓库 adapter 为准；[Secret Service API](https://specifications.freedesktop.org/secret-service/latest-single/)仅作会话、alias、匹配与 Item 删除的接口参考，不据此升级依赖。provider guard 遇默认 collection 缺失 / 锁定直接失败，不自动创建或解锁。若首次测试账户登录未提供可用 default collection，停止 app 初始化；只有确认范围包含该账户的 collection 准备时，才可在原本没有 default alias 的新 profile 中创建一份标记为本批的 collection 并绑定 `default`，密码由用户输入。已有 alias 不替换，归属未知则停止。解锁也只针对已绑定的本批 collection，不能解锁已有用户整个 keyring。collection 对象路径只用于当前会话复验，注销后重新绑定，不把旧 D-Bus path 当长期身份。

随机 slot 不证明空闲。首查发现匹配项、重复项、无法区分 missing 与 unavailable、元数据不符或会话身份漂移均停止，不尝试读取其它 slot。仅本批有创建回执的残留项，才可在后续明确恢复范围内复用；Prepare 失败后不自动重试、换 key 或建新库。

执行确认应覆盖：新账户 / profile 与登录影响、必要的新测试 collection 准备、该唯一 slot 的预检 / 创建 / 固定 label 写入 / 认证读取、两轮 GUI 和中间注销重登、合成库写入 / 精确导出 / 应用层来源删除、日志与证据回收，以及下节常规收尾。建议每平台预算 45–75 分钟，另留 10 分钟收尾；两台串行，不承诺无人值守。每个平台最多两次 GUI 启动、一次 Prepare；任何异常系统授权提示先交用户核对再处理，首批最多三次已确认的 store 提示（不计正常登录密码），同一动作重复提示或超预算即停止，不扩大允许范围或选择永久放行。没有提示不等于已验证取消 / 拒绝。

## GUI 操作与判据

| 步骤 | 操作 | 必须保留的实际证据 |
| --- | --- | --- |
| 1 | 完成同账户会话与精确 slot 预检，启动第一轮 GUI | 正确窗口 / PID / 身份；仅新 `data` 创建 v6，profile 保持；未启动默认库 |
| 2 | 取消一次 Import picker，再导入初始版本并检索 ASCII 标记 | 取消不创建来源；成功后可浏览真实合成正文，记录 source / version |
| 3 | 一次 Prepare key → Migrate / resume bodies → Open encrypted objects | 每步独立确认并等待真实结果；只有一个本批 slot；迁移与认证打开成功；FTS 明文边界仍清晰 |
| 4 | 导入对照来源，更新主来源，分别导出主来源初始 / 当前版本 | lineage / version 正确；导出 bytes 分别匹配冻结 v1 / v2；没有回源代替历史读取 |
| 5 | 正常关闭、等待进程 / worker 退出；注销并重登这个测试账户，再启动第二轮 | 仍是同一 SID / UID 和真实用户 profile；重开显式加密库成功，证明跨进程及本次重登后的读取；不重新 Prepare 或生成 key |
| 6 | 显式 Refresh、Search、Verify，再导出主来源两个版本 | 当前构建的原生只读动作成功；五项比对表中的重登导出一致；无旧视图冒充结果 |
| 7 | 先 Cancel 删除确认，再确认删除主 lineage；核对并导出对照来源；最后删除对照 lineage | Cancel 无删除；真实删除回执按组件记录；主来源不再召回，对照仍可认证导出；最终两条来源的真实状态均可复验 |
| 8 | 正常退出，回收审计和已批准的证据，注销测试账户并正常关机 | 进程退出、无 pending 原请求遗留或明确报告遗留；完整 VM 清单全停后才进入另一平台 |

任何步骤失败即停止后续业务动作，保留库 / slot / pending 与原始错误，按既有安全退出流程处理。不能为了拿到通过结果反复 Prepare、重建空库或删除 key。卡住或关闭失败时保留真实状态并交接，不自动 kill。GUI 不可用、picker 缺失或渲染失败不能用 CLI 导入代替原生通过。

首批不主动制造 key 丢失、损坏 secret、锁定正在使用的 collection、重复项、权限撤回、服务重启、断电或并发 GUI。Windows 无交互提示时不伪造取消 / 拒绝证据；Linux locked / unavailable 与系统取消是否能安全构造，另列后续场景。KDE / KWallet、其它文件系统及正式发行包不在本次 GNOME / Windows 范围。

## 清理、保留与证据边界

| 层次 | 默认处理 | 明确不能宣称 |
| --- | --- | --- |
| 本批应用来源 | 通过 GUI 对两条合成 lineage 执行删除，核对每个真实组件与回执；失败 / pending 如实保留 | 来源删除不销毁共享 KEK，也不删除输入、导出、profile 或系统凭据 |
| GUI / 用户会话 / VM | 正常退出、等待 worker 与测试进程结束；注销本批账户，正常关机，确认全机 VM 全停 | 不以窗口消失代替退出，不停止其它用户会话或其它 VM |
| 复验材料 | 默认保留两平台新测试账户及 profile、各自唯一 slot、任务目录和本机证据，列出实际占用与状态 | “已注销 / 已关机”不等于账户、凭据或资料已删除；本批不默认做账户回收 |
| 之前的资产 | 旧构建缓存 / 输入、历史 H05、macOS key / M01 冻结件、其它用户 profile 与 store 全部保留 | 不跨批清理，不以同名 / 同目录前缀推导所有权 |

**不可恢复清理单独确认**：先明确保留合成库将失去解密能力，选择是否结束该测试阶段；核对只存在本批目标和无待恢复请求后，在该账户仍可登录且 profile 未卸载时精确删除唯一凭据。Windows 使用 exact target + `CRED_TYPE_GENERIC`，随后精确读得到 `ERROR_NOT_FOUND`；Linux 重新按两个 attributes 查找，确认唯一 Item 及 label / 当前 collection 身份后删除该 Item，再查 locked + unlocked 均为 0。未知、重复或删除结果不明确时停止，不能批量删全部匹配项或整个 store。

凭据删除验证完成后才注销、验证本批进程结束 / profile 卸载，按已冻结 SID / UID 删除本批账户及明确归属的新 profile / home 和任务路径，复核账户、路径及授权消失；任一步失败记录残留。删除 profile 会连带销毁其 keyring / 用户配置，只能用于本批新建且未混入其它用途的账户，须在清理确认中直说这一影响。Linux 不单独删除 collection 或改变其它 alias；Windows 不用删除账户代替凭据删除成功证明。没有精确路径清单前不递归删除，保留本机已回收的非 secret 证据；不承诺清除 VM 快照、备份、交换区或系统审计历史。

证据只记录构建 / 输入 / profile 摘要、匹配数量、非敏感元数据判据、稳定错误码、动作结果、退出码和清理状态。slot / SID / 路径的完整绑定仅保留本机受限操作材料；不输出 secret、完整凭据结构、密码、内存 dump 或全 store 清单。原生 stderr 只按该次路径作泄露检查，不扩大为系统诊断全覆盖。跨登录一致导出支持实际重读成功，不等于所有中途撤销窗口通过。

## 准备时待执行门禁

账户 / 目录是否空闲、实际 SID / UID、交互式会话、picker、Linux default collection / provider、精确 slot 空闲和客体产物当前摘要尚未检查。后续先按本单取得具体执行范围确认，再串行建立这些前提；没有批准前不启动 VM、创建账户、传输或访问凭据。准备材料不是通过回执，P1-S05、M01 及 PDF / 图片停止线保持。

本轮准备验证：两份 profile 的格式契约、六份输入与归档逐项摘要、十项导出比对映射及旧产物证据绑定已静态复核；宿主完整离线 `./scripts/check-repo.sh` 退出 0，272 文件基线、fmt、locked Clippy 与 398 项测试通过（25 suites，0 failed，11 个父测试调用的隔离 helper）。记录补齐后另复核文本门禁与差异；没有把这些宿主 / 静态检查记作客体会话或真实 provider 通过。

## 2026-10-24 Windows 真实执行结果

执行绑定 `41663d7` 准备记录及上述 Windows 产物；客体复制前和两轮启动前均复算产物 / profile 摘要。新普通测试账户通过用户手动设置密码并交互式登录建立 profile，凭据预检与两轮 GUI 均在同一 SID、medium integrity `8192`、非管理员 token 下运行。第一轮为 session 2，真实注销并确认 profile 卸载后重登为 session 3。没有借用 SYSTEM 控制通道访问用户凭据；控制通道仅承担已授权的账户 / 文件准备和非 secret 证据回收。

| 判据 | 实际结果 |
| --- | --- |
| 首次精确槽位预检 | `ERROR_NOT_FOUND`，仅本批 target / type；随后只执行一次 Prepare |
| 第一轮产品动作 | Import picker Cancel 未创建来源；导入初始中文合成 Markdown、ASCII 标记检索、Prepare、迁移、显式加密打开、对照来源导入和主来源更新均完成；Verify 为 3 objects、0 pending |
| 版本与精确导出 | 初始版本 106 bytes，更正版 123 bytes；注销前后各导出一次，四份均逐字节匹配冻结输入；主来源删除后对照导出 113 bytes，同样逐字节一致 |
| 重登与读取 | 第二轮默认 legacy 打开按预期拒绝加密库；显式 Open encrypted、Refresh、Search、Verify 和两个历史版本导出成功；没有再次 Prepare |
| 删除 | 先 Cancel 保留两条来源，再删除主 lineage 的两个版本；检索仅剩对照且可认证导出；最后删除对照。两份回执均 `completed`，每份 10 个组件均 `succeeded`，实际效果分别记录 deleted / retained_minimal / not_found |
| 最终审计 | GUI Search 无来源、Verify 为 0 objects / 0 pending；退出后只读 SQLite 审计 schema 11、integrity ok，3 个 Source 版本均 deleted，object refs / inline bodies / fragments / FTS / 活动来源均 0；读取前后数据库摘要不变 |
| 正常退出 | 两轮 GUI 退出 0，stdout / stderr 均为 0 bytes；最终 GUI 进程 0、测试账户已注销、profile 卸载；Windows 正常关机，九台 VM 完整清单全停后才启动 Linux |

本次没有系统凭据交互提示，不能据此宣称取消 / 拒绝、locked / unavailable、权限撤回或中途失效窗口已验收。中文正文在 GUI 显示为方框，记录为既有字体 / 中文显示质量缺口；ASCII 找回及中文 bytes 的精确导出不关闭中文显示或中文检索质量问题。没有安装字体、修改依赖或改变 provider 生产实现。

### 观测脚本异常与修正

首次结束后槽位核对脚本错误地把编码长度固定为 70，实际 canonical `rmkek1:` 前缀为 7 bytes，加 32-byte key 的 64 个十六进制字符应为 71，因而返回 `MetadataOrEncodedLengthMismatch`。保留原始失败回执与脚本，静态核对生产编码和既有测试后另建重查脚本，按 `ASCII.GetByteCount("rmkek1:") + 32 * 2` 计算并保留原全部身份 / metadata 判据；同用户会话内只做一次精确读取重查，结果 `present_metadata_valid`。没有重复 Prepare、写回或替换 key；不能把第一次观测失败改写为通过。

另一次证据整理使用 Windows PowerShell 默认文本解码，未显式 UTF-8，导致含中文的冻结清单解析失败；改为独立 UTF-8 整理脚本并保留原失败。该错误发生在证据整理，不是产品导出失败；最终五份导出以原 bytes 和冻结 SHA-256 双重核对。两项脚本修正均未修改生产代码。

### 保留状态

Windows 新测试账户、profile、唯一 Credential Manager 项、任务目录、输入、五份导出及审计证据全部保留；账户未删除，key 未删除。任务目录统计为 37,351,035 bytes（约 35.6 MiB，不包含系统用户 profile 和控制目录）。应用层来源删除仅完成上述本地组件，不包含原始输入、导出副本、系统历史、VM 快照或备份。

## 2026-10-24 Linux 前置核对

Windows 全停后串行启动 Debian13-ARM64，新建 `rm_p1s05_gui`，UID / GID 1001、仅同名组，无 sudo；密码由用户在客体控制台设置并登录 GNOME。复制前核对候选 ELF 的长度 / SHA-256，输入与 profile 全部匹配冻结摘要；隔离任务目录为 `0700`。会话核对成立：Wayland、`gdm-password`、活动本地 user session、`CapEff=0`，真实 HOME / 用户 bus 与 Secret Service owner 均为 UID 1001。

首轮元数据预检读取 default alias 得到 `/org/freedesktop/secrets/collection/login`，随后读取该对象属性返回 `org.freedesktop.DBus.Error.UnknownMethod: Object does not exist at path`。保留失败回执，未将错误视为空闲槽位，该次失败时尚未执行精确 Item 搜索、产品 GUI 或 Prepare。原授权禁止替换已有 default，因此在继续前单独请求确认失效别名修复范围；不能用 root bus、临时会话、改 PAM 或静默初始化绕过前置条件。

### 测试 collection 修复与第一轮

项目所有者随后明确授权仅修复这个新测试账号的失效 default：创建本批命名 collection，并将已证实不存在的 login 对象别名改指该 collection。首次程序化创建进入密码提示等待，但未观察到可用密码窗口，用户关闭等待终端；之后由用户在 Seahorse 中手动创建 collection。只读检查区分 session collection、失效 login 路径与唯一新建 collection，确认原对象确实不存在、目标已解锁且本批精确 Item 匹配为 0 后，才设置批准的 label / default。原始失败和每次修复回执均保留，没有更改其它用户、PAM 或全局配置。准备交接期间曾出现用户在普通终端输入可能为口令的文本，已立即停止并说明该终端不是受保护密码框；相关两份本机临时截图已移除，未纳入保留证据，但不能宣称已撤回工具会话中显示的图像。后续 collection 密码输入均交由用户在系统密码框完成。

修复后的首次检查为 UID 1001 / session 3，default 已解锁，精确 service / username 的 unlocked、locked、default 匹配数均为 0。第一轮 GUI 在同一会话启动：Import picker Cancel 未创建来源；导入初始版本 104 bytes、ASCII 搜索、一次 Prepare、迁移、显式加密打开、对照来源 111 bytes 导入、主来源更新为 121 bytes 均完成。注销前两份历史 / 当前版本导出逐字节及 SHA-256 匹配冻结输入；Verify 为 3 objects、0 pending。GUI 正常退出 0，stdout / stderr 均为 0 bytes。

退出后元数据检查确认精确 Item 仅有一个、位于当前 default 且未锁定、label 正确，但完整 attributes 与脚本预期不相等。保留原始失败后，另作同用户只读属性诊断：实际仅比两个业务属性多出 `xdg:schema=org.freedesktop.Secret.Generic`。GNOME 上游的 [schema 字段定义](https://github.com/GNOME/gnome-keyring/blob/master/pkcs11/secret-store/gkm-secret-fields.h)与 [属性序列化实现](https://github.com/GNOME/gnome-keyring/blob/master/pkcs11/secret-store/gkm-secret-fields.c)支持 schema 合入返回属性的行为。另建检查脚本，精确匹配实际三项属性，并保留用户身份、唯一性、collection label 与锁定状态判据；重查退出 0。原脚本把完整属性限定为两个字段过严，修正未改变生产 provider、搜索范围或凭据内容；全部诊断只读元数据，未调用 secret 方法，没有再次 Prepare。

### 重登、第二轮与最终结果

第一轮退出后通过 GNOME 正常注销，核对 UID 1001 的会话和进程均为 0；用户重登后为 session 8。首次新会话元数据检查发现本批 collection 仍锁定，门禁失败并未启动第二轮 GUI。打开准确集合后由用户通过系统密码框解锁；独立复核确认同一 UID / 真实用户 bus、准确 collection label、唯一精确 Item 与三项属性，且 collection / Item 均已解锁。原 locked 回执保留；这只能证明验收启动门禁拒绝锁定状态，不能据此声称产品 locked 失败窗口通过。

| 判据 | 实际结果 |
| --- | --- |
| 重登后打开 | 第二轮默认 legacy 入口按预期拒绝加密库，显式 Open encrypted 成功；没有重新 Prepare 或生成 key |
| 只读动作 | 显式 Refresh、ASCII Search 与 Verify 成功；Verify 为 3 objects、0 pending；搜索返回主来源当前版本和独立对照 |
| 五份精确导出 | v1 为 104 bytes、v2 为 121 bytes，注销前后各两份；主 lineage 删除后对照导出 111 bytes。全部逐字节匹配冻结输入且 SHA-256 一致，保留 BOM / CRLF / LF 原始差异 |
| 删除 | Cancel 后两条 lineage 保持；随后主来源两个版本删除，重新检索仅剩对照且可认证导出；最后对照删除。两份回执均 completed，每份 10 个组件均 succeeded，实际效果分别记录 deleted / retained_minimal / not_found |
| 最终状态 | GUI Search 无来源，Verify 为 0 objects、0 pending captures / deletion objects、无 pending abandonment；退出后只读 SQLite 审计 schema 11、integrity ok，3 个 Source 版本均 deleted，object refs / inline bodies / fragments / FTS / 活动来源计数均为 0；数据库摘要读取前后不变 |
| 退出与收尾 | 第一轮 PID 6963、第二轮 PID 12236 均正常退出 0，stdout / stderr 均为 0 bytes；最终 GUI 进程 0，测试账号正常注销且 UID 1001 无会话 / 进程；Debian 正常关机，完整九台 VM 清单全部 stopped |

Linux 任务目录统计为 359,054,046 bytes（约 342.4 MiB，不包含其余用户 profile / 系统配置及控制目录）；新测试账户 / home、本批 collection 和唯一 key、任务库、输入、五份导出及证据均保留。两台平台的来源删除只覆盖各自应用管理的本地组件，不删除原始输入、导出、系统历史或备份。

本批两平台均保持各两次 GUI 启动、一次 Prepare，没有产品动作失败后换库或换 key。除独立受限观测 / 证据脚本修正外，未修改生产代码、依赖、字体、PAM 或全局密钥策略。Linux 中文正文同样显示方框；精确导出证明 bytes 保持，不关闭中文显示 / 检索质量缺口。系统取消 / 拒绝、其它 key store 错误、权限中途撤回、提交 / evidence 中断、重叠 GUI 事务和其它文件系统 / 桌面环境仍不在这次正常链路证据中；P1-S05 与 PDF / 图片停止线保持。

本轮宿主完整离线 `./scripts/check-repo.sh` 已通过 272 文件基线、fmt、locked Clippy 与 398 项测试（25 suites，0 failed，11 个隔离 helper）；第一次沙箱运行的 P1-F17 端口绑定拒绝保留，经权限流程完整重跑退出 0。其后仅补充本文件和状态路由，最终文本 / 链接检查与 `git diff --check` 均通过；未把宿主检查当作客体真实系统证据。
