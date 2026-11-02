# 阶段 1 本地资料库 100 来源 / 多版本成本基线

日期：2026-11-02。状态：固定合成基线已执行，Q04 完整规模 / 冷启动 / 生产预算验收保持开放。

归属：[本地资料库质量计划 Q04](phase1-local-library-quality.md#场景与关闭条件)。本页记录一个有界档位的输入、实际入口、成本与结果；不新增 canonical schema、production API、性能 CI gate 或另一套存储真相。

## 场景、输入与范围

新增显式手动目标 `q04_cost`；默认 CI 只运行同目标中的固定输入检查及 6 来源 / 12 版本正确性 smoke，100 来源成本测试为 ignored 且要求 release。沿既有 Q01 fixture 的独占目录、Runtime / profile 和 opt-in `read_cost` 计量；将旧 `q01_cost` 的 Library 生命周期、搜索分派、逐样本输出和 nearest-rank 计算抽到同一个 test-only `cost` 模块。旧 32 份入口、生成方式、24 组、样本数、命中数与输出字段保持，未把旧命令静默扩大到 100 份。

固定输入只有合成 `.md`，corpus 标识 `q04-hundred-mixed-v1`，由[生成器](../../crates/radishmemory-application/tests/q04/corpus.rs)明确定义，没有随机 seed：

| 项目 | 固定值 |
| --- | --- |
| 逻辑来源 | 100 个 lineage；本文“来源”按 lineage 计数 |
| 正文版本 | 160 个 SourceArtifact 版本；60 个来源 1 版、20 个 2 版、20 个 3 版 |
| 版本规则 | 索引 i 为 0–99；i % 10 为 0 / 1 时 3 版，为 2 / 3 时 2 版，其余 1 版 |
| 长短分布 | i % 4 == 0 的 25 个来源每版 16,384 bytes，其余 75 个每版 768 bytes |
| 正文总量 | 当前版本共 467,200 bytes，全部历史与当前版本共 747,520 bytes |
| 内容边界 | 确定性重复的中英合成段、精确补齐空白与尾部独立版本 marker；无真实个人资料 |
| 写入顺序 | 先按 i 导入全部 v1，再按 i 更新可有 v2 的 40 个来源，再更新可有 v3 的 20 个来源 |
| 读取格式 | 当前默认 HanPhraseV1；本档位不混入旧格式升级 / 降级 |
| 实际入口 | production LocalLibrary 与显式 EncryptedLibrary，真实 SQLite / crypto / object adapter，SyntheticLibraryProvider |

文件准备 / 改写、oracle 重算、结果核对和打印在业务 API 计时之外。plain 导入 / 更新分别调用原 application 方法；encrypted 测量完整 prepare + capture，包含原请求准备的身份 / pending / key / 对象检查，没有以直接 SQLite 插入或整体迁移替代增量产品写入。加密入口先对新空合成库完成原密钥准备 / 空库迁移，再从空库增量写入 160 个版本。

## 正确性、失败与重建

- 100 条目录均对应 receipt 建立的独立 lineage / 当前 source ID 映射，逐条核对当前版本、版本总数与长度；存储观测另外核对 canonical 版本行数 160 与对象数量。
- 多版本 API 核对指定 3 版来源的完整历史 ID、版本、current 标志和长度；6 来源 companion 还逐一覆盖 1 / 2 / 3 版历史。
- 固定七类查询：中文 top-k 1 / 5、长文来源最新唯一 marker、最后一个来源最新唯一 marker、已过期的旧版 marker、无答案、仅允许 Restricted 的拒绝。当前 marker 与无答案 marker 是输入定义，不根据算法输出改标签，也不把预期来源拼入 production query。
- 每个实际 hit 都必须是独立映射里的当前 source，满足预定唯一来源 / 命中数量、版本、lineage、整文件 fragment 的精确正文与 byte range。共同中文查询允许任何合格的当前来源，不能据此宣称排名质量或人工相关性通过。
- 导入 / 更新结束、关闭重开、显式 rebuild 后均执行同一组查询。两个模式、三阶段、每阶段 7 查询的真实结果全部通过：旧 marker / 无答案 / 拒绝均为空，正向对照能命中。没有借常量成功宣称权限过滤成立。
- Verify / Rebuild 均走现有 application API。加密 report 实际验证 160 个 committed 对象，pending / abandoned / deletion 数均为零，profile 保持，rebuild 标志与请求一致；默认 API 的真实 Ok 继续执行后续目录和正文回归。业务错误、缺失结果或计量不平衡使测试失败，不 fallback、不吞错误。

本次只覆盖健康合成库的重建，不替代受损库 failed-open 修复、错误 key / 取消、真实断电或恢复。回源核对仍为整文件 fragment，不证明正文后段高亮定位、UI 结果刷新或第 201 条目录可到达；Q02 / Q03 / Q06 等保持原关闭条件。

## 计量口径与环境

两轮独立进程 A / B 串行执行同一冻结 release binary，返回码均为 0。计量期间没有并行运行本任务的构建 / 测试。每轮各完成两种模式：每模式 305 个 timed API 操作（空库 setup 1、导入 100、更新 60、已填充库 open 12、11 类读取 / 维护各 12），共 610 条样本、每样本 12 个固定阶段。另有 oracle / 准备性打开 / 正确性查询，这些不混入 API 操作时间，但包含在整个 test binary 的资源统计内。

| 操作 | 样本与解释 |
| --- | --- |
| setup | 每模式单次创建 / 显式空库准备，不作为已填充库启动样本 |
| import / update | 100 / 60 次真实增长过程，逐条保留来源数、总版本数、输入长度；p50 / p95 是增长序列的分布，不是固定容量上的独立重复请求 |
| open | close 在计时之外；12 次纯 application open，第 0 次单列，后 11 次汇总。加密使用 open 而非桌面 open_with_snapshot，不等于整个 GUI 启动 |
| catalog | list_sources(0, 200)，返回并核对 100 条；不包含桌面绘制或组合恢复 snapshot |
| versions | 指定 3 版来源的 list_source_versions |
| query / verify / rebuild | 每组先关闭重开；第 0 次为重开后首次，后 11 次为同一进程连续调用；oracle 检查在计时后 |

逐样本保存 inclusive / exclusive / 未归属耗时与计数。共享日志的 `hits` 在搜索中表示 hit 数、目录 / 历史中表示条数、增长序列中表示写入次数、open / verify / rebuild 中表示成功 API 动作数；它不是统一质量分数。nearest-rank p50 / p95 明确由实际样本计算，11 个重复样本的 p95 即最大值，不作稳定生产尾延迟估计。

没有 flush OS 缓存、控制整机后台负载 / 电源或强制物理冷启动。原生系统 key store、GUI、渲染、provider 提示等待、网络和 Windows / Linux 均未测。本机为 macOS 27.0.1 / ARM64，os.cpu_count() 为 10，Rust / Cargo 1.96.0，release 默认优化、现有 locked dev graph 与 acceptance-test-support，默认 production feature 图未增加依赖或 feature。物理硬件容量没有作为已验证条件，不冻结生产容量 / 延迟预算。

基线 commit 为 `e0b9926c8a1ada1ee630fe8f1a67ec7015d2c83c` 加本批 test-only 源码。沿前批算法对已跟踪 Rust / manifest / lock / toolchain 加三份新源码，去重排序后用 path + NUL + bytes + NUL 计算摘要，合计 169 个文件。

源码 SHA-256：`216499752fae46b5a979bff19e2c889a5be155b7de4470a5ea5c9cb5ea0d0f70`；100 来源 binary SHA-256：`3011bfd05b4c1e48b89bd713dc8483f0c78a219759aa098073f1643d91f3b6a0`；旧 32 份回归 binary SHA-256：`d03520d4912f13d7775de293db53973222ae1282e1fe11ae90418603881ccc59`。两份 frozen executable、artifact JSON、身份、源码 patch 与新增源码副本、全部样本和聚合 / resource JSON 保留在本批忽略目录 `.tmp/2026-11-02-q04-hundred/`，不改写前批冻结产物。

## 两轮实测

下表为 p50 / p95，单位 ms；导入 / 更新按上述增长序列解释，读取与维护使用后 11 次重复。原始日志还保留每个首次操作 / 单次写入及所有阶段，未剔除较慢样本。

| 操作 | plain A | plain B | encrypted A | encrypted B |
| --- | ---: | ---: | ---: | ---: |
| import | 50.022 / 121.423 | 12.682 / 24.058 | 203.004 / 349.072 | 195.644 / 514.538 |
| update | 61.158 / 76.481 | 34.797 / 38.140 | 868.825 / 1011.457 | 824.791 / 967.789 |
| open | 21.897 / 22.300 | 20.706 / 21.022 | 105.882 / 110.269 | 104.866 / 110.678 |
| catalog | 12.374 / 13.235 | 11.638 / 11.732 | 317.423 / 361.844 | 286.995 / 290.091 |
| versions | 8.961 / 14.136 | 8.467 / 8.634 | 303.419 / 354.841 | 286.135 / 288.089 |
| han_one | 12.336 / 23.263 | 11.131 / 11.511 | 471.514 / 495.130 | 474.852 / 483.964 |
| han_five | 12.239 / 13.957 | 11.290 / 11.776 | 1201.358 / 1278.059 | 1201.121 / 1213.815 |
| current_long | 11.412 / 11.688 | 10.766 / 11.064 | 499.403 / 530.211 | 471.922 / 476.788 |
| current_last | 11.268 / 11.535 | 10.621 / 10.763 | 565.963 / 1095.650 | 474.322 / 503.116 |
| old_version | 11.554 / 13.374 | 10.649 / 10.846 | 290.678 / 306.636 | 293.216 / 306.885 |
| absent | 11.275 / 11.504 | 10.635 / 10.880 | 291.205 / 292.583 | 292.996 / 301.094 |
| denied | 11.055 / 11.257 | 10.411 / 10.605 | 291.173 / 306.758 | 290.136 / 291.236 |
| verify | 19.746 / 19.855 | 18.646 / 19.080 | 178.701 / 232.179 | 179.654 / 180.277 |
| rebuild | 52.646 / 56.790 | 31.592 / 34.781 | 309.474 / 314.062 | 311.274 / 316.882 |

A / B 的每个阶段 / 操作命中数量、版本排除与字节断言通过。encrypted current_last 在 A 的 p95 为 1,095.650 ms，不能因 B 较快而删除 A 样本；成本仍有环境和操作波动。两种数据集在来源数、历史版本、输入长度和 token 分布上不同，本档位不能与此前 32 份单版本数字直接作优化收益比较，也不能按比例外推 1000 / 10000 来源。

中文 5 命中的墙钟中位样本阶段如下，时间单位 ms；同一次运行的 exclusive 可跨阶段相加，inclusive 包含子阶段，不能再加到父阶段之上。

| 阶段 | A 调用数 | A exclusive | A inclusive | B 调用数 | B exclusive | B inclusive |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| reader_open | 1 | 14.552 | 105.619 | 1 | 14.796 | 105.749 |
| object_authentication | 13 | 868.383 | 945.485 | 13 | 869.312 | 946.257 |
| canonical_validation | 2093 | 231.442 | 288.842 | 2093 | 230.596 | 287.648 |
| catalog_load | 14 | 20.416 | 20.416 | 14 | 20.336 | 20.336 |
| derived_validation | 27 | 11.847 | 62.690 | 27 | 11.754 | 62.316 |
| derived_encoding | 1400 | 5.944 | 5.944 | 1400 | 5.867 | 5.867 |
| fts_integrity | 27 | 44.900 | 44.900 | 27 | 44.694 | 44.694 |
| query_preparation | 1 | 0.275 | 0.275 | 1 | 0.265 | 0.265 |
| fts_match | 1 | 0.459 | 0.459 | 1 | 0.441 | 0.441 |
| result_resolution | 1 | 2.501 | 908.919 | 1 | 2.444 | 909.139 |

每次 5 命中仍为 13 轮全库对象认证、27 次 FTS 内部检查。全库认证面对 160 个历史 / 当前对象；派生字段重算对应 100 个当前可检索来源。认证 exclusive 包含目录 / 文件 IO、对象检查与密码操作，不能称为纯 cipher 耗时。该场景主要成本在对象认证 / IO 与 canonical 复验；任何认证组织调整仍需独立边界评审，本批未减少检查、缓存明文或复用跨操作 session。

## 空间与内存

每轮 storage 同模式的第一条为导入 / 更新完成后，第二条为重复 rebuild 后；下表按真实输出顺序标出阶段。跨轮对应统计一致。used_page_bytes 为 page_count 减 freelist_count 再乘 page_size；object_bytes 只累计 source-objects-v1 的普通对象文件。未包括输入副本、staging、journal、备份或目录 metadata，不称为整个资料库大小。

| 模式 / 阶段 | 数据库 bytes | 已用页 bytes | 对象 bytes |
| --- | ---: | ---: | ---: |
| plain / imported | 3059712 | 3014656 | 0 |
| plain / rebuilt | 3620864 | 3186688 | 0 |
| encrypted / imported | 2498560 | 2453504 | 830693 |
| encrypted / rebuilt | 3059712 | 2625536 | 830693 |

| 运行 | 整个 test binary peak RSS bytes | wall seconds |
| --- | ---: | ---: |
| run-a | 20283392 | 159.750 |
| run-b | 20217856 | 152.392 |

资源由各独立 Python runner 的 getrusage(RUSAGE_CHILDREN) 读取，本机 Darwin 的 maxrss 单位为 bytes。包括两种模式、创建 / 写入 / oracle、全部查询 / 维护、输出与 fixture 清理，不能说成每操作峰值、纯 application 或桌面 GUI 内存。没有引入实时 telemetry。

## 复验与保留边界

```sh
CARGO_NET_OFFLINE=true cargo test -p radishmemory-application --test q04_cost --locked
CARGO_NET_OFFLINE=true cargo test -p radishmemory-application --test q04_cost --release --locked -- --ignored --nocapture --test-threads=1
```

第二条只运行 100 来源 ignored 测量；旧 `q01_cost` 仍单独运行 32 份协议。测试均只创建独占合成目录，结束时清理本进程拥有的输入、数据库与对象；不会清理历史 key、用户库、M01 或旧原生材料。退出 / 错误 / 清理失败如实使测试失败，原始诊断保留。

本轮是合成验收与测量代码，没有改生产实现、schema、字节编码、权限 / 外发 / 删除、依赖或 lockfile。SQLite / FTS 与输入文件明文边界保持，原始对象加密不等于整库静态加密。本 100 来源档位的下一规模、完整启动 / 冷缓存、其它分布与平台、真实 provider、GUI 和生产预算尚待单独验证；后续[两来源库的原生升级 / 降级](phase1-recall-native-acceptance.md#2026-11-02-macos-原生局部结果)不与本档位性能证据互换。Q01 独立人工标签、Q02 第 201 条、Q03、Q06 与 P1-S05 等不由本批关闭。

本批实现与对应文档已提交为 `3f05a13`。完整验证、误差说明与交接见[本批记录](../status/2026-11-02-q04-hundred.md)，后续评审与顺位见[当日日终记录](../status/2026-11-02-chinese-recall.md)及 current。
