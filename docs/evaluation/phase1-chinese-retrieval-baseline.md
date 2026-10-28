# Phase 1 中文找回基线

更新时间：2026-10-28

状态：`Q01 synthetic production baseline measured; quality gaps remain`

## 范围与标注

承接[Q01](phase1-local-library-quality.md)与[当前顺位](../status/current.md)。生产代码基线为 `3152f2c`；本批只增加 application 集成测试、固定合成语料和结果记录，没有修改 tokenizer、查询、schema、索引存储、依赖或 lockfile，也没有访问历史原生验收库。

语料与预期来源集合只定义在[固定 corpus](../../crates/radishmemory-application/tests/q01/corpus.rs)，版本为 `q01-synthetic-v1`：12 个纯合成 Markdown、24 个查询、固定 `top_k = 5`，全为单版本 / 整文件片段。数据是显式枚举的固定文本，不使用随机生成，因此没有 seed；每个查询至多 4 个相关来源，小于 top-k。语料覆盖 R01 原始句子、中文短词 / 整句、简繁中文、标点、英文缩写、连续中英文本、英文对照、无答案和近似词干扰。

这些候选标注由本批拟定、可逐条人工复核，尚未经过独立人工标注评审，也不是用户研究。相关性的候选口径是“同一资料包含查询所指的字面概念”，标点分隔概念，英文缩写按完整词处理但允许紧邻汉字；不假设词干、同义词扩展、简繁转换或用户可输入 FTS 语法。该口径用于量化用户意图与现状的差距，不因写入测试而自动成为已批准的产品公共契约。

预期来源写为 `d01` 至 `d12` 的独立标签；实际结果仅通过 import receipt 的 source ID 映射到标签。测试只将原样 query、top-k 与允许敏感度传给 production API，不将预期标签、扩展词或答案拼进查询，不调用 M0 runner 的检索编排。

## 执行入口与可复验性

[基线测试](../../crates/radishmemory-application/tests/q01_search.rs)调用现有 application 入口，使用两个独立、独占创建的临时目录：

1. 默认 `LocalLibrary::open` → `import_new_source` → `search_sources`。关闭后只读确认 SQLite v6、12 行 inline body；保留同一 runtime / config 重开，重新查询；显式 rebuild / verify 后再查询。
2. 新空库 → `EncryptedLibraryLocation::initialize_key` → 空库 `migrate_bodies` → `open` → `prepare_import_new_source` / `capture_source` → `search_sources`。关闭后只读确认 SQLite v9、0 行 inline body、12 个对象引用；重开和显式 rebuild / verify 后分别重测。真实 crypto / object / SQLite / application 路径保持，key 使用已有 `SyntheticLibraryProvider`，没有系统 key store。
3. 两入口各有 imported / reopened / rebuilt 三组观测，比较逐查询的来源顺序；逐结果核对来源可解析、v1、正文与精确字节范围、无重复与 top-k 上限。对同一查询只允许 Restricted 时应返回空，且英文 / 整句正向对照必须命中，避免把全空结果当作有效拒绝证据。

集成测试会使用现有 dev-dependency 中的 `acceptance-test-support` 合成 provider；它不是普通产品系统凭据运行，也不是 GUI / 跨进程重启。没有新增 feature 或 production 注入入口。临时输入、数据库与对象在测试结束时仅清理本测试独占目录，既有测试 key 和本地产物不受影响。

复验命令：

```sh
CARGO_NET_OFFLINE=true cargo test -p radishmemory-application --test q01_search --locked -- --nocapture
```

输出仅包含 corpus / 查询 / 来源标签、计数、入口与阶段，不打印原文、实际临时路径或 key。每组 summary 明确给出 `quality=gaps_observed` 或 `all_labels_matched`；后者也仅表示本批标签匹配，不能独立关闭 Q01。三个 Rust tests 的通过表示执行、计量、fixture 合法性与一致性 / 保护性断言成立，不是中文质量门禁。已知漏召回没有被写成必须持续失败的断言，修复后可以自然提高指标。

## 2026-10-28 实测

平台为本机 `aarch64-apple-darwin`，Rust / Cargo `1.96.0`，locked / offline debug 测试；production 基线 `3152f2c` 加本批测试文件。每组数据规模相同，六组逐查询输出和顺序完全一致，以下指标只按一组 24 个查询计算，不将重复运行当作更多独立样本。

| 指标 | 结果 | 分母与含义 |
| --- | --- | --- |
| Micro Recall@5 | 15 / 30 = 50% | 所有有答案查询的相关来源命中数 / 标注相关来源数 |
| 有答案查询命中率 | 14 / 20 = 70% | 至少命中一个相关来源的查询数 / 有答案查询数 |
| 来源集合完全匹配 | 12 / 24 = 50% | 含 4 个正确空结果；有答案完全匹配为 8 / 20 |
| 返回结果相关比例 | 15 / 15 = 100% | 本批实际返回结果全部相关；不能用低召回下的该指标宣称质量已达标 |
| 误召回来源数 | 0 | 各查询实际集合减预期集合后的数量之和 |
| 无答案查询误召回率 | 0 / 4 | 4 个无答案 / 近似词查询都为空；样本很小，不外推零误召回保证 |

按查询类型拆分：

| 类型 | 查询数 | 找回相关来源 / 应找回 | 观察 |
| --- | --- | --- | --- |
| 中文短词 | 6 | 4 / 12 | 蓝色、主题、权限、版本、来源及多词组合有漏召回 |
| 中文整句对照 | 2 | 2 / 2 | R01 无句号整句和另一完整句命中 |
| 标点 | 2 | 2 / 3 | 完整句加句号可找回，概念用逗号分隔仍漏连续中文来源 |
| 英文缩写 | 4 | 5 / 6 | 有空格的 API / REST / SQL 命中，紧邻汉字的 API 漏召回 |
| 中英混排 | 2 | 0 / 3 | `API 接口` 与 `UTF-8` 均未找回目标 |
| 繁体词 | 2 | 0 / 2 | `來源`、`繁體資料` 未找回原文包含相应词语的资料 |
| 英文对照 | 2 | 2 / 2 | `local storage`、`exact` 命中 |
| 无答案 / 近似干扰 | 4 | 无相关来源 | 均返回空 |

R01 在 production 入口再次复现：`d01` 正文为“项目默认使用蓝色主题。”，短词 `蓝色` / `主题` 均遗漏 d01，而无句号整句及带句号整句均命中 d01。由于新语料另含标点分隔的 d09，短词查询并非总体零结果；不能因此误判 R01 已修复。

## 判断与下一切片

本批结果排除了“只要重开或 rebuild 就能恢复这些短词”的解释，也没有观察到两种正文存储入口之间的检索差异。静态实现确认[FTS 建表](../../crates/radishmemory-sqlite/migrations/0004_local_recall.sql)仍是 `unicode61`，[共享查询](../../crates/radishmemory-sqlite/src/derived_index.rs)只按空白拆查询后作字面引用。结合连续中文整句命中、短词遗漏和标点分隔对照，证据指向索引 / 查询的词项边界；这不是字体显示问题。

下一切片应先审阅上述标注口径，再设计生产层一致的中文词项与中英边界处理，以及派生索引版本识别 / 重建兼容方案。候选必须与这批未改写的语料对比，并补标点、短词、英文边界和误召回反例；仍保留权限 / namespace / 时间 / 删除资格在查询前检查、精确来源与版本，不把 runner 扩展或全文扫描 fallback 当作修复。任何依赖、索引格式或公共检索语义变化须按对应项目流程说明影响，不由本次基线测试自动授权。

本批没有验证中文 GUI 输入法、最小窗口、Windows / Linux、真实系统 key、长文定位、分页、更新 / 删除旧版本排除、性能预算或 Q01 全部关闭条件。Q01、Q06、Q07 与完整 P1-S05 继续开放。

## 本批验证与交接

定向基线命令退出 0，3 项测试通过；随后完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0，仓库基线检查 286 个文件，notices / 检查器回归、fmt、locked Clippy 与 26 个 Rust suites 通过，合计 406 passed、0 failed、11 个由父测试调用的 ignored helper。Q01 质量结果仍为上述 15/30，不能将 406 个工程测试通过写成中文找回达标。

本机忽略目录保留定向观测、完整检查日志、六组汇总与测试 / corpus 摘要；测试合成库按独占临时目录生命周期清理。没有新 GUI / VM、系统 key、网络服务、依赖安装、远程写入、push 或 PR；本批未修改 production 代码、索引或 schema。Windows / Linux 与真实系统凭据未运行，不从本机集成测试外推。
