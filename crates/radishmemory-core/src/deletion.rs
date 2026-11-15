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

impl DeletionComponentType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceBody => "source_body",
            Self::SourceMetadata => "source_metadata",
            Self::SourceFragment => "source_fragment",
            Self::MemoryProposal => "memory_proposal",
            Self::MemoryDecision => "memory_decision",
            Self::MemoryRecord => "memory_record",
            Self::MemoryStateEvent => "memory_state_event",
            Self::FullTextIndex => "full_text_index",
            Self::ContextCache => "context_cache",
            Self::MinimalAudit => "minimal_audit",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequiredAction {
    Delete,
    Redact,
    RetainMinimal,
}

impl RequiredAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Redact => "redact",
            Self::RetainMinimal => "retain_minimal",
        }
    }
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
    let value = serde_json::Value::Array(target_refs.iter().map(object_ref_json).collect());
    compute_canonical_json_digest(&value.to_string())
}

fn object_ref_json(target: &ObjectRef) -> serde_json::Value {
    serde_json::json!({
        "object_id": target.object_id().as_str(),
        "object_type": target.object_type().as_str(),
    })
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

impl DeletionOverallStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentStatus {
    Pending,
    Succeeded,
    Failed,
}

impl ComponentStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentOutcome {
    Deleted,
    Redacted,
    RetainedMinimal,
    NotFound,
    NotApplicable,
}

impl ComponentOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deleted => "deleted",
            Self::Redacted => "redacted",
            Self::RetainedMinimal => "retained_minimal",
            Self::NotFound => "not_found",
            Self::NotApplicable => "not_applicable",
        }
    }
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

/// Recomputes the legacy local receipt projection in persisted plan order.
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
                "status": result.params().status.as_str(),
            }))
            .collect::<Vec<_>>(),
        "deletion_evidence_id": deletion_evidence_id.as_str(),
        "delete_request_id": delete_request_id.as_str(),
        "overall_status": overall_status.as_str(),
    });
    crate::compute_digest(
        DigestProfile::DeletionEvidenceV1.as_str(),
        &value.to_string(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionEvidenceParams<D = Digest> {
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
    pub evidence_digest: D,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionEvidence(DeletionEvidenceParams);

impl DeletionEvidence {
    /// Constructs a new snapshot with a digest covering its canonical fields.
    pub fn seal(params: DeletionEvidenceParams<()>) -> Result<Self, CoreError> {
        let evidence_digest = compute_full_evidence_digest(&params)?;
        let DeletionEvidenceParams {
            deletion_evidence_id,
            delete_request_id,
            previous_evidence_id,
            namespace_id,
            device_id,
            overall_status,
            component_results,
            started_at,
            finished_at,
            verified_by,
            evidence_digest: (),
        } = params;
        Self::new(DeletionEvidenceParams {
            deletion_evidence_id,
            delete_request_id,
            previous_evidence_id,
            namespace_id,
            device_id,
            overall_status,
            component_results,
            started_at,
            finished_at,
            verified_by,
            evidence_digest,
        })
    }

    /// Recomputes the stored profile. Legacy component results must be supplied
    /// in the original plan order. The profile identifies the verified coverage.
    pub fn verify_digest(&self) -> Result<(), CoreError> {
        let params = &self.0;
        let expected = match params.evidence_digest.profile() {
            DigestProfile::DeletionEvidenceV1 => compute_deletion_evidence_digest(
                &params.deletion_evidence_id,
                &params.delete_request_id,
                params.overall_status,
                &params.component_results,
            )?,
            DigestProfile::DeletionEvidenceV2 => compute_full_evidence_digest(params)?,
            _ => return Err(CoreError::unsupported_profile()),
        };
        if expected != params.evidence_digest {
            return Err(CoreError::digest_mismatch());
        }
        Ok(())
    }

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
        if !matches!(
            params.evidence_digest.profile(),
            DigestProfile::DeletionEvidenceV1 | DigestProfile::DeletionEvidenceV2
        ) {
            return Err(CoreError::unsupported_profile());
        }
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

fn compute_full_evidence_digest<D>(
    params: &DeletionEvidenceParams<D>,
) -> Result<Digest, CoreError> {
    let mut components = params.component_results.iter().collect::<Vec<_>>();
    components.sort_unstable_by(|left, right| {
        left.params()
            .component_key
            .cmp(&right.params().component_key)
    });
    let mut value = serde_json::json!({
        "schema_version": crate::M0_SCHEMA_VERSION,
        "object_type": CanonicalObjectType::DeletionEvidence.as_str(),
        "deletion_evidence_id": params.deletion_evidence_id.as_str(),
        "delete_request_id": params.delete_request_id.as_str(),
        "namespace_id": params.namespace_id.as_str(),
        "scope": "local_device",
        "device_id": params.device_id.as_str(),
        "overall_status": params.overall_status.as_str(),
        "component_results": components.into_iter().map(component_result_json).collect::<Vec<_>>(),
        "started_at": params.started_at.original(),
        "verified_by": {
            "producer_type": params.verified_by.producer_type().as_str(),
            "producer_id": params.verified_by.producer_id().as_str(),
            "producer_version": params.verified_by.producer_version().as_str(),
        },
    });
    if let Some(previous) = &params.previous_evidence_id {
        value["previous_evidence_id"] = previous.as_str().into();
    }
    if let Some(finished) = &params.finished_at {
        value["finished_at"] = finished.original().into();
    }
    crate::compute_digest(
        DigestProfile::DeletionEvidenceV2.as_str(),
        &value.to_string(),
    )
}

fn component_result_json(result: &ComponentResult) -> serde_json::Value {
    let params = result.params();
    let target = match &params.target_ref {
        DeletionTargetRef::Object(target) => object_ref_json(target),
        DeletionTargetRef::FrozenClosure(closure) => {
            let digest = closure.target_refs_digest();
            serde_json::json!({
                "target_refs": closure.target_refs().iter().map(object_ref_json).collect::<Vec<_>>(),
                "target_refs_digest": {
                    "algorithm": digest.algorithm(),
                    "profile": digest.profile().as_str(),
                    "value": digest.value(),
                },
            })
        }
    };
    let mut value = serde_json::json!({
        "component_key": params.component_key.as_str(),
        "component_type": params.component_type.as_str(),
        "target_ref": target,
        "required_action": params.required_action.as_str(),
        "target_count": params.target_count,
        "processed_count": params.processed_count,
        "status": params.status.as_str(),
        "outcome": params.outcome.as_str(),
        "verification_method": params.verification_method.as_str(),
        "checked_at": params.checked_at.original(),
    });
    if let Some(code) = &params.error_code {
        value["error_code"] = code.as_str().into();
    }
    if let Some(retryable) = params.retryable {
        value["retryable"] = retryable.into();
    }
    if let Some(basis) = &params.retention_basis {
        value["retention_basis"] = serde_json::json!({
            "evidence_type": basis.evidence_type().as_str(),
            "evidence_id": basis.evidence_id().as_str(),
        });
    }
    value
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
