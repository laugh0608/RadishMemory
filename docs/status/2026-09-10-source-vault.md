# 2026-09-10 Source Vault 日终记录与明日事项

日期：2026-09-10（Asia/Shanghai）。本记录回顾当天 `dev` 提交及其代码、文档和验证证据；现行顺位与停止线以[当前状态](current.md)为准。

## 当天提交回顾

| 提交 | 内容 | 证据与限制 |
| --- | --- | --- |
| `66dc1aa` `feat(storage): 落地 P1-S03b 不可变加密对象适配器` | 严格 versioned envelope、专用目录 capability、create-new staging、no-overwrite publish、认证回读、精确 attempt 状态及脱敏 I/O 错误 | macOS Source Vault 32 tests、全仓库 160 Rust tests 与默认 features check 通过；未接入 SQLite / host |
| `6a6b611` `test(source-vault): 补充 Windows 文件系统验证` | Windows 目录占用、reparse / 非普通文件补测与平台证据 | Windows ARM64 提升权限下 28 unit tests、2 integration tests、check / Clippy 通过；当时不包含普通用户与同时间戳替换场景 |
| `2971fc9` `fix(source-vault): 使用原生文件身份守护 Windows 对象操作` | 复现并修复 creation-time 身份缺陷；隔离原生 128 位 ID 查询，保留引用防止 ID 重用；补充替换与普通用户 ACL 回归 | Windows ARM64 / NTFS 提升权限和 UAC 开启后的普通用户实测通过；第三方 package 版本与数量不变，新增一个第一方 adapter |

日终另做文档提交，同步下述真相源与明日事项；该提交不修改 Rust、manifest、lockfile 或自动化规则。所有提交均为本地提交，没有 push、PR 或远程 CI。

## 按代码核对文档

- 对照 envelope encoder / parser、`ObjectDirectory` 发布与读取流程、`inspect_attempt` 和相关负向测试，确认严格字段顺序与大小限制、独立预期 metadata、全对象认证、不可覆盖发布及失败残留语义一致。`PublishedObject` 仍只提供 filesystem 证据；attempt 检查不授权 orphan 删除。
- 对照文件身份实现与 Windows 回归，补齐 README、架构、隐私、路线图、ADR 0005 / 0007 / 0008 和当前状态中的平台口径。Windows 当前证据是这台 ARM64 / NTFS 测试机的提升权限、普通用户、目录占用、reparse 和 ACL 场景，不能外推为 ReFS、网络盘、所有 Windows 版本或 desktop 安装器 owner 验收。
- 文件写句柄在 sync 后关闭，但身份引用句柄继续保留到比较结束。修正文档中笼统的“文件关闭”描述；同时明确密文先在内存生成，再直接写入 staging，没有持久化明文临时文件。
- 对照 manifest、Cargo 生成的 lockfile、notices 和唯一 FFI 函数，确认当前为 8 个第一方、423 个第三方、431 个总 package，三目标 notices 并集仍为 344。`windows-sys 0.61.2` / `windows-link 0.2.1` 已有来源和许可证记录；新 adapter 的安全不变量在[依赖基线](../implementation/m0-rust-dependency-baseline.md)中，Source Vault 与 workspace 默认继续禁止 unsafe。
- 今天没有修改 canonical core、SQLite migration / source body、application、desktop、记忆确认或 RadishMind 数据边界。产品范围、记忆模型、canonical schema、RadishMind 专题与同步信任模式无需改写；生产仍是 SQLite v6 inline plaintext body，FTS 保有完整正文副本，R01 至 R06 质量缺口仍未关闭。
- P1-S03a、9 月 3 日归档与 filesystem 记录内的早期测试数量按当时批次保留；当前状态改为引用最终证据，避免把早期 32 / 160 个测试或“未提交”口径当作日终状态。此前真实失败及其日志摘要仍保留，不以修复后结果覆盖历史。

本次是依据当天修改核对相关文档，不构成独立密码学审计，也不扩大产品验收范围。完整失败复现、代码输入摘要与平台运行结果见[filesystem adapter 落地记录](../implementation/phase1-source-vault-filesystem.md)。

## 验证与环境收尾

日终 `./scripts/check-repo.sh` 通过 166 个仓库文件检查、notices、format、Clippy 与 162 个 Rust tests（其中 Source Vault 34 个）；检查器 27 个 unit tests 与 `git diff --check` 通过。Windows 同一组已构建二进制在提升权限和普通用户下分别通过 30 个 Source Vault unit tests、2 个 native tests，以及对应身份的 2 个 integration tests。两个 integration 组合有重叠：目录占用两边均运行，reparse 用提升权限运行，ACL 用普通用户运行；不能将两次运行相加称为四个不同场景。

普通用户实际核验 UAC 已启用、非管理员、中完整性、合成 SID 匹配且控制目录不可写。ACL 拒绝与撤销通过真实合成 fixture 操作产生 `PermissionDenied / OS 5`，恢复后可重新认证读取；未把任务注册成功冒充任务执行成功。

Windows 测试机已正常关闭并确认 `stopped`；临时账户、S4U 任务、profile 和该 SID 的批处理登录权利均已移除，UAC 恢复原 DWORD `0` 并经正常重启复核。隔离源码、Cargo 缓存、二进制、fixture、传输文件和脚本前缀零残留；没有操作 CleanBase。没有清除事件日志或承诺 VM 磁盘逐字节还原。本机任务临时目录仅保留合成运行证据、源码输入包与已批准方案供复核，没有加入 Git。

Linux filesystem、ReFS、网络盘、真实断电、Windows 全 workspace、远程 CI、产品 GUI、真实 key provider 和 SQLite migration 均未在本批验证。当前尚不能声明三平台加密 Source Vault 或产品加密数据流完成。

## 明日事项（2026-09-11）

1. 先确认 `dev`、HEAD、工作树和未推送提交；读取[当前状态](current.md)，避免沿用旧 VM 脚本、临时账户或今天的系统变更授权。
2. 首要推进 Linux filesystem 验收：明确测试 VM、普通用户、实际文件系统、Rust / Cargo 版本和隔离目录；说明 VM 启动、所需离线缓存、持续时间及清理范围后取得对应授权。优先使用现有工具链与锁定缓存；需要安装依赖或修改系统权限时另行说明精确范围。
3. 对最终代码执行 Source Vault check、Clippy、测试和普通用户文件系统验收：完整 publish / reopen / exact read、目录 sync、no-overwrite 竞争、symlink / FIFO / 目录替换、同内容同时间戳文件替换、权限拒绝与撤销、失败残留及精确 attempt 状态。记录真实文件系统差异，目录同步不支持时保留失败，不增加 no-op 或宽权限 fallback。
4. 汇总 macOS、Windows 与 Linux 的实际覆盖；若 Linux 通过，明确 P1-S03b object filesystem 的退出范围与未测介质 / 崩溃边界，再单独安排 platform provider landing。不得由平台测试直接扩展到真实密钥库、SQLite migration 或宿主接入。
5. R01 至 R06 的中文检索、目录分页、派生维护、性能、runner 证据和回源 / 刷新继续按[质量验收计划](../evaluation/phase1-local-library-quality.md)独立收口；PDF / 图片、模型、同步及发行仍保持当前停止线。

本页记录明日建议；未推送提交留待后续明确授权处理。
