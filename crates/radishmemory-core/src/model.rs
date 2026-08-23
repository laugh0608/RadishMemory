use std::collections::BTreeSet;
use std::fmt;
use std::hash::Hash;

use crate::{
    CoreError, Digest, DigestProfile, InvalidCanonicalObjectReason, Timestamp,
    compute_nfc_text_digest,
};

pub const M0_SCHEMA_VERSION: &str = "radishmemory.m0/1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalObjectType {
    SourceArtifact,
    SourceFragment,
    MemoryProposal,
    MemoryDecision,
    MemoryRecord,
    MemoryStateEvent,
    ContextPack,
    DeleteRequest,
    DeletionEvidence,
}

impl CanonicalObjectType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceArtifact => "SourceArtifact",
            Self::SourceFragment => "SourceFragment",
            Self::MemoryProposal => "MemoryProposal",
            Self::MemoryDecision => "MemoryDecision",
            Self::MemoryRecord => "MemoryRecord",
            Self::MemoryStateEvent => "MemoryStateEvent",
            Self::ContextPack => "ContextPack",
            Self::DeleteRequest => "DeleteRequest",
            Self::DeletionEvidence => "DeletionEvidence",
        }
    }
}

pub trait CanonicalObject {
    fn object_type(&self) -> CanonicalObjectType;
    fn object_id(&self) -> &Identifier;
    fn namespace_id(&self) -> &Identifier;

    fn schema_version(&self) -> &'static str {
        M0_SCHEMA_VERSION
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Identifier(Box<str>);

impl Identifier {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, CoreError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::EmptyIdentifier,
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("Identifier").field(&self.0).finish()
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NonEmptyText(Box<str>);

impl NonEmptyText {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, CoreError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::EmptyText,
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn utf8_len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for NonEmptyText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NonEmptyText")
            .field("utf8_bytes", &self.utf8_len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Version(u64);

impl Version {
    pub fn new(value: u64) -> Result<Self, CoreError> {
        if value == 0 {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::ZeroVersion,
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitInterval(u64);

impl UnitInterval {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidUnitInterval,
            ));
        }
        let normalized = if value == 0.0 { 0.0 } else { value };
        Ok(Self(normalized.to_bits()))
    }

    #[must_use]
    pub fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EvidenceType {
    SourceFragment,
    MemoryProposal,
    MemoryDecision,
    MemoryRecord,
    DeleteRequest,
    PolicyBasis,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectRef {
    object_type: CanonicalObjectType,
    object_id: Identifier,
}

impl ObjectRef {
    #[must_use]
    pub const fn new(object_type: CanonicalObjectType, object_id: Identifier) -> Self {
        Self {
            object_type,
            object_id,
        }
    }

    #[must_use]
    pub const fn object_type(&self) -> CanonicalObjectType {
        self.object_type
    }

    #[must_use]
    pub const fn object_id(&self) -> &Identifier {
        &self.object_id
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EvidenceRef {
    evidence_type: EvidenceType,
    evidence_id: Identifier,
}

impl EvidenceRef {
    #[must_use]
    pub const fn new(evidence_type: EvidenceType, evidence_id: Identifier) -> Self {
        Self {
            evidence_type,
            evidence_id,
        }
    }

    #[must_use]
    pub const fn evidence_type(&self) -> EvidenceType {
        self.evidence_type
    }

    #[must_use]
    pub const fn evidence_id(&self) -> &Identifier {
        &self.evidence_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorType {
    User,
    Device,
    Rule,
    Parser,
    TestFixture,
    System,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActorRef {
    actor_type: ActorType,
    actor_id: Identifier,
    actor_version: Option<NonEmptyText>,
}

impl ActorRef {
    #[must_use]
    pub const fn new(
        actor_type: ActorType,
        actor_id: Identifier,
        actor_version: Option<NonEmptyText>,
    ) -> Self {
        Self {
            actor_type,
            actor_id,
            actor_version,
        }
    }

    #[must_use]
    pub const fn actor_type(&self) -> ActorType {
        self.actor_type
    }

    #[must_use]
    pub const fn actor_id(&self) -> &Identifier {
        &self.actor_id
    }

    #[must_use]
    pub fn actor_version(&self) -> Option<&NonEmptyText> {
        self.actor_version.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducerType {
    Rule,
    Parser,
    TestFixture,
    System,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerRef {
    producer_type: ProducerType,
    producer_id: Identifier,
    producer_version: NonEmptyText,
}

impl ProducerRef {
    #[must_use]
    pub const fn new(
        producer_type: ProducerType,
        producer_id: Identifier,
        producer_version: NonEmptyText,
    ) -> Self {
        Self {
            producer_type,
            producer_id,
            producer_version,
        }
    }

    #[must_use]
    pub const fn producer_type(&self) -> ProducerType {
        self.producer_type
    }

    #[must_use]
    pub const fn producer_id(&self) -> &Identifier {
        &self.producer_id
    }

    #[must_use]
    pub const fn producer_version(&self) -> &NonEmptyText {
        &self.producer_version
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sensitivity {
    Personal,
    Sensitive,
    Restricted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgressPolicy {
    LocalOnly,
    TrustedDeviceOnly,
    TrustedServerOnly,
    CloudAllowed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeletionState {
    Active,
    Pending,
    Failed,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionMode {
    UntilDeleted,
    UntilTime,
    Policy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionRule {
    mode: RetentionMode,
    expires_at: Option<Timestamp>,
    policy_id: Option<Identifier>,
}

impl RetentionRule {
    pub fn new(
        mode: RetentionMode,
        expires_at: Option<Timestamp>,
        policy_id: Option<Identifier>,
    ) -> Result<Self, CoreError> {
        let valid = match mode {
            RetentionMode::UntilDeleted => expires_at.is_none() && policy_id.is_none(),
            RetentionMode::UntilTime => expires_at.is_some() && policy_id.is_none(),
            RetentionMode::Policy => expires_at.is_none() && policy_id.is_some(),
        };
        if !valid {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self {
            mode,
            expires_at,
            policy_id,
        })
    }

    #[must_use]
    pub const fn mode(&self) -> RetentionMode {
        self.mode
    }

    #[must_use]
    pub fn expires_at(&self) -> Option<&Timestamp> {
        self.expires_at.as_ref()
    }

    #[must_use]
    pub fn policy_id(&self) -> Option<&Identifier> {
        self.policy_id.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Governance {
    sensitivity: Sensitivity,
    egress_policy: EgressPolicy,
    retention: RetentionRule,
    deletion_state: DeletionState,
    policy_basis: Identifier,
}

impl Governance {
    pub fn new(
        sensitivity: Sensitivity,
        egress_policy: EgressPolicy,
        retention: RetentionRule,
        deletion_state: DeletionState,
        policy_basis: Identifier,
    ) -> Result<Self, CoreError> {
        if egress_policy != EgressPolicy::LocalOnly {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::NonLocalGovernance,
            ));
        }
        Ok(Self {
            sensitivity,
            egress_policy,
            retention,
            deletion_state,
            policy_basis,
        })
    }

    #[must_use]
    pub const fn sensitivity(&self) -> Sensitivity {
        self.sensitivity
    }

    #[must_use]
    pub const fn egress_policy(&self) -> EgressPolicy {
        self.egress_policy
    }

    #[must_use]
    pub const fn retention(&self) -> &RetentionRule {
        &self.retention
    }

    #[must_use]
    pub const fn deletion_state(&self) -> DeletionState {
        self.deletion_state
    }

    #[must_use]
    pub const fn policy_basis(&self) -> &Identifier {
        &self.policy_basis
    }
}

/// Canonical objects whose content can participate in ordinary local recall.
pub trait GovernedCanonicalObject: CanonicalObject {
    fn governance(&self) -> &Governance;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryValueKind {
    Text,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryValue {
    text: NonEmptyText,
    content_digest: Digest,
}

impl MemoryValue {
    pub fn new(text: NonEmptyText, content_digest: Digest) -> Result<Self, CoreError> {
        require_profile(&content_digest, DigestProfile::Utf8NfcTextV1)?;
        if compute_nfc_text_digest(text.as_str()) != content_digest {
            return Err(CoreError::digest_mismatch());
        }
        Ok(Self {
            text,
            content_digest,
        })
    }

    #[must_use]
    pub fn from_text(text: NonEmptyText) -> Self {
        let content_digest = compute_nfc_text_digest(text.as_str());
        Self {
            text,
            content_digest,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> MemoryValueKind {
        MemoryValueKind::Text
    }

    #[must_use]
    pub const fn text(&self) -> &NonEmptyText {
        &self.text
    }

    #[must_use]
    pub const fn content_digest(&self) -> &Digest {
        &self.content_digest
    }
}

pub(crate) fn require_profile(digest: &Digest, profile: DigestProfile) -> Result<(), CoreError> {
    if digest.profile() != profile {
        return Err(CoreError::invalid_canonical_object(
            InvalidCanonicalObjectReason::DigestProfileMismatch,
        ));
    }
    Ok(())
}

pub(crate) fn require_non_empty<T>(values: &[T]) -> Result<(), CoreError> {
    if values.is_empty() {
        return Err(CoreError::invalid_canonical_object(
            InvalidCanonicalObjectReason::EmptyRequiredCollection,
        ));
    }
    Ok(())
}

pub(crate) fn require_unique<T>(values: &[T]) -> Result<(), CoreError>
where
    T: Ord + Clone,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::DuplicateCollectionMember,
            ));
        }
    }
    Ok(())
}

pub(crate) fn require_unique_by<T, K>(values: &[T], key: impl Fn(&T) -> K) -> Result<(), CoreError>
where
    K: Ord,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(key(value)) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::DuplicateCollectionMember,
            ));
        }
    }
    Ok(())
}
