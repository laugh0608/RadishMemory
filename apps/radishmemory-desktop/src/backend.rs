//! Shared controller backend; original plaintext capture lives only in this worker-owned value.
use radishmemory_application::*;
use radishmemory_source_vault::{CaptureAbandonmentTarget, LibraryProvider};

use crate::{DesktopError, DesktopErrorCode, DesktopErrorReason};

pub(crate) enum Library<R: ApplicationRuntime, P: LibraryProvider> {
    Plain(LocalLibrary<R>),
    Encrypted {
        library: EncryptedLibrary<R, P>,
        capture: Option<Box<SourceCapture>>,
        deletion: Option<Box<DeleteRequest>>,
    },
}

macro_rules! read {
    ($self:ident, $method:ident($($arg:expr),*)) => {
        match $self {
            Self::Plain(library) => library.$method($($arg),*),
            Self::Encrypted { library, .. } => library.$method($($arg),*),
        }
    };
}

impl<R: ApplicationRuntime, P: LibraryProvider> Library<R, P> {
    pub(crate) fn list_sources(
        &self,
        offset: u64,
        limit: usize,
    ) -> Result<Vec<SourceLineageSummary>, ApplicationError> {
        read!(self, list_sources(offset, limit))
    }
    pub(crate) fn list_source_versions(
        &self,
        id: &Identifier,
    ) -> Result<Vec<SourceVersionSummary>, ApplicationError> {
        read!(self, list_source_versions(id))
    }
    pub(crate) fn search_sources(
        &mut self,
        query: NonEmptyText,
        limit: usize,
        sensitivity: [Sensitivity; 1],
    ) -> Result<Vec<SourceSearchResult>, ApplicationError> {
        read!(self, search_sources(query, limit, sensitivity))
    }
    pub(crate) fn export_source(
        &self,
        id: &Identifier,
        request: &FileExportRequest,
    ) -> Result<FileExportReceipt, ApplicationError> {
        read!(self, export_source(id, request))
    }
    pub(crate) fn verify_library(&self) -> Result<(), ApplicationError> {
        match self {
            Self::Plain(l) => l.verify_library(),
            Self::Encrypted { library, .. } => library.verify_library().map(|_| ()),
        }
    }
    pub(crate) fn rebuild_recall(&mut self) -> Result<(), ApplicationError> {
        match self {
            Self::Plain(l) => l.rebuild_recall(),
            Self::Encrypted { library, .. } => library.rebuild_recall().map(|_| ()),
        }
    }
    pub(crate) fn import_new_source(
        &mut self,
        request: &FileReadRequest,
    ) -> Result<FileCaptureReceipt, ApplicationError> {
        match self {
            Self::Plain(l) => l.import_new_source(request),
            Self::Encrypted {
                library, capture, ..
            } => {
                // Worker rejects new mutations while any original request is retained.
                *capture = Some(Box::new(library.prepare_import_new_source(request)?));
                let receipt =
                    library.capture_source(capture.as_ref().expect("prepared capture"))?;
                *capture = None;
                Ok(receipt)
            }
        }
    }
    pub(crate) fn update_source(
        &mut self,
        id: &Identifier,
        request: &FileReadRequest,
    ) -> Result<FileCaptureReceipt, ApplicationError> {
        match self {
            Self::Plain(l) => l.update_source(id, request),
            Self::Encrypted {
                library, capture, ..
            } => {
                *capture = Some(Box::new(library.prepare_update_source(id, request)?));
                let receipt =
                    library.capture_source(capture.as_ref().expect("prepared capture"))?;
                *capture = None;
                Ok(receipt)
            }
        }
    }
    pub(crate) fn delete_source_lineage(
        &mut self,
        id: &Identifier,
    ) -> Result<DeletionEvidence, ApplicationError> {
        match self {
            Self::Plain(l) => l.delete_source_lineage(id),
            Self::Encrypted {
                library, deletion, ..
            } => {
                *deletion = Some(Box::new(library.prepare_source_lineage_deletion(id)?));
                let evidence = library.execute_source_lineage_deletion(
                    deletion.as_ref().expect("prepared deletion"),
                )?;
                if evidence.params().overall_status == DeletionOverallStatus::Completed {
                    *deletion = None;
                }
                Ok(evidence)
            }
        }
    }
    pub(crate) fn holds_original(&self) -> bool {
        matches!(
            self,
            Self::Encrypted {
                capture: Some(_),
                ..
            } | Self::Encrypted {
                deletion: Some(_),
                ..
            }
        )
    }
    pub(crate) fn holds_capture(&self) -> bool {
        matches!(
            self,
            Self::Encrypted {
                capture: Some(_),
                ..
            }
        )
    }
    pub(crate) fn recovery(
        &self,
    ) -> Result<(Option<CaptureAbandonmentTarget>, Vec<DeleteRequest>), DesktopError> {
        match self {
            Self::Plain(_) => Ok((None, Vec::new())),
            Self::Encrypted { library, .. } => Ok((
                library.inspect_capture_abandonment().map_err(app_error)?,
                library.unfinished_delete_requests().map_err(app_error)?,
            )),
        }
    }
    pub(crate) fn retry(&mut self) -> Result<Option<DeletionEvidence>, DesktopError> {
        let Self::Encrypted {
            library,
            capture,
            deletion,
        } = self
        else {
            return Err(recovery_required());
        };
        if let Some(request) = capture {
            library.capture_source(request).map_err(app_error)?;
            *capture = None;
            Ok(None)
        } else if let Some(request) = deletion {
            let evidence = library
                .execute_source_lineage_deletion(request)
                .map_err(app_error)?;
            if evidence.params().overall_status == DeletionOverallStatus::Completed {
                *deletion = None;
            }
            Ok(Some(evidence))
        } else {
            Err(recovery_required())
        }
    }
    pub(crate) fn resume_delete(
        &mut self,
        request: DeleteRequest,
    ) -> Result<Option<DeletionEvidence>, DesktopError> {
        let Self::Encrypted { deletion, .. } = self else {
            return Err(recovery_required());
        };
        *deletion = Some(Box::new(request));
        self.retry()
    }
    pub(crate) fn abandon(
        &mut self,
        target: &CaptureAbandonmentTarget,
    ) -> Result<(), DesktopError> {
        let Self::Encrypted {
            library, capture, ..
        } = self
        else {
            return Err(recovery_required());
        };
        library.abandon_capture(target).map_err(app_error)?;
        *capture = None;
        Ok(())
    }
}
pub(crate) fn app_error(error: ApplicationError) -> DesktopError {
    DesktopError::application(&error)
}
pub(crate) fn recovery_required() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::LocalLibrary,
        DesktopErrorReason::RecoveryRequired,
        false,
    )
}
