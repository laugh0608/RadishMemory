# Phase 1 中文检索候选与兼容方案

更新时间：2026-10-28

状态：`Q01 isolated candidates measured; proposal accepted in ADR 0009`

后续状态：项目所有者已接受推荐兼容范围，[ADR 0009](../adr/0009-versioned-chinese-recall.md)冻结生产规则，[落地记录](../implementation/phase1-chinese-recall.md)记录双入口实现及真实限制。下文保留候选阶段当时的实验与建议，不作为当前未批准状态。

## 范围

承接[production 基线](phase1-chinese-retrieval-baseline.md)。基线已提交为 `f2f8e10`；本批增加[独立候选实验](../../crates/radishmemory-application/tests/q01_candidates.rs)，复用原 12 份资料、24 个查询、候选来源标签与 top-k 5，未调整答案来迎合算法。计量代码提取为[共享测试模块](../../crates/radishmemory-application/tests/q01/metrics.rs)，production 基线仍走原 application 双入口。

本批候选只创建内存 FTS 表，没有 production tokenizer、数据库迁移、application / desktop 行为、依赖或 lockfile 改动。以下建议尚未成为 Accepted ADR、存储契约或 Q01 完成证据；不能把实验辅助函数直接接入产品。

## 实验与结果

四个候选均使用现有锁定依赖提供的 SQLite 3.53.2，排序为各自 `bm25` 加来源标签稳定次序。文档内容与 query 独立进入索引 / 编译器，预期标签只进入结果计量；返回内容不在 Rust 中逐正文扫描或过滤，不使用 LIKE / GLOB fallback。标签排序不是 production opaque ID 排序；本实验比较集合，不据此评价同分排名。

| 候选 | 相关来源命中 / 30 | 有答案查询命中 / 20 | 完全匹配 / 24 | 误召回来源数 | 无答案误召回 / 4 |
| --- | --- | --- | --- | --- | --- |
| 现有 unicode61 边界对照 | 15 | 14 | 12 | 0 | 0 |
| 内置 trigram，原查询字面引用 | 13 | 11 | 10 | 5 | 1 |
| 汉字逐字 AND + 独立非汉字字段 | 30 | 20 | 24 | 0 | 0 |
| 汉字连续短语 + 独立非汉字字段 | 30 | 20 | 24 | 0 | 0 |

unicode61 对照与已测 production 计数一致。trigram 在 `API` / `REST` / `SQL` 等查询中返回 d12 的 `APIs` / `RESTful` / `SQLx`，并使本应无答案的 `RESTful API` 命中 d12。中文短词仍有遗漏。[SQLite 官方说明](https://www.sqlite.org/fts5.html#the_trigram_tokenizer)明确 trigram 全文查询不匹配少于三个 Unicode 字符的子串；本机独立 `蓝色` 对照复现空结果。本轮不选择直接替换为 trigram。

原 corpus 太小，逐字 AND 与短语候选都能满分。因此另设固定反例，不修改原基线分母：

| 输入 / 查询 | 逐字 AND 对照 | 连续短语候选 |
| --- | --- | --- |
| 查 `蓝色`；资料含 `蓝色`、`蓝天的颜色`、`蓝，色`、`蓝 色`、`色蓝`、`蓝API色` | 六条均命中，其中五条不含连续 `蓝色` | 仅连续 `蓝色` 命中 |
| 查 `哈哈`；资料含 `哈` 与 `哈哈` | 单字资料也误命中 | 仅双字资料命中 |
| 查 `蓝，色` | 作为两个概念 | 同样作为两个概念，六条均可命中；与查 `蓝色` 明确区分 |
| 查 `api 接口` / `UTF-8` | — | 找回紧邻汉字的目标；`REST` 不匹配 `RESTful` |
| 用户输入边界标记同名词、`OR`、引号、列名或运算符 | — | 标记只查真实非汉字内容；`OR` 作为字面词；不能注入 FTS 列筛选或 OR 运算 |

短语候选还验证单字、倒序、纯标点 / 空白空结果及既有 Latin 去重音行为。反例通过仍只是有限证据，没有独立人工标注或真实用户评测。

复验命令：

```sh
CARGO_NET_OFFLINE=true cargo test -p radishmemory-application --test q01_candidates --locked -- --nocapture
CARGO_NET_OFFLINE=true cargo test -p radishmemory-application --test q01_search --locked -- --nocapture
```

## 推荐的检索语义范围（待审阅）

1. 将连续汉字串作为有顺序、相邻位置的字面短语。允许单字、两字和长串查找，不把汉字集合作为无序 AND。`蓝色` 可以匹配长句中的连续子串，不能跨标点、空白或 Latin 文本拼成命中。
2. 查询中的空白及汉字段间的标点分隔概念，概念之间 AND；非汉字段沿用按空白分项、每项字面引用的规则，`UTF-8` 等项中的 unicode61 token 保持短语顺序。中英接缝成为边界，使 `本地API接口` 可被 `API` 和 `接口` 找回。Latin 仍按完整词匹配，保留既有大小写 / 去重音规则；不增加前缀、词干、同义词、简繁转换或用户 FTS 操作符。
3. 复用 `LocalSearch` 的共享边界，覆盖 `SourceFragment` 与 `MemoryRecord`。先校验 canonical / derived，再计算 namespace、敏感度、时间、状态与删除资格，之后 MATCH 和 top-k；不把拒绝条件放到截断后补过滤。
4. 来源、版本、正文、字节范围与 citation 继续回到原 canonical 对象。变换后的 token 只用于索引，不当作原文偏移，也不成为展示正文。双字段 BM25 会改变长度与词频统计，排名变化属于待验范围，不承诺保持旧顺序。

实验按两个 indexed 字段实现：汉字字段每字一个 token，原文不同汉字段之间插入不能由汉字查询生成的边界 token；非汉字查询字段在汉字段两侧补空格，保留汉字段作为不能由非汉字查询生成的位置屏障，继续使用 unicode61。查询对汉字段生成列限定短语，对非汉字段生成列限定字面词 / 短语。标记只在汉字字段出现，普通同名词在另一字段搜索，避免标记碰撞误召回。

独立反例另验证 `foo-bar` 不会跨 `foo中文bar` 拼接，而空白分项的 `foo bar` 按 AND 可命中两者。保留汉字段的位置屏障意味着额外索引体积，不能从该实现推断空间成本可忽略。

这只是验证相邻短语方向的原型，有两项必须在生产契约中明确并补反例的局限：

- 汉字判定仅覆盖 U+3400–U+4DBF 和 U+4E00–U+9FFF，未覆盖全部 Han、兼容字、扩展平面、组合标记与 variation selector。不能宣称完整 Unicode / CJK 支持。实施前须冻结版本化字符范围及区间外行为，不用系统 locale 或新依赖偷偷改变分词。
- 纯标点判定所用 Rust 字符分类不等同于 unicode61 全部 token 分类，例如私用区与组合字符行为尚未验证；生产版须有统一的受测规则。词项长度、查询上限、超长输入和其它文字系统尚未覆盖。

因此下一步先冻结上述语义与范围，再实施可复用生产索引 / 查询编译器；不因 30/30 将原型原样转为正式实现。

## 派生格式与旧库兼容建议（待批准）

静态核对 [migration.rs](../../crates/radishmemory-sqlite/src/migration.rs)、[vault_session.rs](../../crates/radishmemory-sqlite/src/vault_session.rs) 与[共享检索](../../crates/radishmemory-sqlite/src/derived_index.rs)确认：现有 migration history、表集合以及加密会话的完整 schema 定义都严格校验；普通入口只接受 v6，加密 / 维护入口分别约束 v7 至 v12。不能修改历史 `0004_local_recall.sql`，不能把 v7 重新用作中文索引版本，也不能为了容纳新表放宽未知表校验。

推荐单独登记 **derived recall profile**，与既有 SQLite 正文状态版本正交。它是 adapter 内部的可重建派生格式，不改 canonical schema；具体表名、DDL、profile 标识和字符规则版本须在生产切片的技术决策中冻结。本方案不是增加第二套事实表或同时长期维护两套生产索引。

| 库状态 | 建议行为 |
| --- | --- |
| 旧 v6 或正文迁移完成的 v8–v12，无 profile 且严格符合旧 schema | 识别为旧派生格式，继续按旧查询工作；界面明确中文升级尚未执行，提供显式升级入口 |
| 用户触发升级 | 验证模式、身份、canonical 与原始正文；在单事务中构造新派生字段、复验并登记 profile，提交后整库使用一致的新查询 |
| 新库 | 在新实现验收后直接建立新 profile；普通明文 / 显式加密门禁分别保持 |
| v7、未完成正文迁移的 v8，或有未收口 capture / 删除 | 不启动独立索引升级；先按原契约恢复 pending 状态，不混入另一项维护 |
| 已升级库以后进入 key bootstrap、正文迁移、capture、放弃或删除 | 新实现必须携带并严格校验已知 profile，原事务与恢复规则继续成立；不得重新写回旧字段或误判成 schema 漂移 |
| 未知 / 缺失但结构不符的 profile，部分新字段、未知表或触发器 | 失败关闭，不回退旧查询，不把“没有 profile”视为任意损坏库可升级 |
| 新 profile 交给旧客户端 | 预期旧客户端因额外 profile 表 / schema 不符而拒绝；这是有意的兼容成本，生产迁移前需用旧构建实测 |
| 显式降回旧派生格式 | 从可验真 canonical / 正文重建旧索引并验证，在同事务撤销新 profile；保留正文存储版本。失败回滚保留原状态，不直接删库或恢复旧备份覆盖新资料 |

必须沿现有 schema 校验链加入“基础版本 + 已知派生修订”的完整预期定义，并覆盖全部打开 / 维护 / 恢复 / 写入路径，不允许只让普通 search 接受新表。旧版表结构无记录地变更与增加万能 schema fallback 均不可接受。

升级失败应回滚到原格式与原数据，报告真实错误；不标为升级成功。默认 v6 的 failed-open 维护入口仍有既有缺口，本切片至少需要让**健康旧 v6**可达显式升级；损坏旧 v6 不得借升级绕过验真。若要同时修复其 failed-open 入口，必须单独说明增量范围，不能假设已存在。

## 实施门禁与分批顺序

1. 审阅本批候选标签与字面检索范围，冻结字符覆盖、两类短语边界、纯标点 / 空查询行为、profile DDL、兼容与回滚技术决策；不以测试作者自评替代独立标注审阅。
2. 生产 codec、索引一致校验和显式升级作为一个完整存储切片，覆盖旧 / 新 profile、未知格式、部分损坏、事务中断、并发打开、旧客户端拒绝和降级失败。v6 与已迁移加密库两条链均需覆盖，bootstrap 至历史删除接管也不能漏 profile。
3. 接入 application / desktop 的明确升级状态、动作和错误；现有单 worker、旧视图撤回、原请求恢复与关闭规则继续适用。不能只交付可测试的 migration helper 而让用户无法升级。
4. 通过 production 双入口重跑原 corpus 的导入 / 重开 / 重建，建议这组冻结样本要求 30/30 相关来源、24/24 集合匹配、0 误召回，并将独立反例纳入同一入口测试。补 MemoryRecord、namespace、敏感度、时间、状态、更新旧版本排除与删除资格回归；达到这些数字仍不替代扩充后的人工评测。
5. 记录代表性长文 / 大库的索引大小、构建耗时和查询延迟，审阅具体预算后再形成性能 gate；本实验没有测量性能，不以短测试运行时间宣称更快。原生和跨平台验收另按现有授权流程，不因单机内存 SQLite 通过而关闭 Q01 或 P1-S05。

所有派生字段仍继承原正文敏感度。当前加密对象模式的 SQLite / FTS 仍可读，新 profile 不增加整库静态加密保证；重建 / 降级不承诺清除历史页面、备份或导出。没有模型、网络召回、向量、资料外发或新第三方库。

## 本批验证与交接

定向候选实验 5 项、原 production 基线 3 项测试通过；SQLite 3.53.2 / 本机 aarch64-apple-darwin。完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0，仓库基线检查 289 个文件，fmt、locked Clippy 与 27 个 Rust suites 通过，合计 411 passed、0 failed、11 个由父测试调用的 ignored helper。计数包含两个测试目标各自执行的共享计量回归，不把重复执行当作独立质量样本。没有运行真实 GUI、系统 key、VM 或旧版客户端兼容实验，没有执行现有库升级 / 降级；这些是后续实施门禁，当前没有生产质量改善。

定向逐查询输出、四候选计数汇总、源码摘要与完整检查日志保留在本批忽略目录；只包含合成标签。production 基线已提交，候选测试与本文留在当前工作区供审阅，未 push。没有新增常驻进程；本批不清理此前的原生验收产物或系统测试 key。
