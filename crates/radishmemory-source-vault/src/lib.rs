#![forbid(unsafe_code)]

mod aad;
mod crypto;
mod error;
mod random;

pub use aad::ObjectMetadata;
pub use crypto::{KeyEncryptionKey, SealedObject, open_object, seal_object};
pub use error::{SourceVaultError, SourceVaultErrorCode};

pub const ENVELOPE_PROFILE: &str = "radishmemory.phase1-encrypted-source-vault/1";
pub const OBJECT_CIPHER_PROFILE: &str = "radishmemory.xchacha20poly1305-stream-be32/1";
pub const DEK_WRAP_PROFILE: &str = "radishmemory.xchacha20poly1305-dek-wrap/1";
pub const PROVIDER_PROFILE: &str = "radishmemory.platform-key-store/1";
pub const DIGEST_PROFILE: &str = "exact-bytes-v1";
pub const SEGMENT_PLAINTEXT_BYTES: usize = 1024 * 1024;
pub const MAX_OBJECT_PLAINTEXT_BYTES: usize = 8 * 1024 * 1024;

const STREAM_NONCE_PREFIX_BYTES: usize = 19;
const WRAP_NONCE_BYTES: usize = 24;
const KEY_BYTES: usize = 32;
const AEAD_TAG_BYTES: usize = 16;
