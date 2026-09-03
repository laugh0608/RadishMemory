# ADR 0005：M0 实现栈与模块边界

日期：2026-08-22

状态：Accepted

## 背景

M0 的对象、状态、时间、摘要、fixture 操作和指标已经冻结，首个同步信任模式与 RadishMind 接入阶段也已经确定。进入真实 runner 前仍需选择一种能够承载闭合领域状态、确定性字节、嵌入式全文检索、追加事件、跨平台本地运行和未来多种宿主的实现栈，同时避免把 UI、模型、同步或服务端复杂度提前带入。

兄弟项目证明了按稳定职责隔离内核、适配器和宿主的通用价值，但它们的 `.NET`、Go、Python、TypeScript、Flutter 或 Rust 组合服务于不同业务。本决策只根据 RadishMemory 的 M0 约束选择技术，不复制任何兄弟项目的业务目录或依赖清单。

## 候选判断

| 候选 | 优势 | 对 M0 的主要代价 | 结论 |
| --- | --- | --- | --- |
| Rust | 闭合 enum、显式 `Result`、原生跨平台库、Cargo workspace、SQLite FFI 与未来宿主边界清晰 | 前期类型与所有权建模成本较高，原生依赖需审查 | 采用 |
| Go | CLI / 服务分发简单、构建快、服务端生态成熟 | 本地嵌入与未来客户端复用较弱，闭合状态主要依赖 tag 与手写校验 | 不作为记忆核心 |
| C# / .NET | 类型与 SQLite 生态成熟，服务和桌面开发效率高 | 自包含运行时、Native AOT / FFI 与移动宿主边界更重 | 当前不采用 |
| TypeScript / Node.js | UI 与协议迭代快 | 需要 Node 运行时，SQLite 原生绑定和确定性领域约束更依赖运行时校验 | 留给未来 UI 评审 |
| Python | fixture、研究和评测效率高 | 生产本地核心的分发、类型闭合和嵌入边界不理想 | 只保留工具与评测用途 |

## 决策

M0 产品核心使用 **Rust 2024 edition**，首个实现工具链精确固定为当前已在开发宿主验证可用的 stable **Rust `1.96.0`**。仓库提交 `rust-toolchain.toml`、workspace `Cargo.toml` 和由 Cargo 生成的 `Cargo.lock`；正式检查使用 locked mode，不使用 nightly 或浮动 Git 分支依赖。

Rust 只约束 M0 核心、SQLite 适配器和 runner。它不冻结未来桌面 / 移动 / Web UI、零知识同步服务端、RadishMind adapter、模型 worker 或可信计算节点的实现语言。Python 继续承载现有仓库检查和 fixture oracle，但不是产品记忆真相的第二实现。

### 模块化单体

首个 workspace 只有三个 package：

```text
Cargo.toml
rust-toolchain.toml
crates/
  radishmemory-core/
  radishmemory-sqlite/
apps/
  radishmemory-m0/
```

正式 package 路径固定为 `crates/radishmemory-core/`、`crates/radishmemory-sqlite/` 和 `apps/radishmemory-m0/`，不再为同一职责建立平行入口。

职责与依赖方向：

```text
radishmemory-m0
  ├──► radishmemory-core
  └──► radishmemory-sqlite ──► radishmemory-core
```

- `radishmemory-core`：九种 canonical 对象、共同值类型、状态转换、时间与冲突规则、canonical JSON / 摘要、应用操作和实际需要的存储 / 检索 port。它不依赖 SQLite、文件布局、CLI、网络、Provider 或 RadishMind。
- `radishmemory-sqlite`：SQLite schema、嵌入迁移、事务、Source Vault 小文本存储、追加事件、物化投影、FTS5 和删除组件实现。SQL 与数据库行号不得越出 adapter。
- `radishmemory-m0`：读取冻结 fixture、为每个场景建立隔离临时存储、按顺序调用真实应用操作、比较 assertion / metric，并向标准输出生成最小 JSON 证据。它不承载第二套领域逻辑。

三个 package 运行在一个进程中，不建立微服务、插件系统、全局 service locator、通用 manager 层或跨进程协议。只有已经被 M0 操作真实需要的 port 才能进入核心；不能为未来假设预建空 adapter。

### M0 存储基线

M0 使用单个本地 SQLite 数据库文件作为每个 fixture 场景的隔离存储，使用 FTS5 作为全文基线。SQLite 由 Rust adapter 以 bundled 方式构建，并在启动时显式检查 FTS5 能力；缺失时返回稳定 unsupported capability 错误，不回退到内存扫描或另一种搜索实现。

- 原始 UTF-8 / Markdown 字节作为独立 source body BLOB 保存，保留原始换行和 exact-bytes 摘要；metadata、fragment 和治理字段与正文逻辑分离。
- proposal、decision、memory record、state event、delete request 和 deletion evidence 按不可变对象或追加事件保存；当前状态只作为可重建投影。
- FTS5、ContextPack cache 和当前状态表都是派生数据，不是 canonical truth；索引更新和对应事实写入在同一 adapter 事务中收口，并有重建与一致性测试。
- FTS5 排序使用明确相关性和稳定对象 ID tie-break；数据库 rowid、未稳定浮点分数或查询计划不得进入 canonical 输出。
- schema migration 使用 adapter 内版本化 SQL 文件与单调 schema version，不引入 ORM 或独立 migration framework；未知较新 schema 必须失败关闭。
- 每个 M0 场景在系统临时目录下使用独立数据库，并在结束后清理；不读取用户目录、真实资料库、系统密钥链或默认生产路径。

M0 中删除组件的 `succeeded` 只证明目标行、FTS 条目、投影和缓存已按计划处理，且通过应用接口与完整性检查不再可检索；它不证明 SQLite 空闲页、临时文件、文件系统快照或底层介质已经完成取证级擦除。runner 应避免持久 WAL，关闭连接后清理整个合成场景目录，并把任何残留或清理失败报告为 pending / failed。生产物理清除、密钥销毁和备份到期仍由后续存储与加密决策承载。

把小文本正文暂存为 SQLite BLOB 是 M0 的实施选择，不是长期 Source Vault 格式。阶段 1 已通过 [ADR 0008](0008-phase1-encrypted-source-vault.md) 在 PDF、图片和大对象进入前冻结加密内容寻址对象存储、SQLite metadata 的事务协调和迁移边界；具体 dependency、cipher suite、key provider、adapter 与 migration 仍须独立评审和授权。

M0 不实现静态加密，因此不得因使用本地 SQLite 或 bundled SQLite 宣称加密存储、零知识或生产隐私保证。fixture 只能使用合成数据。

### 确定性与失败边界

- fixture 与证据 JSON 使用 `serde_json` 解析和表示，但摘要字节由本项目实现的 `radishmemory-canonical-json-v1` writer 生成，不依赖 map 默认顺序或 serializer 默认格式。
- UTF-8 原始字节不被规范化；只有 `utf8-nfc-text-v1` 明确要求的语义文本使用 Unicode NFC。
- RFC 3339 时间解析为带 offset 的绝对时刻并比较 UTC 值，同时保留外部表示所表达的精度事实；不使用本机 locale 或隐式系统时区。
- fixture ID 由冻结输入确定；M0 不引入随机 ID、UUID、随机排序或把系统时钟作为测试事实。
- 搜索、引用、状态转换、删除和指标聚合返回类型化错误；未知版本、枚举、operation、assertion、metric、悬空引用和摘要不一致均失败关闭。
- 第一方 `radishmemory-core`、`radishmemory-sqlite` 和 `radishmemory-m0` 源码禁止 `unsafe`。未来 FFI 或平台特化若需要 `unsafe`，必须隔离到新 adapter 并单独说明安全不变量。

### 首批依赖白名单

首个实现只允许以下直接依赖；精确解析版本由第一次受审阅的 `Cargo.lock` 固定：

| 依赖 | 范围 | 用途与限制 |
| --- | --- | --- |
| `serde`、`serde_json` | core / runner | fixture 与证据 mapping；不得替代自定义 canonical JSON writer |
| `sha2` | core | SHA-256 profile；不得用摘要替代授权或真实性 |
| `unicode-normalization` | core | 仅实现明确的 NFC profile |
| `time` | core | RFC 3339 解析、UTC 比较与精度边界 |
| `rusqlite` | SQLite adapter | 参数化 SQL、事务和 bundled SQLite / FTS5 |
| `tempfile` | runner / test | 场景隔离临时目录和可靠清理 |

`rusqlite` 的 bundled 模式会引入 SQLite C 源码、build script 和原生编译工具链，这是已知供应链与构建代价；生成 lockfile 时必须记录实际传递依赖、SQLite 版本、启用特性、来源、许可证和三平台构建证据。

首批明确不引入 `tokio`、HTTP client、Web framework、ORM、消息队列、向量数据库、模型 / Provider SDK、日志上传、遥测、加密协议库、插件运行时、随机 ID 库或通用依赖注入框架。新增任何生产依赖都必须说明用途、替代方案、许可证、build script / proc macro / native code、网络与数据外发影响，并更新 lockfile。

### 运行与验证入口

根入口保持：

```bash
./scripts/check-repo.sh
```

Windows 保持：

```powershell
pwsh ./scripts/check-repo.ps1
```

workspace 建立后，两者必须聚合执行：

- `cargo fmt --all --check`；
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`；
- `cargo test --workspace --all-targets --all-features --locked`；
- M0 runner 对冻结 fixture 的真实执行与指标校验；
- 现有仓库治理、链接、fixture oracle 和 diff 检查。

GitHub 继续只要求稳定聚合 context `Candidate Quality`。Rust 检查作为其组件加入，并至少在 Linux、macOS 和 Windows 运行 M0 contract；三平台实际运行结果不能用单平台交叉编译代替。

M0 产品 crate 不引入网络依赖或网络 adapter。runner 还必须通过能力记录和测试执行环境证明网络请求计数为零；如果执行环境无法提供系统级网络隔离，证据必须如实区分“代码无网络能力”与“操作系统已阻断网络”，不能把前者写成强沙箱保证。

## 被拒绝的方案

### 先用 Python 实现 runner、以后重写 Rust

这会产生两套状态、摘要和删除语义，并让 fixture 先验证临时实现。Python oracle 继续独立检查 fixture 自洽，但真实 runner 直接使用生产核心。

### 从第一天拆成多个服务

M0 无网络、无同步、无模型，服务拆分只会引入协议、部署和失败面。模块边界先在同一进程内通过 package 和 port 表达。

### 为 M0 引入 PostgreSQL、向量库或对象服务

它们不能提高当前 fixture 的证明力，却会破坏本地、断网和最小依赖目标。阶段 1 与同步阶段根据真实数据量和信任模式另行选择。

### 直接采用兄弟项目完整技术栈

Radish、RadishMind、RadishFlow 和 RadishAxiom 的语言组合、UI、服务与构建矩阵来自不同产品边界。复用原则不等于复用业务实现或依赖图。

## 后果

收益：领域不变量、SQLite 与 runner 依赖方向明确；M0 可以用真实生产核心而不是测试重写；本地单进程、跨平台和后续 FFI / 服务适配都有稳定起点；依赖和网络面保持可审查。

代价：首批需要承担 Rust 与 bundled SQLite 原生编译；SQLite BLOB 不是长期大对象方案；手写 canonical JSON、错误类型和迁移需要更多确定性测试；保留直接核心会让未来 UI 和服务通过明确边界接入，而不能任意访问数据库。

## 重新评估条件

只有出现可复验证据时才替代本决策：Rust 无法在目标三平台稳定承载必需 SQLite / FTS5 与 canonical 字节；M0 纵向切片显示语言阻抗显著高于其它候选且无法通过局部设计解决；必需依赖出现不可接受许可证、原生分发或安全风险；或未来宿主无法通过稳定 FFI / IPC / API 消费核心。单次编译较慢、某个框架更流行或兄弟项目改用其它语言，不构成替代理由。

## 参考

- [Rust 2024 Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
- [Cargo.toml 与 Cargo.lock](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html)
- [SQLite FTS5](https://sqlite.org/fts5.html)
- [rusqlite](https://github.com/rusqlite/rusqlite)
