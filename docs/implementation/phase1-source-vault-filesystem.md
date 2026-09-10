# Phase 1 Source Vault immutable object filesystem adapter

日期：2026-09-10

状态：`P1-S03b implemented and locally validated — platform runtime validation pending`

基线：`6581b59`，在 `dev` 上实施；本记录与本批实现一同提交。

范围：落实 [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)及 [P1-S03a](phase1-source-vault-portable-crypto.md)定义的 versioned envelope、应用专用 object / staging capability、durable no-overwrite publish、认证 read-back 与单次 attempt identity。只扩展 `radishmemory-source-vault`；不改变 frozen cipher / AAD profile、canonical schema、manifest、`Cargo.lock` 或 notices，不接真实 key store、SQLite、application / UI，不处理真实资料。

## 格式与参考契约

对象格式为 adapter-private `RMOBJ\x01`，不属于 canonical schema。每个字段由 one-byte tag、unsigned 32-bit big-endian length 和原始 bytes 组成，tag 必须严格按 1 至 16 出现且各一次；不接受缺失、重复、重排、未知字段、尾随字节或长度越界。

| Tag | 内容 |
| --- | --- |
| 1 | `radishmemory.phase1-encrypted-source-vault/1` |
| 2 | 固定生成器 `radishmemory.source-object-writer/1` |
| 3 | `radishmemory.xchacha20poly1305-stream-be32/1` |
| 4 | `radishmemory.xchacha20poly1305-dek-wrap/1` |
| 5 | `radishmemory.platform-key-store/1` |
| 6 | `exact-bytes-v1` |
| 7 / 8 | namespace / 精确 source ID 的 UTF-8 bytes |
| 9 / 10 / 11 | 32-byte exact digest / 8-byte big-endian plaintext length / media type |
| 12 | 4-byte big-endian segment size，固定 1 MiB |
| 13 / 14 / 15 | 19-byte stream nonce prefix / 24-byte wrap nonce / 48-byte wrapped DEK |
| 16 | 按原顺序拼接的 STREAM ciphertext segments，包括各段 16-byte tag |

空对象仍有一个 final segment；其余段数为 `ceil(plaintext length / 1 MiB)`。非末段大小固定为 `1 MiB + 16`，末段由预期正文长度确定，不接受磁盘自报的替代 segment layout。正文上限继续为 8 MiB；本 envelope 的 namespace、source、media type 各限制为 4096 bytes，总读取上限为 `8 MiB + 3 × 4096 + 2048`，限制发生在非受信长度驱动分配之前。该局部上限不修改 canonical ID 语义或既有 AAD codec。

读取要求调用方独立提供 `ObjectMetadata`；磁盘字段必须逐字匹配预期事实，随后通过既有 wrap / object AAD、全对象 AEAD、length / digest 验证。生成器和所有 profile 只接受上述常量，不能成为算法协商或 fallback。parser 为 crate-private，不把未认证磁盘 metadata 暴露为已确认事实。

格式 oracle 使用独立 Python `struct.pack('>BI', tag, len(value))` 构造合成字段，不调用 Rust encoder：固定样本长度 463 bytes，SHA-256 为 `6fdab8f60cb1938cd9daaea3a88458b17928fe09446dc316ef74fdd43bbdf253`。这验证序列化布局，不代替 P1-S03a 已冻结的密码 known-answer vectors；两类 oracle 均保留。

## 目录与私有标识

`ObjectDirectory::open_application_directory` 必须显式接收受信平台调用方已解析、已准备的 RadishMemory 专用目录；不会默认选择目录，也不会修改现有目录权限。拒绝相对路径、parent traversal、最终 symlink / reparse point、非目录、文件系统根、当前目录、home 和系统临时根。测试使用独立 owner-only 合成临时子目录；这不是把临时根授权为永久 Source Vault。

根下仅创建两个固定子目录：`source-objects-v1` 和 `source-staging-v1`。Unix 新目录为 `0700`、新对象为 `0600`，已有目录权限宽于 owner-only 时拒绝。目录 capability 持有打开的 handle；每个关键阶段重新验证 canonical path 和目录身份，读取拒绝 symlink / reparse point、非普通文件与超限文件，并复核读取前后身份、长度和修改时间。macOS / Linux 使用 no-follow、nonblocking open，避免最终文件替换成 symlink 或 FIFO 后跟随或阻塞。该路径实现不升级 ADR 0008 的威胁模型，不承诺对抗已解锁设备上的恶意进程在任意 syscall 间竞态替换路径。

`ObjectLocator` 是 `SHA-256(domain || length-prefixed namespace || source || exact digest)` 的 64 字符小写 hex，domain 为 `radishmemory.object-locator/1\0`，三个 length 都为 4-byte big-endian。读取必须重新由预期 metadata 计算并核对 locator。不同 source 的相同正文不能共用对象；最终名为 `<locator>.rmo`，导入内容不能提供任意路径。

`ObjectWrite::seal` 在内存中完成已有密码操作，随后生成 envelope 与 `AttemptId`；未持有 KEK 或明文缓冲。attempt 为 `SHA-256("radishmemory.object-attempt/1\0" || locator-ascii || authenticated-stream-nonce-prefix)` 的 64 字符小写 hex。通过现有 AEAD 绑定的随机 nonce 识别一次 sealing，不改变 AAD，也不向诊断暴露 nonce。staging 名为 `<locator>.<attempt>.stage`。

`token()` 仅供后续 adapter-private reference / attempt 持久化；`from_token()` 只接受精确小写 hex，不授予路径、删除或来源授权。locator、attempt、write、directory、published result 的 `Debug` 全部脱敏。I/O 错误仅保留稳定 code / operation reason、`ErrorKind` 与可选 OS code，不保留原始带路径的 error text；未新增日志 sink。

## 发布、读取与中断

顺序为：

1. 在磁盘变更前认证内存 envelope，拒绝错误 key；调用方已经可以取得本次 locator / attempt，以便未来 P1-S04 先持久化关系。
2. `create_new` staging，直接写 envelope / ciphertext，`flush`、文件 `sync_all`、关闭并同步 staging 目录；没有持久化明文临时文件。
3. 独立打开 staging，核对写入后的文件身份、完整 bytes、envelope、AEAD 与 length / digest。
4. 再验证目录与 staging 身份，使用 `fs::hard_link` 原子建立最终名字，不覆盖已有目标；随后同步 objects 目录。
5. 从最终名字重新读取，核对与 staging 相同的文件身份和完整 bytes，并再次认证。
6. 只删除本次已验证 staging link，同步 staging 目录，再复核目录和最终对象身份，返回 `PublishedObject`。

文件关闭前显式 `sync_all`，关闭后独立回读；Rust `File` 的 `Drop` 不报告 close 错误，不能把 drop 当作持久化证据。参考：[Rust File](https://doc.rust-lang.org/std/fs/struct.File.html)、[Rust hard_link](https://doc.rust-lang.org/std/fs/fn.hard_link.html)。

`PublishedObject` 仅证明本次 filesystem publish 的验证结果，不是 canonical capture receipt；SQLite reference commit、commit 后从正式 reference read-back、binding / audit / 幂等结果均留给 P1-S04 / P1-S05。同一目标再次 publish 返回 `ObjectExists`，不覆盖、不重新加密旧对象，也不假装完成业务层幂等。

任何失败都保留已有的精确残留，不做自动 orphan cleanup。`inspect_attempt` 只检查给定 locator / attempt 对应的两个名字：返回 absent、authenticated staging、authenticated published candidate 或两者并存；不完整 / 损坏 envelope、错误 key、不同 attempt 的最终对象、目录 / 文件变化均报错，保留原状。存在可认证文件不等于此前失败的 sync 已具备 durable 保证，也不等于对象没有 committed reference。目录枚举、未知文件策略、业务重试、orphan reconciliation / 删除和崩溃恢复仍属于 P1-S04；本批没有第二套恢复器。

## 验收与实际证据

本机为 macOS ARM64，Rust `1.96.0`。全部数据是合成字节、固定测试 key / random 或系统随机生成的临时测试材料，根目录按测试实例隔离并在结束时清理。

本批新增 20 个 unit tests，加上既有 12 个，package 共 32 个测试：

- empty、短对象、1 MiB、跨段、8 MiB 的 publish → 关闭目录 capability → 重开 → exact read；
- 独立 binary format oracle，每个字段的 missing / duplicate / reordered / oversized，所有前缀截断、尾随字节、未知 version / profile / generator；
- metadata、ciphertext / tag / nonce / wrapped DEK、错误 key、locator 和 attempt 的失败关闭；
- 不同 source 相同 bytes 的独立对象；重复目标、发布瞬间目标抢占及两个真实线程竞争时只有一个成功，失败方 staging 保留；
- 11 个发布 checkpoint 中断后的精确状态，partial ciphertext write 后的 `StorageFull`，真实目录写权限撤销；
- directory / object / staging symlink、非普通文件、目录替换、staging / final 文件替换与诊断脱敏；
- production `ObjectWrite::seal` 使用系统随机，确定性 random 和故障操作只能由 crate-private unit test seam 注入。

磁盘满是写入部分密文后返回 `ErrorKind::StorageFull` 的可控 I/O 注入；sync / publish 中断使用 crate-private checkpoint 注入，另有真实目标占用 / hard-link 竞争。没有填满实际磁盘、切断电源、强杀进程或进行物理介质持久化实验。关闭重开的是 filesystem capability，不是 SQLite / GUI 重启。

实际验证：

- `cargo test -p radishmemory-source-vault --locked --offline`：32 个测试通过；
- `cargo clippy -p radishmemory-source-vault --all-targets --locked --offline -- -D warnings`：通过；
- `./scripts/check-repo.sh`：162 个仓库文件检查、notices 再生成校验、workspace format / Clippy `-D warnings` 与 160 个 all-features Rust tests 全部通过；首次沙箱内运行在既有 `P1-F17` loopback observer bind 处遇到 `PermissionDenied`，经原命令沙箱外重跑通过，没有放宽检查；
- `cargo check --workspace --all-targets --locked --offline`：默认 features 通过；
- `git diff --check`：通过；manifest、`Cargo.lock`、notices 没有变化。

ADR 场景对应的是 `P1-SF04`、`P1-SF06`、`P1-SF07`、`P1-SF10`、`P1-SF13`、`P1-SF18` 的 filesystem 子路径及 `P1-SF02` 的物理独立性，不宣称这些完整 production 场景已经全部通过；SQLite commit、中断协调、key provider、migration、删除和 host 行为均未由本批证明。

## 平台限制与下一步

macOS 具备本机实际测试；Linux / Windows 尚未编译或运行本批。Windows 分支使用 `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT` 打开目录、禁止 delete sharing 保持目录 handle；普通读取禁止 write / delete sharing，并拒绝 reparse point。参照 [Microsoft Directory Handles](https://learn.microsoft.com/en-us/windows/win32/fileio/obtaining-a-handle-to-a-directory)和 [FlushFileBuffers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)请求同步所需写访问；文件系统或权限不支持目录 `sync_all` 时直接失败，不返回假成功或执行 no-op。跨平台运行前不能宣称 Windows durable publish 已可用；若实测暴露 std-only 能力不足，须单独评审所需依赖和平台范围。

后续先补 Linux / Windows 文件系统运行证据与差异处置，再分别推进 platform provider landing、P1-S04 SQLite coordination / migration、P1-S05 application / host acceptance。真实 key store、GUI / VM、依赖变更和远程动作仍需对应范围授权。本批不修复 R01 至 R06；SQLite v6 inline plaintext body、FTS 完整正文副本、中文找回与目录 / 维护缺口保持现行真实口径，PDF / 图片与模型仍不进入实现。
