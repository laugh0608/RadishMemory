# 2026-10-01 Source Vault 日终记录与明日事项

日期：2026-10-01（Asia/Shanghai）。按本地所有 refs 的当天提交时间回顾，共 8 个提交：7 个实现 / 测试提交、1 个合并提交。最后另做本页所述文档提交。现行阶段、顺位和停止线以[当前状态](current.md)为准；本页不安排自动化或后台执行。

## 今天的全部提交

| 提交 | 实际变更 | 证据与边界 |
| --- | --- | --- |
| `480b834` | 修复 Windows provider 引用上游私有 `Cred` 的编译阻断；同版本 vendor 仅调整该结构的可见性，保留冻结 target/account 与 Local persistence | 新增无系统访问的构造回归；补齐上游来源、许可证、逐文件摘要、notices 与检查器。不等于真实 Credential Manager 验收 |
| `82ec1ce` | 将 abandonment 数据库路径替换回归按 Unix / Windows 区分 | Unix 验证替换后拒绝清理；Windows 验证 SQLite 活连接阻止 rename、对象保留及中断后恢复。Windows 专属测试不计作 macOS 实际运行 |
| `689c166` | 合并 PR #4，将此前 Source Vault 阶段基线合入稳定主线 | 本地 Git 记录可确认合并；今天的全部工作不能笼统称为“只有未推送的本地提交”。本次收尾没有重新查询远程 checks，不由 merge 推断每项原生验收通过 |
| `8867e8a` | 提取 application 共用读取用例，增加消费旧库连接的显式 close | 为加密接入复用目录、回源、搜索与导出，不增加第二套 canonical port |
| `6f3ebf4` | 显式加密 location / library、准备 / 迁移 / 打开及每次用例的认证读取生命周期 | 增加 application → Source Vault 连线和 opt-in 合成 provider；完整导出期间持锁，结束释放锁与 key；读取批次全仓 321 项 Rust tests 通过 |
| `bd0c11b` | 加密 import / update、原请求恢复、精确 lineage 删除、verify / rebuild | 复用原快照及同一删除闭包；不重读原件、重新生成请求或扩大删除；写用例批次全仓 330 项 Rust tests 通过 |
| `b982cec` | 认证发现未完成的原删除授权，向 application 暴露精确 capture inspect / abandon | 查询先验证持久 evidence / 组件结果再排除 Completed；不以状态字段代替校验；前置批次全仓 334 项 Rust tests 通过 |
| `6a413f8` | 串行桌面 worker、共用 controller、显式加密 / 维护 UI、内存原请求重试及日志抑制 | 提交与刷新结果分开，失败隐藏旧视图；busy、关闭与恢复交互已接线；合成全仓 345 项 Rust tests 通过，真实 Keychain / GUI 待验收 |

批次数量是当时全仓通过数，不能相加作为独立测试总数。日终开始时 `dev` 工作区干净，领先本地 `origin/dev` 5 个提交，即合并之后的五项 application / desktop 工作；不以本地 tracking ref 代替远程实时状态。日终只修改文档，不改 Rust、manifest、lockfile、自动化规则或第三方代码，不 push、创建 PR 或发布。

## 代码—文档复核与更新

| 核对代码 | 核对结论与文档处理 |
| --- | --- |
| Windows concrete credential、vendor provenance 与平台条件测试 | 精确身份与同版本补丁边界成立；补丁不是密码算法或 Win32 行为变更。保留首轮 CI 失败及平台差异记录，不把 macOS 通过计为 Windows 运行 |
| application `read.rs`、`LocalLibrary::close`、`EncryptedLibraryLocation` / `with_reader` | 共用用例且完整操作持锁。依赖基线旧“三个第一方 library”改为历史事实，明确当前四个 runtime 第一方依赖；ADR 和 application 记录不再将已完成写用例、host 接线列作下一步 |
| capture 准备 / 执行、删除闭包与 canonical 请求 | 原请求恢复、已提交事实及旧版本关系未扩大；架构中“业务幂等和引用协调仍待后续”“filesystem 尚未接入正文路径”改为区分独立 adapter、显式加密入口和默认 v6 |
| `unfinished_delete_requests` 与 `load_evidence` | 不只相信 Completed status，恢复必须复验请求与真实组件结果；既有 `evidence_digest` 未统一复算的缺口仍存在，不把本次恢复发现等同于整库抗篡改证明 |
| worker、backend、controller 与 UI | 明确本批已提交；提交成功后的刷新 / 恢复检查错误单独展示，原请求只在 worker 内存持有。质量计划补记 Q03 / Q06 / Q07 局部进展，未关闭完整质量场景 |
| `Worker::start`、`ApplicationPaths::resolve`、`main` | **新确认的静态准备缺口**：生产入口只解析默认应用目录，没有原生测试目录参数；Engine 的测试注入不等于 GUI 已有隔离入口。将该项列为明天首步，不直接访问默认日常资料库 |
| logger 安装、panic hook 与有界错误 | provider 记录同步“抑制实现已落地，原生运行仍未验收”；无输出 logger、已有 logger 拒绝和敏感 panic payload 有子进程证据，不外推到系统 stderr / 崩溃收集 |
| manifests、Cargo.lock、notices 生成器 | 补齐 desktop 直接 Source Vault / `log` 连线的 notices 记录；新直接边沿用既有版本，未增加第三方 package。早期批次摘要保留为历史值，当前生成清单为准 |

相关更新位于[架构](../architecture.md)、[ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)、[application 记录](../implementation/phase1-source-vault-application.md)、[宿主记录](../implementation/phase1-source-vault-host.md)、[provider 记录](../implementation/phase1-source-vault-key-provider.md)、[依赖基线](../implementation/m0-rust-dependency-baseline.md)、[notices 记录](../implementation/phase1-third-party-notices.md)和[质量验收计划](../evaluation/phase1-local-library-quality.md)。索引与当前状态同步指向本页。

这是按当天代码变化进行的文档和证据复核，不是独立密码学审计或完整产品验收。现有隐私模型已经准确区分对象、FTS、内存快照和真实系统证据；本次不改产品范围、canonical schema、记忆确认、数据所有权、RadishMind 职责、零知识同步目标或许可证。

## 验证与环境收尾

实现基线 `6a413f8` 的完整检查已通过：249 个仓库文件、notices、fmt、locked all-targets / all-features Clippy，25 个 Rust test suites 共 345 项通过；11 个 ignored helper 由父测试实际调用。另有 43 项 Python 检查器回归、M0 fixture（12 场景 / 86 操作 / 12 gate）、1 个 compile-fail doctest 和默认 desktop 构建通过。生产 normal / build / features 图不包含 `acceptance-test-support`。

日终文档修改后的 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 完整通过：250 个仓库文件、notices、fmt、locked Clippy 与 345 项 Rust tests，11 个 ignored helper 由父测试执行。已知 P1-F17 合成临时端口需要沙箱外绑定，本次沿权限流程获准运行同一检查，未跳过或放宽门禁；最终文本与 `git diff --check` 复核通过。本次不重复真实系统、GUI、VM 或远程验证；不能把本地文档检查记为跨平台验收。测试只使用合成资料与隔离目录，任务日志保留在仓库外临时目录；无长期测试服务或后台开发安排。

## 明日事项（2026-10-02）

1. **先核对交接与隔离入口**：检查 Git 状态、今日最终文档提交和[当前状态](current.md)。application 与 worker 已接线，不重复实现。先为原生验收确定最小、显式的独立测试根 / host profile 入口及其约束，确保不访问默认日常资料库；需要实现启动参数时另按明天的任务范围推进，不用替换 HOME 或全局配置制造隔离。
2. **准备可审批的原生验收计划**：列出合成 namespace / device、精确 Keychain slot、测试目录、执行命令、预计时长、可能的授权提示、失败保留和最终清理方式，再取得真实 Keychain / GUI 操作授权。不能沿用今天的依赖或合成测试授权。
3. **先做 macOS 最小真实闭环**：用合成文件验证显式 key 准备 → 正文迁移 → 加密打开 → 导入 / 更新 / 搜索 / 历史导出 → 关闭重开 → 删除和真实 evidence。观察缺 key、拒绝 / 取消、logger 输出及多实例锁行为；按实际可安全构造的系统状态记录 locked 等覆盖，未执行项保持待验，不影响其它凭据。
4. **验证恢复与 GUI 交互**：核对 busy 禁止重复操作、关闭时原快照损失提示、原请求重试、重启后精确放弃和原删除授权恢复，以及“写入成功但刷新失败”的提示。对派生损坏库走完整 GUI 维护 / 重开路径；已有 headless 测试不能替代可见入口验收。
5. **完成后再进集中跨平台检查点**：补齐 Windows / Linux 当前代码的 locked 编译、合成运行及约定的真实 provider / 宿主验收；保留 Windows 数据库分享模式与 Unix rename 的差异。macOS 或 PR #4 的旧基线不覆盖今天新增的全部代码。
6. **独立处理已知缺口**：`evidence_digest` 的摘要覆盖、历史 fixture / 数据兼容及统一复算先形成受审阅方案，不借验收任务静默改变协议。中文检索、201 条目录、正文定位、性能和默认 v6 的 failed-open 修复继续按 Q01 至 Q08 跟踪。
7. **保持停止线**：默认启动仍为 v6，显式加密模式的 FTS 仍有完整明文；不进入真实个人资料、PDF / 图片、模型、同步、整体备份恢复、签名发行或部署。未推送的本地提交保留，远程操作另行授权。

今天到文档提交为止。以上是明日工作建议，不是定时提醒、自动运行或今晚继续开发的授权。
