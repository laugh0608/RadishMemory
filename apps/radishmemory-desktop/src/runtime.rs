use std::error::Error;
use std::fmt;

use radishmemory_application::{
    ApplicationIdentifierKind, ApplicationRuntime, Identifier, Timestamp,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Clone, Copy, Debug, Default)]
pub struct ProductionRuntime;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionRuntimeError {
    RandomSourceUnavailable,
    ClockUnavailable,
}

impl fmt::Display for ProductionRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RandomSourceUnavailable => "operating system random source is unavailable",
            Self::ClockUnavailable => "UTC system clock is unavailable",
        })
    }
}

impl Error for ProductionRuntimeError {}

impl ApplicationRuntime for ProductionRuntime {
    type Error = ProductionRuntimeError;

    fn next_identifier(
        &mut self,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, Self::Error> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| ProductionRuntimeError::RandomSourceUnavailable)?;
        let prefix = match kind {
            ApplicationIdentifierKind::Namespace => "namespace-",
            ApplicationIdentifierKind::Device => "device-",
            ApplicationIdentifierKind::OriginBinding => "origin-binding-",
            ApplicationIdentifierKind::Source => "source-",
            ApplicationIdentifierKind::Lineage => "lineage-",
            ApplicationIdentifierKind::Fragment => "fragment-",
            ApplicationIdentifierKind::DeleteRequest => "delete-request-",
            ApplicationIdentifierKind::DeletionEvidence => "deletion-evidence-",
        };
        Identifier::new(format!("{prefix}{}", lowercase_hex(bytes)))
            .map_err(|_| ProductionRuntimeError::RandomSourceUnavailable)
    }

    fn now(&mut self) -> Result<Timestamp, Self::Error> {
        let value = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|_| ProductionRuntimeError::ClockUnavailable)?;
        Timestamp::parse(&value).map_err(|_| ProductionRuntimeError::ClockUnavailable)
    }
}

fn lowercase_hex(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(32);
    for byte in bytes {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_identifiers_are_kind_prefixed_lowercase_hex() {
        let mut runtime = ProductionRuntime;
        for (kind, prefix) in [
            (ApplicationIdentifierKind::Namespace, "namespace-"),
            (ApplicationIdentifierKind::Device, "device-"),
            (ApplicationIdentifierKind::OriginBinding, "origin-binding-"),
            (ApplicationIdentifierKind::Source, "source-"),
            (ApplicationIdentifierKind::Lineage, "lineage-"),
            (ApplicationIdentifierKind::Fragment, "fragment-"),
            (ApplicationIdentifierKind::DeleteRequest, "delete-request-"),
            (
                ApplicationIdentifierKind::DeletionEvidence,
                "deletion-evidence-",
            ),
        ] {
            let value = runtime.next_identifier(kind).unwrap();
            let suffix = value.as_str().strip_prefix(prefix).unwrap();
            assert_eq!(suffix.len(), 32);
            assert!(suffix.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(suffix, suffix.to_ascii_lowercase());
        }
    }

    #[test]
    fn production_clock_returns_core_valid_utc_time() {
        let mut runtime = ProductionRuntime;
        let timestamp = runtime.now().unwrap();
        assert_eq!(timestamp.offset_seconds(), 0);
    }
}
