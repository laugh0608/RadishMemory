//! Canonical domain primitives for RadishMemory M0.
//!
//! This crate owns deterministic time, digest, and JSON behavior. Storage and
//! command-line concerns remain outside this boundary.

mod canonical_json;
mod digest;
mod error;
mod temporal;

pub use canonical_json::{CanonicalJson, canonicalize_json};
pub use digest::{
    Digest, DigestProfile, compute_canonical_json_digest, compute_digest,
    compute_exact_bytes_digest, compute_nfc_text_digest, verify_digest,
};
pub use error::{CoreError, CoreErrorCode, InvalidTimeReason, NonCanonicalJsonReason};
pub use temporal::{TimePrecision, Timestamp, TimestampPrecision, ValidTime, ValidTimeMode};
