# RadishMemory 与 RadishMind 的边界

## 结论

RadishMemory 与 RadishMind 是职责不同的独立项目：

- RadishMemory 是用户个人资料、长期记忆、隐私、同步和上下文编译的真相源。
- RadishMind 是可选的模型网关、工作流、工具编排、用量追踪和评测平台。

两者可以集成，但不能共用业务真相、数据库或隐式授权。

## 职责矩阵

| 能力 | RadishMemory | RadishMind |
| --- | --- | --- |
| 原始个人文件与对象 | 拥有 | 不拥有 |
| Observation / Claim / Preference / Episode | 拥有 | 不拥有 |
| 用户资料权限、敏感度和保留策略 | 拥有 | 只消费本次授权结果 |
| 多端同步和设备密钥 | 拥有 | 不拥有 |
| 检索与 ContextPack 编译 | 拥有 | 可执行受托工作流，但不持久化真相 |
| Provider/Profile 路由 | 可直接实现基础适配 | 可作为主要实现 |
| GPT/Claude/Gemini 等协议兼容 | 保留直接调用能力 | 适合统一承载 |
| Workflow、工具编排和模型评测 | 只定义所需契约 | 适合承载 |
| 用量、成本和调用 Trace | 保存与个人请求相关的引用 | 可提供运行记录 |
| MemoryProposal 最终确认 | 拥有 | 只能提出候选 |
| 虚拟形象和个人伴侣 UI | 拥有 | 不拥有产品人格真相 |

## 推荐调用流

```text
RadishMemory
  1. 接收用户任务
  2. 执行权限与外发策略
  3. 检索并编译 ContextPack
  4. 生成 OutboundContextManifest
           │
           ▼
RadishMind（可选）
  5. 选择 Provider/Profile
  6. 执行模型或受控工作流
  7. 返回 ModelResponse、UsageRecord、Trace 和 MemoryProposal
           │
           ▼
RadishMemory
  8. 展示回答和引用
  9. 校验、审查并决定是否写入记忆
```

## 最小集成契约

RadishMemory 向 RadishMind 提交的请求不应包含资料库访问凭据，而应包含已经编译完成的最小上下文：

```text
request_id
task
context_pack
provider_constraints
capability_requirements
privacy_classification
timeout / cancellation
expected_output_schema
```

RadishMind 返回：

```text
request_id
model_response
citations_or_source_handles
memory_proposals[]
usage_record
provider_trace
sanitized_failure
```

RadishMind 不应收到 RadishMemory 的数据库连接、根密钥、全库搜索 Token 或无范围限制的长期访问凭据。

## 安全约束

- RadishMemory 在调用 RadishMind 前完成权限和外发过滤。
- RadishMind 不得扩大 ContextPack、回查未授权资料或把上下文用于其它 workflow。
- MemoryProposal 必须回到 RadishMemory 规则层，RadishMind 不直接提交已确认记忆。
- 两个项目分别保留审计 ID，通过 `request_id / trace_id` 关联，不复制完整私密正文。
- 失败不得回退到权限更宽、隐私等级更低或未经用户允许的 Provider。
- RadishMind 不可用时，RadishMemory 的本地采集、浏览、搜索和资料导出仍应可用。

## 可复用能力

RadishMind 已有或适合继续发展的能力包括：

- Provider registry 和多协议适配；
- 模型选择、健康检查和失败分类；
- API Key、用量、成本和请求历史；
- Workflow、RAG 与 evaluation 基础设施；
- canonical schema、审计和 `requires_confirmation` 治理经验。

这些能力应通过稳定 API 或未来独立 SDK 复用，不通过复制 RadishMind 全部数据库和产品面复用。

## 集成阶段

### 阶段 1：无依赖

M0 不使用 RadishMind、直接模型适配器或生成模型，只以确定性本地流程验证采集、检索、ContextPack 和记忆治理。M0 完成后，后续单机阶段可以使用 mock 或一个直接模型适配器验证模型调用，但不得改变已冻结的记忆真相与确认边界。

### 阶段 2：可选 Gateway

通过稳定请求契约接入 RadishMind，验证至少两个 Provider 与一个本地模型，并比较返回结构、引用和用量。

### 阶段 3：受控 Workflow

把复杂整理、评测或批量任务交给 RadishMind，但保持资料授权短期、最小化和可撤销。

### 阶段 4：共享协议包

只有在两个项目的真实实现稳定后，才考虑提取共享 schema/SDK；不在需求尚未验证时建立过早的跨仓库抽象。
