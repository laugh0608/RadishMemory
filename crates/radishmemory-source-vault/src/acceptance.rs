//! Opt-in synthetic provider: never use with personal data or production libraries.
//! All instances use public, fixed test key material and no operating-system store.
//! The real coordinators still validate database eligibility, objects and locks.
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use radishmemory_core::{
    ComponentResult, DeleteRequest, LocalDeletionExecution, SourceCapture, SourceCaptureResult,
};

use crate::{
    BodyMigrationReport, KeyEncryptionKey, KeySlot, LibraryProvider, LibraryReader,
    ObjectDirectory, SourceVaultError, SourceVaultErrorCode, VaultMaintenanceError,
};

type Result<T> = std::result::Result<T, VaultMaintenanceError>;

#[derive(Default)]
struct State {
    slot: RefCell<Option<KeySlot>>,
    failure: Cell<Option<SourceVaultErrorCode>>,
    wrong_key: Cell<bool>,
    loads: Cell<usize>,
    creations: Cell<usize>,
    interrupt_migration: Cell<bool>,
    interrupt_capture: Cell<Option<crate::capture::Step>>,
    interrupt_deletion: Cell<Option<crate::deletion::Step>>,
    interrupt_evidence: Cell<Option<bool>>,
}

/// Clones share a synthetic key-store state, never a database connection or KEK.
#[derive(Clone, Default)]
pub struct SyntheticLibraryProvider(Rc<State>);

impl SyntheticLibraryProvider {
    pub fn existing(namespace: &str, device: &str) -> Result<Self> {
        let provider = Self::default();
        *provider.0.slot.borrow_mut() = Some(KeySlot::new(namespace, device)?);
        Ok(provider)
    }

    pub fn forget_key(&self) {
        *self.0.slot.borrow_mut() = None;
    }

    pub fn fail_key_access(&self, failure: Option<SourceVaultErrorCode>) {
        self.0.failure.set(failure);
    }

    pub fn use_wrong_key(&self, wrong: bool) {
        self.0.wrong_key.set(wrong);
    }

    pub fn key_loads(&self) -> usize {
        self.0.loads.get()
    }

    pub fn key_creations(&self) -> usize {
        self.0.creations.get()
    }

    pub fn interrupt_next_migration_after_publish(&self) {
        self.0.interrupt_migration.set(true);
    }

    pub fn interrupt_next_capture_after_publish(&self) {
        self.0
            .interrupt_capture
            .set(Some(crate::capture::Step::Published));
    }

    pub fn interrupt_next_capture_after_commit(&self) {
        self.0
            .interrupt_capture
            .set(Some(crate::capture::Step::Committed));
    }
    pub fn interrupt_next_deletion_after_intent(&self) {
        self.0
            .interrupt_deletion
            .set(Some(crate::deletion::Step::IntentCommitted));
    }
    pub fn interrupt_next_deletion_after_execution(&self) {
        self.0
            .interrupt_deletion
            .set(Some(crate::deletion::Step::Executed));
    }
    pub fn interrupt_next_deletion_evidence(&self, after_commit: bool) {
        self.0.interrupt_evidence.set(Some(after_commit));
    }

    fn load(&self, slot: &KeySlot) -> std::result::Result<KeyEncryptionKey, SourceVaultError> {
        self.0.loads.set(self.0.loads.get() + 1);
        if let Some(code) = self.0.failure.get() {
            return Err(SourceVaultError::new(code, "synthetic key access"));
        }
        match self.0.slot.borrow().as_ref() {
            None => {
                return Err(SourceVaultError::new(
                    SourceVaultErrorCode::KeyMissing,
                    "synthetic missing key",
                ));
            }
            Some(found) if found != slot => {
                return Err(SourceVaultError::new(
                    SourceVaultErrorCode::KeyMetadataMismatch,
                    "synthetic key slot mismatch",
                ));
            }
            Some(_) => {}
        }
        Ok(KeyEncryptionKey::new(
            [if self.0.wrong_key.get() { 0x43 } else { 0x42 }; 32],
        ))
    }

    /// Execute a real capture using public synthetic key material.
    pub fn capture(
        &self,
        directory: &ObjectDirectory,
        namespace: &str,
        device: &str,
        request: &SourceCapture,
    ) -> Result<SourceCaptureResult> {
        let slot = KeySlot::new(namespace, device)?;
        let interrupt = self.0.interrupt_capture.take();
        crate::capture::capture_with_step(
            directory,
            namespace,
            device,
            request,
            || self.load(&slot),
            |step| {
                if interrupt == Some(step) {
                    Err(interrupted())
                } else {
                    Ok(())
                }
            },
        )
    }

    pub fn execute_deletion(
        &self,
        directory: &ObjectDirectory,
        request: &DeleteRequest,
        execution: &LocalDeletionExecution,
    ) -> Result<Vec<ComponentResult>> {
        let p = request.params();
        let slot = KeySlot::new(p.namespace_id.as_str(), p.device_id.as_str())?;
        let interrupt = self.0.interrupt_deletion.take();
        crate::deletion::execute_with_step(
            directory,
            request,
            execution,
            || self.load(&slot),
            |step| {
                if interrupt == Some(step) {
                    Err(interrupted())
                } else {
                    Ok(())
                }
            },
        )
    }

    pub fn rebuild(
        &self,
        directory: &ObjectDirectory,
        namespace: &str,
        device: &str,
    ) -> Result<crate::VerificationReport> {
        let slot = KeySlot::new(namespace, device)?;
        crate::maintenance::maintain(directory, namespace, device, true, || self.load(&slot))
    }
}

fn interrupted() -> VaultMaintenanceError {
    SourceVaultError::new(
        SourceVaultErrorCode::Io,
        "synthetic coordination interruption",
    )
    .into()
}

impl crate::provider::sealed::Sealed for SyntheticLibraryProvider {}

impl LibraryProvider for SyntheticLibraryProvider {
    fn initialize_library_key(
        &self,
        directory: &ObjectDirectory,
        namespace: &str,
        device: &str,
    ) -> Result<KeyEncryptionKey> {
        let slot = KeySlot::new(namespace, device)?;
        crate::bootstrap::initialize(directory, namespace, device, |initialized| {
            if !initialized && self.0.slot.borrow().is_none() && self.0.failure.get().is_none() {
                *self.0.slot.borrow_mut() = Some(slot.clone());
                self.0.creations.set(self.0.creations.get() + 1);
            }
            self.load(&slot)
        })
    }

    fn migrate_library_bodies(
        &self,
        directory: &ObjectDirectory,
        namespace: &str,
        device: &str,
    ) -> Result<BodyMigrationReport> {
        let slot = KeySlot::new(namespace, device)?;
        let interrupt = self.0.interrupt_migration.replace(false);
        crate::body_migration::migrate_with_step(
            directory,
            namespace,
            device,
            || self.load(&slot),
            |step| {
                if interrupt && step == crate::body_migration::Step::Published {
                    Err(interrupted())
                } else {
                    Ok(())
                }
            },
        )
    }

    fn open_library_reader<'a>(
        &self,
        directory: &'a ObjectDirectory,
        namespace: &str,
        device: &str,
    ) -> Result<LibraryReader<'a>> {
        let slot = KeySlot::new(namespace, device)?;
        crate::reader::open(directory, namespace, device, || self.load(&slot))
    }
    fn capture_library_source(
        &self,
        directory: &crate::ObjectDirectory,
        namespace: &str,
        device: &str,
        capture: &radishmemory_core::SourceCapture,
    ) -> Result<radishmemory_core::SourceCaptureResult> {
        self.capture(directory, namespace, device, capture)
    }

    fn execute_library_deletion(
        &self,
        directory: &crate::ObjectDirectory,
        request: &radishmemory_core::DeleteRequest,
        execution: &radishmemory_core::LocalDeletionExecution,
    ) -> Result<Vec<radishmemory_core::ComponentResult>> {
        self.execute_deletion(directory, request, execution)
    }

    fn store_library_deletion_evidence(
        &self,
        directory: &crate::ObjectDirectory,
        evidence: &radishmemory_core::DeletionEvidence,
    ) -> Result<()> {
        let p = evidence.params();
        let slot = KeySlot::new(p.namespace_id.as_str(), p.device_id.as_str())?;
        let interrupt = self.0.interrupt_evidence.take();
        if interrupt == Some(false) {
            return Err(interrupted());
        }
        crate::deletion::store_evidence(directory, evidence, || self.load(&slot))?;
        if interrupt == Some(true) {
            return Err(interrupted());
        }
        Ok(())
    }

    fn verify_library_objects(
        &self,
        directory: &crate::ObjectDirectory,
        namespace: &str,
        device: &str,
    ) -> Result<crate::VerificationReport> {
        let slot = KeySlot::new(namespace, device)?;
        crate::maintenance::maintain(directory, namespace, device, false, || self.load(&slot))
    }

    fn rebuild_library_derivations(
        &self,
        directory: &crate::ObjectDirectory,
        namespace: &str,
        device: &str,
    ) -> Result<crate::VerificationReport> {
        self.rebuild(directory, namespace, device)
    }
    fn inspect_library_capture_abandonment(
        &self,
        directory: &crate::ObjectDirectory,
        namespace: &str,
        device: &str,
    ) -> Result<Option<crate::CaptureAbandonmentTarget>> {
        let slot = KeySlot::new(namespace, device)?;
        crate::abandonment::inspect(directory, namespace, device, || self.load(&slot))
    }

    fn abandon_library_capture(
        &self,
        directory: &crate::ObjectDirectory,
        namespace: &str,
        device: &str,
        target: &crate::CaptureAbandonmentTarget,
    ) -> Result<crate::AbandonmentReport> {
        let slot = KeySlot::new(namespace, device)?;
        crate::abandonment::abandon(directory, namespace, device, target, || self.load(&slot))
    }
}

impl fmt::Debug for SyntheticLibraryProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SyntheticLibraryProvider([REDACTED])")
    }
}
