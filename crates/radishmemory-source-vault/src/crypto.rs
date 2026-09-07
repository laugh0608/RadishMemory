use std::fmt;

use aead_stream::aead::{Aead, KeyInit, Payload};
use aead_stream::{DecryptorBE32, EncryptorBE32, Nonce, StreamBE32};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::aad::ObjectMetadata;
use crate::error::{SourceVaultError, SourceVaultErrorCode};
use crate::random::{RandomSource, SystemRandom};
use crate::{
    AEAD_TAG_BYTES, KEY_BYTES, MAX_OBJECT_PLAINTEXT_BYTES, SEGMENT_PLAINTEXT_BYTES,
    STREAM_NONCE_PREFIX_BYTES, WRAP_NONCE_BYTES,
};

type StreamNonce = Nonce<XChaCha20Poly1305, StreamBE32<XChaCha20Poly1305>>;

pub struct KeyEncryptionKey(Zeroizing<[u8; KEY_BYTES]>);

impl KeyEncryptionKey {
    pub fn new(bytes: [u8; KEY_BYTES]) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

impl fmt::Debug for KeyEncryptionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KeyEncryptionKey([REDACTED])")
    }
}

pub struct SealedObject {
    stream_nonce_prefix: [u8; STREAM_NONCE_PREFIX_BYTES],
    wrap_nonce: [u8; WRAP_NONCE_BYTES],
    wrapped_dek: [u8; KEY_BYTES + AEAD_TAG_BYTES],
    segments: Vec<Vec<u8>>,
}

impl SealedObject {
    pub fn stream_nonce_prefix(&self) -> &[u8; STREAM_NONCE_PREFIX_BYTES] {
        &self.stream_nonce_prefix
    }

    pub fn wrap_nonce(&self) -> &[u8; WRAP_NONCE_BYTES] {
        &self.wrap_nonce
    }

    pub fn wrapped_dek(&self) -> &[u8; KEY_BYTES + AEAD_TAG_BYTES] {
        &self.wrapped_dek
    }

    pub fn segments(&self) -> &[Vec<u8>] {
        &self.segments
    }
}

impl fmt::Debug for SealedObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SealedObject")
            .field("stream_nonce_prefix", &"[REDACTED]")
            .field("wrap_nonce", &"[REDACTED]")
            .field("wrapped_dek", &"[REDACTED]")
            .field("segment_count", &self.segments.len())
            .finish()
    }
}

pub fn seal_object(
    key_encryption_key: &KeyEncryptionKey,
    metadata: &ObjectMetadata,
    plaintext: &[u8],
) -> Result<SealedObject, SourceVaultError> {
    seal_object_with_random(key_encryption_key, metadata, plaintext, &mut SystemRandom)
}

pub fn open_object(
    key_encryption_key: &KeyEncryptionKey,
    metadata: &ObjectMetadata,
    sealed: &SealedObject,
) -> Result<Vec<u8>, SourceVaultError> {
    validate_declared_plaintext(metadata, None)?;
    validate_segment_layout(metadata.plaintext_len(), &sealed.segments)?;

    let wrap_aad = Zeroizing::new(metadata.wrap_aad(&sealed.stream_nonce_prefix));
    let dek = unwrap_dek(
        key_encryption_key,
        &sealed.wrap_nonce,
        &sealed.wrapped_dek,
        &wrap_aad,
    )?;
    let object_aad = Zeroizing::new(metadata.object_aad(&sealed.stream_nonce_prefix));
    let mut plaintext = decrypt_stream(
        &dek,
        &sealed.stream_nonce_prefix,
        &sealed.segments,
        &object_aad,
    )?;

    if plaintext.len() as u64 != metadata.plaintext_len() {
        plaintext.zeroize();
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::LengthMismatch,
            "decrypted plaintext length does not match authenticated metadata",
        ));
    }
    if Sha256::digest(&plaintext).as_slice() != metadata.exact_digest() {
        plaintext.zeroize();
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::DigestMismatch,
            "decrypted plaintext digest does not match authenticated metadata",
        ));
    }
    Ok(plaintext)
}

fn seal_object_with_random<R: RandomSource>(
    key_encryption_key: &KeyEncryptionKey,
    metadata: &ObjectMetadata,
    plaintext: &[u8],
    random: &mut R,
) -> Result<SealedObject, SourceVaultError> {
    validate_declared_plaintext(metadata, Some(plaintext))?;

    let mut dek = Zeroizing::new([0_u8; KEY_BYTES]);
    random.fill(dek.as_mut())?;
    let mut stream_nonce_prefix = [0_u8; STREAM_NONCE_PREFIX_BYTES];
    random.fill(&mut stream_nonce_prefix)?;
    let mut wrap_nonce = [0_u8; WRAP_NONCE_BYTES];
    random.fill(&mut wrap_nonce)?;

    let object_aad = Zeroizing::new(metadata.object_aad(&stream_nonce_prefix));
    let segments = encrypt_stream(&dek, &stream_nonce_prefix, plaintext, &object_aad)?;
    let wrap_aad = Zeroizing::new(metadata.wrap_aad(&stream_nonce_prefix));
    let wrapped_dek = wrap_dek(key_encryption_key, &dek, &wrap_nonce, &wrap_aad)?;

    Ok(SealedObject {
        stream_nonce_prefix,
        wrap_nonce,
        wrapped_dek,
        segments,
    })
}

fn validate_declared_plaintext(
    metadata: &ObjectMetadata,
    plaintext: Option<&[u8]>,
) -> Result<(), SourceVaultError> {
    if metadata.plaintext_len() > MAX_OBJECT_PLAINTEXT_BYTES as u64 {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::PlaintextTooLarge,
            "plaintext length exceeds the Phase 1 object limit",
        ));
    }
    if let Some(plaintext) = plaintext {
        if plaintext.len() as u64 != metadata.plaintext_len() {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::LengthMismatch,
                "plaintext length does not match object metadata",
            ));
        }
        if Sha256::digest(plaintext).as_slice() != metadata.exact_digest() {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::DigestMismatch,
                "plaintext digest does not match object metadata",
            ));
        }
    }
    Ok(())
}

fn encrypt_stream(
    dek: &[u8; KEY_BYTES],
    stream_nonce_prefix: &[u8; STREAM_NONCE_PREFIX_BYTES],
    plaintext: &[u8],
    associated_data: &[u8],
) -> Result<Vec<Vec<u8>>, SourceVaultError> {
    let nonce = StreamNonce::from(*stream_nonce_prefix);
    let mut encryptor = EncryptorBE32::<XChaCha20Poly1305>::new(dek.into(), &nonce);
    let final_offset = if plaintext.is_empty() {
        0
    } else {
        ((plaintext.len() - 1) / SEGMENT_PLAINTEXT_BYTES) * SEGMENT_PLAINTEXT_BYTES
    };
    let mut segments = Vec::with_capacity(expected_segment_count(plaintext.len() as u64));
    for chunk in plaintext[..final_offset].chunks(SEGMENT_PLAINTEXT_BYTES) {
        segments.push(
            encryptor
                .encrypt_next(Payload {
                    msg: chunk,
                    aad: associated_data,
                })
                .map_err(|_| encryption_failed())?,
        );
    }
    segments.push(
        encryptor
            .encrypt_last(Payload {
                msg: &plaintext[final_offset..],
                aad: associated_data,
            })
            .map_err(|_| encryption_failed())?,
    );
    Ok(segments)
}

fn decrypt_stream(
    dek: &[u8; KEY_BYTES],
    stream_nonce_prefix: &[u8; STREAM_NONCE_PREFIX_BYTES],
    segments: &[Vec<u8>],
    associated_data: &[u8],
) -> Result<Vec<u8>, SourceVaultError> {
    let nonce = StreamNonce::from(*stream_nonce_prefix);
    let mut decryptor = DecryptorBE32::<XChaCha20Poly1305>::new(dek.into(), &nonce);
    let mut plaintext = Vec::new();

    for segment in &segments[..segments.len() - 1] {
        let chunk = decryptor
            .decrypt_next(Payload {
                msg: segment,
                aad: associated_data,
            })
            .map_err(|_| authentication_failed(&mut plaintext))?;
        let chunk = Zeroizing::new(chunk);
        plaintext.extend_from_slice(&chunk);
    }
    let final_chunk = decryptor
        .decrypt_last(Payload {
            msg: segments.last().expect("validated non-empty segment set"),
            aad: associated_data,
        })
        .map_err(|_| authentication_failed(&mut plaintext))?;
    let final_chunk = Zeroizing::new(final_chunk);
    plaintext.extend_from_slice(&final_chunk);
    Ok(plaintext)
}

fn wrap_dek(
    key_encryption_key: &KeyEncryptionKey,
    dek: &[u8; KEY_BYTES],
    nonce: &[u8; WRAP_NONCE_BYTES],
    associated_data: &[u8],
) -> Result<[u8; KEY_BYTES + AEAD_TAG_BYTES], SourceVaultError> {
    let cipher = XChaCha20Poly1305::new((&*key_encryption_key.0).into());
    let encrypted = cipher
        .encrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: dek,
                aad: associated_data,
            },
        )
        .map_err(|_| encryption_failed())?;
    let encrypted = Zeroizing::new(encrypted);
    encrypted
        .as_slice()
        .try_into()
        .map_err(|_| encryption_failed())
}

fn unwrap_dek(
    key_encryption_key: &KeyEncryptionKey,
    nonce: &[u8; WRAP_NONCE_BYTES],
    wrapped_dek: &[u8; KEY_BYTES + AEAD_TAG_BYTES],
    associated_data: &[u8],
) -> Result<Zeroizing<[u8; KEY_BYTES]>, SourceVaultError> {
    let cipher = XChaCha20Poly1305::new((&*key_encryption_key.0).into());
    let plaintext = cipher
        .decrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: wrapped_dek,
                aad: associated_data,
            },
        )
        .map_err(|_| {
            SourceVaultError::new(
                SourceVaultErrorCode::AuthenticationFailed,
                "wrapped data encryption key authentication failed",
            )
        })?;
    let plaintext = Zeroizing::new(plaintext);
    let dek: [u8; KEY_BYTES] = plaintext.as_slice().try_into().map_err(|_| {
        SourceVaultError::new(
            SourceVaultErrorCode::MalformedCiphertext,
            "wrapped data encryption key has an invalid length",
        )
    })?;
    Ok(Zeroizing::new(dek))
}

fn validate_segment_layout(
    plaintext_len: u64,
    segments: &[Vec<u8>],
) -> Result<(), SourceVaultError> {
    let expected_count = expected_segment_count(plaintext_len);
    if segments.len() != expected_count {
        return Err(malformed_ciphertext());
    }
    let final_plaintext_len = if plaintext_len == 0 {
        0
    } else {
        ((plaintext_len - 1) % SEGMENT_PLAINTEXT_BYTES as u64 + 1) as usize
    };
    for segment in &segments[..segments.len() - 1] {
        if segment.len() != SEGMENT_PLAINTEXT_BYTES + AEAD_TAG_BYTES {
            return Err(malformed_ciphertext());
        }
    }
    if segments.last().map(Vec::len) != Some(final_plaintext_len + AEAD_TAG_BYTES) {
        return Err(malformed_ciphertext());
    }
    Ok(())
}

fn expected_segment_count(plaintext_len: u64) -> usize {
    if plaintext_len == 0 {
        1
    } else {
        plaintext_len.div_ceil(SEGMENT_PLAINTEXT_BYTES as u64) as usize
    }
}

fn encryption_failed() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::EncryptionFailed,
        "authenticated encryption operation failed",
    )
}

fn malformed_ciphertext() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::MalformedCiphertext,
        "ciphertext segment layout does not match authenticated metadata",
    )
}

fn authentication_failed(plaintext: &mut Vec<u8>) -> SourceVaultError {
    plaintext.zeroize();
    SourceVaultError::new(
        SourceVaultErrorCode::AuthenticationFailed,
        "ciphertext stream authentication failed",
    )
}

#[cfg(test)]
mod tests {
    use aead_stream::aead::array::Array;

    use super::*;

    struct FixedRandom {
        bytes: Vec<u8>,
        offset: usize,
    }

    impl FixedRandom {
        fn new(bytes: Vec<u8>) -> Self {
            Self { bytes, offset: 0 }
        }
    }

    impl RandomSource for FixedRandom {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), SourceVaultError> {
            let end = self.offset + destination.len();
            destination.copy_from_slice(&self.bytes[self.offset..end]);
            self.offset = end;
            Ok(())
        }
    }

    struct FailingRandom;

    impl RandomSource for FailingRandom {
        fn fill(&mut self, _destination: &mut [u8]) -> Result<(), SourceVaultError> {
            Err(SourceVaultError::new(
                SourceVaultErrorCode::RandomSourceUnavailable,
                "synthetic random failure",
            ))
        }
    }

    fn metadata(plaintext: &[u8]) -> ObjectMetadata {
        ObjectMetadata::new(
            "namespace-synthetic",
            "source-synthetic",
            Sha256::digest(plaintext).into(),
            plaintext.len() as u64,
            "text/markdown",
        )
        .unwrap()
    }

    fn fixed_random() -> FixedRandom {
        FixedRandom::new((0_u8..75).collect())
    }

    fn sealed_digest(sealed: &SealedObject) -> String {
        let mut digest = Sha256::new();
        digest.update(sealed.stream_nonce_prefix);
        digest.update(sealed.wrap_nonce);
        digest.update(sealed.wrapped_dek);
        for segment in &sealed.segments {
            digest.update((segment.len() as u64).to_be_bytes());
            digest.update(segment);
        }
        hex(digest.finalize().as_slice())
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn system_random_failure_is_not_replaced_with_deterministic_bytes() {
        let plaintext = b"synthetic";
        let error = seal_object_with_random(
            &KeyEncryptionKey::new([0x44; 32]),
            &metadata(plaintext),
            plaintext,
            &mut FailingRandom,
        )
        .expect_err("random failure must stop sealing");
        assert_eq!(error.code(), SourceVaultErrorCode::RandomSourceUnavailable);
    }

    #[test]
    fn project_owned_stream_vectors_cover_phase1_size_boundaries() {
        let cases = [
            (
                "empty",
                Vec::new(),
                "032b861653930b67e222843a9aec4f4a3d81caee3509a3dd565002e83ec2d3be",
            ),
            (
                "single",
                b"radishmemory synthetic stream vector".to_vec(),
                "cbc0a3ddc48f2cadb6bd06aa3a008b03272bc9213c92fa43d00e9e128614d3e5",
            ),
            (
                "one-mib",
                vec![0x31; SEGMENT_PLAINTEXT_BYTES],
                "d375839563a44b29435be818ac71269174a6ff1dfc0e1b0006e631576b346b4c",
            ),
            (
                "cross-segment",
                vec![0x52; SEGMENT_PLAINTEXT_BYTES + 17],
                "27f9fa0bedaf515173822c763ca1ec790e2708eaa532e94b60c6be536b7aa9c2",
            ),
            (
                "eight-mib",
                vec![0x73; MAX_OBJECT_PLAINTEXT_BYTES],
                "abc781521321810394eb29836d6ba6a0736a37e743213cd491ecbccf45f0a43f",
            ),
        ];
        for (name, plaintext, expected_digest) in cases {
            let sealed = seal_object_with_random(
                &KeyEncryptionKey::new([0xa5; 32]),
                &metadata(&plaintext),
                &plaintext,
                &mut fixed_random(),
            )
            .unwrap();
            assert_eq!(
                sealed_digest(&sealed),
                expected_digest,
                "frozen STREAM vector changed for {name}"
            );
            assert_eq!(
                sealed.segments.len(),
                expected_segment_count(plaintext.len() as u64)
            );
            assert_eq!(
                open_object(
                    &KeyEncryptionKey::new([0xa5; 32]),
                    &metadata(&plaintext),
                    &sealed
                )
                .unwrap(),
                plaintext
            );
        }
    }

    #[test]
    fn project_owned_wrap_vector_is_stable() {
        let metadata = metadata(b"synthetic wrap vector");
        let nonce_prefix = [0x33; STREAM_NONCE_PREFIX_BYTES];
        let wrapped = wrap_dek(
            &KeyEncryptionKey::new([0x11; KEY_BYTES]),
            &[0x22; KEY_BYTES],
            &[0x44; WRAP_NONCE_BYTES],
            &metadata.wrap_aad(&nonce_prefix),
        )
        .unwrap();
        assert_eq!(
            hex(&wrapped),
            "fd30a33a3a00e24b95b4fcbdfc1a258e6b91abef385f3866fda506cc4077a360ea10ea44195c5ea68e31ebb5367a857d"
        );
        assert_eq!(
            *unwrap_dek(
                &KeyEncryptionKey::new([0x11; KEY_BYTES]),
                &[0x44; WRAP_NONCE_BYTES],
                &wrapped,
                &metadata.wrap_aad(&nonce_prefix)
            )
            .unwrap(),
            [0x22; KEY_BYTES]
        );
    }

    #[test]
    fn tampering_truncation_reordering_and_metadata_changes_fail_closed() {
        let plaintext = vec![0x42; SEGMENT_PLAINTEXT_BYTES * 2 + 3];
        let key = KeyEncryptionKey::new([0x11; 32]);
        let mut sealed =
            seal_object_with_random(&key, &metadata(&plaintext), &plaintext, &mut fixed_random())
                .unwrap();

        sealed.segments.swap(0, 1);
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );
        sealed.segments.swap(0, 1);
        sealed.segments[0][0] ^= 0x80;
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );
        sealed.segments[0][0] ^= 0x80;

        let removed_byte = sealed.segments[0].pop().unwrap();
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::MalformedCiphertext
        );
        sealed.segments[0].push(removed_byte);

        let original_final = sealed.segments.pop().unwrap();
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::MalformedCiphertext
        );
        sealed.segments.push(original_final.clone());
        sealed.segments.push(original_final);
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::MalformedCiphertext
        );
        sealed.segments.pop();

        sealed.stream_nonce_prefix[0] ^= 1;
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );
        sealed.stream_nonce_prefix[0] ^= 1;

        let final_index = sealed.segments.len() - 1;
        let final_byte_index = sealed.segments[final_index].len() - 1;
        sealed.segments[final_index][final_byte_index] ^= 1;
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );
        sealed.segments[final_index][final_byte_index] ^= 1;

        sealed.wrap_nonce[0] ^= 1;
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );
        sealed.wrap_nonce[0] ^= 1;
        sealed.wrapped_dek[KEY_BYTES + AEAD_TAG_BYTES - 1] ^= 1;
        assert_eq!(
            open_object(&key, &metadata(&plaintext), &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );
    }

    #[test]
    fn wrong_metadata_key_and_final_flag_fail_authentication() {
        let plaintext = b"metadata-bound synthetic plaintext";
        let key = KeyEncryptionKey::new([0x66; KEY_BYTES]);
        let sealed =
            seal_object_with_random(&key, &metadata(plaintext), plaintext, &mut fixed_random())
                .unwrap();
        assert_eq!(
            open_object(
                &KeyEncryptionKey::new([0x67; KEY_BYTES]),
                &metadata(plaintext),
                &sealed
            )
            .unwrap_err()
            .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );
        let wrong_metadata = ObjectMetadata::new(
            "other-namespace",
            "source-synthetic",
            Sha256::digest(plaintext).into(),
            plaintext.len() as u64,
            "text/markdown",
        )
        .unwrap();
        assert_eq!(
            open_object(&key, &wrong_metadata, &sealed)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::AuthenticationFailed
        );

        let dek = unwrap_dek(
            &key,
            &sealed.wrap_nonce,
            &sealed.wrapped_dek,
            &metadata(plaintext).wrap_aad(&sealed.stream_nonce_prefix),
        )
        .unwrap();
        let nonce = StreamNonce::from(sealed.stream_nonce_prefix);
        let mut decryptor = DecryptorBE32::<XChaCha20Poly1305>::new((&*dek).into(), &nonce);
        assert!(
            decryptor
                .decrypt_next(Payload {
                    msg: &sealed.segments[0],
                    aad: &metadata(plaintext).object_aad(&sealed.stream_nonce_prefix),
                })
                .is_err()
        );
    }

    #[test]
    fn invalid_length_digest_and_limit_fail_before_random_or_encryption() {
        let plaintext = b"synthetic";
        let wrong_length = ObjectMetadata::new(
            "namespace",
            "source",
            Sha256::digest(plaintext).into(),
            plaintext.len() as u64 + 1,
            "text/plain",
        )
        .unwrap();
        assert_eq!(
            seal_object_with_random(
                &KeyEncryptionKey::new([1; KEY_BYTES]),
                &wrong_length,
                plaintext,
                &mut FailingRandom
            )
            .unwrap_err()
            .code(),
            SourceVaultErrorCode::LengthMismatch
        );
        let wrong_digest = ObjectMetadata::new(
            "namespace",
            "source",
            [0; 32],
            plaintext.len() as u64,
            "text/plain",
        )
        .unwrap();
        assert_eq!(
            seal_object_with_random(
                &KeyEncryptionKey::new([1; KEY_BYTES]),
                &wrong_digest,
                plaintext,
                &mut FailingRandom
            )
            .unwrap_err()
            .code(),
            SourceVaultErrorCode::DigestMismatch
        );
        let too_large = ObjectMetadata::new(
            "namespace",
            "source",
            [0; 32],
            MAX_OBJECT_PLAINTEXT_BYTES as u64 + 1,
            "text/plain",
        )
        .unwrap();
        assert_eq!(
            open_object(
                &KeyEncryptionKey::new([1; KEY_BYTES]),
                &too_large,
                &SealedObject {
                    stream_nonce_prefix: [0; STREAM_NONCE_PREFIX_BYTES],
                    wrap_nonce: [0; WRAP_NONCE_BYTES],
                    wrapped_dek: [0; KEY_BYTES + AEAD_TAG_BYTES],
                    segments: vec![vec![0; AEAD_TAG_BYTES]],
                }
            )
            .unwrap_err()
            .code(),
            SourceVaultErrorCode::PlaintextTooLarge
        );
    }

    #[test]
    fn debug_output_never_contains_key_nonce_tag_or_ciphertext_bytes() {
        let plaintext = b"debug-secret-synthetic";
        let sealed = seal_object_with_random(
            &KeyEncryptionKey::new([0x99; 32]),
            &metadata(plaintext),
            plaintext,
            &mut fixed_random(),
        )
        .unwrap();
        let debug = format!("{sealed:?} {:?}", KeyEncryptionKey::new([0x99; 32]));
        assert!(!debug.contains("debug-secret"));
        assert!(!debug.contains("9999"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn cfrg_xchacha20poly1305_appendix_a1_vector_matches() {
        const KEY: [u8; 32] = [
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d,
            0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b,
            0x9c, 0x9d, 0x9e, 0x9f,
        ];
        const NONCE: [u8; 24] = [
            0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d,
            0x4e, 0x4f, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57,
        ];
        const AAD: [u8; 12] = [
            0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
        ];
        const PLAINTEXT: &[u8] = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        const EXPECTED: &[u8] = &[
            0xbd, 0x6d, 0x17, 0x9d, 0x3e, 0x83, 0xd4, 0x3b, 0x95, 0x76, 0x57, 0x94, 0x93, 0xc0,
            0xe9, 0x39, 0x57, 0x2a, 0x17, 0x00, 0x25, 0x2b, 0xfa, 0xcc, 0xbe, 0xd2, 0x90, 0x2c,
            0x21, 0x39, 0x6c, 0xbb, 0x73, 0x1c, 0x7f, 0x1b, 0x0b, 0x4a, 0xa6, 0x44, 0x0b, 0xf3,
            0xa8, 0x2f, 0x4e, 0xda, 0x7e, 0x39, 0xae, 0x64, 0xc6, 0x70, 0x8c, 0x54, 0xc2, 0x16,
            0xcb, 0x96, 0xb7, 0x2e, 0x12, 0x13, 0xb4, 0x52, 0x2f, 0x8c, 0x9b, 0xa4, 0x0d, 0xb5,
            0xd9, 0x45, 0xb1, 0x1b, 0x69, 0xb9, 0x82, 0xc1, 0xbb, 0x9e, 0x3f, 0x3f, 0xac, 0x2b,
            0xc3, 0x69, 0x48, 0x8f, 0x76, 0xb2, 0x38, 0x35, 0x65, 0xd3, 0xff, 0xf9, 0x21, 0xf9,
            0x66, 0x4c, 0x97, 0x63, 0x7d, 0xa9, 0x76, 0x88, 0x12, 0xf6, 0x15, 0xc6, 0x8b, 0x13,
            0xb5, 0x2e, 0xc0, 0x87, 0x59, 0x24, 0xc1, 0xc7, 0x98, 0x79, 0x47, 0xde, 0xaf, 0xd8,
            0x78, 0x0a, 0xcf, 0x49,
        ];
        let cipher = XChaCha20Poly1305::new(&Array::from(KEY));
        let actual = cipher
            .encrypt(
                &XNonce::from(NONCE),
                Payload {
                    msg: PLAINTEXT,
                    aad: &AAD,
                },
            )
            .unwrap();
        assert_eq!(actual, EXPECTED);
    }
}
