use std::fmt;

use crate::error::{SourceVaultError, SourceVaultErrorCode};
use crate::{
    DEK_WRAP_PROFILE, DIGEST_PROFILE, ENVELOPE_PROFILE, OBJECT_CIPHER_PROFILE, PROVIDER_PROFILE,
    SEGMENT_PLAINTEXT_BYTES, STREAM_NONCE_PREFIX_BYTES,
};

const AAD_CODEC_PREFIX: &[u8] = b"RMAAD\x01";
const OBJECT_AAD_DOMAIN: &[u8] = b"radishmemory.source-object-aad/1";
const WRAP_AAD_DOMAIN: &[u8] = b"radishmemory.source-object-dek-wrap-aad/1";

#[derive(Clone, Eq, PartialEq)]
pub struct ObjectMetadata {
    namespace_id: String,
    source_id: String,
    exact_digest: [u8; 32],
    plaintext_len: u64,
    media_type: String,
}

impl ObjectMetadata {
    pub fn new(
        namespace_id: impl Into<String>,
        source_id: impl Into<String>,
        exact_digest: [u8; 32],
        plaintext_len: u64,
        media_type: impl Into<String>,
    ) -> Result<Self, SourceVaultError> {
        let metadata = Self {
            namespace_id: namespace_id.into(),
            source_id: source_id.into(),
            exact_digest,
            plaintext_len,
            media_type: media_type.into(),
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn plaintext_len(&self) -> u64 {
        self.plaintext_len
    }

    pub fn exact_digest(&self) -> &[u8; 32] {
        &self.exact_digest
    }

    fn validate(&self) -> Result<(), SourceVaultError> {
        for value in [&self.namespace_id, &self.source_id, &self.media_type] {
            if value.is_empty() || value.len() > u32::MAX as usize {
                return Err(SourceVaultError::new(
                    SourceVaultErrorCode::InvalidMetadata,
                    "object metadata contains an empty or oversized field",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn object_aad(
        &self,
        stream_nonce_prefix: &[u8; STREAM_NONCE_PREFIX_BYTES],
    ) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(AAD_CODEC_PREFIX);
        push_field(&mut output, 1, OBJECT_AAD_DOMAIN);
        push_field(&mut output, 2, ENVELOPE_PROFILE.as_bytes());
        push_field(&mut output, 3, OBJECT_CIPHER_PROFILE.as_bytes());
        push_field(&mut output, 4, self.namespace_id.as_bytes());
        push_field(&mut output, 5, self.source_id.as_bytes());
        push_field(&mut output, 6, DIGEST_PROFILE.as_bytes());
        push_field(&mut output, 7, &self.exact_digest);
        push_field(&mut output, 8, &self.plaintext_len.to_be_bytes());
        push_field(&mut output, 9, self.media_type.as_bytes());
        push_field(
            &mut output,
            10,
            &(SEGMENT_PLAINTEXT_BYTES as u32).to_be_bytes(),
        );
        push_field(&mut output, 11, stream_nonce_prefix);
        push_field(&mut output, 12, DEK_WRAP_PROFILE.as_bytes());
        push_field(&mut output, 13, PROVIDER_PROFILE.as_bytes());
        output
    }

    pub(crate) fn wrap_aad(
        &self,
        stream_nonce_prefix: &[u8; STREAM_NONCE_PREFIX_BYTES],
    ) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(AAD_CODEC_PREFIX);
        push_field(&mut output, 1, WRAP_AAD_DOMAIN);
        push_field(&mut output, 2, ENVELOPE_PROFILE.as_bytes());
        push_field(&mut output, 3, DEK_WRAP_PROFILE.as_bytes());
        push_field(&mut output, 4, PROVIDER_PROFILE.as_bytes());
        push_field(&mut output, 5, self.namespace_id.as_bytes());
        push_field(&mut output, 6, self.source_id.as_bytes());
        push_field(&mut output, 7, DIGEST_PROFILE.as_bytes());
        push_field(&mut output, 8, &self.exact_digest);
        push_field(&mut output, 9, &self.plaintext_len.to_be_bytes());
        push_field(&mut output, 10, self.media_type.as_bytes());
        push_field(&mut output, 11, stream_nonce_prefix);
        output
    }
}

impl fmt::Debug for ObjectMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObjectMetadata")
            .field("namespace_id", &"[REDACTED]")
            .field("source_id", &"[REDACTED]")
            .field("exact_digest", &"[REDACTED]")
            .field("plaintext_len", &self.plaintext_len)
            .field("media_type", &"[REDACTED]")
            .finish()
    }
}

fn push_field(output: &mut Vec<u8>, tag: u8, value: &[u8]) {
    output.push(tag);
    output.extend_from_slice(&(value.len() as u32).to_be_bytes());
    output.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_rejects_empty_fields_without_echoing_them() {
        let error = ObjectMetadata::new("", "source", [0; 32], 0, "text/plain")
            .expect_err("empty namespace must fail");
        assert_eq!(error.code(), SourceVaultErrorCode::InvalidMetadata);
        assert!(!format!("{error:?}").contains("source"));
    }

    #[test]
    fn debug_redacts_identity_digest_and_media_type() {
        let metadata = ObjectMetadata::new(
            "synthetic-namespace",
            "synthetic-source",
            [0xa5; 32],
            7,
            "text/x-sensitive-fixture",
        )
        .unwrap();
        let debug = format!("{metadata:?}");
        assert!(!debug.contains("synthetic"));
        assert!(!debug.contains("a5"));
        assert!(debug.contains("plaintext_len: 7"));
    }

    #[test]
    fn aad_codec_matches_frozen_byte_level_vectors() {
        let metadata = ObjectMetadata::new(
            "namespace-vector",
            "source-vector",
            [0x5a; 32],
            17,
            "text/markdown",
        )
        .unwrap();
        let hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        const OBJECT_AAD_HEX: &str = concat!(
            "524d41414401",
            "01000000207261646973686d656d6f72792e736f757263652d6f626a6563742d6161642f31",
            "020000002c7261646973686d656d6f72792e7068617365312d656e637279707465642d736f757263652d7661756c742f31",
            "030000002c7261646973686d656d6f72792e786368616368613230706f6c79313330352d73747265616d2d626533322f31",
            "04000000106e616d6573706163652d766563746f72",
            "050000000d736f757263652d766563746f72",
            "060000000e65786163742d62797465732d7631",
            "07000000205a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
            "08000000080000000000000011",
            "090000000d746578742f6d61726b646f776e",
            "0a0000000400100000",
            "0b0000001333333333333333333333333333333333333333",
            "0c000000297261646973686d656d6f72792e786368616368613230706f6c79313330352d64656b2d777261702f31",
            "0d000000217261646973686d656d6f72792e706c6174666f726d2d6b65792d73746f72652f31",
        );
        const WRAP_AAD_HEX: &str = concat!(
            "524d41414401",
            "01000000297261646973686d656d6f72792e736f757263652d6f626a6563742d64656b2d777261702d6161642f31",
            "020000002c7261646973686d656d6f72792e7068617365312d656e637279707465642d736f757263652d7661756c742f31",
            "03000000297261646973686d656d6f72792e786368616368613230706f6c79313330352d64656b2d777261702f31",
            "04000000217261646973686d656d6f72792e706c6174666f726d2d6b65792d73746f72652f31",
            "05000000106e616d6573706163652d766563746f72",
            "060000000d736f757263652d766563746f72",
            "070000000e65786163742d62797465732d7631",
            "08000000205a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
            "09000000080000000000000011",
            "0a0000000d746578742f6d61726b646f776e",
            "0b0000001333333333333333333333333333333333333333",
        );
        assert_eq!(hex(&metadata.object_aad(&[0x33; 19])), OBJECT_AAD_HEX);
        assert_eq!(hex(&metadata.wrap_aad(&[0x33; 19])), WRAP_AAD_HEX);
    }

    #[test]
    fn every_caller_supplied_metadata_field_changes_both_aad_domains() {
        let nonce = [0x33; 19];
        let baseline =
            ObjectMetadata::new("namespace", "source", [0x5a; 32], 17, "text/markdown").unwrap();
        let variants = [
            ObjectMetadata::new("other-namespace", "source", [0x5a; 32], 17, "text/markdown")
                .unwrap(),
            ObjectMetadata::new("namespace", "other-source", [0x5a; 32], 17, "text/markdown")
                .unwrap(),
            ObjectMetadata::new("namespace", "source", [0x5b; 32], 17, "text/markdown").unwrap(),
            ObjectMetadata::new("namespace", "source", [0x5a; 32], 18, "text/markdown").unwrap(),
            ObjectMetadata::new("namespace", "source", [0x5a; 32], 17, "text/plain").unwrap(),
        ];
        for variant in variants {
            assert_ne!(baseline.object_aad(&nonce), variant.object_aad(&nonce));
            assert_ne!(baseline.wrap_aad(&nonce), variant.wrap_aad(&nonce));
        }
    }
}
