# Windows keyring 上游最小补丁

本目录只维护经项目所有者于 2026-10-01 批准的 `windows-native-keyring-store 1.1.0` 可见性补丁。它是第三方代码，不继承 RadishMemory 的源码许可证，也不是第一方 workspace package。

## 来源与变更

- 原始来源：[crates.io 1.1.0 发布包](https://crates.io/crates/windows-native-keyring-store/1.1.0)；[上游仓库](https://github.com/open-source-cooperative/windows-native-keyring-store)。作者归属保留在原 manifest 和许可证文本。
- 原始 `.crate` SHA-256：`063426e76fdec7438d56bb777f67e318a84a25c707b07e575cb8b78e10c028f8`。
- 分发选择 MIT，保留原 [MIT](windows-native-keyring-store-1.1.0/LICENSE-MIT) 与 [Apache-2.0](windows-native-keyring-store-1.1.0/LICENSE-APACHE) 全文；不移除上游的替代授权。
- 唯一源码补丁：`src/cred.rs` 的 `pub(crate) struct Cred` 改为 `pub struct Cred`。字段原本已是 public；不改变构造、Win32 调用、编码、权限、persistence 或 setter 行为。
- 保留编译源码、上游 unit tests、examples、README、发布 manifest 与许可证，共 11 个原发布文件；不复制上游开发 lockfile、原始开发 manifest 或 VCS 元数据。各文件的原始和补丁后摘要见 [provenance.json](windows-native-keyring-store-1.1.0/provenance.json)。除该可见性单行外，全部字节包括 CRLF 和上游空白保持原样。

根 manifest 用精确 `[patch.crates-io]` 路径替换同版本；vendor 从 workspace 排除，不新增第三方版本或 feature。发布 manifest 的上游 dev-dependencies / examples 保留用于追溯，但没有被加入第一方 workspace 测试范围。未来上游提供满足 exact target/account 的公开 API 后，应另行审阅替换，不静默删除补丁或改凭据映射。

## 可复验与门禁

1. 取得上述版本原发布 archive，先核验 archive SHA-256。
2. 对 `provenance.json` 列出的 11 个文件核验原始字节摘要，仅对 `src/cred.rs` 应用上述一行可见性替换。
3. 核验全部补丁后文件摘要，再执行 `python3 scripts/generate-third-party-notices.py --check` 和 `./scripts/check-repo.sh`。

生成器固定 provenance 文件摘要，拒绝缺失 / 新增文件、内容漂移、symlink 或未知第三方 path dependency。notices 不把本地补丁称为拥有 Cargo registry checksum，而分别记录原 archive 与受审阅 provenance 摘要。Git attributes 与文本检查只对这份受摘要约束的上游副本保留原格式；第一方文件的文本门禁不变。

补丁解决 Windows 编译可见性，不代表真实 Credential Manager、日志过滤、prompt / deny / cancel、系统密钥恢复或生产宿主已验收。固定 target、account、Local persistence、ASCII secret bytes、失败关闭与 SQLite 串行化边界仍由 Source Vault 的既有契约约束。
