# 能力专题、实施切片与验收地图

整体范围见[产品定义](product-scope.md)，阶段 0–5 的顺序、依赖和出口见[MVP 路线图](mvp-roadmap.md)。本页连接能力主线、既有子专题及任务 / 验收入口，不重写 canonical schema、ADR、算法、格式或证据。

## 层级与来源

大专题按跨阶段能力归组；子专题复用架构 / 领域章节、ADR 及既有 implementation / evaluation 文档。P1-I、P1-H、P1-S 是原实施单元，P1-F / HF / SF 与 Q 是原验收或质量场景；一个子专题可以分多个切片，不新增第四层规划或替换编号。

current 维护实际能力、跨专题顺位、未关闭问题和停止线；原实施文档维护对应切片的版本、结果与剩余范围；evaluation 保存验收标准及实际证据，status 日期记录保存历史交接。地图只登记来源，不复制执行状态或另建任务卡。方案被接受、有限实现、局部验收与完整阶段通过分别判断。

近期工作以 [current 后续事项](status/current.md#后续事项)的五项顺序为准；“当前顺位”提供接入依赖与剩余范围。索引中的旧实现摘要、数字和平台记录只在其原版本范围成立，不能覆盖后续 current 或实际证据。

## 大专题

| 大专题 | 价值、范围与设计依据 | 阶段 / 协同 | 完成口径 |
| --- | --- | --- | --- |
| 资料库与安全存储 | 原始来源、版本、文件入口、对象加密、迁移、读取与删除；[Source Vault](architecture.md#阶段-1-加密内容寻址-source-vault-边界)、[隐私边界](privacy-threat-model.md#阶段-1-加密-source-vault-信任边界) | M0 / 阶段 1，后续同步复用；客户端与检索协同 | 存储、key provider、协调与宿主分别验收；对象加密不等于 SQLite / FTS 全库加密或完整恢复 |
| 检索与上下文 | 派生索引、权限过滤、中文找回、回源及 Context Compiler；[检索编排](architecture.md#7-retrieval-orchestrator)、[记忆编译](architecture.md#8-memory-compiler) | 阶段 1 / 3；依赖原始真相、时间和外发策略 | 相关性、无答案、权限拒绝、引用、成本与刷新分别有证据；合成命中不关闭完整 Q01 / Q04 |
| 记忆生命周期 | 候选、确认、更正、时间、冲突与遗忘；[记忆模型](memory-model.md) | M0 基线 / 阶段 2；检索与模型集成协同 | 模型输出不直接生成确认事实；拒绝、版本、冲突和删除按原场景验证 |
| 客户端体验 | 本地宿主、目录、可见交互、恢复窗口与中文显示；[宿主架构](architecture.md#阶段-1-本地资料库宿主边界) | 阶段 1，阶段 5 伴侣体验后置；存储与检索协同 | 正常 / 取消 / 错误 / 重开及平台可见证据成立；headless 不代替原生窗口，虚拟形象不独立持有画像 |
| 同步与模型集成 | 官方 / 自部署端点、设备域隔离、零知识同步与可选 Gateway；[连接规划](client-server-connection-plan.md)、[Mind 边界](radishmind-boundary.md) | 直接 adapter 基线 / 阶段 3 Gateway / 阶段 4 同步 | 每次外发可追溯且失败关闭；双端点、恢复与吊销实际验收；当前不因此启动同步、模型或网络 |
| 交付、质量与评测 | M0 固定评测、平台 / 原生验收、质量关闭、依赖与分发说明 | 贯穿各阶段，整合其他主线证据 | 输入、版本、平台、预算、结果和未验证面明确；有限正常链路不关闭 P1-S05，测试数不等于产品完成度 |

## 子专题主要归属

以下入口各有唯一主要归属；跨专题只链接协同。ADR、schema、隐私与案例是共同约束，不复制为各主线的另一套合同。

| 主要归属 | 子专题 / 原来源 | 实施单元与协同 | 验收与实际结果来源 |
| --- | --- | --- | --- |
| 资料库与安全存储 | [文件入口](adr/0006-phase1-text-markdown-file-entry.md) | P1-I01–I04；客户端提供显式选择，检索使用派生文本 | 原 P1-F01–F18 与 current；原件精确导出不代表整体记忆恢复 |
| 资料库与安全存储 | [对象加密合同](adr/0008-phase1-encrypted-source-vault.md)、[依赖与密码套件](implementation/phase1-encrypted-source-vault-dependency-review.md)、[portable crypto](implementation/phase1-source-vault-portable-crypto.md) | P1-S01 / S02 / S03a；原密钥、AAD、对象边界 | ADR 原 P1-SF01–SF18 与各实施记录，不扩大信任保证 |
| 资料库与安全存储 | [对象文件系统](implementation/phase1-source-vault-filesystem.md)、[key provider](implementation/phase1-source-vault-key-provider.md) | P1-S03b / S03c-2；原平台 adapter 范围 | 各文档的平台证据和限制；完整原生接入统一由 P1-S05 收集 |
| 资料库与安全存储 | [密钥初始化](implementation/phase1-source-vault-key-bootstrap.md)、[正文迁移](implementation/phase1-source-vault-body-migration.md)、[加密 capture](implementation/phase1-source-vault-capture.md) | P1-S04a / S04b 及后续协调；v7 / v8 / v9 保持原维护身份 | 原故障与中断证据；默认 v6 与显式已迁移入口不混用 |
| 资料库与安全存储 | [inventory 核对](implementation/phase1-source-vault-reconciliation.md)、[显式放弃](implementation/phase1-source-vault-abandonment.md)、[verify / rebuild](implementation/phase1-source-vault-maintenance.md) | P1-S04；精确请求 / pending / 派生维护 | 各原实施记录；不自动推进未授权删除或清理用户材料 |
| 资料库与安全存储 | [对象删除](implementation/phase1-source-vault-deletion.md)、[历史请求兼容](implementation/phase1-source-vault-legacy-deletion.md)、[认证读取](implementation/phase1-source-vault-reader.md) | P1-S04；v11 / v12、原请求、目录 / 搜索与精确导出 | 原结果、历史请求检查及跨进程证据；派生与备份保留范围单列 |
| 资料库与安全存储 | [Keychain 有限诊断](evaluation/phase1-keychain-prompt-diagnostic-plan.md) | D01 / D02 / M01；客户端与原生验证协同 | 原冻结件与日期结果；M01 按 current 暂缓，不沿用旧授权或绕过摘要 |
| 检索与上下文 | [版本化中文检索](implementation/phase1-chinese-recall.md)、[检索编排](architecture.md#7-retrieval-orchestrator)、[Context Compiler](architecture.md#8-memory-compiler) | ADR 0009、Q01 / Q04 / Q05 / Q06；模型外发协同 | 原 production 修复记录；相关来源标签、规模成本与可见切换独立验收 |
| 记忆生命周期 | [状态机与确认](memory-model.md#状态机)、[时间与冲突](memory-model.md#时间与冲突)、[遗忘与删除](memory-model.md#遗忘与删除) | M0 / 阶段 2；跨设备传播属于阶段 4 协同 | [M0 场景](evaluation/m0-local-memory-loop.md)与阶段 2 原出口；runner 通过不代表完整控制台 |
| 客户端体验 | [本地宿主](adr/0007-phase1-local-library-host.md)、[application 加密接入](implementation/phase1-source-vault-application.md)、[桌面加密宿主](implementation/phase1-source-vault-host.md) | P1-H02–H05 / P1-S04 宿主接线；存储主线提供契约 | 原 P1-HF01–HF12、原生宿主记录及 P1-S05；默认 v6 派生维护缺口单列 |
| 客户端体验 | [中文显示](implementation/phase1-desktop-chinese-display.md) | 固定字体 / 中文窗口；检索不以字形通过代替召回 | 原字体、headless 与可见窗口记录；760×520、Windows / Linux 新字体按 current 补验 |
| 同步与模型集成 | [端点与客户端连接](client-server-connection-plan.md)、[Mind 接入](radishmind-boundary.md) | 阶段 4 与阶段 3；ADR 0003 / 0004 保持原边界 | 原阶段出口与外发策略；零知识、Gateway 与服务选择不是现已交付事实 |
| 交付、质量与评测 | [M0 合成评测](evaluation/m0-local-memory-loop.md)、[fixture 合同](evaluation/m0-fixture-contract.md) | M0 固定输入、操作与指标 | 原 runner 与 schema 证据，不更改样本 / 评分或用 production 输出反推期望 |
| 交付、质量与评测 | [本地资料库质量](evaluation/phase1-local-library-quality.md)、[中文基线](evaluation/phase1-chinese-retrieval-baseline.md)、[候选比较](evaluation/phase1-chinese-retrieval-candidates.md) | Q01–Q08；相应主线负责修复 | 原质量关闭条件、标签与结果；Q01–Q06 优先，Q07 / Q08 沿原进入条件 |
| 交付、质量与评测 | [原生验收](evaluation/phase1-source-vault-native-acceptance.md)、[Windows / Linux provider 与 GUI](evaluation/phase1-windows-linux-provider-gui-plan.md) | P1-S05 整合；存储 / 客户端 / 检索协同 | 原操作单与结果，保持固定输入、授权与未覆盖窗口；不重新消费历史操作包 |
| 交付、质量与评测 | [依赖基线](implementation/m0-rust-dependency-baseline.md)、[桌面依赖](implementation/phase1-desktop-dependency-review.md)、[第三方 notices](implementation/phase1-third-party-notices.md) | 工程与分发跨阶段约束 | 原锁定图、许可和目标范围；构建不等于发行包或供应链已全面验收 |

## 当前五项后续的任务与验收链

不改变 [current 后续事项](status/current.md#后续事项)的编号、顺序或暂停条件；本表只给归属和来源。

| 接续项 | 牵头 / 协同 | 原任务与验收来源 |
| --- | --- | --- |
| 1. Q01 标签与 Q04 成本 | 检索与上下文；质量主线独立复核 | 原[质量场景](evaluation/phase1-local-library-quality.md#场景与关闭条件)、中文基线 / 修复及[100 来源成本](evaluation/phase1-local-library-scale.md)；人工标签与下一档预算独立评审，不删安全校验 |
| 2. 新格式有限可见验收 | 客户端体验；检索、存储和质量协同 | current 原操作范围、中文显示 / 检索记录及[新批次原生判据与结果](evaluation/phase1-recall-native-acceptance.md)；macOS 已观察尺寸的切换 / 重开 / 导出已有证据，最小窗口与平台证据分列 |
| 3. 文本资料库质量闭环 | 客户端体验；检索与存储协同 | Q02 / Q03 / Q06 原关闭条件；第 201 条、默认 v6 failed-open、回源 / 更新 / 删除刷新分别关闭，已通过范围不重复列为未验 |
| 4. 其余失败窗口与契约 | 资料库与安全存储；客户端、质量协同 | [有限检查点](evaluation/phase1-source-vault-native-acceptance.md#2026-10-13-剩余宿主窗口核对与有限检查点)；key 取消、pending 关闭和 evidence_digest 契约各自评审；完整 P1-S05 与 PDF / 图片停止线保留 |
| 5. M01 暂缓 | 资料库与安全存储；质量主线协同 | 原诊断冻结件及[10 月 24 日记录](status/2026-10-24-source-vault.md)；先审阅绑定与独立范围，不能重签、改 key / ACL 或跳过身份检查 |

## 后续能力与维护

阶段 2 记忆控制台、阶段 3 多模型 / Context Compiler、阶段 4 零知识同步和阶段 5 伴侣体验继续按原路线图进入；不为远期子专题创建占位任务或新依赖。[长期记忆研究](long-term-memory-research.md)提供问题和连续性验证线索，不能替代当前顺位、ADR 或实现证据。

活跃工作完成时更新原实施 / 验收文档与 current 引用；精确结果与交接进入日期记录。原始对象、结构化记忆和派生投影的所有权、规范版本和隐私边界保持原来源。维护规则见[协作专题](governance/agent-collaboration.md#规划层级与状态维护)。
