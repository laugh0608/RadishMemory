# 2026-09-15 Source Vault 日终记录与明日事项

日期：2026-09-15（Asia/Shanghai）。本记录回顾当天 `dev` 的全部实现 / 验证提交，对照代码复核相关文档，并保存次日交接。现行顺位和停止线以[当前状态](current.md)为准。

## 当天提交回顾

| 提交 | 内容 | 证据与限制 |
| --- | --- | --- |
| `9d88319` `test(source-vault): 补充 Linux 文件系统验收场景` | 新增普通用户权限拒绝 / 恢复、FIFO 拒绝及 objects / staging 目录替换三项集成测试；拒绝 root 与非零 effective capabilities 执行 | Linux ARM64 / ext4 普通用户 check、Clippy、34 个 unit tests 与 3 个 integration tests 通过；没有修改 production adapter |
| `5acdf74` `docs(source-vault): 收口 Linux 文件系统验收与阶段状态` | 汇总 Linux 输入复验、实际覆盖、环境清理及 P1-S03b 对象文件系统退出范围；同步专题和阶段检查器 | P1-S03b 的 macOS、Windows ARM64 / NTFS、Linux ARM64 / ext4 各有合成证据，不能外推为后续代码或所有文件系统通过 |
| `146ea8f` `feat(source-vault): 落地独立平台密钥 provider` | 正式落地六项精确直接依赖、22 个第三方增量、平台 store、严格编码 / 属性复验、私有 bootstrap 和脱敏错误；包含此前隔离预检记录 | macOS 构建及 47 个 Source Vault tests、全仓库 175 个 Rust tests 通过；真实系统 key store 和 Windows / Linux provider 编译未验收 |
| `b484656` `feat(source-vault): 实现 SQLite 密钥初始化事务协调` | 同库 writer 事务内验真、对象目录 / 数据库身份检查、密钥创建 / 复用与 v7 `key_ready` 维护 checkpoint；补齐并发和失败重试 | 全仓库 191 个 Rust tests、独立子进程锁验证、真实 `SQLITE_BUSY` commit 失败复用通过；普通产品仍停在 v6，正文迁移与宿主接入未完成 |

日终另做一次文档提交，汇总本页、修正下述过期口径并补齐索引；该提交不修改 Rust、manifest、lockfile 或自动化规则。今天全部变更留在本地 `dev`，没有 push、PR、远程 CI 或发布。

## 对照代码的文档复核

| 核对对象 | 核对结果与文档处理 |
| --- | --- |
| Linux public filesystem tests 与 `ObjectDirectory` / identity 检查 | 已有证据准确对应权限、FIFO 和目录替换；保留基线与合成限制。filesystem 记录的“key provider next”改标为当批退出状态，补上 provider / bootstrap 的后续链接，修正过期下一步 |
| 三平台 provider、strict ASCII codec 与 error mapping | 独立实现已落地，真实 Keychain / Credential Manager / Secret Service 及 logger 过滤仍待验收；将“尚未提交”和“P1-S04 全部未实现”改为有时间范围的历史事实及当前进展 |
| `initialize_library_key`、SQLite live transaction 与 `0007_source_vault_key.sql` | v7 只保存单例 namespace / device / provider / `key_ready`，没有正文迁移；普通 `SqliteDatabase::open` 仍拒绝 v7。修正架构中笼统的“SQLite 协调未验收”，明确密钥初始化已测、正文对象提交协调待实现 |
| SQLite 正文验真与测试 | 检查全部剩余 inline BLOB 的 digest / length，对 active source 重用既有 decoder / fragment 校验；新增测试覆盖非当前版本损坏。双连接验证共享合成 key，独立子进程验证 SQLite 锁和状态重查，二者不能写成已完成真实跨进程 OS key-store 验收 |
| manifest、Cargo lockfile 与 notices | 当前是 8 个第一方、445 个第三方、453 个总 package；notices 为 366 项。依赖表补上 Source Vault → SQLite runtime 连线和测试 `rusqlite`；说明独立 Source Vault 现已可达 bundled SQLite C 构建，不能把 workspace 并集无增量误写为该 package 编译面没有变化 |
| 文档索引与当前状态 | 索引的当前 notices 数量从旧 344 修正为 366，补充 provider、初始化协调和本日记录入口；当前状态引用日终证据，并明确旧平台 filesystem 验收不覆盖今天新增的 provider / bootstrap |
| 产品范围、记忆、隐私和集成边界 | 没有修改 canonical 对象语义、用户所有权、记忆确认、RadishMind 边界、同步信任模式或许可证，不改写对应长期承诺。隐私专题已准确保留 v6 inline body / FTS 完整正文、真实凭据未测和历史明文未清除等限制 |

这是依据当天修改核对文档和证据，不构成独立密码学审计。P1-S03a、9 月 10 日和隔离预检中的早期数量 / 失败按当批时间保留；不以新结果重写历史。

## 验证与环境收尾

日终 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过：180 个仓库文件、notices、fmt、workspace Clippy 与 191 个 Rust tests 全部通过。另行运行的 35 个检查器测试和 1 个 compile-fail doctest 通过；`git diff --check` 通过。跨进程 helper 在主 harness 中标记 ignored，但由父测试以独立子进程实际执行成功，不额外算进 191 个主测试数。最终验证包含 `b484656` 中保留 SQLite numeric error code 的实现。

- Linux filesystem 批次使用普通账户与已有工具链 / 离线缓存；guest 中任务源码、缓存、二进制、fixture 和传输文件已清理，VM 已正常关机并确认 `stopped`。本机合成输入和验收证据保留，范围见[filesystem 记录](../implementation/phase1-source-vault-filesystem.md#linux-arm64--ext4-普通用户验收2026-09-15)。
- provider 预检 / 落地的隔离源码与专用 Cargo cache 已清理；保留本地依赖图、校验、运行与清理证据。正式 Cargo cache 和仓库忽略的编译产物保留。
- P1-S04a 的合成数据库、对象、临时 marker 和测试子进程由验收清理；没有访问真实个人资料或系统密钥库，没有启动 GUI、改系统配置或修改远程状态。
- Windows / Linux 的当前 provider / bootstrap 编译与运行、真实 native store / logger、断电及完整宿主链路尚未验收。按项目所有者安排，跨平台验证后置到阶段检查点；没有将“后置”记成“通过”。

## 明日事项（2026-09-16）

1. **确认起点**：检查 `dev`、HEAD、工作区和未推送提交，先读[当前状态](current.md)与 [P1-S04a 记录](../implementation/phase1-source-vault-key-bootstrap.md)。今天完成的是 key checkpoint，不能按“v7 已加密”继续开发。
2. **首项推进 P1-S04b 的正文迁移切片**：先落实 adapter-private object reference、capture / migration attempt 的字段、唯一约束与状态转换，再贯通合成 v6 来源的正文迁移。复用现有 `ObjectMetadata`、`ObjectWrite`、`ObjectDirectory` 和 SQLite migration 真相源，不建立第二套 canonical schema 或平行恢复器。
3. **守住提交顺序**：inline body 先复验 exact digest / length，密文 durable publish 后提交 reference，按 committed reference 解密回读成功后才能移除 active inline body。Source ID、lineage、version、fragment、citation、governance、binding、audit 和 deletion state 保持不变；迁移未完成时普通入口不可混合返回两种正文来源。
4. **把恢复与首次初始化分开**：已有 `key_ready` 或 migration / object facts 时只加载原 key；缺 key 失败关闭。当前 P1-S04a 要求完整 inline facts 和空对象目录，不能把它直接当作部分正文已外置后的恢复入口。下一批必须从已持久化 attempt / reference 判定恢复位置，复用已提交对象，不重新生成 KEK 或新 provenance。
5. **先做故障验收再扩大切片**：覆盖 publish 后未 commit、reference commit 后未 read-back、read-back 后未移除 inline body 的中断与重开；覆盖重复重试、旧正文损坏、对象缺失 / tamper、错误 key 和 ambiguous state。未知对象继续拒绝，不借恢复入口扩大删除范围；完整 orphan reconciliation、verify / rebuild 与删除执行分别收口。
6. **阶段检查点**：正文迁移 / 恢复形成完整合成链路后，再推进 application / macOS 宿主及真实 Keychain / logger 验收，处理已有 v6 连接和多实例生命周期；随后集中补齐 Windows / Linux 编译与运行。真实密钥库、GUI / VM、依赖或远程动作仍按精确范围授权，不把本页建议当作系统操作授权。
7. **继续保持停止线**：R01–R06 产品质量问题仍独立跟踪；PDF / 图片、模型、同步、恢复方案和发行不提前进入本批。不因为独立 adapter 通过就宣称产品加密可用或整个资料库已静态加密。

本页只保存明日开发建议，今天停止在文档收尾；不安排后台继续开发或自动执行。
