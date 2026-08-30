use crate::{
    ActorRef, CanonicalObject, CanonicalObjectType, CoreError, Digest, DigestProfile, EvidenceRef,
    EvidenceType, Identifier, InvalidCanonicalObjectReason, NonEmptyText, ObjectRef, ProducerRef,
    Timestamp, compute_canonical_json_digest, require_non_empty, require_profile, require_unique,
    require_unique_by,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestedGuarantee {
    StopRecall,
    LocalPurge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeletionScope {
    LocalDevice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeletionComponentType {
    SourceBody,
    SourceMetadata,
    SourceFragment,
    MemoryProposal,
    MemoryDecision,
    MemoryRecord,
    MemoryStateEvent,
    FullTextIndex,
    ContextCache,
    MinimalAudit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequiredAction {
    Delete,
    Redact,
    RetainMinimal,
}

const LOCAL_PURGE_PROFILE: [(&str, DeletionComponentType, RequiredAction); 10] = [
    (
        "source-body",
        DeletionComponentType::SourceBody,
        RequiredAction::Delete,
    ),
    (
        "source-metadata",
        DeletionComponentType::SourceMetadata,
        RequiredAction::RetainMinimal,
    ),
    (
        "source-fragment",
        DeletionComponentType::SourceFragment,
        RequiredAction::Delete,
    ),
    (
        "memory-proposal",
        DeletionComponentType::MemoryProposal,
        RequiredAction::Redact,
    ),
    (
        "memory-decision",
        DeletionComponentType::MemoryDecision,
        RequiredAction::RetainMinimal,
    ),
    (
        "memory-record",
        DeletionComponentType::MemoryRecord,
        RequiredAction::Redact,
    ),
    (
        "memory-state-event",
        DeletionComponentType::MemoryStateEvent,
        RequiredAction::RetainMinimal,
    ),
    (
        "full-text-index",
        DeletionComponentType::FullTextIndex,
        RequiredAction::Delete,
    ),
    (
        "context-cache",
        DeletionComponentType::ContextCache,
        RequiredAction::Delete,
    ),
    (
        "minimal-audit",
        DeletionComponentType::MinimalAudit,
        RequiredAction::RetainMinimal,
    ),
];

/// Builds the frozen ten-component M0 local-purge plan for one exact semantic closure.
pub fn build_local_purge_targets(
    target_refs: &[ObjectRef],
) -> Result<Vec<DeletionTarget>, CoreError> {
    require_non_empty(target_refs)?;
    require_unique(target_refs)?;
    if target_refs.iter().any(|target| {
        !matches!(
            target.object_type(),
            CanonicalObjectType::SourceArtifact | CanonicalObjectType::MemoryRecord
        )
    }) {
        return Err(CoreError::invalid_canonical_object(
            InvalidCanonicalObjectReason::InvalidFieldCombination,
        ));
    }

    let mut sorted = target_refs.to_vec();
    sorted.sort();
    let target_ref = if sorted.len() == 1 {
        DeletionTargetRef::Object(sorted[0].clone())
    } else {
        DeletionTargetRef::FrozenClosure(FrozenTargetClosure::freeze(sorted)?)
    };
    let target_count = u64::try_from(target_refs.len()).map_err(|_| {
        CoreError::invalid_canonical_object(InvalidCanonicalObjectReason::CountMismatch)
    })?;

    LOCAL_PURGE_PROFILE
        .iter()
        .map(|(key, component_type, action)| {
            DeletionTarget::new(
                Identifier::new(*key)?,
                *component_type,
                target_ref.clone(),
                target_count,
                *action,
            )
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenTargetClosure {
    target_refs: Vec<ObjectRef>,
    target_refs_digest: Digest,
}

impl FrozenTargetClosure {
    pub fn new(target_refs: Vec<ObjectRef>, target_refs_digest: Digest) -> Result<Self, CoreError> {
        require_non_empty(&target_refs)?;
        require_unique(&target_refs)?;
        if target_refs.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::UnsortedTargetClosure,
            ));
        }
        require_profile(&target_refs_digest, DigestProfile::CanonicalJsonV1)?;
        if target_refs_digest != compute_target_refs_digest(&target_refs)? {
            return Err(CoreError::digest_mismatch());
        }
        Ok(Self {
            target_refs,
            target_refs_digest,
        })
    }

    /// Sorts a nonempty target set and freezes its canonical closure digest.
    pub fn freeze(mut target_refs: Vec<ObjectRef>) -> Result<Self, CoreError> {
        target_refs.sort();
        require_non_empty(&target_refs)?;
        require_unique(&target_refs)?;
        let target_refs_digest = compute_target_refs_digest(&target_refs)?;
        Self::new(target_refs, target_refs_digest)
    }

    #[must_use]
    pub fn target_refs(&self) -> &[ObjectRef] {
        &self.target_refs
    }

    #[must_use]
    pub const fn target_refs_digest(&self) -> &Digest {
        &self.target_refs_digest
    }
}

/// Computes the canonical JSON digest that binds one frozen deletion closure.
pub fn compute_target_refs_digest(target_refs: &[ObjectRef]) -> Result<Digest, CoreError> {
    let value = serde_json::Value::Array(
        target_refs
            .iter()
            .map(|target_ref| {
                serde_json::json!({
                    "object_id": target_ref.object_id().as_str(),
                    "object_type": target_ref.object_type().as_str(),
                })
            })
            .collect(),
    );
    compute_canonical_json_digest(&value.to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeletionTargetRef {
    Object(ObjectRef),
    FrozenClosure(FrozenTargetClosure),
}

impl DeletionTargetRef {
    fn target_len(&self) -> usize {
        match self {
            Self::Object(_) => 1,
            Self::FrozenClosure(closure) => closure.target_refs.len(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionTarget {
    component_key: Identifier,
    component_type: DeletionComponentType,
    target_ref: DeletionTargetRef,
    target_count: u64,
    required_action: RequiredAction,
}

impl DeletionTarget {
    pub fn new(
        component_key: Identifier,
        component_type: DeletionComponentType,
        target_ref: DeletionTargetRef,
        target_count: u64,
        required_action: RequiredAction,
    ) -> Result<Self, CoreError> {
        if target_count == 0 || usize::try_from(target_count) != Ok(target_ref.target_len()) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::CountMismatch,
            ));
        }
        Ok(Self {
            component_key,
            component_type,
            target_ref,
            target_count,
            required_action,
        })
    }

    #[must_use]
    pub const fn component_key(&self) -> &Identifier {
        &self.component_key
    }

    #[must_use]
    pub const fn component_type(&self) -> DeletionComponentType {
        self.component_type
    }

    #[must_use]
    pub const fn target_ref(&self) -> &DeletionTargetRef {
        &self.target_ref
    }

    #[must_use]
    pub const fn target_count(&self) -> u64 {
        self.target_count
    }

    #[must_use]
    pub const fn required_action(&self) -> RequiredAction {
        self.required_action
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteRequestParams {
    pub delete_request_id: Identifier,
    pub namespace_id: Identifier,
    pub requested_by: ActorRef,
    pub authorization_basis: NonEmptyText,
    pub requested_guarantee: RequestedGuarantee,
    pub device_id: Identifier,
    pub target_refs: Vec<ObjectRef>,
    pub planned_components: Vec<DeletionTarget>,
    pub reason_code: NonEmptyText,
    pub requested_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteRequest(DeleteRequestParams);

impl DeleteRequest {
    pub fn new(params: DeleteRequestParams) -> Result<Self, CoreError> {
        require_non_empty(&params.target_refs)?;
        require_unique(&params.target_refs)?;
        require_non_empty(&params.planned_components)?;
        require_unique_by(&params.planned_components, |target| {
            target.component_key().clone()
        })?;
        Ok(Self(params))
    }

    #[must_use]
    pub const fn scope(&self) -> DeletionScope {
        DeletionScope::LocalDevice
    }

    #[must_use]
    pub const fn params(&self) -> &DeleteRequestParams {
        &self.0
    }
}

impl CanonicalObject for DeleteRequest {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::DeleteRequest
    }

    fn object_id(&self) -> &Identifier {
        &self.0.delete_request_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeletionOverallStatus {
    Pending,
    Partial,
    Failed,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentOutcome {
    Deleted,
    Redacted,
    RetainedMinimal,
    NotFound,
    NotApplicable,
}

/// Deterministic inputs required by one local deletion execution attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalDeletionExecution {
    checked_at: Timestamp,
    retention_basis: EvidenceRef,
}

impl LocalDeletionExecution {
    pub fn new(checked_at: Timestamp, retention_basis: EvidenceRef) -> Result<Self, CoreError> {
        if retention_basis.evidence_type() != EvidenceType::PolicyBasis {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self {
            checked_at,
            retention_basis,
        })
    }

    #[must_use]
    pub const fn checked_at(&self) -> &Timestamp {
        &self.checked_at
    }

    #[must_use]
    pub const fn retention_basis(&self) -> &EvidenceRef {
        &self.retention_basis
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentResultParams {
    pub component_key: Identifier,
    pub component_type: DeletionComponentType,
    pub target_ref: DeletionTargetRef,
    pub required_action: RequiredAction,
    pub target_count: u64,
    pub processed_count: u64,
    pub status: ComponentStatus,
    pub outcome: ComponentOutcome,
    pub verification_method: NonEmptyText,
    pub checked_at: Timestamp,
    pub error_code: Option<NonEmptyText>,
    pub retryable: Option<bool>,
    pub retention_basis: Option<EvidenceRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentResult(ComponentResultParams);

impl ComponentResult {
    pub fn new(params: ComponentResultParams) -> Result<Self, CoreError> {
        if params.target_count == 0
            || usize::try_from(params.target_count) != Ok(params.target_ref.target_len())
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::CountMismatch,
            ));
        }
        if params.processed_count > params.target_count {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::CountMismatch,
            ));
        }
        let failure_fields_match = if params.status == ComponentStatus::Failed {
            params.error_code.is_some() && params.retryable.is_some()
        } else {
            params.error_code.is_none() && params.retryable.is_none()
        };
        if !failure_fields_match {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        if params.status == ComponentStatus::Succeeded
            && params.processed_count != params.target_count
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::CountMismatch,
            ));
        }

        let outcome_matches_action = params.status != ComponentStatus::Succeeded
            || params.outcome == ComponentOutcome::NotFound
            || matches!(
                (params.required_action, params.outcome),
                (RequiredAction::Delete, ComponentOutcome::Deleted)
                    | (RequiredAction::Redact, ComponentOutcome::Redacted)
                    | (
                        RequiredAction::RetainMinimal,
                        ComponentOutcome::RetainedMinimal
                    )
            );
        if !outcome_matches_action {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }

        let retention_basis_matches = if params.status == ComponentStatus::Succeeded
            && params.outcome == ComponentOutcome::RetainedMinimal
        {
            params
                .retention_basis
                .as_ref()
                .is_some_and(|basis| basis.evidence_type() == EvidenceType::PolicyBasis)
        } else {
            params.retention_basis.is_none()
        };
        if !retention_basis_matches {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn params(&self) -> &ComponentResultParams {
        &self.0
    }
}

/// Computes the deterministic M0 evidence digest payload used by the local execution path.
pub fn compute_deletion_evidence_digest(
    deletion_evidence_id: &Identifier,
    delete_request_id: &Identifier,
    overall_status: DeletionOverallStatus,
    component_results: &[ComponentResult],
) -> Result<Digest, CoreError> {
    let value = serde_json::json!({
        "component_results": component_results
            .iter()
            .map(|result| serde_json::json!({
                "component_key": result.params().component_key.as_str(),
                "processed_count": result.params().processed_count,
                "status": component_status_str(result.params().status),
            }))
            .collect::<Vec<_>>(),
        "deletion_evidence_id": deletion_evidence_id.as_str(),
        "delete_request_id": delete_request_id.as_str(),
        "overall_status": deletion_overall_status_str(overall_status),
    });
    crate::compute_digest(
        DigestProfile::DeletionEvidenceV1.as_str(),
        &value.to_string(),
    )
}

const fn component_status_str(status: ComponentStatus) -> &'static str {
    match status {
        ComponentStatus::Pending => "pending",
        ComponentStatus::Succeeded => "succeeded",
        ComponentStatus::Failed => "failed",
    }
}

const fn deletion_overall_status_str(status: DeletionOverallStatus) -> &'static str {
    match status {
        DeletionOverallStatus::Pending => "pending",
        DeletionOverallStatus::Partial => "partial",
        DeletionOverallStatus::Failed => "failed",
        DeletionOverallStatus::Completed => "completed",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionEvidenceParams {
    pub deletion_evidence_id: Identifier,
    pub delete_request_id: Identifier,
    pub previous_evidence_id: Option<Identifier>,
    pub namespace_id: Identifier,
    pub device_id: Identifier,
    pub overall_status: DeletionOverallStatus,
    pub component_results: Vec<ComponentResult>,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub verified_by: ProducerRef,
    pub evidence_digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionEvidence(DeletionEvidenceParams);

impl DeletionEvidence {
    pub fn new(params: DeletionEvidenceParams) -> Result<Self, CoreError> {
        require_non_empty(&params.component_results)?;
        require_unique_by(&params.component_results, |result| {
            result.params().component_key.clone()
        })?;
        let finished_matches = (params.overall_status == DeletionOverallStatus::Pending)
            == params.finished_at.is_none();
        if !finished_matches {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        if params
            .finished_at
            .as_ref()
            .is_some_and(|finished| finished < &params.started_at)
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::TimeOrder,
            ));
        }
        if params.overall_status == DeletionOverallStatus::Completed
            && params
                .component_results
                .iter()
                .any(|result| result.params().status != ComponentStatus::Succeeded)
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        if params.overall_status == DeletionOverallStatus::Failed
            && params
                .component_results
                .iter()
                .all(|result| result.params().status != ComponentStatus::Failed)
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        require_profile(&params.evidence_digest, DigestProfile::DeletionEvidenceV1)?;
        Ok(Self(params))
    }

    #[must_use]
    pub const fn scope(&self) -> DeletionScope {
        DeletionScope::LocalDevice
    }

    #[must_use]
    pub const fn params(&self) -> &DeletionEvidenceParams {
        &self.0
    }
}

impl CanonicalObject for DeletionEvidence {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::DeletionEvidence
    }

    fn object_id(&self) -> &Identifier {
        &self.0.deletion_evidence_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}
