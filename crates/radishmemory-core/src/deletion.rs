use crate::{
    ActorRef, CanonicalObject, CanonicalObjectType, CoreError, Digest, DigestProfile, EvidenceRef,
    EvidenceType, Identifier, InvalidCanonicalObjectReason, NonEmptyText, ObjectRef, ProducerRef,
    Timestamp, require_non_empty, require_profile, require_unique, require_unique_by,
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
        Ok(Self {
            target_refs,
            target_refs_digest,
        })
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
