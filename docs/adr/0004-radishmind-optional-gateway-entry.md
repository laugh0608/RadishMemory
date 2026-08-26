# ADR 0004：RadishMind 首次以可选 Gateway 接入

日期：2026-08-22

状态：Accepted

## 背景

RadishMemory 必须证明记忆、检索、权限和 ContextPack 不依赖某个模型平台，同时又需要在完整 MVP 中验证多 Provider、用量、失败和可解释外发。RadishMind 适合承载模型网关、路由、运行观测和后续受控工作流，但如果过早成为必需服务，会让 M0、本地资料库、记忆状态机和离线能力依赖另一个仓库的运行时与发布节奏。

RadishMind 当前已经存在多协议 Gateway、Provider / Profile 路由、用量、失败和开发测试态治理资产，但其真相源明确区分开发测试能力与生产就绪，也没有为 RadishMemory 提供已冻结的生产专属协议。本项目不能把兄弟仓库中的现有 Copilot、Application 或 Workflow schema 直接当作自己的记忆协议。

## 决策

RadishMind 首次在完整 MVP 的阶段 3“多模型与 Context Compiler”中，以显式配置、可关闭的 Model Gateway 接入。M0、阶段 1 单机资料库和阶段 2 长期记忆生命周期均不依赖 RadishMind；RadishMemory 必须先用 mock 或至少一个直接模型适配器验证自己的 canonical model boundary，再增加 Gateway adapter。

首次接入只使用模型网关能力，不接入 RadishMind Workflow、Tooling、RAG 数据所有权、Session owner、Copilot 业务协议或业务写回。RadishMemory 始终保留直接 Provider adapter 和本地模型路径，但不同路径之间不得隐式回退。

### 首次接入的前置条件

开始真实 Gateway 接线前必须具备：

- M0 runner 已通过冻结 fixture，证明本地记忆闭环与模型无关；
- RadishMemory 的 `ModelRequest`、`ModelResponse`、`UsageRecord`、`OutboundContextManifest` 和错误分类已冻结；
- 至少一个 mock 或直接 adapter 已验证流式 / 非流式响应、取消、超时、结构化输出和失败关闭；
- RadishMind 提供版本化、受支持的 Gateway route、认证方式、能力发现、取消和 sanitized failure 契约；
- 双方都明确当前环境是 dev/test 还是 production，不把开发测试 route、fixture、fake Provider 或静态契约当作生产就绪。

任一前置条件缺失时，可以继续离线 contract test，但不得把 RadishMind 写入必需启动链或宣称真实集成完成。

### 所有权与适配边界

- RadishMemory 拥有来源、记忆、权限、ContextPack、外发决定、citation 解释、MemoryProposal 校验和最终决定。
- RadishMind 拥有 Gateway 内的 Provider / Profile 目录、受控路由、Provider credential、调用尝试、规范化用量和运行失败记录。
- RadishMemory 的 Gateway adapter 负责在本项目 canonical model contract 与 RadishMind 受支持的公开 northbound contract 之间翻译；兼容层不是第二套业务真相。
- 两个项目不共享数据库、根密钥、资料库凭据、全库搜索 Token、内部 repository、迁移或进程内业务类型。
- 不直接复制 RadishMind 的 Copilot、Application、Workflow 或 Session schema。只有双方真实使用稳定后，才评估独立版本化协议包。

模型输出若包含记忆候选，只是受 `expected_output_schema` 约束的不可信输出。RadishMind 可以传输该输出，但不确认、不持久化为 RadishMemory 记忆，也不绕过本项目的 proposal、decision 和来源校验。

### 最小逻辑交换

首次接入必须表达以下逻辑信息，但本 ADR 不冻结 JSON 字段名、HTTP route、SDK 或序列化格式。

请求侧至少包括：

- contract version、`request_id` 和幂等 / 重放约束；
- task 与已经编译、裁剪并带 citation map 的 `ContextPack`；
- `OutboundContextManifest` 引用和本次允许的 Gateway、Provider / Profile、用途与区域集合；
- capability、结构化输出、流式、超时和取消要求；
- privacy classification、保留限制和禁止 fallback 约束。

响应侧至少包括：

- contract version 与原 `request_id`；
- model response、结构化输出和只指向本次 ContextPack 的 citation handles；
- 实际 Provider / Profile attempt、fallback / retry 情况和最终选择；
- 规范化 usage、Gateway trace 引用和 sanitized failure；
- 若存在，仍待 RadishMemory 校验和决定的候选输出。

未知 contract version、缺少必需能力、引用越出 ContextPack、实际 Provider 不在允许集合或响应无法通过 schema 校验时必须失败关闭。

### 外发与凭据

RadishMind 是独立数据接收方，即使与 RadishMemory 部署在同一台用户服务器上也不能被隐去。若它继续调用外部 Provider，则 Gateway 和每个实际 Provider attempt 都是外发链路的一部分。

- 调用前的 manifest 记录 Gateway、允许的 Provider / Profile 集合、用途、资料引用和授权依据；调用后追加实际 attempts、用量与 trace 引用。
- 每个接收方都必须满足资料的 egress policy；Gateway 不得把 `local_only` 或 `trusted_device_only` 内容升级为可外发。
- `trusted_server_only` 只有在 Gateway 与最终模型端点都属于用户授权的可信服务器集合时才允许继续；`cloud_allowed` 仍受 Provider、区域、用途和保留限制。
- RadishMemory 只持有调用 Gateway 所需的最小作用域凭据，不接收 RadishMind 的 Provider secret；RadishMind 不接收 RadishMemory 的根密钥、Provider 之外的资料密钥或管理凭据。
- 请求正文、ContextPack、模型响应和 citation 内容默认不得进入普通 history、日志、错误或 trace；需要保留时必须由独立、显式的保留策略授权。

### 路由、重试与失败

- 默认单一已授权 route，retry / fallback 默认关闭。
- 只有本次 manifest 预先允许全部候选 Provider / Profile，且 RadishMind 能强制执行该集合时，才可显式启用受控 retry / fallback。
- 超时、取消或连接中断后的结果不确定性必须显式记录；不得用同一业务请求无界重放并造成重复计费或重复候选。
- RadishMind 不可用、能力不匹配或返回失败时，不得自动改走直接 Provider、本地模型或权限更宽的 route。用户或上层策略必须重新作出一个仍满足原外发约束的明确选择。
- Gateway failure 不改变资料、记忆或 decision 状态；usage、attempt 和失败记录只通过稳定引用关联，不复制私密正文。

### 后续 Workflow 边界

受控 Workflow、批量整理、工具调用和评测编排不属于首次接入。它们只有在 Gateway 路径已稳定、任务授权可短期化、输入输出 schema 已冻结、工具与写回需要独立确认且审计落点明确后，才可通过新的决策进入。

RadishMind Workflow 即使未来参与，也只能产生回答、证据、评测结果或 `MemoryProposal` 候选，不能拥有 Source Vault、Memory Store、同步密钥或删除真相。

## 验收门槛

首次 Gateway 集成至少使用合成或明确脱敏的数据证明：

1. RadishMind 未启动或不可达时，RadishMemory 的 M0、本地采集、浏览、检索、记忆审查、导出和删除仍可运行。
2. 同一 canonical request 可以分别通过直接 adapter 和 Gateway adapter 执行，且不修改记忆 schema 或 ContextPack 语义。
3. Gateway 只收到已编译 ContextPack 和最小调用凭据，无法访问 Source Vault、Memory Store、根密钥或全库检索接口。
4. manifest 能区分 Gateway、允许 Provider 集合和实际 Provider attempts；未获授权 route 在发送给 Provider 前失败。
5. `local_only`、`trusted_device_only`、越权 citation、未知版本、schema 不匹配和能力缺失均失败关闭。
6. retry / fallback 关闭时不会隐式换 Provider；显式开启时不会越出预授权集合。
7. timeout、cancel、ambiguous outcome、usage 和 sanitized failure 可关联到同一请求，且普通日志、history 和错误不复制正文。
8. 候选模型输出不能绕过 `MemoryProposal` 校验和 `MemoryDecision` 直接成为 confirmed memory。

这些门槛未完成前，只能声明“RadishMind 可选 Gateway 接入阶段已规划或正在验证”，不能声明生产集成、多 Provider 生产可用或隐私保证已经成立。

## 被拒绝的方案

### M0 或阶段 1 强依赖 RadishMind

这会让最小记忆闭环依赖网络、模型平台和跨仓库运行状态，无法证明 RadishMemory 自身的离线与模型无关边界。

### 只保留 RadishMind 一条模型路径

这会把个人记忆核心绑定到单一平台，也会让 Gateway 故障影响本地资料能力。RadishMemory 必须保留直接和本地模型适配边界。

### 首次接入 Workflow 或 Tooling

Workflow 和工具会同时引入长任务、执行权限、确认、重放和业务副作用，超过首个模型网关切片需要证明的范围。

### 直接复用或复制兄弟项目业务 schema

RadishMind 的 Copilot、Application、Workflow 和 Session contract 有各自 owner 与阶段停止线。直接复用会混淆业务真相，复制则会造成双份协议漂移。

## 后果

收益：M0 和本地记忆核心保持独立；Gateway 接入有清楚的数据、凭据、外发和失败边界；可以复用 RadishMind 的 Provider 路由与观测能力，而不复制其业务实现或数据库；未来 Workflow 扩展有明确前置条件。

代价：RadishMemory 需要维护 canonical model boundary 和至少一个非 RadishMind 路径；Gateway adapter 增加一次版本、能力和外发映射；动态 route、retry 与 fallback 必须与 manifest 做双边约束；两个仓库的生产 readiness 需要分别验证。

## 后续决策

实现阶段 3 前仍须冻结 RadishMemory canonical model contract、OutboundContextManifest 的 preflight / actual-attempt 语义、Gateway 认证与作用域、版本协商、错误分类、保留策略和跨仓库 contract fixture。共享 SDK 或 Workflow 接入只有在真实双端实现稳定后另行评审。
