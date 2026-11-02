# 2026-11-02 中文格式切换原生局部验收

## 结果

项目所有者按[准备包](2026-11-02-recall-native-preparation.md)授权执行，又在失败启动计满原预算后追加一次相同包装启动，只完成既定 N03。三轮真实 macOS 窗口完成明文 / 加密的升级、降级 Cancel / Confirm、中文实际找回、关闭重开、四份精确导出与 Verify，均正常退出 0；760×520 原生尺寸未取得证据。详细输入、操作、错误与限制见[验收结果](../evaluation/phase1-recall-native-acceptance.md#2026-11-02-macos-原生局部结果)。

基线为 dev / 3f05a13，加本批两个未提交 test-only 源文件；未修改生产逻辑、依赖 / lockfile、canonical schema、认证或格式兼容语义。181 份构建输入 / 13 份不可变文件与操作单在各次启动及最终收尾复验保持，没有改写冻结摘要、重新签名或复用旧诊断包。

## 实际验收与收尾

- 独立 LegacyUnicode61 v6 库为 2 lineage / 3 版本；旧格式蓝色和旧 ASCII 词为 0、当前 ASCII 词只命中 A v2。升级 Cancel 保持，Confirm 后旧结果清空；Han 的蓝色只命中 A v2、红色旧版本为 0、主题命中 A v2 / B v1，中文标题与简繁 / API 正文可读。
- 明文重开保持 Han。明文 / 加密降级的 Cancel 均保持原状态，Confirm 后清空结果并如实说明短词遗漏、不恢复备份及不改变正文 / body-version；实测蓝色为 0、ASCII 当前词为 1。
- 一次 Prepare 建立本批唯一专用 key，一次迁移完成三份正文；第一次显式 OpenEncrypted 保持 Legacy，加密升级通过；最后一轮默认明文打开 v8 拒绝且无旧视图，显式加密打开后 Han / 中文命中保持，再显式降级。
- 四份导出均为 A v2 的精确 103 bytes，保留 UTF-8 BOM / CRLF，全部字节与 SHA-256 一致；两个明文 Verify 成功，两个加密 Verify 均报告 3 objects / 零 pending。
- 最终 v8 / Legacy / objects_ready，3 references、3 retired migration attempts、0 inline bodies；全部初始 canonical / audit rows 与 source IDs 保持，三个密文对象相对 N02 的摘要不变，SQLite integrity / foreign-key check 通过。初始 v6、预演副本和已迁移库保持各自事实，没有覆盖“回滚”。
- 首次沙箱进程预检失败后仍错误执行了启动，约 0.1 秒后退出 -6、无业务点击，初始数据库摘要未变。其计入原三次预算；随后两轮原生完成即停止新启动，用户追加一次授权后才完成 N03，总计 4 次尝试 / 3 个可见窗口。三轮窗口均串行，UTC 13:41:42–13:48:00、13:50:08–13:54:22、13:57:03–13:58:40，累计约 12 分 11 秒；最终无本批进程或数据库句柄。
- 三个可见进程 stdout 为空，stderr 各有一条 151 字节的 `TSM AdjustCapsLockLEDForKeyTransitionHandling` / `Inhibit` 行，无正文或密钥。收尾曾误断言 stdout / stderr 都为空；读取实际记录后保留该系统行，没有修改应用日志、屏蔽错误或把非零退出改成成功。
- 未出现可见系统凭据提示，未选择永久允许、输入凭据或读取 key bytes；不能据此认定系统调用为零、重复提示已解决或真实取消通过。没有 ACL、全局字体、系统设置、VM、Delete / Rebuild / Refresh、远程动作或 push。

## 尚未覆盖与下一步

两次窗口角 / 边缘拖动均未实际调整，50 份截图均为 2160×1504 像素；没有证明原生 content rect / scale，因此 760×520 仍待验。现有 AX 输入不可靠时改用截图坐标完成实际查询；升级后未在 GUI 再跑旧 ASCII 词，不把升级前的零命中或合成测试替代此项。早期截图字节为 JPEG 却沿用 png 后缀，原字节保留，以 sips 解码计尺寸；收尾状态断言按正式状态机确认迁移终态 retired，不把中间 referenced 当完成态。

下一步先准备可核实原生内容尺寸的受限方法，再按具体目标 / 时长请求剩余窗口或跨平台执行；不改产品启动默认值、增加诊断开关或反复盲拖来补证据。Q01 独立人工标签、Windows / Linux 中文显示与切换、Q04 规模 / 延迟预算、Q02 第 201 条、Q03 默认 failed-open 及 Q06 回源 / 刷新各自保持，M01 继续暂缓。

## 验证与保留

原生结果更新后重新运行 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh`，退出 0：313 文件仓库基线 / checker 回归、fmt、workspace all-targets / all-features locked Clippy、30 个 Rust suites 通过，合计 435 passed、0 failed、14 ignored（12 个由父测试调用的隔离 helper，另 2 个成本测试未在本批重跑）。没有改动 production 源码，默认 binary 仍与准备构建一致；181 / 13 冻结输入及 manifest 摘要再次复验保持。收尾文档基线、git diff --check、工作区 / 未跟踪文件与文件权限复核通过，无依赖 / lockfile、缓存、秘密或越界文件变化。

全部本地材料在 `.tmp/2026-11-02-recall-native/` 保留，实际合成库、四份导出、原输入、包装、50 份截图 / AX、阶段快照、退出与无句柄记录及唯一新系统 slot 均保留；应用未删除来源，不产生删除回执。历史 M01 / D01 / D02 和 10 月 28 日材料 / key 保持。

本批新增测试与文档未提交，dev 仍领先 origin/dev 三个既有提交，未推送；无遗留后台进程。原始对象加密不覆盖 FTS 全文、SQLite metadata、历史明文、导出、快照或备份，不声明整体静态加密、恢复或生产可用。
