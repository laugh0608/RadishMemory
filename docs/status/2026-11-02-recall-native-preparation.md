# 2026-11-02 旧格式与最小窗口原生验收准备

本文保留授权前准备状态；随后取得精确授权并执行的实际结果见[原生执行记录](2026-11-02-recall-native.md)，最小窗口仍未覆盖。

## 结果与顺位

按项目所有者要求先复核并提交 100 来源工作区，提交为 `3f05a13`（`test(perf): 建立百来源多版本资料库基线`），使用既有用户身份、无 AI 协作者署名；提交后工作区干净，dev 本地领先 origin/dev 三个提交，未 push。

随后承接 current 第 2 项，准备[旧检索格式与最小窗口有限验收](../evaluation/phase1-recall-native-acceptance.md#2026-11-02-旧检索格式与最小窗口验收准备)。第 1 项的独立人工标注不能由 AI 代做，Q04 后续规模预算尚未冻结；本批不扩到 1000 / 10000 或未经评审的认证优化。

默认新库已经采用 HanPhraseV1，因此本批以 production API 真实创建资料，再显式切换到 LegacyUnicode61，形成 v6 / 2 lineage / 3 source versions 的独立合成旧库。A v1 红色、v2 蓝色与简繁 / API 混排，另有 B 干扰来源；A v2 为 103 bytes、UTF-8 BOM / CRLF。3 inline bodies / 2 当前 FTS rows，无 vault、pending 或删除请求；生成后关闭连接再复制到独立预演目录。

## 已完成与验证

- 新增 test-only 760×520 headless 回归，复用 Harness 可配置 viewport；两种模式均走真实 worker，覆盖多版本 / 干扰来源下关键按钮与升级 / 降级确认框的 viewport / clip 边界，以及 Cancel 无命令 / 无 key load、Confirm 单次派发和成功撤回旧结果。目标测试 1 passed、0 failed。
- 私有有限生成器链接既有默认 Cargo 构建产物，未创建新 Cargo package 或 lockfile，未调用 key provider；public ProductionRuntime 仅提供文件业务所需的随机合成 ID / 时间。独立副本通过 production 明文升级 / 中文命中 / 旧版本排除 / 精确导出 / 重开 / 降级 / 精确导出 / Verify；待执行 data / profile 字节摘要不变。
- 独立新 app 包装仅复制默认 debug binary，无重签或安装。唯一新 profile、三个输入、精确命令、三轮串行正常链路、四份导出、760×520 实测要求、系统提示和动作预算均已写入本地操作单；当前 export / native-evidence 为空。
- 只读 preflight 对构建输入与包内不可变文件逐项验 hash，首次额外复验 v6 / 旧 FTS / canonical IDs 与正文 / 当前版本 / 无 pending / 空输出；运行后只能使用 immutable 核对，并追加实际阶段快照，禁止拿初始数据库摘要盖回。六项负向副本验证通过，未修改原待执行库。
- 初轮脚本查询把 FTS fragment ID 当 source ID，已修正关联查询；初轮 headless 读取新建 modal 太早，补齐 egui 稳定帧后目标测试通过。均未修改业务逻辑、输入或 oracle 来掩盖失败。

- 完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：312 个文件的仓库基线、fmt、workspace all-targets / all-features locked Clippy、30 个 Rust suites 通过，合计 435 passed、0 failed、14 ignored（12 个由父测试调用的隔离 helper，另 2 个历史成本测试本批未重跑）。补充完整确认文案的 viewport / clip 断言后，目标测试再次 1 passed / 0 failed，最终全 workspace Clippy 再次退出 0。
- 最终 `CARGO_NET_OFFLINE=true cargo build -p radishmemory-desktop --locked` 退出 0，包装与默认 binary 逐字节一致；181 份构建输入、13 份不可变文件及初始数据库摘要已冻结，initial / immutable 两种预检均通过。原生数据 / profile 未受预演、负向检查或构建影响，export / native-evidence 仍为空。
- 首轮仓库基线因旧原生验收文档超过 500 行失败；将本批内容路由到独立验收文档并保留原入口链接后通过，没有放宽检查或改写旧证据。最终文档基线、fmt、git diff --check、Git 状态 / 未跟踪文件与文件权限复核通过；未发现依赖 / lockfile、生产逻辑、秘密或越界变更。

## 保留与未执行

全部私有产物保留在本机忽略目录 `.tmp/2026-11-02-recall-native/`；冻结信息与实际身份不复制到正式长期文档。旧 M01 / D01 / D02、10 月 28 日包装与测试 key 均不复用、不改写。新库未查询或创建真实系统 slot，没有启动 GUI / VM、访问真实 key store、改 ACL / 全局字体 / 系统设置、安装依赖或执行远程动作。

下一步只请求冻结操作单的精确原生授权：最多三次串行 GUI、一次新 key Prepare、一次三正文迁移、两种模式的升级 / 降级与中文检索、四份导出和最小窗口，预计 40–60 分钟及 5 分钟正常收尾；保留合成资料与唯一新 key，不承诺迁移回滚。真实窗口尺寸、中文可见命中、跨平台、首尾 key 取消及完整 Q01 / Q04 / P1-S05 均仍未验。

本批 test-only 与文档为后续未提交改动；3f05a13 及此前两个提交均未推送。本批准备无遗留后台进程，fixture 运行均有限结束；最终状态以收尾 Git 复核为准。
