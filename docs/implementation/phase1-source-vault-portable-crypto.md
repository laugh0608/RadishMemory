# Phase 1 Source Vault portable crypto 落地记录

日期：2026-09-03

状态：`Accepted — P1-S03a portable crypto dependency landing complete`

范围：落实 [P1-S02 依赖与密码套件评审](phase1-encrypted-source-vault-dependency-review.md)冻结的 portable object cipher、DEK wrap、AAD、random 与 secret-memory profile。本文只覆盖第一方 `radishmemory-source-vault` package、精确 lockfile、合成密码测试和供应链证据；不加入或访问平台 key store，不创建对象目录，不修改 SQLite、application service 或 UI，也不证明 encrypted Source Vault 已接入 production data flow。

## 实现边界

新 package 只直接依赖：

```toml
aead-stream = { version = "=0.6.0", default-features = false, features = ["alloc"] }
chacha20poly1305 = { version = "=0.11.0", default-features = false, features = ["alloc", "zeroize"] }
getrandom = { version = "=0.4.3", default-features = false }
sha2 = { version = "0.11.0", default-features = false }
zeroize = { version = "=1.9.0", default-features = false, features = ["alloc"] }
```

`sha2` 与 `getrandom` 是 workspace 既有依赖；前者复验 `exact-bytes-v1`，后者只实现 production `SystemRandom`。确定性 random seam 与直接指定 DEK / nonce 的 vector 入口保持 crate-private 且只由 unit test 调用，public `seal_object` 不能接受调用方注入的 deterministic generator。

portable API 当前只表达 `ObjectMetadata`、zeroizing `KeyEncryptionKey`、`SealedObject`、`seal_object` 与 `open_object`。对象 profile 是 `radishmemory.xchacha20poly1305-stream-be32/1`，DEK wrap profile 是 `radishmemory.xchacha20poly1305-dek-wrap/1`：

- `ObjectMetadata` 固定 digest profile、cipher profile、wrap profile、provider profile 与 1 MiB segment size；调用方只提供 namespace、source、32-byte exact digest、长度和 media type；
- `seal_object` 在加密前核对 8 MiB 上限、声明长度和 exact digest，分别从系统随机源取得 32-byte DEK、19-byte stream prefix 与 24-byte wrap nonce；随机失败不会回退；
- object body 使用 `StreamBE32<XChaCha20Poly1305>`，空对象也产生一个 final authenticated segment；每个 segment 增加 16-byte tag；
- DEK 使用独立 XChaCha20-Poly1305 invocation 和 wrap-domain AAD 生成固定 48-byte ciphertext + tag；
- `open_object` 先检查 segment count / size，再认证 wrapped DEK 与完整 STREAM chain，最后复验 plaintext length 与 SHA-256；只有全部成功才返回明文，失败路径会 zeroize 已累积明文和 secret buffer；
- key、nonce、tag、wrapped DEK、ciphertext、digest、namespace、source 与 media type 不进入 `Debug` / `Display` / error；错误只返回稳定 code / reason。

该 package 还没有 versioned envelope parser、serialized object format、filesystem locator 或 no-overwrite publish。公开 accessors 只让后续 adapter 读取当前内存结果，不接受外部 bytes 反序列化为已验真对象。

## Deterministic AAD 与测试向量

AAD codec 使用 `RMAAD\x01` prefix、固定递增 one-byte field tag、unsigned 32-bit big-endian length 和原始 field bytes。整数按固定宽度 big-endian 编码；object 与 wrap 使用不同 domain。byte-level object / wrap hex fixture 已固定，任一 namespace、source、digest、length 或 media type 变化都会同时改变两个 AAD domain；profile 与 provider 不由调用方覆盖。

当前 12 个 package unit test 覆盖：

1. expired CFRG XChaCha draft Appendix A.1 的完整 key / nonce / AAD / ciphertext / tag known-answer vector；
2. repository-owned empty、单段、恰好 1 MiB、跨段和恰好 8 MiB STREAM vectors；每个 expected ciphertext-layout SHA-256 都是 hard-coded oracle；
3. repository-owned 32-byte DEK wrap 的固定 48-byte expected vector；
4. deterministic object / wrap AAD 的完整 byte-level hex vectors，以及每个 caller-supplied metadata field 的绑定；
5. 等长非末段重排、ciphertext / final tag 篡改、segment 局部或整段截断、重复尾段、stream prefix、wrap nonce、wrapped-DEK tag、错误 KEK、错误 metadata 与伪造 final flag；
6. 空字段、长度 / digest / 8 MiB 上限、random failure 与 `Debug` 脱敏。

测试只使用公开合成常量，没有真实 key、路径、资料、key-store value 或生产数据库。roundtrip 只作为补充；公开与 repository-owned fixed vectors 才承担 regression oracle。

## Locked 依赖与许可证

`Cargo.lock` 当前为 430 个 package：7 个第一方 workspace package 与 423 个 crates.io package，没有 Git dependency。P1-S03a 精确新增 11 个第三方 package：

- `aead 0.6.1`、`aead-stream 0.6.0`；
- `chacha20 0.10.2`、`chacha20poly1305 0.11.0`、`cipher 0.5.2`；
- `cmov 0.5.4`、`ctutils 0.4.2`、`inout 0.2.2`；
- `poly1305 0.9.1`、`universal-hash 0.6.1`、`zeroize 1.9.0`。

11 项都来自 crates.io、带 lockfile checksum，声明 `MIT OR Apache-2.0` 或等价顺序，本项目 distribution basis 选择已有 MIT 文本；没有新增 license identifier、NOTICE 文件、build script、proc macro、native `links`、OpenSSL、FFI、网络 client 或 async runtime。`cargo tree --locked -e normal,build,features` 对 `aarch64-apple-darwin`、`aarch64-unknown-linux-gnu`、`aarch64-pc-windows-msvc` 得到相同 portable crypto / digest 子图；只有既有 `getrandom` / `sha2` 在目标上解析各自系统随机与 CPU feature 条件。

notices 生成器现在从 `radishmemory-desktop` 与尚未接入 desktop 的 `radishmemory-source-vault` 两个分发根取并集，避免独立 package 的依赖被遗漏。三个目标分别有 215、285、209 个 crates.io 条目，并集 344；inventory SHA-256 为 `67e767a36884963bd2ddc5b2db932226a1cdba076ad974630eec357d52dd2e9a`，`Cargo.lock` 文件 SHA-256 为 `fa8ca6b9a79ff49bd426124b83deef430b5f419334baa721eea40c02110d1463`。

## Advisory 复核

本机没有预装 `cargo-audit` 或 `cargo-deny`，本单元没有安装工具。评审把 RustSec `advisory-db` 当前 commit `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5` 下载到任务临时目录，并按上述 11 个新增 package 精确检索：

- `RUSTSEC-2026-0003` 涉及 `cmov <=0.4.3` 的 ARM32 constant-time codegen；当前解析 `cmov 0.5.4` 满足 `>=0.4.4` patched constraint，且当前目标均为 ARM64；
- `RUSTSEC-2019-0029` 涉及 `chacha20 <0.2.3` counter overflow；当前解析 `chacha20 0.10.2` 满足 patched constraint，advisory 也明确说明 `chacha20poly1305` 调用路径不受该问题影响；
- 其余九个新增 package 名没有匹配当前数据库记录。

这只证明该数据库 commit 对当前新增 package / version 的静态复核结果，不是未来“零漏洞”承诺，也不替代后续 CI 或升级时重新运行机器可读 advisory scanner。

## P1-S03a 退出与下一最小单元

P1-S03a 的 package、精确依赖、lockfile、AAD codec、公开 / repository vectors、负向测试、三目标 metadata / feature graph、license / checksum / notices 与当前 RustSec finding 已收口。最终 `./scripts/check-repo.sh` 在本机通过 152 个仓库文件检查、notices 再生成校验、workspace format、Clippy `-D warnings` 与 140 个 locked test；其中新 package 为 12 个 test。仍未加入 P1-S02 选定的三个 platform provider，也没有访问系统 key store；未运行包含该单元的远程三平台 CI。

下一最小单元是 `P1-S03b immutable object filesystem adapter`：

- 范围：冻结并实现 versioned envelope serialization / parser、应用专用 object / staging capability、create-new immutable write、flush / sync / close、no-overwrite publish、认证 read-back 与单次任务 orphan identity；只使用合成临时目录和 crate-private deterministic test key；
- 非目标：不加入真实 key-store provider，不修改 SQLite schema / source body，不接 application / UI，不迁移数据库，不处理真实资料，不启动 GUI / VM，不 push / PR / remote CI；
- 前置决策：需单独授权 filesystem implementation 与 envelope format；若需要新增第三方依赖、改变已冻结 AAD/profile 或扩大数据目录权限，先停下重新评审；
- 验收：empty / 1 MiB / cross-segment / 8 MiB publish-read exact bytes，object / envelope / locator tamper，已存在目标、symlink、磁盘 / sync / publish 故障、重启 read-back、任务 orphan 与诊断脱敏均失败关闭；本地完整仓库门禁通过。

platform provider landing、SQLite coordination / migration 和 application / host acceptance 继续分别授权；P1-S03a 证据不能替代这些单元。
