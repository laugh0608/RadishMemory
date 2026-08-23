use std::error::Error;
use std::fmt;

/// Stable top-level categories returned by the M0 canonical core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreErrorCode {
    /// A digest profile is not implemented by this schema version.
    UnsupportedProfile,
    /// An RFC 3339 timestamp or `ValidTime` value is invalid.
    InvalidTime,
    /// JSON cannot be mapped to the frozen canonical representation.
    NonCanonicalJson,
    /// A computed digest does not match the expected value.
    DigestMismatch,
    /// A canonical value or object violates the frozen M0 field contract.
    InvalidCanonicalObject,
}

/// Stable detail for rejected M0 canonical values and objects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCanonicalObjectReason {
    EmptyIdentifier,
    EmptyText,
    ZeroVersion,
    InvalidUnitInterval,
    EmptyRequiredCollection,
    DuplicateCollectionMember,
    DuplicateOrdinal,
    InvalidFieldCombination,
    InvalidByteRange,
    ContentLengthMismatch,
    InvalidDigestValue,
    DigestProfileMismatch,
    NonLocalGovernance,
    BudgetExceeded,
    CountMismatch,
    UnsortedTargetClosure,
    InvalidStateTransition,
    TimeOrder,
}

/// Stable detail for an invalid time without copying the rejected input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidTimeReason {
    /// RFC 3339 parsing failed.
    Parse,
    /// The boundaries do not match the selected `ValidTime` mode.
    BoundaryCombination,
    /// An interval is empty or runs backwards.
    IntervalOrder,
}

/// Stable detail for rejected canonical JSON without copying JSON content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NonCanonicalJsonReason {
    /// JSON syntax is invalid or trailing input remains.
    Syntax,
    /// An object contains the same decoded key more than once.
    DuplicateKey,
    /// M0 input contains a JSON `null` value.
    NullForbidden,
    /// JSON nesting exceeds the bounded parser depth.
    NestingLimit,
    /// A number cannot be expanded safely to ordinary decimal notation.
    NumberExpansionLimit,
}

/// Core failure with a stable category and an optional lower-level parser cause.
///
/// The rejected timestamp, JSON text, and source body are deliberately not
/// retained, so ordinary error reporting cannot reproduce user content.
#[derive(Debug)]
pub struct CoreError {
    code: CoreErrorCode,
    invalid_time_reason: Option<InvalidTimeReason>,
    canonical_json_reason: Option<NonCanonicalJsonReason>,
    invalid_canonical_object_reason: Option<InvalidCanonicalObjectReason>,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl CoreError {
    pub(crate) fn unsupported_profile() -> Self {
        Self {
            code: CoreErrorCode::UnsupportedProfile,
            invalid_time_reason: None,
            canonical_json_reason: None,
            invalid_canonical_object_reason: None,
            source: None,
        }
    }

    pub(crate) fn invalid_time(
        reason: InvalidTimeReason,
        source: Option<time::error::Parse>,
    ) -> Self {
        Self {
            code: CoreErrorCode::InvalidTime,
            invalid_time_reason: Some(reason),
            canonical_json_reason: None,
            invalid_canonical_object_reason: None,
            source: source.map(|error| Box::new(error) as Box<_>),
        }
    }

    pub(crate) fn non_canonical_json(
        reason: NonCanonicalJsonReason,
        source: Option<serde_json::Error>,
    ) -> Self {
        Self {
            code: CoreErrorCode::NonCanonicalJson,
            invalid_time_reason: None,
            canonical_json_reason: Some(reason),
            invalid_canonical_object_reason: None,
            source: source.map(|error| Box::new(error) as Box<_>),
        }
    }

    pub(crate) fn digest_mismatch() -> Self {
        Self {
            code: CoreErrorCode::DigestMismatch,
            invalid_time_reason: None,
            canonical_json_reason: None,
            invalid_canonical_object_reason: None,
            source: None,
        }
    }

    pub(crate) fn invalid_canonical_object(reason: InvalidCanonicalObjectReason) -> Self {
        Self {
            code: CoreErrorCode::InvalidCanonicalObject,
            invalid_time_reason: None,
            canonical_json_reason: None,
            invalid_canonical_object_reason: Some(reason),
            source: None,
        }
    }

    /// Returns the stable category intended for programmatic matching.
    #[must_use]
    pub const fn code(&self) -> CoreErrorCode {
        self.code
    }

    /// Returns the stable time detail when the category is `InvalidTime`.
    #[must_use]
    pub const fn invalid_time_reason(&self) -> Option<InvalidTimeReason> {
        self.invalid_time_reason
    }

    /// Returns the stable JSON detail when the category is `NonCanonicalJson`.
    #[must_use]
    pub const fn canonical_json_reason(&self) -> Option<NonCanonicalJsonReason> {
        self.canonical_json_reason
    }

    /// Returns the stable canonical-object detail without retaining rejected content.
    #[must_use]
    pub const fn invalid_canonical_object_reason(&self) -> Option<InvalidCanonicalObjectReason> {
        self.invalid_canonical_object_reason
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            CoreErrorCode::UnsupportedProfile => "unsupported digest profile",
            CoreErrorCode::InvalidTime => "invalid time",
            CoreErrorCode::NonCanonicalJson => "non-canonical JSON input",
            CoreErrorCode::DigestMismatch => "digest mismatch",
            CoreErrorCode::InvalidCanonicalObject => "invalid canonical object",
        };
        formatter.write_str(message)
    }
}

impl Error for CoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
