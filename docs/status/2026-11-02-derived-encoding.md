# 2026-11-02 派生字段编码局部优化

## 提交与结果

项目所有者先要求提交工作区、再继续下一步。已复核上一切片 20 个文件并使用当前 Git 身份提交为 `2156759`（`test(perf): 建立中文标签审阅与读取成本基线`），提交后 dev 工作区干净，未 push。随后按 current 第一项细分派生复验并实现局部编码优化；本后续切片尚未暂存或提交。

[实现、输入与对照](../implementation/phase1-chinese-recall.md#2026-11-02-派生字段编码局部优化)保存两份源码 / binary 摘要、完整四进程样本矩阵、空间、资源范围和未验证面。新增两项子计时，将四位十六进制 Han token 直接写入既有字段，移除每个 run 的临时 String；不更改字节规则、schema、权限、事务、身份、认证或删除语义，无依赖 / lockfile 变化。

两组相同 release 合成 fixture 的加密中文 5 命中 p50：509.168 → 468.397 ms、502.620 → 454.724 ms，约改善 8.0% / 9.5%；字段重算从约 52 ms 降至约 10 ms。13 轮对象认证、27 次 FTS 内部检查、448 次字段重算等调用次数保持。数据只覆盖 32 份单版本、704,918 bytes 合成资料，不冻结生产预算或规模保证。

## 验证

- codec 定向测试：5 passed / 0 failed，含两个新兼容性回归；验证固定字段 bytes、冻结范围全部码点与原格式化 oracle、既有字符 / 词项边界与查询限额。
- workspace / all-targets / all-features locked Clippy：退出 0。
- 两份 release 成本 binary 构建均退出 0；按 before-a → after-a → before-b → after-b 串行执行，各 1 passed / 0 failed、进程返回 0，结果数量、阶段调用次数和空间统计一致。
- 完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 退出 0：305 个文件的仓库基线 / checker 回归、fmt、workspace all-targets / all-features locked Clippy 通过；29 个 Rust suites 合计 432 passed、0 failed、13 ignored（12 个父测试调用的隔离 helper、1 个已独立手动运行的成本测试）。共享指标回归不计为独立质量样本。
- 默认 `CARGO_NET_OFFLINE=true cargo build -p radishmemory-desktop --locked` 退出 0；normal / build feature 图确认计量 feature 未启用。
- 最终源码摘要与 after 测量一致，两份冻结 binary 摘要一致，Cargo.lock 无差异；逐样本交叉核对四次运行的 312 × 12 个阶段调用计数均相同。收尾复核 git diff / status、未跟踪文件与文档基线，未发现越界文件、秘密、意外换行或权限变化。

## 保留范围

没有启动 GUI / VM、访问真实系统 key store、修改历史验收库 / M01 冻结件或执行远程动作。独占合成 fixture 由各测试清理，原始日志、两份冻结 binary、artifact JSON、源码 patch 和身份 / 资源统计保留在忽略目录 `.tmp/2026-11-02-derived-cost/`，上一批产物与测试 key 保留。全部测试进程均有界执行，无新增常驻进程。

独立人工标签、原生升级 / 降级 / 最小窗口、Windows / Linux、新规模与多版本、物理冷缓存、真实断电均未执行；Q01 / Q04 / P1-S05、SQLite / FTS 明文和 PDF / 图片停止线保持。后续切片的改动留在 dev 工作区，未提交、未 push。

提交收口：项目所有者随后要求提交当前工作区，本批 8 个源码 / 文档文件随本次提交纳入 dev。上文“未提交”保留为优化实施收尾快照；精确提交标识以 Git 历史为准，未 push。本次提交复核源码摘要与既有 432 passed / 0 failed 结果一致，未重跑完整 Rust 测试；仅更新提交交接文字并重新执行仓库文档基线与 diff 检查。
