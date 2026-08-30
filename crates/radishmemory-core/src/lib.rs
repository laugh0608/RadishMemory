//! Canonical domain primitives for RadishMemory M0.
//!
//! This crate owns M0 canonical values and objects plus deterministic time,
//! digest, and JSON behavior. Storage and command-line concerns remain outside
//! this boundary.

mod canonical_json;
mod context;
mod deletion;
mod digest;
mod error;
mod invariants;
mod library;
mod memory;
mod model;
mod ports;
mod search;
mod source;
mod temporal;

pub use canonical_json::{CanonicalJson, canonicalize_json};
pub use context::*;
pub use deletion::*;
pub use digest::{
    Digest, DigestProfile, compute_canonical_json_digest, compute_digest,
    compute_exact_bytes_digest, compute_nfc_text_digest, verify_digest,
};
pub use error::{
    CoreError, CoreErrorCode, CrossObjectInvariantReason, InvalidCanonicalObjectReason,
    InvalidTimeReason, NonCanonicalJsonReason,
};
pub use invariants::*;
pub use library::*;
pub use memory::*;
pub use model::*;
pub use ports::{
    DeletionStore, LocalSearch, MemoryStore, SourceCaptureStore, SourceCatalog, SourceVault,
};
pub use search::{LocalSearchHit, LocalSearchRequest};
pub use source::*;
pub use temporal::{TimePrecision, Timestamp, TimestampPrecision, ValidTime, ValidTimeMode};
