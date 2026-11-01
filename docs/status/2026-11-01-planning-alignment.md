# 2026-11-01 规划结构与来源整理

日期：Asia/Shanghai。按所有者确认，仅整理整体规划、能力专题、实施切片与验收来源。

## 结果与边界

- 建立六条能力主线，串联原 P1-I / H / S 切片、Q01–Q08、平台验收与当前五项后续工作，子专题各有唯一主要归属。
- 路线图移出阶段 1 重复实现流水，current 精简逐批验证日志；原 current 和路线图全文在[整理前快照](planning-before-alignment-2026-11-01.md)保留，仅重定位相对链接。
- 原阶段目标 / 范围 / 出口、current 五项后续顺序、已接受边界与停止线保留；M01 暂缓、完整 P1-S05、PDF / 图片停止线、对象加密与 SQLite / FTS 明文边界未改。
- 原 ADR、canonical schema、fixture、implementation / evaluation 正文、代码、依赖、根入口与检查器未修改。新增地图不代表质量项关闭或运行授权。

## 验证

- 默认 Python 3.9 缺少 `tomllib`，首轮 notices / 检查器回归未通过；改用本机已有 Python 3.14.5 执行 `CARGO_NET_OFFLINE=true python3.14 -B scripts/check-repo.py`，仓库基线通过（302 文件），包含原 fixture、notices 和检查器回归。未安装或更新工具链。
- 本批 7 份文档的文本、本地链接和锚点独立核对通过；38 个子专题主要来源入口无重复，current 的 9 个原标题锚点全部保留。
- current 的实际阶段、能力、接入顺位、边界、停止线和五项后续事项保留原文；读取 / 迁移版本限制、删除复验、维护不推进删除与全库认证成本保留在现行验证入口。current 与路线图原文除相对链接重定位外完整归档，原阶段目标、范围与出口保留。
- 295 份范围外已跟踪文件与整理前摘要一致，包含 ADR、schema、fixture、implementation / evaluation、源码、依赖、根入口与检查器；`git diff --check` 与新增文档的独立差异卫生检查通过。
- 按文档影响面未执行 shell 聚合入口内的 Rust fmt / Clippy / tests、桌面构建、GUI / VM、真实 key store 或 M01；仓库基线通过不等于产品运行验收或完整 P1-S05 通过。

## 交接

未暂存、提交或推送。开始时 `dev` 与本地 `origin/dev` 一致且工作区干净，未 fetch 或重新核验远端。未安装依赖、访问真实资料 / key store、启动 GUI / VM 或清理历史保留材料。
