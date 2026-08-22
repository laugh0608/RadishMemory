# RadishMemory 当前状态

更新时间：2026-08-22

## 当前阶段

`M0 implementation entry`

产品、记忆、隐私、评测、同步信任、RadishMind 接入和 M0 实现栈基线已经冻结，最小 Rust workspace 与三平台检查合同已经建立。当前目标是按顺位实现 canonical core、SQLite adapter 和真实 M0 runner，不扩大到 PDF、Embedding、模型、UI、同步或服务端。

## 已冻结的首个切片

`M0 Local Memory Loop` 已通过 [ADR 0002](../adr/0002-m0-local-memory-loop.md) 冻结为单用户、单设备、本地、合成文本 / Markdown、无模型和无网络的 M0 代码范围。它按固定场景验证来源、引用、proposal / decision、时间更正、失败关闭和单设备删除证据，不包含 PDF、向量、Provider、RadishMind 或同步实现。

M0 字段级 canonical schema 已在 [M0 Canonical Schema](../schema/m0-canonical-schema.md) 冻结为九种顶层对象及共同逻辑类型。它确定字段、必填性、条件约束、时间、治理标签、事件和删除证据关系，但不绑定数据库、生产 ID 编码或语言类型。

[M0 Fixture 与指标契约](../evaluation/m0-fixture-contract.md) 已冻结合成 JSON mapping、fixture ID、摘要 profile、12 个场景的 86 个有序操作和 12 个指标 gate。仓库校验器只验证这些输入与 oracle 自洽；真实 M0 runner 和产品能力仍未实现。

首个多设备同步信任模式已通过 [ADR 0003](../adr/0003-zero-knowledge-sync-first.md) 冻结为零知识同步服务：默认自部署服务端只中继密文对象、加密操作日志、密钥封装和已枚举最小元数据，不持有内容解密能力，也不运行语义索引、检索或 ContextPack 编译。可信计算节点后置为显式可选能力。该决定不把同步加入 M0，也不代表零知识同步已经实现。

RadishMind 首次运行接入已通过 [ADR 0004](../adr/0004-radishmind-optional-gateway-entry.md) 冻结在完整 MVP 阶段 3：只在 mock 或直接 adapter 基线成立后，以显式可关闭的 Model Gateway 接入。M0、阶段 1 单机资料库和阶段 2 记忆生命周期均不依赖它；首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回，也不复制兄弟项目业务 schema。

M0 实现栈已通过 [ADR 0005](../adr/0005-m0-implementation-stack.md) 冻结为 Rust 2024 模块化单体：`radishmemory-core`、`radishmemory-sqlite` 和 `radishmemory-m0` 三个 package，首个工具链固定为 Rust `1.96.0`，本地存储采用 bundled SQLite 与 FTS5。M0 不包含网络、异步运行时、ORM、模型 SDK 或静态加密，也不冻结未来 UI 和同步服务端语言。

`M0-I01` 已建立且仅建立上述三个可编译 package，提交 Cargo 生成的 lockfile，并把 fmt、Clippy 和 locked test 接入本地双平台入口与 PR 的 Linux / macOS / Windows matrix。当前 lockfile 只有三个第一方 workspace package，没有 registry、Git 或传递第三方依赖；`rusqlite`、bundled SQLite 与 FTS5 尚未进入实现。精确清单和证据边界见 [M0 Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)。

## 当前顺位

1. `M0-I02 canonical core`：先实现稳定错误、RFC 3339 时间、Unicode NFC、SHA-256 profile 和 canonical JSON，再实现九种 canonical 对象、条件校验、引用与状态转换；不接数据库或 CLI 业务捷径。
2. `M0-I03 SQLite adapter`：实现版本化 migration、Source Vault 小文本、追加事件、FTS5、派生投影和删除组件，并验证 bundled SQLite 能力与事务边界。
3. `M0-I04 fixture runner`：按冻结操作顺序调用真实 core 与 adapter，执行全部 assertion / metric，输出最小 JSON 证据并报告零网络能力边界。

## 明日事项（2026-08-23）

主任务是完成 `M0-I02` 的第一个独立评审单元，只修改 `radishmemory-core` 及其直接测试和依赖记录：

1. 审查并加入本单元实际使用的 `serde_json`、`sha2`、`unicode-normalization` 和 `time`；记录解析版本、许可证、feature、build script / proc macro、网络与 native code 影响并更新 `Cargo.lock`，不提前加入仅供后续对象 mapping 使用的依赖。
2. 建立稳定且不包含正文的 core 错误类型，只覆盖本单元真实产生的 unsupported profile、invalid time、non-canonical JSON 和 digest mismatch；保留底层原因，但不把错误字符串当作协议字段。
3. 实现 RFC 3339 解析与 UTC 比较，同时保留外部表示的精度事实；实现 `ValidTime` 的 `unknown`、`instant`、`interval`、`open_ended` 条件校验和半开区间；实现 `exact-bytes-v1`、`utf8-nfc-text-v1` 与 SHA-256 小写十六进制摘要。
4. 实现 `radishmemory-canonical-json-v1` parser / writer：解析时拒绝重复 key 并保留规范化 number 所需事实，写出时按 Unicode code point 排序 object key、保留 array 顺序、使用必需转义、最短整数与普通十进制 fraction，并输出无空白、无尾随换行的 UTF-8 bytes；M0 输入校验在进入 canonicalizer 前拒绝 `null`，实现不依赖 map 默认顺序或 `serde_json` 默认序列化格式。
5. 用冻结 fixture 的 digest vectors 和补充负例验证偏移时间、无效区间、NFC、转义、key 排序、整数 / fraction 边界、未知 profile 与摘要不匹配；运行 package 级和仓库级 fmt、Clippy、locked test。
6. 同步 [M0 Rust 依赖基线](../implementation/m0-rust-dependency-baseline.md)与必要检查器，复核 diff 后独立提交。只有该单元的正例、负例和 frozen vectors 全部通过，才进入九种 canonical 对象与跨对象校验；明日不启动 SQLite、migration、FTS5 或 runner。

## 当前门禁

- 产品、架构、记忆或隐私语义变化必须同步更新对应真相源，不能只改入口摘要或检查器。
- M0 语言、首批直接依赖范围和 SQLite / FTS5 已冻结；新增依赖必须审查许可证、原生构建、网络与数据影响并更新 lockfile。UI、服务端、向量实现和 Provider SDK 仍未冻结。
- 仓库只允许代码、规范、治理资产和合成 / 明确脱敏的 fixture；真实个人资料、记忆库、ContextPack、Embedding 输入和密钥不得进入 Git、Issue、PR 或 CI。
- GitHub 远端以 `master` 为默认稳定分支、`dev` 为常态开发分支，启用 merge commit 与 rebase merge，并禁用 squash merge；Private vulnerability reporting、Secret scanning 和 push protection 已启用。Ruleset 与 required check 必须以 API、workflow run 和目标分支有效规则复核，不能把仓库模板本身当作已生效证据。
- 当前仓库检查证明治理、文本、链接、配置合同及空 Rust workspace 的格式、lint 和编译测试成立，不证明 canonical 类型、SQLite / FTS5、runner、隐私协议、删除或同步已经实现。

## 当前不做

- 不制作虚拟形象、主动陪伴或大而全的聊天产品面；
- 不发明新的长期记忆算法或直接引入图数据库、消息队列和微服务；
- 不把模型推断自动写成已确认记忆；
- 不宣称零知识同步、端到端加密、可证明永久删除或生产可用；
- 不建立自动发布、tag Ruleset、装饰性 CODEOWNERS 或无真实评审人的审批门禁；
- 不复制兄弟项目的技术栈、业务清单、CI 组件或目录结构。

## M0 实现退出条件

- 已完成：经评审的 M0 采集、检索、引用、确认、更正和删除闭环；
- 已完成：与首个切片对应的字段级 canonical schema；
- 已完成：与首个切片对应的可执行合成 fixture 与指标 oracle；
- 已完成：明确的记忆状态机、时间与冲突语义；
- 已完成：首个同步信任模式决策；
- 已完成：可测量的召回、污染、删除与隐私指标；
- 已完成：RadishMind 首批参与方式的明确决定；
- 已完成：记录实现栈选择、替代方案、迁移边界和风险的 ADR；
- 已完成：精确 Rust 工具链、三 package workspace、第一方依赖锁和三平台 Rust CI 合同；
- 待完成：canonical core、SQLite / FTS5 adapter 和 M0 runner 真实实现；
- 待完成：冻结 fixture 的全部 assertion、metric 和三平台运行门禁通过。

## 当前验证入口

macOS / Linux：

```bash
./scripts/check-repo.sh
./scripts/check-m0-fixtures.py
```

Windows：

```powershell
pwsh ./scripts/check-repo.ps1
```
