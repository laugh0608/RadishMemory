//! One worker owns all storage and original retry requests. UI gets data snapshots only.
use crate::backend::{Library, app_error, recovery_required};
use crate::controller::LibraryView;
use crate::{
    ApplicationPaths, DesktopError, DesktopErrorCode, DesktopErrorReason, LibraryController,
    ProductionRuntime, load_or_create_host_profile,
};
use radishmemory_application::*;
use radishmemory_source_vault::{CaptureAbandonmentTarget, LibraryProvider, PlatformKeyProvider};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

pub(crate) enum Command {
    OpenPlain,
    OpenEncrypted,
    InitializeKey,
    Migrate,
    Verify,
    Rebuild,
    Refresh,
    Import(FileReadRequest),
    Update(FileReadRequest),
    Export(FileExportRequest),
    Search(String),
    SelectLineage(Identifier),
    SelectVersion(Identifier),
    SelectResult(Identifier, Identifier),
    Delete,
    RetryOriginal,
    ResumeDelete(Identifier),
    InspectRecovery,
    Abandon,
    Shutdown,
}
#[derive(Default)]
pub(crate) struct Snapshot {
    pub view: Option<LibraryView>,
    pub encrypted: bool,
    pub holds_original: bool,
    pub holds_capture: bool,
    pub abandonment_available: bool,
    pub recovery_known: bool,
    pub deletion_requests: Vec<Identifier>,
    pub last_deletion: Option<DeletionEvidence>,
    pub message: Option<String>,
    pub error: Option<DesktopError>,
    pub refresh_error: Option<DesktopError>,
}

pub(crate) struct Engine<P: LibraryProvider, F: Fn() -> P> {
    paths: ApplicationPaths,
    provider: F,
    controller: Option<LibraryController<ProductionRuntime, P>>,
    encrypted: bool,
    target: Option<CaptureAbandonmentTarget>,
    deletions: Vec<DeleteRequest>,
    recovery_known: bool,
    view_valid: bool,
}
impl<P: LibraryProvider, F: Fn() -> P> Engine<P, F> {
    pub(crate) fn new(paths: ApplicationPaths, provider: F) -> Self {
        Self {
            paths,
            provider,
            controller: None,
            encrypted: false,
            target: None,
            deletions: Vec::new(),
            recovery_known: false,
            view_valid: false,
        }
    }
    fn location(&self) -> Result<EncryptedLibraryLocation<P>, DesktopError> {
        let profile = load_or_create_host_profile(&self.paths, &mut ProductionRuntime)?;
        let config = LocalLibraryConfig::phase1_local(
            profile.namespace_id().clone(),
            profile.device_id().clone(),
        )
        .map_err(app_error)?;
        EncryptedLibraryLocation::with_provider(
            self.paths.data_directory(),
            config,
            (self.provider)(),
        )
        .map_err(app_error)
    }
    fn controller(&mut self) -> Result<&mut LibraryController<ProductionRuntime, P>, DesktopError> {
        self.controller.as_mut().ok_or_else(recovery_required)
    }
    fn inspect(&mut self) -> Result<(), DesktopError> {
        self.recovery_known = false;
        self.target = None;
        self.deletions.clear();
        if let Some(controller) = &mut self.controller {
            let (target, deletions) = controller.backend().recovery()?;
            self.target = target;
            self.deletions = deletions;
        } else if self.encrypted {
            self.target = self
                .location()?
                .inspect_capture_abandonment()
                .map_err(app_error)?;
            // Deletion discovery requires successful authenticated open, not an empty fallback.
            return Err(recovery_required());
        }
        self.recovery_known = true;
        Ok(())
    }
    fn refresh_after_write(&mut self, snapshot: &mut Snapshot) {
        let refreshed = self
            .controller()
            .and_then(LibraryController::refresh_sources);
        let inspected = self.inspect();
        snapshot.refresh_error = refreshed.and(inspected).err();
    }
    fn held(&mut self) -> bool {
        self.controller
            .as_mut()
            .is_some_and(|c| c.backend().holds_original())
    }
    fn blocked(&mut self) -> bool {
        !self.view_valid
            || self.held()
            || self.target.is_some()
            || !self.deletions.is_empty()
            || !self.recovery_known
    }
    fn perform(&mut self, command: Command, snapshot: &mut Snapshot) -> Result<(), DesktopError> {
        let changes_library = matches!(
            command,
            Command::OpenPlain | Command::OpenEncrypted | Command::InitializeKey | Command::Migrate
        );
        if changes_library {
            if self.held() {
                return Err(recovery_required());
            }
            self.recovery_known = false;
            self.target = None;
            self.deletions.clear();
        }
        if matches!(
            command,
            Command::Import(_) | Command::Update(_) | Command::Delete
        ) && self.blocked()
        {
            return Err(recovery_required());
        }
        match command {
            Command::OpenPlain => {
                self.controller = None;
                self.encrypted = false;
                let profile = load_or_create_host_profile(&self.paths, &mut ProductionRuntime)?;
                let config = LocalLibraryConfig::phase1_local(
                    profile.namespace_id().clone(),
                    profile.device_id().clone(),
                )
                .map_err(app_error)?;
                let library =
                    LocalLibrary::open(self.paths.database_path(), ProductionRuntime, config)
                        .map_err(app_error)?;
                self.controller = Some(LibraryController::from_library(Library::Plain(library))?);
                self.inspect()?;
                snapshot.message = Some("Legacy plaintext library opened.".into());
            }
            Command::OpenEncrypted => {
                self.controller = None;
                self.encrypted = true;
                let library = self
                    .location()?
                    .open(ProductionRuntime)
                    .map_err(app_error)?;
                self.controller = Some(LibraryController::from_library(Library::Encrypted {
                    library,
                    capture: None,
                    deletion: None,
                })?);
                self.inspect()?;
                snapshot.message = Some(
                    "Encrypted source objects opened. FTS still contains full plaintext.".into(),
                );
            }
            Command::InitializeKey | Command::Migrate => {
                self.controller = None; // Drop legacy database before migration/provider access.
                self.encrypted = true;
                self.recovery_known = false;
                let location = self.location()?;
                if matches!(command, Command::InitializeKey) {
                    location.initialize_key().map_err(app_error)?;
                    snapshot.message =
                        Some("Key checkpoint prepared. Explicitly migrate bodies next.".into());
                } else {
                    location.migrate_bodies().map_err(app_error)?;
                    snapshot.message = Some(
                        "Body migration finished. Explicitly open encrypted objects next.".into(),
                    );
                }
            }
            Command::Verify | Command::Rebuild => {
                if self.encrypted {
                    let location = self.location()?;
                    let report = if matches!(command, Command::Verify) {
                        location.verify_library()
                    } else {
                        location.rebuild_recall()
                    }
                    .map_err(app_error)?;
                    snapshot.message = Some(format!(
                        "Verified {} objects; pending captures: {}; abandonment pending: {}; deletion pending objects: {}; derivations rebuilt: {}.",
                        report.committed_objects_verified,
                        report.pending_captures,
                        report.abandonment_pending,
                        report.deletion_pending_objects,
                        report.derivations_rebuilt
                    ));
                } else if matches!(command, Command::Verify) {
                    self.controller()?.verify()?;
                    snapshot.message = Some("Canonical facts and recall verified.".into());
                } else {
                    self.controller()?.rebuild()?;
                    snapshot.message = Some("Derived recall rebuilt from verified facts.".into());
                }
            }
            Command::Refresh => {
                self.controller()?.refresh_sources()?;
                self.inspect()?;
            }
            Command::Import(request) => {
                self.controller()?.import_source(&request)?;
                snapshot.message = Some("Capture committed; original request released.".into());
            }
            Command::Update(request) => {
                self.controller()?.update_selected(&request)?;
                snapshot.message = Some("Capture committed; original request released.".into());
            }
            Command::Export(request) => {
                self.controller()?.export_selected(&request)?;
                snapshot.message = Some("Managed bytes exported without overwrite.".into());
            }
            Command::Search(query) => {
                self.controller()?.search(&query)?;
            }
            Command::SelectLineage(id) => self.controller()?.select_lineage(&id)?,
            Command::SelectVersion(id) => self.controller()?.select_source_version(&id)?,
            Command::SelectResult(lineage, source) => {
                self.controller()?.select_lineage(&lineage)?;
                self.controller()?.select_source_version(&source)?;
            }
            Command::Delete => {
                snapshot.last_deletion = Some(self.controller()?.delete_selected_lineage()?);
            }
            Command::RetryOriginal => {
                snapshot.last_deletion = self.controller()?.backend().retry()?;
                snapshot.message =
                    Some("Original request executed; inspect deletion evidence if present.".into());
                self.refresh_after_write(snapshot);
            }
            Command::ResumeDelete(id) => {
                if self.held() {
                    return Err(recovery_required());
                }
                let request = self
                    .deletions
                    .iter()
                    .find(|r| r.params().delete_request_id == id)
                    .cloned()
                    .ok_or_else(recovery_required)?;
                snapshot.last_deletion = self.controller()?.backend().resume_delete(request)?;
                self.refresh_after_write(snapshot);
            }
            Command::InspectRecovery => self.inspect()?,
            Command::Abandon => {
                // User confirmed exactly the target retained by the preceding inspect.
                let target = self.target.as_ref().ok_or_else(recovery_required)?;
                if let Some(controller) = &mut self.controller {
                    controller.backend().abandon(target)?;
                } else {
                    self.location()?
                        .abandon_capture(target)
                        .map_err(app_error)?;
                }
                snapshot.message =
                    Some("Exact uncommitted capture abandoned; committed sources retained.".into());
                self.refresh_after_write(snapshot);
            }
            Command::Shutdown => return Err(recovery_required()),
        }
        Ok(())
    }
    pub(crate) fn execute(&mut self, command: Command) -> Snapshot {
        let refreshes = matches!(
            command,
            Command::OpenPlain
                | Command::OpenEncrypted
                | Command::Refresh
                | Command::RetryOriginal
                | Command::ResumeDelete(_)
                | Command::Abandon
        );
        let mut snapshot = Snapshot::default();
        snapshot.error = self.perform(command, &mut snapshot).err();
        if snapshot.error.is_some() {
            self.view_valid = false;
        } else if refreshes {
            self.view_valid = true;
        }
        if let Some(controller) = &mut self.controller {
            if self.view_valid {
                snapshot.view = Some(controller.snapshot());
            }
            snapshot.holds_original = controller.backend().holds_original();
            snapshot.holds_capture = controller.backend().holds_capture();
            if snapshot.refresh_error.is_none() {
                snapshot.refresh_error = controller.refresh_error.take();
            }
        }
        if snapshot.refresh_error.is_some() {
            snapshot.view = None;
            self.view_valid = false;
        }
        snapshot.encrypted = self.encrypted;
        snapshot.abandonment_available = self.target.is_some();
        snapshot.recovery_known = self.recovery_known;
        snapshot.deletion_requests = self
            .deletions
            .iter()
            .map(|r| r.params().delete_request_id.clone())
            .collect();
        snapshot
    }
}

pub(crate) struct Worker {
    sender: Sender<Command>,
    receiver: Receiver<Snapshot>,
    thread: Option<JoinHandle<()>>,
    pub busy: bool,
    pub stopped: bool,
}
impl Worker {
    pub(crate) fn start() -> Result<Self, DesktopError> {
        crate::logging::install()?;
        Self::spawn(|| {
            let paths = ApplicationPaths::resolve()?;
            Ok(Engine::new(paths, PlatformKeyProvider::new))
        })
    }
    fn spawn<P, F, B>(build: B) -> Result<Self, DesktopError>
    where
        P: LibraryProvider + 'static,
        F: Fn() -> P + 'static,
        B: FnOnce() -> Result<Engine<P, F>, DesktopError> + Send + 'static,
    {
        let (sender, commands) = mpsc::channel();
        let (results, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("radishmemory-library".into())
            .spawn(move || {
                let mut engine = match build() {
                    Ok(engine) => engine,
                    Err(error) => {
                        let _ = results.send(Snapshot {
                            error: Some(error),
                            ..Snapshot::default()
                        });
                        return;
                    }
                };
                while let Ok(command) = commands.recv() {
                    if matches!(command, Command::Shutdown) {
                        break;
                    }
                    if results.send(engine.execute(command)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| stopped())?;
        Ok(Self {
            sender,
            receiver,
            thread: Some(thread),
            busy: false,
            stopped: false,
        })
    }
    pub(crate) fn send(&mut self, command: Command) -> Result<(), DesktopError> {
        if self.stopped {
            return Err(stopped());
        }
        if self.busy {
            return Err(recovery_required());
        }
        self.sender.send(command).map_err(|_| {
            self.stopped = true;
            stopped()
        })?;
        self.busy = true;
        Ok(())
    }
    pub(crate) fn poll(&mut self) -> Option<Snapshot> {
        match self.receiver.try_recv() {
            Ok(snapshot) => {
                self.busy = false;
                Some(snapshot)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) if !self.stopped => {
                self.busy = false;
                self.stopped = true;
                Some(Snapshot {
                    error: Some(stopped()),
                    ..Snapshot::default()
                })
            }
            Err(TryRecvError::Disconnected) => None,
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn stopped() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::Runtime,
        DesktopErrorReason::WorkerStopped,
        false,
    )
}

#[cfg(test)]
mod tests;
