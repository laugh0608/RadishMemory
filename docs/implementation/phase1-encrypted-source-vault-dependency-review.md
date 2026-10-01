# Phase 1 加密 Source Vault 依赖与密码套件评审

日期：2026-09-03

状态：`Accepted — profile 已冻结；P1-S03a portable graph 与 P1-S03c-2 isolated providers 已落地；真实平台待验收`

范围：`P1-S02 dependency and cipher review`。本文选择 [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md) 所需的对象 AEAD、streaming construction、DEK wrap、随机源、secret memory 边界和 macOS / Windows / Linux key provider，并冻结实现前门禁。本文不修改 production manifest、`Cargo.lock`、SQLite schema、application service 或 UI，不访问真实系统 key store，也不证明加密 Source Vault 已实现。

## 结论

首个对象 cipher profile 冻结为 `radishmemory.xchacha20poly1305-stream-be32/1`：每个对象使用随机 256-bit DEK，以 `XChaCha20Poly1305` 为底层 AEAD、`aead-stream::StreamBE32` 为分段构造。首个 DEK wrap profile 冻结为 `radishmemory.xchacha20poly1305-dek-wrap/1`：设备本地 256-bit KEK 使用一次独立 XChaCha20-Poly1305 invocation 包装 32-byte DEK，不把它误称为 RFC 3394 AES-KW。

随机字节继续来自 workspace 已固定的 `getrandom =0.4.3`；secret-bearing byte / string buffer 使用 `zeroize =1.9.0` 的 `Zeroizing` 或等价 drop zeroization。production platform key store 使用 `keyring-core =1.0.0` 的共同 error / entry model，但不链接 all-in-one `keyring`：macOS 精确选择 `apple-native-keyring-store =1.0.2` 的 legacy `keychain` feature，Windows 选择 `windows-native-keyring-store =1.1.0` 并关闭默认 `search`，Linux 选择 `zbus-secret-service-keyring-store =1.0.1` 的 `crypto-rust` feature。

这些选择中 portable crypto 部分已由 [P1-S03a 落地记录](phase1-source-vault-portable-crypto.md)完成 crate 下载、checksum / lockfile、三目标图、许可证 / notices、advisory 复核与 known-answer / tamper tests。P1-S03c-1 已完成[隔离解析与源码预检](2026-09-15-source-vault-provider-preflight.md)，补充 API 方案随后获得授权；[P1-S03c-2](phase1-source-vault-key-provider.md) 已使三个 platform provider 与两个补充直接依赖进入正式 manifest / lockfile，实现独立 provider，但没有访问真实系统 store；正式落地的 feature、许可证、native surface 或 advisory 与本文不符时仍会重新打开 P1-S02。

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
- label：固定 human-readable `RadishMemory Source Vault key`，不包含 namespace / device / path；macOS / Linux 使用 label，Windows 映射到 `comment`（P1-S03c-2 已批准）；
- secret value：ASCII `rmkek1:` 加 64 个 lowercase hexadecimal characters，解码后必须恰好 32 bytes。为兼容只可靠支持 UTF-8 secret 的 KDE Wallet，不直接持久化任意 binary；
- SQLite 只保存 provider profile ID `radishmemory.platform-key-store/1` 和稳定 key-slot reference，不保存 secret value、可逆 path 或 credential dump。

出现零个 entry、一个合法 entry、多个匹配 entry、损坏 value 和 provider failure 必须区分。多个匹配永远是 `ambiguous`，不能取 first / newest；读取后必须严格重编码复验。实现只从受审阅的底层错误中提取稳定 code / operation 和可取得的数值 OS code；含原始字节的 payload 必须零化，不向公开 error 附带原始 source chain。

### macOS

```toml
[target.'cfg(target_os = "macos")'.dependencies]
apple-native-keyring-store = { version = "=1.0.2", default-features = false, features = ["keychain"] }
```

P1-S03c-2 另直接声明 `security-framework =3.7.0` / no defaults，仅通过安全 API 补齐固定 label 读写与 User-domain 精确查询，不引入第一方 unsafe。

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

P1-S03c-2 另直接声明 `secret-service =5.2.0` / no defaults / `crypto-rust`，以只读 default collection guard 补齐既有项读取前后检查；不调用 `get_any_collection` 或持久化 D-Bus path。

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

P1-S02 评审当时只依据 upstream manifest / API 文档和 OS specification 做选择，没有运行 registry resolution 或 advisory scanner。随后 P1-S03a 已对 portable 11-package 增量执行下列供应链落地并记录在专项证据中；P1-S03c-2 随后已完成 platform provider graph 落地与源码复核，见[实现记录](phase1-source-vault-key-provider.md)。以下要求继续约束后续依赖变更，真实平台凭据验收仍待执行：

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

## Platform provider 实施前核对与批次范围（2026-09-15）

状态：`P1-S03c-1 preflight complete — supplemental API decisions accepted for P1-S03c-2`。

本节保留当批授权范围与初始待证项；已确认的解析、源码行为、补充方案和证据限制见[隔离预检结果](2026-09-15-source-vault-provider-preflight.md)。

P1-S03b 已在其对象 filesystem 范围完成 macOS、Windows ARM64 / NTFS、Linux ARM64 / ext4 验收，见[落地记录](phase1-source-vault-filesystem.md)。下一单元命名为 `P1-S03c platform key provider`。本节只落实实施前核对与拟执行范围，不修改上文已接受的 provider / slot / value / bootstrap 契约，不把 API 文档核对当成 provider 已实现或系统密钥库已验收。

### 本轮发现与待证事项

| 项目 | 已核对事实 | 实现约束与待证事项 |
| --- | --- | --- |
| 明确选择 store | `keyring-core` 的 `Entry::new` 使用全局 default store；具体 store 提供自身 builder | adapter 应持有精确平台 store，核验其直接 build 路径；不读取或修改全局 default store，不接受应用外部注入的其它 provider |
| 写入不是 create-only | `Entry::set_password` / `set_secret` 会更新已有 secret | 不把上游 setter 暴露为任意调用方可用的 KEK 创建入口；P1-S04 的 SQLite `IMMEDIATE` 串行化、eligibility 重验、existing entry 复用与 read-back 是完整 bootstrap 的必要条件；P1-S03c 的合成编排不替代两实例产品验收 |
| 错误也可能携带 secret | `BadEncoding` / `BadDataFormat` 带有原始字节，`Ambiguous` 带有匹配 entries | 不能直接透传其 Debug、Display 或 source chain；核验并零化可取得的 secret payload，公开错误只保留稳定 code / operation 与允许的数值 OS code。`NoStorageAccess` 不能一律猜作 locked；无法分类的错误保持 provider failure |
| Windows 既有 persistence | `Local` modifier 仅在写入时生效；既有 entry 的 persistence 可从 attribute 读取 | 写入前固定 Local；读取既有 entry 时也验证 Local，Enterprise / Session / 属性不可验证均失败关闭，不自动改写 persistence |
| macOS 固定 label | `apple-native-keyring-store 1.0.2` 的 legacy keychain 文档说明该模块忽略 credential attributes | 与本项目固定 label 要求存在 API 能力待证项；须检查发布源码和既有依赖提供的安全接口，不能忽略失败或自行放宽 label 契约；若必须新增原生能力、依赖或改变要求，提交具体差异再决策 |
| Linux collection 与 ambiguity | 默认 collection 控制新建位置；精确 service / username 的既有项搜索跨 collections | 不通过新增 target、只保留默认 collection 或取首项隐藏 multiple matches；核验 default collection 缺失、跨 collection 同身份与创建行为，不让 lookup 触发替代 collection 创建 |
| Linux feature 文档差异 | Rustdoc 的 Features 段仍描述四种 runtime / crypto 组合；前次评审选择为 `crypto-rust` | 以 `1.0.1` 发布 manifest、依赖 feature 传播和实际三目标图核验；不依据文档措辞自行切换 runtime 或 provider，也不据此宣称已选 feature 无效 |

本表记录解析前的 API 文档发现；其后已取得发布源码 / checksum 和依赖解析证据，真实 OS 行为仍未验收。初始依据：[Entry 1.0.0](https://docs.rs/keyring-core/1.0.0/keyring_core/struct.Entry.html)、[Error 1.0.0](https://docs.rs/keyring-core/1.0.0/keyring_core/error/enum.Error.html)、[macOS legacy keychain 1.0.2](https://docs.rs/apple-native-keyring-store/1.0.2/apple_native_keyring_store/keychain/index.html)、[Windows provider 1.1.0](https://docs.rs/windows-native-keyring-store/1.1.0/windows_native_keyring_store/)、[Linux provider 文档](https://docs.rs/zbus-secret-service-keyring-store/latest/zbus_secret_service_keyring_store/)（本次页面标示 `1.0.1`；后续复验必须固定发布版本）。

### P1-S03c-1：隔离依赖与 API 可行性核验

项目所有者已确认并完成执行的预检范围如下：

1. 在本机任务专用临时目录复制当前受控 manifest / lockfile，保持原 workspace 的 target 条件和 feature 关系；仅在副本中加入上文固定的 `keyring-core =1.0.0`、`apple-native-keyring-store =1.0.2` / `keychain`、`windows-native-keyring-store =1.1.0` / no defaults、`zbus-secret-service-keyring-store =1.0.1` / `crypto-rust`。不创建另一份长期 schema 或 production package。
2. 使用已有 Rust / Cargo `1.96.0`、独立 `CARGO_HOME` 和 target 目录；从 crates.io 下载精确 provider 与其必要传递依赖，按发布 checksum 核对。临时副本执行 `cargo metadata --format-version 1` 解析，再以 `--locked --filter-platform` 分别导出 `aarch64-apple-darwin`、`aarch64-pc-windows-msvc`、`aarch64-unknown-linux-gnu` 图；不使用 `cargo update` 做无关升级。
3. 对比正式 lockfile：记录直接 / 传递增量、已有版本变更、feature、license、build script、proc macro、`links` 与 native / runtime 面；复核当前 RustSec advisory 数据及可达性。没有 locked compile 证据时只报告解析结果，不宣称平台构建通过。
4. 阅读发布源码，逐项验证表中的 store builder、upsert、原始错误字节、Windows Local attribute、macOS label、Linux feature / collection 行为；产出拟修改文件与依赖差异清单。若上游能力不足，列出最小可行方案及其对既有契约的影响，先决策再落地。
5. 本批只形成可审阅的依赖 / API 证据；正式仓库 `Cargo.toml`、`Cargo.lock`、notices、Rust 代码保持不变。临时解析与源码阅读不执行 key-store API、不编译或运行第三方 build script，不启动 VM / GUI、不安装全局工具、不改系统配置、不访问真实 key store、不 push 或触发远程 CI。

预计 30–60 分钟，主要副作用是 crates.io / RustSec 资料下载及任务专用磁盘缓存；不上传项目资料或凭据。使用唯一临时目录，收尾保留最小审阅证据，精确清理该目录内的源码副本与依赖缓存。没有正式 manifest / lockfile 修改，因此该批无需数据库、密钥或产品回滚。

### 后续实现切片与完成条件

- **P1-S03c-2 isolated provider adapter**：P1-S03c-1 解决兼容性问题并取得正式依赖 / 实现授权后，在现有 `radishmemory-source-vault` 扩展窄 provider 模块，复用 `KeyEncryptionKey`、随机源、zeroization 和错误体系。slot 输入来自经过验真的 host profile；遵循现有 `namespace-` / `device-` 加 32 位小写 hex 的宿主边界，避免 `:` / `/` 拼接歧义，不改变通用 canonical Identifier。
- **读取与 value 验收**：严格 `rmkek1:` + 64 个小写 hex，拒绝非 UTF-8、空白、额外尾随内容、大小写替代和错误长度；缺 key、ambiguous、损坏、取消、拒绝、不可用各有真实失败证据。load 不调用 setter，不触发 replacement KEK；敏感缓冲及上游错误不得进入日志。
- **bootstrap 验收归属**：分别定义 `load_existing` 与受控 `create_if_absent_for_bootstrap`，不提供 `bootstrap=true` 或调用方自报 `empty=true` 的授权捷径。provider 底层写操作保持内部边界；正式创建资格、事务锁和两实例验证由 P1-S04 接入，同一 slot 已有合法 key 时复用，已有坏 key 时失败关闭。
- **P1-S03c-3 platform evidence**：按具体测试账户、专用 slot、系统交互及清理范围分别批准后，再运行真实 macOS Keychain、Windows Local Credential Manager、Linux GNOME / KDE-compatible Secret Service 验收；包括创建、read-back、关闭重开、取消、锁定、拒绝、重复匹配和精确测试凭据清理。默认仓库测试不自动访问 OS store。合成 store tests、单平台成功和清理命令成功均不能替代实际平台或清理结果证据。
- P1-S03c 不接 SQLite migration、application / UI，也不新增 key rotation、恢复、同步或通用 credential 删除入口；这些仍按 ADR 0008 的后续单元推进。

### 本批记录

2026-09-15 基线 `5acdf74`；完成授权的隔离解析与源码核对：22 个新增 package、467 个发布文件校验通过，原有锁定版本不变，三目标 metadata / feature tree 和当前 RustSec 静态复核完成。正式 manifest / lockfile 和 Rust 实现未修改；候选 provider 编译、真实 key store、VM 与平台 provider 测试均未运行。macOS label、Linux default collection guard 的两个直接依赖补充及 Windows comment 映射随后已批准并在 P1-S03c-2 落地；预检当批范围详见[预检结果与下一批范围](2026-09-15-source-vault-provider-preflight.md#建议决策与下一批精确范围)。

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

## P1-S04a 衔接

`b484656` 已实现受 SQLite `IMMEDIATE` transaction 约束的 `initialize_library_key`、真实数据库 / 对象目录资格检查和 v7 `key_ready` checkpoint，见[初始化协调记录](phase1-source-vault-key-bootstrap.md)。原 slot-only 创建入口没有公开；历史章节中的等待 P1-S04 指当批顺位。正文对象 migration、完整重启恢复与真实平台密钥库仍未验收，当前顺位以[当前状态](../status/current.md)为准。
