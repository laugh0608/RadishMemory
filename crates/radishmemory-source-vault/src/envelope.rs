//! Adapter-private, bounded, strictly ordered TLV format. This does not change the AAD codec.
use crate::{
    AEAD_TAG_BYTES, DEK_WRAP_PROFILE, DIGEST_PROFILE, ENVELOPE_PROFILE, MAX_OBJECT_PLAINTEXT_BYTES,
    OBJECT_CIPHER_PROFILE, ObjectMetadata, PROVIDER_PROFILE, SEGMENT_PLAINTEXT_BYTES, SealedObject,
    SourceVaultError, SourceVaultErrorCode,
};

const MAGIC: &[u8] = b"RMOBJ\x01";
const GENERATOR: &[u8] = b"radishmemory.source-object-writer/1";
const MAX_METADATA_FIELD: usize = 4096;
pub(crate) const MAX_ENVELOPE_BYTES: usize =
    MAX_OBJECT_PLAINTEXT_BYTES + 3 * MAX_METADATA_FIELD + 2048;

pub(crate) fn encode(
    metadata: &ObjectMetadata,
    sealed: &SealedObject,
) -> Result<Vec<u8>, SourceVaultError> {
    validate_metadata(metadata)?;
    let mut out = MAGIC.to_vec();
    for (tag, bytes) in [
        (1, ENVELOPE_PROFILE.as_bytes()),
        (2, GENERATOR),
        (3, OBJECT_CIPHER_PROFILE.as_bytes()),
        (4, DEK_WRAP_PROFILE.as_bytes()),
        (5, PROVIDER_PROFILE.as_bytes()),
        (6, DIGEST_PROFILE.as_bytes()),
        (7, metadata.namespace_id().as_bytes()),
        (8, metadata.source_id().as_bytes()),
        (9, metadata.exact_digest()),
        (10, &metadata.plaintext_len().to_be_bytes()),
        (11, metadata.media_type().as_bytes()),
        (12, &(SEGMENT_PLAINTEXT_BYTES as u32).to_be_bytes()),
        (13, sealed.stream_nonce_prefix()),
        (14, sealed.wrap_nonce()),
        (15, sealed.wrapped_dek()),
    ] {
        field(&mut out, tag, bytes);
    }
    let length: usize = sealed.segments().iter().map(Vec::len).sum();
    out.push(16);
    out.extend_from_slice(&(length as u32).to_be_bytes());
    for segment in sealed.segments() {
        out.extend_from_slice(segment);
    }
    Ok(out)
}

pub(crate) fn decode(
    bytes: &[u8],
    expected: &ObjectMetadata,
) -> Result<SealedObject, SourceVaultError> {
    validate_metadata(expected)?;
    if bytes.len() > MAX_ENVELOPE_BYTES || !bytes.starts_with(MAGIC) {
        return Err(malformed());
    }
    let mut input = &bytes[MAGIC.len()..];
    for (tag, profile) in [
        (1, ENVELOPE_PROFILE.as_bytes()),
        (2, GENERATOR),
        (3, OBJECT_CIPHER_PROFILE.as_bytes()),
        (4, DEK_WRAP_PROFILE.as_bytes()),
        (5, PROVIDER_PROFILE.as_bytes()),
        (6, DIGEST_PROFILE.as_bytes()),
    ] {
        if take(&mut input, tag)? != profile {
            return Err(malformed());
        }
    }
    // Compare against independently supplied canonical facts; disk metadata never becomes authority.
    for (tag, value) in [
        (7, expected.namespace_id().as_bytes()),
        (8, expected.source_id().as_bytes()),
        (9, expected.exact_digest()),
        (10, &expected.plaintext_len().to_be_bytes()),
        (11, expected.media_type().as_bytes()),
    ] {
        if take(&mut input, tag)? != value {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::MetadataMismatch,
                "envelope does not match expected object metadata",
            ));
        }
    }
    if take(&mut input, 12)? != (SEGMENT_PLAINTEXT_BYTES as u32).to_be_bytes() {
        return Err(malformed());
    }
    let stream_nonce_prefix = take(&mut input, 13)?.try_into().map_err(|_| malformed())?;
    let wrap_nonce = take(&mut input, 14)?.try_into().map_err(|_| malformed())?;
    let wrapped_dek = take(&mut input, 15)?.try_into().map_err(|_| malformed())?;
    let ciphertext = take(&mut input, 16)?;
    let length = expected.plaintext_len() as usize;
    let count = length.div_ceil(SEGMENT_PLAINTEXT_BYTES).max(1);
    if !input.is_empty() || ciphertext.len() != length + count * AEAD_TAG_BYTES {
        return Err(malformed());
    }
    let segments = ciphertext
        .chunks(SEGMENT_PLAINTEXT_BYTES + AEAD_TAG_BYTES)
        .map(<[u8]>::to_vec)
        .collect();
    Ok(SealedObject {
        stream_nonce_prefix,
        wrap_nonce,
        wrapped_dek,
        segments,
    })
}

pub(crate) fn validate_metadata(metadata: &ObjectMetadata) -> Result<(), SourceVaultError> {
    if metadata.plaintext_len() > MAX_OBJECT_PLAINTEXT_BYTES as u64 {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::PlaintextTooLarge,
            "object length exceeds the filesystem adapter limit",
        ));
    }
    if [
        metadata.namespace_id(),
        metadata.source_id(),
        metadata.media_type(),
    ]
    .iter()
    .any(|value| value.len() > MAX_METADATA_FIELD)
    {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::InvalidMetadata,
            "filesystem envelope metadata field exceeds its limit",
        ));
    }
    Ok(())
}

fn field(out: &mut Vec<u8>, tag: u8, bytes: &[u8]) {
    out.push(tag);
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn take<'a>(input: &mut &'a [u8], tag: u8) -> Result<&'a [u8], SourceVaultError> {
    if input.len() < 5 || input[0] != tag {
        return Err(malformed());
    }
    let size = u32::from_be_bytes(input[1..5].try_into().map_err(|_| malformed())?) as usize;
    let body = input.get(5..).ok_or_else(malformed)?;
    let value = body.get(..size).ok_or_else(malformed)?;
    *input = &body[size..];
    Ok(value)
}

fn malformed() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::InvalidEnvelope,
        "unsupported or malformed object envelope",
    )
}

#[cfg(test)]
#[path = "envelope_tests.rs"]
mod tests;
