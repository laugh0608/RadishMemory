# Source Vault 密钥初始化与 SQLite 事务协调

日期：2026-09-15

状态：`P1-S04a key bootstrap coordination implemented — synthetic acceptance`

基线：`146ea8f`（P1-S03c-2 provider 提交）。项目所有者确认提交工作区并推进下一批，范围为 [ADR 0008](../adr/0008-phase1-encrypted-source-vault.md)下的初始化资格、SQLite writer 串行化、失败重试与合成验证。Windows / Linux 编译和运行验收按[当前状态](../status/current.md)后置集中执行。

## 交付与边界

- `PlatformKeyProvider::initialize_library_key` 是完整的维护编排入口：从 `ObjectDirectory` 取得同一应用目录内的 `library.sqlite3`，验证 namespace / device 形状，在 SQLite `IMMEDIATE` transaction 内检查真实事实，再调用原 provider 的 strict load / bootstrap，最后提交 key checkpoint。没有公开 slot-only creator、setter 或 caller eligibility flag。
- `radishmemory-sqlite` 拥有 `SourceVaultKeyDatabase`、不可复制的 `KeyInitializationTransaction` 和 SQL；Source Vault 组合现有 SQLite adapter、对象目录与 provider，不重复实现 canonical schema 或 body decoder。为此增加 Source Vault → SQLite 的第一方依赖；测试复用现有 `rusqlite`。不存在反向依赖或新 package。
- 全新库在同一事务内建立既有 v6 基础表；v6 库复验后进入初始化；v7 库只允许读取既有 key。v1–v5 必须先通过原有 v6 migration；更高版本、未知表 / view / trigger、列定义漂移和错误 migration history 均拒绝。
- `0007_source_vault_key.sql` 只新增单例 `radishmemory_source_vault_key_profile`，记录 namespace、device、provider profile 和 `key_ready`。没有 KEK、secret blob、wrapped DEK、路径或正文。v7 是维护准备 checkpoint；正文对象引用 / attempt / migration 数据格式由下一批继续实现。
- 普通 `SqliteDatabase::open` 与默认 migrations 仍停在 v6，遇到 v7 拒绝打开；本批没有接入 application / UI，也没有把任何实际用户数据库升级为 v7。这个维护类型不提供普通读取、写入或搜索操作。

## 初始化资格如何成立

1. SQLite 打开使用 `SQLITE_OPEN_NOFOLLOW`，沿用 foreign keys、`trusted_schema=OFF`、`synchronous=FULL` 和 DELETE journal policy；正式宿主负责提供已解析的应用目录与经过验真的 host profile。Source Vault 创建新空数据库时使用 create-new / 私有权限、file sync 与目录 sync，打开前后保留文件身份引用。
2. 获取 `IMMEDIATE` writer lock 后重新读取 schema、migration history 和 checkpoint；资格不能在等待锁之前缓存。drop / error 自动回滚，事务及资格不能 clone 或与连接脱离。
3. 从既有 migration SQL 构造内存期望 schema，比较 table / index / view / trigger 定义；执行 quick check、foreign key check，并验证 canonical 对象 namespace。
4. 复验全部剩余 inline BLOB 的 exact digest / length，包括历史版本和删除残留；对全部 active source 重用既有 source / fragment decoder 与来源关系验证，再检查 origin bindings 和派生索引。当前搜索只覆盖 lineage tip，不足以证明全部旧正文可迁移。
5. 扫描实际 object / staging 目录。任意文件名、子目录、symlink、published object 或 staging 残留都使其不再具备新建资格；不自动识别为垃圾或删除。此时只读 provider：missing 原样返回 `KeyMissing`；key 存在仍返回待 reconciliation 的失败，不伪造正文迁移完成。
6. 仅无 checkpoint 且目录为空时进入私有 bootstrap；有合法 key 就复用，损坏 / ambiguity / denied 等错误不覆盖。已存在 checkpoint 时只能 strict load，绝不因缺 key 进入生成逻辑。
7. provider 写后 read-back、label 检查与值比较继续复用 P1-S03c 实现。返回后再次检查目录和数据库身份；提交 provider profile / `key_ready`，commit 成功后才返回 KEK capability。

## 失败、重试与保证范围

- provider write 成功、SQLite commit 失败时，SQLite 回滚而同 slot 的 key 保留；下一次在完整资格检查后复用原值。没有自动删除孤立 credential、自动轮换或重建替代 key。
- 目录或数据库身份在调用中变化时拒绝成功。SQLite 与文件系统、系统密钥库没有共同原子事务；前后检查不防御恶意同权限进程的任意瞬时替换，也不能替代后续宿主生命周期协调。
- 接入宿主前必须暂停普通 library operations，并处理旧进程持有的 v6 连接。当前入口的 v7 open 拒绝不等于已存在的普通连接会自动失效；本批没有宣称产品多实例迁移已验收。
- 真实 native store 调用仍可能 prompt / 写入，需明确测试账户、slot 与清理范围；默认 tests 只使用合成 provider。上游 logger 过滤仍是实际 OS 接入前置条件，不能由第一方错误脱敏代替。
- `KeyInitializationError` 只保留固定的 SQLite code / storage reason / extended numeric code 或已脱敏 Source Vault error，不保留任意 SQLite source chain、SQL 文本、slot、路径或正文。
- v7 checkpoint 不证明正文已加密；inline body、FTS 与 metadata 未迁移，历史明文也没有物理清除。既有 key 若被外部替换成另一合法编码，最终仍须由后续 encrypted object authentication 识别；本批无对象时没有声称具备该认证证据。

## 验证

合成验收覆盖：

- 全新库初始化、提交与重开；同 key 继续解密先前合成 ciphertext；一个 slot 只写入一次。
- 两个独立 SQLite 连接竞争 writer lock；等待者在锁内重新看到 `key_ready` 并读取同一个 key。
- 独立测试子进程在首个 writer 提交前不能获得资格，提交后必须读到已初始化状态。子进程 helper 以 ignored test 形式只由父测试精确启动，不是未执行的验收项。
- 一个真实 SQLite reader 持有锁，导致 checkpoint COMMIT 返回失败；释放 reader 后重试复用已写 key，v6 → v7 成立。
- provider missing / corrupt / ambiguous / locked / cancelled / denied / unavailable；checkpoint 后丢 key 不补发。
- object / staging / unknown entry 拦截、provider 调用期间目录变化、数据库 symlink / 身份替换拒绝。
- v6 正文、版本和片段保留；非当前版本正文损坏或缺失、片段越界、namespace / device / provider profile 不匹配、checkpoint 丢失、未来 schema 与隐藏结构漂移拒绝。
- fresh / v6 transaction drop 的回滚；错误 Display / Debug / source chain 不泄漏合成敏感字段。

本机离线 `./scripts/check-repo.sh` 通过：179 个仓库文件、workspace fmt / Clippy（all targets / features）与 191 个 Rust tests 通过；另有 35 个 Python 检查器测试和 1 个 compile-fail doctest 通过。一个 ignored 子进程 helper 已由父测试实际运行成功，不计入上述主 harness 的 191 个通过数。`git diff --check` 通过。最终错误诊断补充保留 SQLite numeric code 后，再次通过 SQLite / Source Vault all-targets Clippy 与 10 个 bootstrap 回归；真实 commit 失败明确断言为 `SQLITE_BUSY`。

首次新增 SQLite 验收因 macOS 临时路径的 symlink 被 `SQLITE_OPEN_NOFOLLOW` 拒绝；随后把合成 fixture 的父目录解析为真实路径，保留 production NOFOLLOW 边界后全部通过。真实 Keychain、Windows / Linux provider 编译、跨平台进程锁、断电、真实库迁移与宿主验收均不在本批证据内。

## 依赖复核与后续

Cargo 以 offline metadata 更新 lockfile，只增加 Source Vault 的第一方 `radishmemory-sqlite` 和测试 `rusqlite` 两条连线。453 个 package 的 name / version / source / checksum 集合不变；重生成 notices 仍为 366 项，目标数为 macOS 222、Linux 301、Windows 214，inventory SHA-256 保持 `fc17c7a1f4f93e93761c8668beb988fa83290fbbc81ef592f0ab0efe60692bf3`。lockfile SHA-256 为 `b4efc35519a91c33eca592251116f9434f8aaa3baa3e293fba3c9dea1fc1b698`。

下一批为 `P1-S04b`：持久化 object references 与 capture / migration attempts、逐项正文迁移、read-back 与可中断恢复；之后继续 orphan、verify / rebuild、deletion execution 和 P1-S05 application / host acceptance。不能在这些步骤完成前进入 PDF / 图片解析或声明 encrypted Source Vault 可用。

本批未访问真实密钥库、启动 GUI / VM、修改系统配置或执行远程动作。合成数据库、对象和测试子进程由各验收清理；常规忽略的编译缓存保留。P1-S03c-2 已提交为 `146ea8f`；P1-S04a 更改暂留工作区供审阅，未 push。
