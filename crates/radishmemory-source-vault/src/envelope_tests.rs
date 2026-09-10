use super::*;
use crate::crypto::seal_object_with_random;
use crate::test_support::{FixedRandom, key, metadata};
use crate::{KeyEncryptionKey, open_object};
use sha2::{Digest, Sha256};

fn fixture() -> (ObjectMetadata, Vec<u8>) {
    let bytes = b"synthetic envelope content";
    let metadata = metadata(bytes);
    let sealed = seal_object_with_random(&key(), &metadata, bytes, &mut FixedRandom(0)).unwrap();
    (metadata.clone(), encode(&metadata, &sealed).unwrap())
}

fn bounds(bytes: &[u8], wanted: u8) -> (usize, usize) {
    let mut offset = MAGIC.len();
    for tag in 1..=16 {
        assert_eq!(bytes[offset], tag);
        let length = u32::from_be_bytes(bytes[offset + 1..offset + 5].try_into().unwrap()) as usize;
        if tag == wanted {
            return (offset, offset + 5 + length);
        }
        offset += 5 + length;
    }
    panic!("unknown test tag")
}

#[test]
fn envelope_format_matches_independent_binary_oracle() {
    // Oracle built independently with Python struct.pack('>BI', tag, len(value));
    // synthetic opaque cipher bytes exercise layout, not cryptographic correctness.
    let metadata =
        ObjectMetadata::new("n", "s", Sha256::digest(b"abc").into(), 3, "text/plain").unwrap();
    let sealed = SealedObject {
        stream_nonce_prefix: [0x11; 19],
        wrap_nonce: [0x22; 24],
        wrapped_dek: [0x33; 48],
        segments: vec![vec![0x44; 19]],
    };
    let bytes = encode(&metadata, &sealed).unwrap();
    assert_eq!(bytes.len(), 463);
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest,
        "6fdab8f60cb1938cd9daaea3a88458b17928fe09446dc316ef74fdd43bbdf253"
    );
    let decoded = decode(&bytes, &metadata).unwrap();
    assert!(decoded.stream_nonce_prefix() == sealed.stream_nonce_prefix());
    assert!(decoded.wrap_nonce() == sealed.wrap_nonce());
    assert!(decoded.wrapped_dek() == sealed.wrapped_dek());
    assert!(decoded.segments() == sealed.segments());
}

#[test]
fn every_field_rejects_missing_duplicate_reordered_and_oversized_encoding() {
    let (metadata, bytes) = fixture();
    for tag in 1..=16 {
        let (start, end) = bounds(&bytes, tag);
        let mut missing = bytes.clone();
        missing.drain(start..end);
        assert!(decode(&missing, &metadata).is_err(), "missing field {tag}");
        let mut duplicate = bytes.clone();
        duplicate.splice(start..start, bytes[start..end].iter().copied());
        assert!(
            decode(&duplicate, &metadata).is_err(),
            "duplicate field {tag}"
        );
        let mut changed_tag = bytes.clone();
        changed_tag[start] = 255;
        assert!(
            decode(&changed_tag, &metadata).is_err(),
            "unknown field {tag}"
        );
        let mut oversized = bytes.clone();
        oversized[start + 1..start + 5].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(
            decode(&oversized, &metadata).is_err(),
            "oversized field {tag}"
        );
        if tag < 16 {
            let (_, next_end) = bounds(&bytes, tag + 1);
            let mut reordered = bytes.clone();
            reordered.splice(
                start..next_end,
                bytes[end..next_end]
                    .iter()
                    .chain(bytes[start..end].iter())
                    .copied(),
            );
            assert!(
                decode(&reordered, &metadata).is_err(),
                "reordered field {tag}"
            );
        }
    }
}

#[test]
fn truncated_trailing_wrong_version_profiles_and_generator_fail_closed() {
    let (metadata, bytes) = fixture();
    for cut in 0..bytes.len() {
        assert!(decode(&bytes[..cut], &metadata).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing, &metadata).is_err());
    let mut version = bytes.clone();
    version[5] = 2;
    assert!(decode(&version, &metadata).is_err());
    for tag in [1, 2, 3, 4, 5, 6, 12] {
        let (start, _) = bounds(&bytes, tag);
        let mut unknown = bytes.clone();
        unknown[start + 5] ^= 1;
        assert_eq!(
            decode(&unknown, &metadata).unwrap_err().code(),
            SourceVaultErrorCode::InvalidEnvelope
        );
    }
}

#[test]
fn external_metadata_and_all_encrypted_components_remain_authenticated() {
    let (metadata, bytes) = fixture();
    for tag in 7..=11 {
        let (start, _) = bounds(&bytes, tag);
        let mut wrong = bytes.clone();
        wrong[start + 5] ^= 1;
        assert_eq!(
            decode(&wrong, &metadata).unwrap_err().code(),
            SourceVaultErrorCode::MetadataMismatch
        );
    }
    for tag in 13..=16 {
        let (start, end) = bounds(&bytes, tag);
        for index in [start + 5, end - 1] {
            let mut wrong = bytes.clone();
            wrong[index] ^= 1;
            let decoded = decode(&wrong, &metadata).unwrap();
            assert_eq!(
                open_object(&key(), &metadata, &decoded).unwrap_err().code(),
                SourceVaultErrorCode::AuthenticationFailed
            );
        }
    }
    let decoded = decode(&bytes, &metadata).unwrap();
    assert_eq!(
        open_object(&KeyEncryptionKey::new([0xa6; 32]), &metadata, &decoded)
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::AuthenticationFailed
    );
    assert!(open_object(&key(), &metadata, &decoded).unwrap() == b"synthetic envelope content");
}

#[test]
fn parser_bounds_metadata_and_total_allocation() {
    let (_, bytes) = fixture();
    let huge_metadata =
        ObjectMetadata::new("n".repeat(4097), "s", [0; 32], 0, "text/plain").unwrap();
    assert_eq!(
        decode(&bytes, &huge_metadata).unwrap_err().code(),
        SourceVaultErrorCode::InvalidMetadata
    );
    let huge_length = ObjectMetadata::new(
        "n",
        "s",
        [0; 32],
        MAX_OBJECT_PLAINTEXT_BYTES as u64 + 1,
        "text/plain",
    )
    .unwrap();
    assert_eq!(
        decode(&bytes, &huge_length).unwrap_err().code(),
        SourceVaultErrorCode::PlaintextTooLarge
    );
    assert_eq!(
        decode(&vec![0; MAX_ENVELOPE_BYTES + 1], &metadata(b""))
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidEnvelope
    );
}
