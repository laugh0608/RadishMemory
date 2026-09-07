# Phase 1 加密 Source Vault 依赖与密码套件评审

日期：2026-09-03

状态：`Accepted — profile 已冻结；P1-S03a portable graph 已落地，platform providers 待后续单元`

范围：`P1-S02 dependency and cipher review`。本文选择 [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md) 所需的对象 AEAD、streaming construction、DEK wrap、随机源、secret memory 边界和 macOS / Windows / Linux key provider，并冻结实现前门禁。本文不修改 production manifest、`Cargo.lock`、SQLite schema、application service 或 UI，不访问真实系统 key store，也不证明加密 Source Vault 已实现。

## 结论

首个对象 cipher profile 冻结为 `radishmemory.xchacha20poly1305-stream-be32/1`：每个对象使用随机 256-bit DEK，以 `XChaCha20Poly1305` 为底层 AEAD、`aead-stream::StreamBE32` 为分段构造。首个 DEK wrap profile 冻结为 `radishmemory.xchacha20poly1305-dek-wrap/1`：设备本地 256-bit KEK 使用一次独立 XChaCha20-Poly1305 invocation 包装 32-byte DEK，不把它误称为 RFC 3394 AES-KW。

随机字节继续来自 workspace 已固定的 `getrandom =0.4.3`；secret-bearing byte / string buffer 使用 `zeroize =1.9.0` 的 `Zeroizing` 或等价 drop zeroization。production platform key store 使用 `keyring-core =1.0.0` 的共同 error / entry model，但不链接 all-in-one `keyring`：macOS 精确选择 `apple-native-keyring-store =1.0.2` 的 legacy `keychain` feature，Windows 选择 `windows-native-keyring-store =1.1.0` 并关闭默认 `search`，Linux 选择 `zbus-secret-service-keyring-store =1.0.1` 的 `crypto-rust` feature。

这些选择中 portable crypto 部分已由 [P1-S03a 落地记录](phase1-source-vault-portable-crypto.md)完成 crate 下载、checksum / lockfile、三目标图、许可证 / notices、advisory 复核与 known-answer / tamper tests。三个 platform key-store provider 仍只冻结精确选择，尚未进入 manifest / lockfile，也没有访问真实系统 store；其最终解析版本、feature、许可证、native surface 或 advisory 与本文不符时仍会重新打开 P1-S02。

## Object cipher profile

### Primitive 与 streaming construction

精确依赖和 feature 选择为：

```toml
aead-stream = { version = "=0.6.0", default-features = false, features = ["alloc"] }
chacha20poly1305 = { version = "=0.11.0", default-features = false, features = ["alloc", "zeroize"] }
zeroize = { version = "=1.9.0", default-features = false, features = ["alloc"] }
```

- `chacha20poly1305 0.11.0` 是 RustCrypto 的 pure-Rust ChaCha20-Poly1305 / XChaCha20-Poly1305 实现，使用 256-bit key、XChaCha 的 192-bit nonce 和 128-bit authentication tag；本 profile 不启用 reduced-round variant，也不启用 crate 自带 `getrandom` feature；
- `aead-stream 0.6.0` 是 RustCrypto 从 `aead 0.6` 独立出的 STREAM online authenticated-encryption implementation。选择 `StreamBE32`，让 crate 已评审构造负责 32-bit big-endian counter 与 final-segment flag，拒绝自行拼装一个容易遗漏 reorder / truncation 防护的 chunk protocol；
- `StreamBE32<XChaCha20Poly1305>` 从 24-byte AEAD nonce 中保留 19-byte per-object random stream nonce prefix，最后 5 bytes 由 32-bit counter 和 one-byte last flag 构成；任何 counter overflow、非法 final position、额外尾段、缺失尾段、重排或重复段都失败关闭；
- 首个固定 plaintext segment size 为 1 MiB。当前 `.txt` / `.md` 上限仍是 8 MiB；空对象也必须产生一个 authenticated final segment，不能以零 ciphertext / 零 tag 代表成功；
- 每段增加 16-byte tag。segment count 和 ciphertext length 必须由经过认证的 plaintext length、固定 chunk size 与 tag size 唯一计算；envelope parser 不接受自报的任意 offset / segment count 绕过上限；
- P1-S03 不得在完整 segment chain、final flag、envelope metadata、plaintext length 与 `exact-bytes-v1` digest 全部验证前向 parser、FTS、citation、UI 或成功 export 暴露明文。首个 8 MiB 范围可以在任务私有内存中完成全对象验证；未来扩大对象上限前必须独立评审 bounded verified handoff，不能用“streaming”降低这条完成条件。

`XChaCha20-Poly1305` 的长 nonce 适合每对象随机前缀，但仍不是 nonce-misuse-resistant cipher。同一 DEK 下任何完整 stream nonce prefix 都只能使用一次；失败重试若可能重用 DEK，必须复用已发布对象或分配新 DEK 与新 prefix，不能用新 plaintext 继续旧 `(DEK, prefix)`。

### Versioned associated data

object segments 使用同一份 deterministic、length-delimited binary AAD。P1-S03 必须为 encoding 写下 byte-level fixture；首版至少按固定顺序认证：

1. domain `radishmemory.source-object-aad/1`；
2. envelope contract `radishmemory.phase1-encrypted-source-vault/1`；
3. cipher profile `radishmemory.xchacha20poly1305-stream-be32/1`；
4. namespace、`source_id`；
5. digest profile `exact-bytes-v1` 与 digest value；
6. exact plaintext length、media type、fixed segment size；
7. 19-byte stream nonce prefix；
8. key-wrap profile 与非秘密 provider profile ID。

字符串使用 UTF-8，variable-length field 使用拒绝 overflow 的固定-width big-endian length prefix；整数使用 unsigned big-endian。未知 field、重复 field、非 canonical length、尾随 bytes 或无法精确重编码的 header 都失败关闭。wrapped DEK、ciphertext 与 tag 不进入自身 AAD；它们分别由 DEK-wrap tag 和 segment tag 认证。物理 locator、SQLite rowid、path、key secret 和外部 origin 不进入 AAD。

## DEK wrap profile

`radishmemory.xchacha20poly1305-dek-wrap/1` 直接复用 `chacha20poly1305 0.11.0` 的 `XChaCha20Poly1305`：

- KEK 32 bytes、DEK plaintext 32 bytes；
- 每次 wrap 使用独立随机 24-byte nonce；
- 输出固定为 32-byte encrypted DEK 加 16-byte tag；
- wrap AAD 是 deterministic、length-delimited encoding，至少绑定 domain `radishmemory.source-object-dek-wrap-aad/1`、envelope / wrap profile、provider profile ID、namespace、`source_id`、digest profile / value、plaintext length、media type 和 object stream nonce prefix；
- unwrap 只尝试 envelope 声明的精确 profile 和精确 key slot。认证失败不轮询其它 key、旧 key、明文 BLOB 或 origin file；
- wrap nonce 与 object stream nonce prefix 分别生成、分别持久化，不切片复用一块随机输入。

这是一种 application-specific AEAD key wrapping profile，不是标准 AES Key Wrap。选择它是为了复用同一受审阅 primitive、获得 AAD 绑定并避免额外 AES / key-wrap dependency；未来互操作、HSM 或同步协议若要求标准化 key wrap，必须新增 profile 和 migration，不静默重解释 version 1。

## Random 与 secret memory

workspace 已直接固定 `getrandom =0.4.3` 且关闭 default features。P1-S03 继续直接调用其 `fill`，为每个 object DEK、19-byte stream prefix、24-byte wrap nonce 分别请求系统首选随机源；任一调用失败都会终止 operation，不能回退时间、路径、process ID、counter、hash digest、`rand` 默认 generator 或自定义 backend。

production random capability 与 deterministic test capability 必须通过窄边界注入。测试只使用公开、固定 synthetic bytes，并在类型或 feature 边界上保证不会进入 production constructor；production 不启用 `getrandom` 的 `custom`、`unsupported`、`wasm_js` 或 `sys_rng` feature。

DEK、KEK、解码后的 key-store value、AEAD key buffer、尚未写出的 plaintext working buffer 和失败路径上的临时 secret 使用 `Zeroizing` / explicit `Zeroize`。`zeroize` 不等于阻止 compiler copy、swap、core dump、debugger、kernel 或 hostile process；类型不得实现泄露内容的 `Debug` / `Display` / serialization，错误、receipt 和 test failure 也不能输出 secret、nonce、tag 或 wrapped DEK。

## Known-answer 与负向密码测试

P1-S03a 必须把以下测试作为 production profile 的实现门禁，而不只做 encrypt → decrypt roundtrip：

1. 精确复现 expired CFRG XChaCha draft `draft-irtf-cfrg-xchacha-03` Appendix A.1 的公开 AEAD vector，包括 32-byte key、24-byte IV、AAD、ciphertext 和 tag；该文档不是正式 RFC，测试只作为广泛实现的互操作 vector，算法基础同时引用 RFC 8439 与 RustCrypto implementation；
2. 使用公开 synthetic bytes 冻结一个 repository-owned object STREAM vector，覆盖 empty、单段、恰好 1 MiB、跨段和 8 MiB boundary；expected envelope / ciphertext digest 必须写死，不能在 assertion 内调用被测实现生成 oracle；
3. 使用公开 synthetic KEK / DEK / nonce / AAD 冻结一个 repository-owned wrap vector；错误 namespace、source、digest、length、media type、provider profile、nonce、ciphertext 或 tag 均拒绝；
4. 删除、重复、重排、中间截断、伪造 final flag、追加尾段、non-canonical header 和 counter boundary 都必须得到稳定 authentication / format error，不得 panic 或产生 partial success；
5. test fixture、snapshot 和失败输出不包含 production key-store value、真实路径或真实个人资料。

上游 RustCrypto 文档记录 ChaCha20Poly1305 implementation lineage 已接受一次 NCC Group audit 且无重大结论；这不能替代 RadishMemory profile、framing、AAD 和调用顺序的本项目测试，也不能外推为未来版本自动通过。

## Platform key provider

### 共同 key slot 与 value

不采用 all-in-one `keyring 4.2.0`。其 upstream 文档明确建议需要控制平台 store 的 application 直接依赖 `keyring-core` 与精确 provider；本项目也需要排除 sample file store、Linux keyutils、SQLite keystore 和 provider fallback。

共同 direct dependency 冻结为：

```toml
keyring-core = { version = "=1.0.0", default-features = false }
```

共同 credential identity 为：

- service：`io.github.laugh0608.RadishMemory.source-vault`；
- user / account：`v1:<namespace_id>:<device_id>`，两项均来自已验真的 host profile，不从 path、OS username、machine name 或 origin file 推导；
- label：固定 human-readable `RadishMemory Source Vault key`，不包含 namespace / device / path；
- secret value：ASCII `rmkek1:` 加 64 个 lowercase hexadecimal characters，解码后必须恰好 32 bytes。为兼容只可靠支持 UTF-8 secret 的 KDE Wallet，不直接持久化任意 binary；
- SQLite 只保存 provider profile ID `radishmemory.platform-key-store/1` 和稳定 key-slot reference，不保存 secret value、可逆 path 或 credential dump。

出现零个 entry、一个合法 entry、多个匹配 entry、损坏 value 和 provider failure 必须区分。多个匹配永远是 `ambiguous`，不能取 first / newest；读取后必须严格重编码复验。provider 的底层错误可以保留为受限 source chain，但公开 error 只使用稳定脱敏 reason。

### macOS

```toml
[target.'cfg(target_os = "macos")'.dependencies]
apple-native-keyring-store = { version = "=1.0.2", default-features = false, features = ["keychain"] }
```

当前 desktop 没有 provisioning profile，因此首版选择 legacy Keychain Services generic-password store；不启用 `protected`，也不启用 biometric、access group、iCloud synchronization 或 Secure Enclave 声明。service / account 构成精确 lookup；duplicate、locked keychain、user denial、interaction not allowed 和其它 OSStatus 均显式失败。未来签名 sandbox app 若改用 protected-data keychain，属于 provider profile migration，不能只切 feature。

### Windows

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows-native-keyring-store = { version = "=1.1.0", default-features = false }
```

关闭默认 `search`，避免不需要的 `regex` surface；通过 exact `target` modifier 使用 `io.github.laugh0608.RadishMemory/source-vault/v1/<namespace_id>/<device_id>`，不依赖 service / user delimiter 拼接。credential type 为 generic，secret value 远低于 Windows 2560-byte credential blob 上限。

provider 默认 persistence 是 `Enterprise`，本项目必须显式指定 `Local`，对应同一 Windows user、同一 computer 的后续 logon session；不得让首版本地 KEK 随 roaming profile 跨设备。credential set 绑定当前 token / logon session；network logon、missing credential set、write / read / delete failure 均失败关闭，不自行改用 DPAPI file 或 machine-wide secret。

### Linux

```toml
[target.'cfg(target_os = "linux")'.dependencies]
zbus-secret-service-keyring-store = { version = "=1.0.1", default-features = false, features = ["crypto-rust"] }
```

选择 Secret Service default collection，不传会创建 / 选择另一 collection 的 `target` modifier；entry 通过 service 与 username attributes 精确搜索，不持久化或信任 D-Bus object path。`crypto-rust` 避免 OpenSSL 和项目未采用的 async runtime feature；D-Bus / Secret Service 是本地 IPC / session service，不是产品 HTTP / TLS 能力。

service 缺失、default collection 缺失、collection / item locked、prompt 被取消、session 建立失败或 multiple matches 都显式失败；不得回退 plaintext file、sample store、Linux keyutils 或新 collection。Secret Service 允许 unlock / create / delete 触发 prompt，且服务可随时重新锁定；P1-S05 必须在真实 GNOME 和至少一个 KDE / KWallet-compatible 环境分别验证 UTF-8 value、prompt、cancel、reopen 和 deletion behavior。WSL 与 headless Linux 不在首版受支持宿主集合，不能用 known-password 自动解锁脚本冒充 production evidence。

## Bootstrap、并发与 key loss

KeyProvider API 必须区分 `load_existing` 与受控 `create_if_absent_for_bootstrap`，调用方不能对任意 missing-key error 自动创建新 KEK。

允许创建 KEK 的状态只有：

1. 经过验真的全新 library：host profile 合法，database / object reference / migration attempt / object directory 中没有既有事实；
2. 经过验真的 SQLite v6 plaintext library 首次进入冻结 migration：所有 inline body 与 canonical facts 先通过既有 verify，且还没有 encrypted object reference、published object 或已开始的 key profile。

任何 v7+ key reference、published / committed object、migration attempt 或 ambiguous object state 已存在而 key entry 缺失时，都必须报告 `key_missing` 并保持 library 不可用，绝不能生成 replacement KEK。

bootstrap 必须由 SQLite `IMMEDIATE` transaction 串行化同一 library 的 writer：transaction 内重新读取 schema / migration / key reference 与 provider entry，只有仍满足 eligibility 才生成、写入并立即 read-back 比较 KEK，然后提交 provider profile / migration state。key-store write 成功而 SQLite commit 失败会留下同 identity 的可复用 entry；重试必须读取并复用，不能覆盖。首版不提供 parallel process key rotation，P1-S03 / P1-S04 还必须证明第二实例不能产生两个不同 KEK。

key store locked、prompt cancel、temporary service unavailable 可以标记为 retryable，但 retry 仍只读取同 key slot；missing、ambiguous、corrupt 或 wrong key 是 persistent failure。首版没有 recovery code、password recovery、key escrow、rotation、cross-device transfer 或 remote unlock。删除 library / host profile 不自动枚举或删除未知 credential；显式 key destruction 与 orphan credential cleanup 必须另行冻结用户操作和 evidence。

## 依赖、许可证与构建影响

| direct package | 固定版本 / feature | 许可证 | 主要影响 |
| --- | --- | --- | --- |
| `chacha20poly1305` | `=0.11.0`, `alloc, zeroize` | `Apache-2.0 OR MIT` | pure-Rust AEAD；引入 RustCrypto cipher / Poly1305 graph |
| `aead-stream` | `=0.6.0`, `alloc` | `Apache-2.0 OR MIT` | pure-Rust STREAM state / framing；依赖 `aead 0.6` |
| `zeroize` | `=1.9.0`, `alloc` | `Apache-2.0 OR MIT` | pure-Rust secret drop zeroization，无 FFI |
| `getrandom` | 复用既有 `=0.4.3` | `Apache-2.0 OR MIT` | 三平台系统随机；不新增 feature |
| `keyring-core` | `=1.0.0`, no default feature | `Apache-2.0 OR MIT` | common entry / error API；不启用 insecure sample store |
| `apple-native-keyring-store` | `=1.0.2`, `keychain` | `Apache-2.0 OR MIT` | macOS Security.framework / legacy Keychain |
| `windows-native-keyring-store` | `=1.1.0`, no default feature | `Apache-2.0 OR MIT` | Windows Credential Manager / `windows-sys`;不启用 search |
| `zbus-secret-service-keyring-store` | `=1.0.1`, `crypto-rust` | `Apache-2.0 OR MIT` | Linux Secret Service、zbus / D-Bus、本地 session crypto |

workspace Rust `1.96.0` 高于上述 direct crates 声明的 MSRV。crypto crates 不需要第三方 native library；平台 providers 必然扩大 macOS framework、Windows system API 与 Linux D-Bus runtime surface。`crypto-rust` 的预期图不使用 OpenSSL，但只有实际 `Cargo.lock`、三目标 `cargo metadata / tree`、build script / proc macro / `links` inventory 和 locked build 才能证明最终解析结果。

RustCrypto 依赖有公开 specification / test vectors 和 audit lineage；keyring provider 是从既有 keyring ecosystem 拆分出的较新 1.x package，文档覆盖和独立 adoption 仍有限。这是当前最大供应链剩余风险，因此必须精确 pin、target-gate、禁止 provider fallback，并用三平台真实 key-store behavior 补足。未来 patch / minor upgrade 都重新执行 advisory、license、source / checksum、feature 和 host evidence 评审。

P1-S02 评审当时只依据 upstream manifest / API 文档和 OS specification 做选择，没有运行 registry resolution 或 advisory scanner。随后 P1-S03a 已对 portable 11-package 增量执行下列供应链落地并记录在专项证据中；以下要求继续适用于尚未落地的 platform provider graph：

- 只从 crates.io 解析并记录 source / checksum，无 Git dependency；
- 更新 dependency baseline、目标依赖清单、`THIRD_PARTY_NOTICES.md`、许可证文本和仓库检查器；
- 运行当前 advisory 数据源检查并人工确认 reachable finding；不得把“搜索未发现”写成长期零漏洞承诺；
- 分别核对 Linux、macOS、Windows target graph，不能用完整 lockfile 或 macOS tree 代替三平台 artifact；
- 若出现 GPL-only、AGPL、未知许可证、OpenSSL / native build、额外 async runtime、网络 client 或未评审系统 capability，停止并重新评审。

## 被拒绝的方案

### 自行拼装 chunk nonce 与 final marker

简单 `random-prefix || counter` 容易漏掉 truncation、reorder、last-block 和 counter overflow 语义。使用 RustCrypto `aead-stream::StreamBE32` 保留相同 primitive，同时把这些状态交给已有 STREAM construction。

### 单次把未来大对象全部交给 allocating AEAD

当前文本上限虽只有 8 MiB，Source Vault 是 PDF / 图片前置。一次性 AEAD 会把未来对象大小变成峰值内存承诺，也没有稳定 chunk / recovery 格式；首版因此从 version 1 就使用固定 segment profile，但仍要求全对象认证完成后才产生业务可见明文。

### AES-GCM、AES-KW 或自研通用 crypto abstraction

AES-GCM 的 96-bit nonce 与硬件差异没有为当前跨平台软件基线提供更小风险；AES-KW 还会新增 primitive 且不能直接绑定本文 metadata AAD。当前只需要两个精确 profile，不建立多算法 registry、自动 fallback 或未来假设 abstraction。

### all-in-one `keyring` 或自动 provider fallback

它会把未使用 store 和 feature 带入依赖图，并弱化平台失败语义。上游也建议需要精确控制的 application 直接使用 `keyring-core` 与特定 stores；RadishMemory 对 missing / locked / ambiguous 必须失败关闭。

### file-stored KEK、sample store、Linux keyutils 或 origin-derived key

与 object ciphertext 同目录保存 KEK 不能提供所需 trust separation；sample store 明确不适合 production；Linux keyutils 不等同跨桌面登录 Secret Service；从 path、digest、profile ID 或用户资料推导 KEK 会破坏随机密钥和删除 / 迁移边界。

### macOS protected-data / iCloud、Windows Enterprise persistence

当前 macOS app 没有 provisioning profile，protected store 会引入 entitlement 与潜在 sync 语义；Windows provider 默认 Enterprise 可能 roaming。首版设备本地 KEK 必须分别使用 legacy Keychain 与 Local persistence，未来迁移另行评审。

## P1-S02 退出条件与后续落地状态

P1-S02 在以下条件同时成立时完成：精确 primitive / STREAM / wrap / random / zeroization profile 已冻结；三平台 store、identity、persistence、prompt、bootstrap / key-loss 语义已冻结；direct versions / features / licenses / expected native surface 已记录；公开 vector、project vectors 和负向 tests 已列为实现门禁；状态、路线图、ADR 与检查器一致；production manifest / lockfile / code 保持不变。

`P1-S03a portable crypto dependency landing` 已按下列原定最小范围完成，没有把 object filesystem、SQLite migration 和真实 key store 合成一个大批次：

- 范围：新增第一方 `radishmemory-source-vault` package 的 portable cipher / wrap profile；加入 `chacha20poly1305`、`aead-stream`、`zeroize` 与既有 `getrandom`，生成精确 lockfile；实现 deterministic AAD codec、synthetic key provider / random seam、known-answer / tamper / truncation tests；更新完整 dependency / notices 证据；
- 非目标：不加入或调用三平台 key-store provider，不创建 object directory，不修改 SQLite schema / source body、application service 或 UI，不迁移任何数据库，不启动 GUI / VM，不使用真实资料 / 密钥，不 push / PR / remote CI；
- 前置决策：项目所有者须授权 manifest / `Cargo.lock` / notices 变化、crates.io 依赖解析与新第一方 package；若最终 graph 与本文预期不符先停下；
- 验收：公开 XChaCha vector、repository-owned STREAM / wrap vectors 和所有负向场景通过；resolved source / checksum / license / feature / build-script / proc-macro / `links` / advisory inventory 完整；`./scripts/check-repo.sh` 与 portable package locked tests 通过；工作树只含该单元文件；
- 实际结果：新增独立 package 与 11 个 crates.io package，完成 CFRG / repository vectors、AAD byte fixture、tamper / truncation / reorder / final-flag、random failure、secret / diagnostic 边界、三目标 portable graph、344 项 notices 和当前 RustSec database 静态复核；没有加入 platform provider、对象目录、SQLite 或 application dependency edge；
- 后续授权：下一最小单元为 `P1-S03b immutable object filesystem adapter`；平台 provider landing / 真实 key-store 交互、`P1-S04` SQLite coordination / migration 与 `P1-S05` host acceptance 继续分别授权，前一单元证据不能替代后一单元。

## 官方依据

- [RFC 8439: ChaCha20 and Poly1305](https://www.rfc-editor.org/rfc/rfc8439.html)
- [CFRG XChaCha draft 与 Appendix A vectors](https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03)
- [RustCrypto chacha20poly1305 0.11.0](https://docs.rs/crate/chacha20poly1305/0.11.0)
- [RustCrypto aead-stream 0.6.0](https://docs.rs/crate/aead-stream/0.6.0)
- [getrandom 0.4.3](https://docs.rs/getrandom/0.4.3/getrandom/)
- [zeroize 1.9.0](https://docs.rs/zeroize/1.9.0/zeroize/)
- [keyring-core 1.0.0](https://docs.rs/crate/keyring-core/1.0.0)
- [keyring upstream 对 application 精确 provider 的建议](https://docs.rs/keyring/4.2.0/keyring/)
- [apple-native-keyring-store 1.0.2](https://docs.rs/crate/apple-native-keyring-store/1.0.2)
- [Apple Keychain Services](https://developer.apple.com/documentation/security/keychain-services)
- [windows-native-keyring-store 1.1.0](https://docs.rs/crate/windows-native-keyring-store/1.1.0)
- [Microsoft CREDENTIAL structure](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw)
- [zbus-secret-service-keyring-store 1.0.1](https://docs.rs/crate/zbus-secret-service-keyring-store/1.0.1)
- [freedesktop Secret Service specification](https://specifications.freedesktop.org/secret-service/latest-single/)
