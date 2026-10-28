# ADR 0009：版本化中文检索与显式派生索引升级

日期：2026-10-28

状态：Accepted（本任务已确认汉字连续短语、显式升级及旧客户端拒绝 / 显式降级范围；实施与验收另记）

契约标识：`radishmemory.recall.han-basic-a/1`

## 背景与范围

[production 基线](../evaluation/phase1-chinese-retrieval-baseline.md)复现中文短词遗漏；[候选实验](../evaluation/phase1-chinese-retrieval-candidates.md)排除了直接 trigram 与无序逐字 AND。项目所有者接受汉字连续短语、旧库显式升级派生索引，以及升级后旧客户端拒绝、显式重建旧格式才能降级的兼容边界。

本决策只修改本地派生全文检索，不改变 canonical schema、记忆确认 / 历史 / 删除语义、正文存储模式、权限或 Provider 信任边界。没有依赖或许可证变化。固定样本标签的独立人工评审、跨平台与真实 GUI 验收仍需单独报告，批准实现不等于这些证据已取得。

## 检索规则

- 首个 profile 固定拆分 U+3400–U+4DBF 与 U+4E00–U+9FFF。区间内字符编码为 `h` 加小写十六进制码点，一个字符对应一个 FTS token；区间外保持 SQLite unicode61 的字面词规则，不声明完整 Han / CJK 支持。以后扩大字符集必须使用新 profile，不依赖运行系统的 Unicode 版本改变已建索引。
- 连续汉字段按相邻且有序的 token 短语查询。不同汉字段之间插入 `b` token，禁止跨标点、Latin、空白、组合字符或 variation selector 拼接。不同原始码点保持不同，不做规范化等价、简繁转换、词干、前缀或同义词扩展。
- 非汉字查询字段保留非汉字原文；每个汉字段替换成空格包围的固定汉字 `界`，占据不可由非汉字查询生成的位置，避免 Latin 短语跨汉字段拼接。汉字查询限定在汉字字段，非汉字查询限定在非汉字字段，不暴露标记或原始 FTS 操作符。
- 查询先按上述汉字 / 非汉字边界分段，非汉字段再按空白分项。每个汉字段是一个短语；每个非汉字项由锁定 SQLite 3.53.2 的 unicode61 生成有序 token 短语；各项 AND。标点无 token 的项忽略；全无 token 返回空集合。`蓝色` 不等于 `蓝，色`，`foo-bar` 保持短语而 `foo bar` 是两个 AND 项。
- 非汉字分词使用独立内存 SQLite FTS / vocab，只处理当前查询，没有资料正文或磁盘路径，不注册 unsafe tokenizer、不引入另一个 Unicode 分类表。正常结束 / 错误均释放连接，不持久化查询，不以全文扫描或 fallback 查询代替 MATCH。
- 新 profile 每次查询最多 4096 个 UTF-8 bytes、128 个非空项；超出返回明确 `SearchQueryLimit`，不截断后成功。单项不会触及 SQLite 的超长 token 截断。旧 profile 保留旧查询行为。
- 共享 `LocalSearch` 同时处理 SourceFragment 与 MemoryRecord。canonical / derived 校验及 namespace、敏感度、时间、状态、删除资格先于查询与 top-k；结果继续返回精确 canonical 来源、版本与字节范围。变换后的 token 不作正文、高亮或 citation 偏移。双字段 BM25 可能改变排名。

## 派生 schema 与识别

现有 SQLite v6 / v7–v12 继续表示正文及恢复状态，历史 migration SQL 保持。旧派生格式为 `LegacyUnicode61`，没有 profile 表且必须严格符合旧结构。新格式为 `HanPhraseV1`：

- `radishmemory_recall_profile` 是 STRICT 单例表：`singleton INTEGER PRIMARY KEY CHECK(singleton = 1)`，`profile TEXT NOT NULL`，唯一合法值为上述契约标识；恰有一行。
- `radishmemory_recall_fts` 保留 `object_kind`、`object_id`、`namespace_id`、`sensitivity` 和精确 `content`，均 UNINDEXED；新增 indexed `han_terms`、`word_terms`，tokenizer 固定 unicode61。每行三个内容字段都须与验真的 canonical 重算结果一致，并检查 FTS 内部索引完整性。
- 基础表集合 / schema 定义由历史 migration 推导，再仅对已知 profile 应用明确的派生定义。任何未知 profile、缺行、多行、部分结构、profile 缺失但新字段仍在、未知表 / view / trigger 均失败关闭。不放宽全局 schema 校验。
- 新创建的库直接使用新 profile；旧库正常打开不自动升级。新版本可读旧 profile，使用与其一致的旧查询，并在桌面显示升级动作。新 profile 的额外表使旧客户端拒绝，不能在旧客户端中忽略后继续写入。

## 原子升级、降级与恢复边界

健康 v6 和正文迁移完成的 v8–v12 支持显式切换 profile。操作取得现有事务 / 独占会话，验证结构、身份、canonical、正文与当前派生数据，确认无未完成 capture / 放弃 / 删除请求，再重建目标 FTS 和登记 / 移除 profile，复验后单事务提交。不得以升级修复未知损坏或清理 pending。默认 v6 failed-open 修复仍不在本决策范围。

对象模式通过现有 ObjectDirectory、provider、数据库身份与认证正文链执行，commit 前重新验真对象，commit 后 read-back；不访问 inline 明文或外部原件 fallback，不创建 key。失败发生在 commit 前时回滚旧格式；commit 后复验失败如实返回失败，恢复应重新检查真实 profile，不报告“未修改”或自动再切换。重复请求相同 profile 仍验真，成功可幂等返回。

升级 / 降级不改变 `user_version` 或正文模式，不删除事实、对象、原请求及证据。已升级的 v6 以后进入 key bootstrap、迁移、capture、放弃、删除及历史删除接管，所有路径必须保留并校验 profile。v7 / 未迁移完 v8 不开放独立切换；已有 profile 可以跟随原正文恢复链完成。

降级是显式地从当前验真 canonical 重建旧索引并移除新 profile；不是还原旧数据库备份。失败 rollback 保留之前状态。旧客户端只恢复到其本来支持的正文版本，普通旧 v6 入口不会因此获得打开加密库的权限。

## 桌面与验证要求

单 worker 提供可见格式状态、升级 / 降级确认与真实结果。界面说明中文短词能力和旧客户端兼容代价；busy、未验证视图及原请求恢复期间禁用切换。取消确认不发命令。切换后清空旧搜索结果并刷新；失败时撤回旧视图，显式刷新重新检查，保持既有关闭与原请求规则。

验证必须覆盖：两种 production 入口的固定语料与独立反例；SourceFragment / MemoryRecord 的权限、namespace、时间、状态与删除；旧库打开保持、升级 / 降级、跨连接可见、真实旧代码拒绝、事务失败及后验失败；新 profile 通过正文初始化 / 迁移及恢复链；unknown / drift 失败关闭。固定样本建议 30/30、24/24 集合匹配、0 误召回，且不因此关闭人工评测、长文性能或跨平台 / 原生门禁。

FTS 与新增字段继承正文敏感度，仍以明文存在 SQLite。此变更不提供整库静态加密、历史页擦除、备份清除或零知识保证。大小、耗时与查询延迟必须实测后记录；不存在未经测量的性能承诺。
