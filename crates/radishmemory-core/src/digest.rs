use sha2::{Digest as _, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::{CanonicalJson, CoreError, InvalidCanonicalObjectReason};

/// Frozen M0 SHA-256 digest profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestProfile {
    ExactBytesV1,
    Utf8NfcTextV1,
    CanonicalJsonV1,
    FixtureSuiteV1,
    ContextPackV1,
    DeletionEvidenceV1,
}

impl DigestProfile {
    /// Parses a frozen profile identifier and fails closed on unknown input.
    pub fn parse(profile: &str) -> Result<Self, CoreError> {
        match profile {
            "exact-bytes-v1" => Ok(Self::ExactBytesV1),
            "utf8-nfc-text-v1" => Ok(Self::Utf8NfcTextV1),
            "canonical-json-v1" => Ok(Self::CanonicalJsonV1),
            "fixture-suite-v1" => Ok(Self::FixtureSuiteV1),
            "context-pack-v1" => Ok(Self::ContextPackV1),
            "deletion-evidence-v1" => Ok(Self::DeletionEvidenceV1),
            _ => Err(CoreError::unsupported_profile()),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactBytesV1 => "exact-bytes-v1",
            Self::Utf8NfcTextV1 => "utf8-nfc-text-v1",
            Self::CanonicalJsonV1 => "canonical-json-v1",
            Self::FixtureSuiteV1 => "fixture-suite-v1",
            Self::ContextPackV1 => "context-pack-v1",
            Self::DeletionEvidenceV1 => "deletion-evidence-v1",
        }
    }
}

/// A validated M0 SHA-256 digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Digest {
    profile: DigestProfile,
    value: String,
}

impl Digest {
    /// Validates a persisted digest without retaining the rejected value in errors.
    pub fn parse(algorithm: &str, profile: &str, value: &str) -> Result<Self, CoreError> {
        if algorithm != "sha256" {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidDigestValue,
            ));
        }
        let profile = DigestProfile::parse(profile)?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidDigestValue,
            ));
        }
        Ok(Self {
            profile,
            value: value.to_owned(),
        })
    }

    #[must_use]
    pub const fn algorithm(&self) -> &'static str {
        "sha256"
    }

    #[must_use]
    pub const fn profile(&self) -> DigestProfile {
        self.profile
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Computes SHA-256 over exact bytes without Unicode or newline normalization.
#[must_use]
pub fn compute_exact_bytes_digest(input: &[u8]) -> Digest {
    make_digest(DigestProfile::ExactBytesV1, input)
}

/// Normalizes semantic text to NFC, encodes it as UTF-8, then computes SHA-256.
#[must_use]
pub fn compute_nfc_text_digest(input: &str) -> Digest {
    let normalized = input.nfc().collect::<String>();
    make_digest(DigestProfile::Utf8NfcTextV1, normalized.as_bytes())
}

/// Canonicalizes JSON with `radishmemory-canonical-json-v1` and computes SHA-256.
pub fn compute_canonical_json_digest(input: &str) -> Result<Digest, CoreError> {
    let canonical = CanonicalJson::parse(input)?;
    Ok(make_digest(
        DigestProfile::CanonicalJsonV1,
        canonical.as_bytes(),
    ))
}

/// Computes one of the frozen digest profiles for UTF-8 M0 input.
pub fn compute_digest(profile: &str, input: &str) -> Result<Digest, CoreError> {
    match DigestProfile::parse(profile)? {
        DigestProfile::ExactBytesV1 => Ok(compute_exact_bytes_digest(input.as_bytes())),
        DigestProfile::Utf8NfcTextV1 => Ok(compute_nfc_text_digest(input)),
        DigestProfile::CanonicalJsonV1
        | DigestProfile::FixtureSuiteV1
        | DigestProfile::ContextPackV1
        | DigestProfile::DeletionEvidenceV1 => {
            compute_profiled_canonical_json_digest(DigestProfile::parse(profile)?, input)
        }
    }
}

fn compute_profiled_canonical_json_digest(
    profile: DigestProfile,
    input: &str,
) -> Result<Digest, CoreError> {
    let canonical = CanonicalJson::parse(input)?;
    Ok(make_digest(profile, canonical.as_bytes()))
}

/// Recomputes a digest and returns a stable mismatch without retaining content.
pub fn verify_digest(
    profile: &str,
    expected_value: &str,
    input: &str,
) -> Result<Digest, CoreError> {
    let actual = compute_digest(profile, input)?;
    if actual.value == expected_value {
        Ok(actual)
    } else {
        Err(CoreError::digest_mismatch())
    }
}

fn make_digest(profile: DigestProfile, input: &[u8]) -> Digest {
    let bytes = Sha256::digest(input);
    let mut value = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        value.push(HEX[usize::from(byte >> 4)] as char);
        value.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    Digest { profile, value }
}
