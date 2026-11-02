# Phase 1 版本化中文检索落地

首次记录：2026-10-28；最近复核：2026-11-02。

状态：`production dual-entry implemented; synthetic and limited macOS verification recorded; full Q01/Q04 acceptance open`

## 范围与实现

项目所有者在候选评审后确认[ADR 0009](../adr/0009-versioned-chinese-recall.md)的汉字连续短语、旧库显式升级、旧客户端拒绝及显式降级范围。输入为 `dev` / `f2f8e10` 加本任务尚未提交的候选实验；本记录覆盖随后的生产实现。没有新增依赖、改动 lockfile、调用模型或更改 canonical / 正文版本。

SQLite adapter 使用 `radishmemory.recall.han-basic-a/1` 标记派生格式。新建库默认启用；已有库保持原格式，正常打开不会升级。索引和查询共用冻结字符规则，基本汉字及扩展 A 按相邻有序短语检索，非汉字仍使用锁定 SQLite 的 unicode61。每次查询限制 4096 UTF-8 bytes / 128 个非空项，超限明确返回 `SearchQueryLimit`，没有截断或扩大权限 fallback。

健康默认 v6 和正文迁移完成的显式加密入口均提供原子切换。切换前验真 canonical、全部历史正文、派生数据及未完成请求，事务内重建 FTS，commit 后回读真实格式。对象模式沿原独占会话认证密文，commit 前再次验证；不创建 key、不读取外部原件。未知 / 部分 profile、schema 漂移和 pending capture / 删除阻止切换。正文版本、原始 bytes、对象、记忆状态及删除证据保持。

application 与桌面单 worker 已接线。健康视图显示最后刷新得到的格式，升级 / 降级有兼容性确认；Cancel 不派发，busy / pending / 无有效视图禁用。完成后清空搜索并刷新；操作失败撤回视图，commit 后独立刷新失败与写入结果区分。后续显式 Refresh 以数据库实际格式恢复，不假定失败一定未提交。

## 质量与故障证据

固定 `q01-synthetic-v1` 的 12 份资料 / 24 个查询及来源标签保持，top-k 5。默认 v6 与显式加密 production 入口，各在导入、重开、重建后取得：相关来源 **30/30**，有答案查询 **20/20**，完全匹配 **24/24**，额外来源 **0**，无答案误召回 **0/4**。原句中文短词遗漏已在固定样本中修复。历史 15/30 观测保留于[基线记录](../evaluation/phase1-chinese-retrieval-baseline.md)，没有改写为历史成功。

独立 production 反例覆盖分散 / 倒序汉字、标点与 Latin 边界、完整英文词；codec 回归覆盖字符区间端点、扩展 B 不冒充子串支持、简繁不转换、组合字符、variation selector、私用字符、重音与 FTS 操作符字面处理。SourceFragment / 已确认 MemoryRecord 的 namespace、敏感度、时间、撤回、更新与删除继续按原资格过滤；proposal 不直接成为记忆。

格式往返验证包括旧库打开保持、两连接看到实际格式、旧版本精确导出、更新后旧词排除、删除后不复活、新格式随 key bootstrap / 中断正文迁移 / capture / 删除恢复链保留。真实 SQLite deferred foreign-key commit 失败在两个方向均回滚；对象协调的 commit 前后故障、对象晚期损坏与独立进程退出分别验证旧 / 新完整状态及 canonical / 密文保持。桌面 headless 使用真实 worker，覆盖确认、取消、单次派发、原请求阻止切换与提交后刷新失败恢复。

首次全量测试中，历史正文迁移 fixture 直接写旧 FTS 列，却由新建库得到新格式，17 项中的 16 项在 bootstrap 阶段失败。已让该 fixture 显式建立旧格式，定向 17 项通过；生产新格式迁移由 `q01_upgrade` 保持覆盖，没有放宽一致性检查。

## 真实旧代码兼容

从 `f2f8e10` 的 `git archive` 构建真实旧 SQLite adapter，并与本批当前源码探针使用独立 target 目录。每步核对探针身份、退出码、profile 表及 `user_version`：旧格式可由旧代码打开；升级后旧代码返回 `SchemaDrift`；显式降级后再次可打开。六步均符合预期，基础正文版本一直为 v6。

首轮共享 target 目录复用了旧探针产物，已判无效并保留原记录；有效结果来自独立构建后的重新执行。该证据只覆盖空合成 v6 和实际 adapter，不代表旧 GUI 或所有加密版本兼容。完整资料的双入口升级、导出、更新、删除另由 production 集成测试覆盖。

## 有限长文成本

手动执行 `cargo test -p radishmemory-application --test q01_cost --locked -- --ignored --nocapture`。本机 macOS ARM64、debug 构建、32 份固定重复合成资料，共 704,918 bytes；无随机 seed。每种查询依次测 5 次，表中为中位毫秒。只做有限观测，无冷 / 热缓存控制，不设未经评审的 CI 时间阈值；加密使用合成 provider，未包含真实系统凭据延迟。

| 入口 / 格式 | 数据库文件 bytes | 使用页 bytes | 英文唯一 marker ms | 中文短词 ms |
| --- | ---: | ---: | ---: | ---: |
| 默认 / 旧 | 1,961,984 | 1,933,312 | 21.050 | 20.802 |
| 默认 / 新 | 3,518,464 | 3,518,464 | 57.279 | 58.723 |
| 加密 / 旧 | 1,990,656 | 1,290,240 | 1,530.080 | 934.709 |
| 加密 / 新 | 2,875,392 | 2,875,392 | 1,741.119 | 4,680.263 |

默认 / 加密升级耗时分别 327 / 1,160 ms。英文查询均命中 1 份；旧中文查询遗漏为 0 命中，新格式达到 top-k 5，因此两种中文耗时并非相同输出工作量。新格式的汉字 token、完整性复算与索引存储增加成本；既有加密全库认证和结果回源继续有明显代价。数据库统计不含对象文件、临时 journal、进程内存或备份；不将 SQLite 文件大小说成整个资料库大小。这不是性能优化或生产容量验收，Q04 保持开放。

## 验证与交接

完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：298 个文件的仓库基线、fmt、locked Clippy、29 个 Rust suites 通过，合计 427 passed、0 failed、13 ignored（12 个父测试调用的隔离 helper、1 个已单独手动执行的成本测试）。共享计量回归在两个测试目标重复执行，计数不当作独立质量样本。默认 `cargo build -p radishmemory-desktop --locked` 成功。

收尾复核修正了“提交后格式被另一连接改变”的错误文案，随后 fmt、全 workspace / all-targets / all-features locked Clippy、SQLite 全目标 85 项测试（0 failed）与默认 desktop 构建再次通过。定向质量、升级、存储故障、正文迁移、桌面 headless、手动成本与旧代码兼容均已执行；原始日志、计数、探针及源码摘要保留在本批忽略目录，仅含合成数据。

未执行新切换 UI 的原生可见验收、真实系统 key store、Windows / Linux、真实断电或大规模 release 性能测试；候选标注仍待独立人工评审，完整 CJK、简繁同义词与向量检索不在本 profile。Q01 / Q04 和 P1-S05 不整体关闭。FTS 精确正文及新增派生字段仍为 SQLite 明文，token 编码不是加密，也不保证旧页面 / 备份擦除。

本批未启动 GUI、VM 或常驻进程，没有远程动作，未提交、未 push；不修改或清理既有原生验收库、系统测试 key 与冻结产物。

日终更新：上述候选实验、ADR、production 实现及对应文档已按项目所有者要求提交为 `4c9b0be`，此前“未提交”描述是实施收尾快照。完整日终回顾和当前后续顺位见[10 月 28 日记录](../status/2026-10-28-chinese-recall.md)及[当前状态](../status/current.md#后续事项)。

## 2026-11-02 分阶段读取成本

状态：本批完成 Q01 标签静态审阅和 Q04 有限 release 基线，未实施性能优化，未关闭 Q01 / Q04 / P1-S05。

### 实现与复验方法

沿既有 `q01_cost` 扩展，保留原手动测试名称；成本测试现在明确要求 `--release`，默认仓库测试仍将其 ignored。SQLite 新增同名 `acceptance-test-support` feature，由 Source Vault 既有合成 feature 转发，不新增依赖或改动 lockfile。三个模块沿原 application / reader / SQLite 路径设置固定阶段 span；默认构建的 span 为无时钟 / 无 collector 的空操作。显式合成观测只在当前线程内存累计阶段枚举、次数和 Duration，不持久化查询、正文、路径、ID 或 key，没有环境变量开关、外发、日志 sink 或回调可跳过校验。普通测试开启 feature 但未调用 `observe` 时也不创建时钟或 collector。

阶段包括 reader 打开、合成 key load、对象认证、canonical 复验、catalog 装配、派生复验、查询准备、FTS MATCH、结果回源和 snapshot 刷新。每个阶段同时保存 inclusive 与扣除嵌套阶段后的 exclusive 耗时；两者不能混加。collector 三项回归覆盖嵌套 / 重复计数、不重叠耗时、原错误与 unwind 清理、线程隔离和嵌套 observation 拒绝。production 查询、事务、校验、结果和错误传递顺序保持。

复验入口：

```sh
CARGO_NET_OFFLINE=true cargo test -p radishmemory-application --test q01_cost --release --locked -- --ignored --nocapture --test-threads=1
```

最终有效运行 E / F 从 Cargo JSON artifact 定位并校验同一个 release test binary，分别直接执行该 binary，返回码均为 0；两个进程串行运行，测量期间未并行执行构建或测试。每个进程各包含 24 组（两入口 × 两格式 × 五种查询及一次刷新），每组独立关闭 / 重开，单列 open 与第一次调用，再连续调用 11 次；每次结果数量都有断言。重开首次调用不等于磁盘冷缓存，没有清系统缓存或控制系统其它进程。p50 / p95 使用 nearest-rank，11 个重复样本的 p95 即最大值，仅描述本批样本，不作为生产尾延迟估计或 CI 时间门禁。

默认刷新调用现有 application 的目录与首个来源版本读取；加密刷新调用显式 `read_snapshot(None, 200)`，包括未完成请求发现、末尾 key / 对象复验。刷新与搜索分开计时；这不是 GUI 的绘制或交互耗时。两种刷新承担的治理工作不同，不作完全等工作量比较。

固定 corpus 为 `q04-repeat-v1`：沿用此前 32 份、共 704,918 bytes 的生成方式，每份单版本 / 单整文件片段，含唯一英文 marker 和重复 256 次的固定中英合成段；无随机 seed。数据顺序和查询顺序固定，包含 marker（1 命中）、中文 top-k 1 / 5、无答案、仅允许 Restricted 的权限拒绝及 32 条目录刷新。旧格式中文未命中，新格式分别返回 1 / 5 条，不能把这两种工作量直接解释为等价查询回归；marker 是相同输出工作量的对照。

运行环境为 macOS 27.0.1 / ARM64，Rust 1.96.0 / Cargo 1.96.0，release 默认优化配置，锁定 SQLite 3.53.2；`os.cpu_count()` 返回 10。物理内存与更完整硬件查询受沙箱限制，未取得，未提权。key 使用 SyntheticLibraryProvider，没有访问真实系统 key store。源码基线为 `f1394a4cf1d0e4142412127951c7a9b1205f0efc` 加本批未提交源码：所有已跟踪 Rust / manifest / lock / toolchain 文件加新计量文件，去重后共 166 个，按相对路径排序，用 `path + NUL + bytes + NUL` 合并计算 SHA-256。

源码集合 SHA-256：`278ee9b2abdde3e382e7a09e5ffcc3976be622824038acb4715685b9b842d059`；test binary SHA-256：`9ccd15d193aeeec7abc973fd70d6c5184a79ac1c638e5e7472d7ea22d73b7165`。源码集合共 166 个文件；文档不在该摘要内。原始 Cargo artifact JSON、identity、E / F 样本与 resource JSON、聚合结果保留在本批独占的忽略目录 `.tmp/2026-11-02-q01-q04/`，只含代码身份、固定标签、计数和耗时。

前两次 A / B 使用 `/usr/bin/time -l`：binary 日志均显示 1 passed / 0 failed，但 wrapper 在查询 `kern.clockrate` 时被沙箱拒绝而返回 1，没有取得资源数据。随后直接执行的 C / D 均返回 0，但对应收尾前的 debug 模式拒绝写法；Clippy 修正该写法后重新构建、计算源码与 binary 摘要，再执行最终 E / F。下表只采用最终输入的 E / F，前四轮日志和身份仍保留，不挑选较快的一轮替换最终结果。E / F 通过 Python 标准库 `resource.getrusage(RUSAGE_CHILDREN)` 读取该唯一 test binary 的资源计数；本机 Darwin 的 maxrss 单位为 bytes。

### 实测结果

下表单位均为 ms，E / F 为两个独立进程。原始日志另外保存每组首次调用、每次重开和所有阶段的逐样本耗时。

| 入口 | 格式 | 操作 | E 首次 / p50 / p95 | F 首次 / p50 / p95 |
| --- | --- | --- | ---: | ---: |
| plain | legacy | marker | 4.223 / 4.174 / 4.315 | 4.560 / 4.335 / 4.663 |
| plain | legacy | han_one | 4.069 / 4.104 / 4.255 | 4.452 / 4.267 / 4.442 |
| plain | legacy | han_five | 4.112 / 4.109 / 4.236 | 4.436 / 4.239 / 4.416 |
| plain | legacy | absent | 4.342 / 4.103 / 4.248 | 4.254 / 4.257 / 4.315 |
| plain | legacy | denied | 4.526 / 4.165 / 4.204 | 4.042 / 4.229 / 4.401 |
| plain | legacy | refresh | 6.239 / 6.362 / 6.495 | 6.741 / 6.669 / 6.838 |
| plain | han | marker | 12.058 / 12.172 / 12.561 | 11.805 / 11.872 / 12.491 |
| plain | han | han_one | 12.016 / 12.388 / 12.574 | 12.257 / 12.333 / 13.288 |
| plain | han | han_five | 12.988 / 12.666 / 13.134 | 12.330 / 12.966 / 13.581 |
| plain | han | absent | 11.742 / 12.004 / 12.218 | 12.409 / 11.754 / 12.326 |
| plain | han | denied | 11.856 / 11.787 / 12.088 | 11.690 / 11.579 / 11.925 |
| plain | han | refresh | 6.323 / 6.356 / 6.418 | 6.198 / 6.088 / 6.200 |
| encrypted | legacy | marker | 190.560 / 174.977 / 401.064 | 171.712 / 172.449 / 173.666 |
| encrypted | legacy | han_one | 527.265 / 113.097 / 172.757 | 113.364 / 112.966 / 127.933 |
| encrypted | legacy | han_five | 111.701 / 111.054 / 121.304 | 107.222 / 107.742 / 113.327 |
| encrypted | legacy | absent | 110.350 / 108.937 / 114.173 | 109.278 / 108.103 / 108.844 |
| encrypted | legacy | denied | 113.777 / 113.643 / 116.415 | 106.532 / 107.508 / 109.798 |
| encrypted | legacy | refresh | 357.860 / 345.130 / 356.234 | 333.474 / 330.327 / 333.082 |
| encrypted | han | marker | 202.139 / 203.358 / 209.550 | 200.606 / 202.122 / 205.999 |
| encrypted | han | han_one | 202.948 / 205.401 / 213.688 | 200.957 / 200.917 / 202.219 |
| encrypted | han | han_five | 500.364 / 504.053 / 536.197 | 512.539 / 500.386 / 523.678 |
| encrypted | han | absent | 128.638 / 132.533 / 476.406 | 128.136 / 127.587 / 128.906 |
| encrypted | han | denied | 301.737 / 137.636 / 225.942 | 127.462 / 129.099 / 162.044 |
| encrypted | han | refresh | 405.919 / 389.544 / 416.193 | 369.814 / 375.276 / 379.943 |

两轮空间统计相同；object_bytes 只统计 `source-objects-v1` 的普通对象文件。下表不包含合成外部输入、staging、journal、进程内存或备份，不是整个资料库大小。

| 入口 / 格式 | 数据库 bytes | 已用页 bytes | 对象文件 bytes |
| --- | ---: | ---: | ---: |
| plain / legacy | 1961984 | 1933312 | 0 |
| plain / han | 3518464 | 3518464 | 0 |
| encrypted / legacy | 1990656 | 1290240 | 721531 |
| encrypted / han | 2875392 | 2875392 | 721531 |

E 的进程峰值 RSS 为 21,659,648 bytes（约 20.66 MiB）。F 的进程峰值 RSS 为 21,790,720 bytes（约 20.78 MiB）。两者包含建库 / 导入 / 迁移、全部查询、打印和清理的完整 test binary，不能称为单次查询峰值内存或桌面 GUI 内存预算。

### 成本归属与首个优化范围

以下为 E 的新格式加密中文 top-k 5 中位墙钟样本（sample 2），各项 exclusive 可以相加；回源 inclusive 包含其内部的认证 / 复验，只用于说明调用位置，不能再加到 exclusive 总和。实际样本编号以原始日志为准。

| 阶段 | 调用次数 | exclusive ms | inclusive ms |
| --- | ---: | ---: | ---: |
| reader_open | 1 | 8.864 | 46.293 |
| key_load | 1 | 0.000 | 0.000 |
| object_authentication | 13 | 214.712 | 240.603 |
| canonical_validation | 429 | 111.059 | 227.023 |
| catalog_load | 14 | 9.572 | 9.572 |
| derived_validation | 27 | 156.234 | 156.234 |
| query_preparation | 1 | 0.203 | 0.203 |
| fts_match | 1 | 0.390 | 0.390 |
| result_resolution | 1 | 2.424 | 374.060 |

静态链路与计数互相印证：reader 打开先认证 1 轮；search 的 read 在查询前后各认证 1 轮；每条 application 回源又走一次 read、前后各认证 1 轮。因此本语料零命中为 3 轮、1 命中为 5 轮、5 命中为 13 轮。新格式刷新记录 10 轮对象认证、2 次合成 key load。计数表示当前链路实际工作量，不是要求永久保留此实现形状的新契约。

E / F 的新格式加密 5 命中分别为 504.053 / 500.386 ms，回源 inclusive 为 374.060 / 371.106 ms，FTS MATCH 为 0.390 / 0.392 ms。E 中 object_authentication exclusive 约占 43%，derived_validation 约占 31%；canonical 与 catalog 另列。对象认证阶段还包含目录 inventory、文件读取与对象状态检查，不等于纯密码算法耗时；derived_validation 同时包含派生字段重算 / 比较与 FTS integrity-check，本批不能进一步断言其中哪一部分最慢。

最终样本保留可见波动：E 的新格式加密 absent p50 / p95 为 132.533 / 476.406 ms。没有控制整机后台负载、电源或物理缓存，也没有据此剔除离群样本；两轮小样本不能证明稳定的生产 p95。

据此确定下一优化切片：先限定于 `derived_index::load_actual_recall_rows` / `verify_catalog` 及其调用的既有 codec，补分字段重算与 FTS 内部核对的子计时，再针对实测热点减少同一次验证内的重复编码、复制或临时分配。保持 profile 字节规则、全部历史来源 / canonical / FTS 校验、调用边界、权限与删除资格、事务和身份检查。是否需要调整多结果回源的认证组织另做边界评审，本批未缓存明文、复用跨操作 session 或减少前后认证。

下一切片回归至少包含 Q01 双入口标签 / 独立反例、格式升级 / 降级、派生字段和内部索引损坏拒绝、对象 / 身份变化与权限 / 时间 / 删除过滤，并在同机器、同 release、同 fixture、相同命中数量下重复 A / B。性能预算决策为：当前只建立观测基线，尚不足以冻结生产延迟、容量或 CI 时间阈值；后续结合局部改动对阶段耗时和总耗时的实际影响再评审目标。不能把此前约 4.68 秒的 debug 数字与本批 release 数字解释为已实施优化或十倍改进。

本批未执行独立人工标签确认、GUI / 最小窗口 / 字体或跨平台可见验收、真实 key store、物理冷启动、真实断电、多版本及 100 / 1000 / 10000 来源规模验收。Q01 标签审阅详情见[基线追加表](../evaluation/phase1-chinese-retrieval-baseline.md#2026-11-02-标签静态审阅)；完整验证与交接见[本批记录](../status/2026-11-02-read-cost.md)。Q01 / Q04 / P1-S05 和 PDF / 图片停止线保持。

## 2026-11-02 派生字段编码局部优化

状态：基线批次已提交为 `2156759`，本后续切片在该提交之上细分成本并完成字段编码局部优化；本节只报告固定合成 release 对照，不关闭 Q01 / Q04 / P1-S05。

### 热点与改动边界

沿前述 opt-in collector 新增 `derived_encoding` 与 `fts_integrity` 两个子阶段，阶段总数由 10 变为 12；历史 E / F 仍解释为原十阶段观测，不改写旧日志。先在编码未优化的源码上构建独立 binary，再按子计时定位热点。第一轮新格式加密中文 top-k 5 中位样本的字段重算约 52.418 ms，FTS 内部检查约 81.737 ms；二者都包含在原派生复验 inclusive 中，不能再与父阶段相加。

局部改动仅在既有 `recall_codec`：直接把每段汉字 token 追加到同一个派生字段 String，移除每个 Han run 的中间 String；冻结的两个字符范围都恰为四位十六进制，因此使用固定小写数字表输出原来的 `h` + 四位码点，替代每字符通用格式化。query 的汉字段与索引仍共用同一编码器。保留原 runs 分类、汉字相邻 / 有序约束、` b ` 分隔、` 界 ` 屏障和非汉字原样字节。

没有新增 profile 或 schema，没有重建、迁移或修复用户库；生成 bytes 保持，所以正常旧索引继续接受。没有增加依赖或改动 Cargo.lock，没有全文 / 明文缓存或跨操作 session。canonical、对象认证、身份、权限、时间、删除及 FTS 完整性检查的数量和顺序保持；本批没有调整多结果回源的认证组织。FTS、历史页、备份、导出及真实系统凭据边界保持。

新增两个兼容性回归：固定字段 golden bytes 覆盖空串、纯 Latin、交替 Han / Latin、标点、冻结字符边界内外、组合字符、variation selector、NUL 与扩展 B；逐个遍历冻结范围的全部码点，将输出与独立的原 `h{codepoint:x}` 格式化 oracle 对照。原 production 查询规则与限额、字符范围、权限及故障回归继续运行。

### 同条件 release 对照

输入仍为 `q04-repeat-v1`：32 份单版本资料、704,918 bytes、两 production 入口、两格式、五类查询和独立刷新。每组重开后单列首次调用，再重复 11 次；nearest-rank p95 仍是该小样本最大值。构建模式、工具链、平台、缓存限制及 synthetic provider 沿本页前节。未清 OS 缓存或控制整机负载 / 电源，未启动 GUI 或真实 key store。

运行顺序为 before-a → after-a → before-b → after-b；四个 binary 进程串行，测量期间不执行其它构建 / 测试。before 与 after 各冻结一份 executable，运行前校验 SHA-256，避免后续 Cargo 产物覆盖；before 只有新增子计时，after 使用相同子计时并包含编码改动。两份源码 patch / artifact JSON / identity 和全部样本、资源计数保留在本批忽略目录 `.tmp/2026-11-02-derived-cost/`，历史忽略目录保留。

源码摘要沿前节 166 个 Rust / manifest / lock / toolchain 文件的排序与 NUL 分隔算法。基线均为 `2156759f43ac8e5b6ac215078335134956a63e42` 加本批源码差异。

| 变体 | 源码 SHA-256 | binary SHA-256 |
| --- | --- | --- |
| before | `61834261b720567b24679ce3b454f68e88d02c6d5e67a6a428ad79cd2e05c32f` | `9586e0f8b6191d16635da6306882dbbef92971789beb12d33074f3660d0a3d3f` |
| after | `d50106825d74f740cc7dce6c80980b9431dfb9a3153997af91c4664caf91bf93` | `386a386f9ac8cb243a1c1c7172d2ca23700fb15544dfe1b2a97e975116794c17` |

四次进程均退出 0，各 1 passed / 0 failed。各次包括 24 组、每组 1 次首次与 11 次重复，共 288 次测量操作，另有 24 次重开；每次调用都断言实际命中数量。聚合验证全部样本的 12 阶段不重叠 exclusive 与未归属耗时合计、组数、百分位和空间统计；各变体的调用次数与结果数量一致。下表为重复样本 p50 / p95，单位 ms；原始日志另外保留各次重开与首次调用。

| 入口 | 格式 | 操作 | before-a | after-a | before-b | after-b |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| plain | legacy | marker | 5.698 / 11.939 | 4.221 / 4.408 | 4.124 / 4.432 | 3.994 / 4.431 |
| plain | legacy | han_one | 4.901 / 5.880 | 3.970 / 4.343 | 3.883 / 4.218 | 3.937 / 4.642 |
| plain | legacy | han_five | 4.808 / 5.519 | 4.537 / 4.997 | 3.918 / 4.163 | 4.111 / 4.373 |
| plain | legacy | absent | 4.399 / 6.362 | 4.254 / 4.505 | 3.957 / 4.178 | 4.088 / 4.255 |
| plain | legacy | denied | 4.369 / 5.512 | 4.463 / 5.475 | 3.923 / 4.200 | 4.027 / 4.171 |
| plain | legacy | refresh | 6.694 / 7.431 | 6.852 / 7.475 | 6.179 / 6.404 | 6.255 / 6.547 |
| plain | han | marker | 16.791 / 17.974 | 9.799 / 10.155 | 11.937 / 12.278 | 9.059 / 9.275 |
| plain | han | han_one | 14.221 / 19.579 | 9.340 / 9.877 | 12.192 / 12.540 | 9.233 / 9.499 |
| plain | han | han_five | 13.396 / 14.410 | 9.669 / 12.618 | 12.719 / 13.234 | 9.426 / 9.671 |
| plain | han | absent | 13.476 / 17.364 | 8.850 / 9.189 | 11.935 / 12.858 | 9.203 / 9.575 |
| plain | han | denied | 13.062 / 14.676 | 8.943 / 9.311 | 11.988 / 12.397 | 8.916 / 9.197 |
| plain | han | refresh | 6.624 / 7.403 | 6.418 / 7.442 | 6.247 / 6.417 | 6.368 / 6.583 |
| encrypted | legacy | marker | 212.764 / 231.955 | 174.648 / 179.625 | 172.468 / 173.867 | 174.003 / 176.364 |
| encrypted | legacy | han_one | 133.101 / 148.269 | 108.342 / 118.124 | 106.488 / 107.033 | 108.823 / 114.429 |
| encrypted | legacy | han_five | 133.090 / 145.598 | 109.583 / 113.346 | 109.012 / 113.766 | 111.144 / 116.129 |
| encrypted | legacy | absent | 133.055 / 144.115 | 108.545 / 113.903 | 110.983 / 156.458 | 108.989 / 111.254 |
| encrypted | legacy | denied | 109.975 / 126.252 | 108.655 / 112.804 | 107.283 / 114.514 | 109.203 / 115.244 |
| encrypted | legacy | refresh | 346.150 / 466.427 | 336.361 / 342.733 | 335.388 / 348.371 | 337.280 / 348.560 |
| encrypted | han | marker | 206.929 / 236.195 | 187.301 / 198.212 | 208.005 / 217.199 | 184.517 / 190.948 |
| encrypted | han | han_one | 207.370 / 214.054 | 187.478 / 205.694 | 204.743 / 207.065 | 183.404 / 185.833 |
| encrypted | han | han_five | 509.168 / 528.164 | 468.397 / 482.796 | 502.620 / 539.566 | 454.724 / 460.847 |
| encrypted | han | absent | 137.291 / 144.383 | 122.198 / 127.833 | 129.096 / 135.558 | 116.700 / 274.224 |
| encrypted | han | denied | 136.760 / 142.920 | 117.278 / 120.574 | 130.263 / 132.233 | 116.967 / 118.554 |
| encrypted | han | refresh | 405.691 / 417.837 | 352.562 / 364.284 | 384.088 / 391.111 | 347.572 / 352.204 |

同一操作的墙钟中位样本拆分如下，时间单位 ms；三项均为 inclusive，用于各自前后比较，不跨父子求和。

| 新格式加密 top-k 5 | before-a | after-a | before-b | after-b |
| --- | ---: | ---: | ---: | ---: |
| derived_encoding | 52.418 | 10.137 | 52.438 | 10.139 |
| fts_integrity | 81.737 | 82.555 | 83.717 | 79.286 |
| result_resolution | 378.499 | 348.559 | 372.539 | 338.059 |

两组中文 5 命中的 p50 分别下降约 8.0% / 9.5%；字段重算从约 52 ms 降至约 10 ms。四个中位样本均记录 13 轮对象认证、27 次 FTS integrity-check、448 次字段编码；局部 CPU / 分配改进没有减少检查。最终聚合还对原始逐样本阶段调用次数做交叉核对。两格式、两入口的空间统计与基线一致，没有把较快数据选择性替换为正式结果。

| 进程 | 整个 test binary 峰值 RSS bytes |
| --- | ---: |
| before-a | 21364736 |
| after-a | 20905984 |
| before-b | 21168128 |
| after-b | 21757952 |

RSS 通过各独立 Python runner 的 `getrusage(RUSAGE_CHILDREN)` 读取，本机 Darwin 单位为 bytes；包括建库 / 导入 / 迁移、打印、全部操作与清理，不代表单次查询或 GUI 内存预算。小样本仍有环境波动，不能据本批改善冻结生产容量、尾延迟或 CI 时间阈值。主要剩余成本仍包括对象认证 / 文件 IO、canonical 复验和 FTS 内部检查。

legacy 路径不调用新派生字段编码，其观测快慢不能计入本优化收益。after-b 的新格式加密 absent p95 为 274.224 ms，高于 before-b 的 135.558 ms，原始样本与完整矩阵均保留；局部编码 CPU 改善不证明所有查询的尾延迟下降。

复验成本的标准命令仍为前节 `q01_cost --release --locked -- --ignored --nocapture --test-threads=1`；本批前后精确差异、冻结 binary 与原始样本便于复验。完整验证、提交与保留状态见[后续切片记录](../status/2026-11-02-derived-encoding.md)。独立人工标签、原生格式切换 / 最小窗口 / 跨平台、多版本和规模 / 物理冷缓存尚未验收，Q01 / Q04 / P1-S05 继续开放。

## 2026-11-02 规模基线与原生后续

上述分阶段基线和字节等价优化分别提交为 `2156759` / `e0b9926`。随后 `3f05a13` 增加独立 `q04_cost` 的 100 lineage / 160 版本固定档位，沿现有 production API 与同一 opt-in collector 测量；旧 `q01_cost` 仍为 32 份协议，共享 helper 只收敛测试生命周期、输出和百分位计算。两轮新格式加密中文 5 命中 p50 为 1,201.358 / 1,201.121 ms，每次仍有 13 轮全库对象认证；输入和结果见[规模基线](../evaluation/phase1-local-library-scale.md)。它不与 32 份样本直接算优化收益，不代表 1000 / 10000、冷缓存或生产预算通过；前节“多版本与规模未验”为该切片当时状态。

`3cde127` 收录两种模式的 760×520 headless 入口 / 完整确认文案边界回归及[macOS 有限原生结果](../evaluation/phase1-recall-native-acceptance.md#2026-11-02-macos-原生局部结果)：明文 / 加密升级、降级 Cancel / Confirm、中文蓝色 / 红色 / 主题找回、关闭重开及四份精确导出已有证据。真实窗口仍未调整到已核实的 760×520 内容尺寸，Han 升级后的旧 ASCII 词未在 GUI 重跑，Windows / Linux、真实 key 首尾取消及独立人工标签仍待完成；不将正常链路、headless 或无可见提示扩成完整 Q01 / P1-S05 通过。

本日完整提交、代码与文档复核及接续建议见[日终记录](../status/2026-11-02-chinese-recall.md)；下一次实际顺位以[当前状态](../status/current.md#后续事项)为准。
