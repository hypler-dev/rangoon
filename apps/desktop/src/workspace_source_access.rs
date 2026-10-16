//! Private typed source, capability and workflow routing. Production construction remains legacy;
//! managed encrypted Owner construction is confined to synthetic tests.
#[cfg(test)]
use super::admit;
use super::{
    AdmissionError, WorkspaceResult,
    capabilities::CapabilityResult,
    compilation::{self, CompilationResult},
    composition::{
        self, CompositionHandle, CompositionResult, CompositionSession, RoutedCompositionPreview,
    },
    data_controls::{self, BackupSession, DataResult, DeletionSession, NativeRecordKind},
    instruction_bundles::{self, BundleResult, OutputState},
    session::Session,
    workflows::{
        self, BeginRequest, ClearRequest, Confirmation, InspectRequest, OpenRequest, Selection,
        WorkflowResult, WorkflowSession,
    },
};
use rangoon_desktop::workspace_lifecycle::{Drain, Gate, Lane, Lease};
use rangoon_desktop::workspace_session::{
    Error as ControlOwnerError, Layout as ControlLayout, Preview as ControlPreview, RecoveryPreview,
};
use rangoon_domain::{
    AnalysisReport,
    capability_v1::{CapabilityDetail, CapabilitySummary},
};
use rangoon_host::{PublicError, SelectionResult};
use rangoon_store::{
    CapabilityReceiptV1, CompositionReceipt, SaveReceipt, SnapshotMetadata, StoreError,
    WorkflowDetail, WorkflowSavePlan, WorkflowSaveReceipt, WorkflowSummary, WorkflowWorkspaceData,
    Workspace, WorkspaceCompilation,
};
use rangoon_store::{
    WorkflowDeletionPlan as DeletionPlan, WorkflowRecordKind as RecordKind,
    WorkflowRestorePlan as RestorePlan, WorkspaceBackup as Backup,
};
use rangoon_workflow::records::WorkflowRevision;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, TryLockError},
};

#[derive(Clone, Copy)]
enum Error {
    Store(StoreError),
    #[cfg(all(test, feature = "encrypted-workspace"))]
    Managed(rangoon_desktop::workspace_session::Error),
    Cleared,
    WriteUncertain,
    SaveCompleted,
    WriteCompleted,
    EncryptedRecoveryUnavailable,
    Host(&'static str, &'static str),
}
impl From<StoreError> for Error {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}
impl Error {
    fn public(self) -> PublicError {
        let (code, message) = match self {
            Self::Store(error) => error.public(),
            Self::Host(code, message) => (code, message),
            Self::EncryptedRecoveryUnavailable => (
                "encrypted_recovery_unavailable",
                "Encrypted recovery export is not available. No archive was read and no file was created.",
            ),
            Self::Cleared => (
                "analysis_cleared",
                "This analysis was cleared. Choose or open a source again.",
            ),
            Self::WriteUncertain => (
                "workspace_write_uncertain",
                "The write outcome is uncertain. Inspect the workspace after reopening before attempting another change.",
            ),
            Self::SaveCompleted => (
                "workspace_save_completed",
                "The save completed before the workspace closed. Reopen and inspect the saved list; closing did not undo the save.",
            ),
            Self::WriteCompleted => (
                "workspace_write_completed",
                "The local change completed before the workspace closed. Reopen and inspect saved content; closing did not undo the change.",
            ),
            #[cfg(all(test, feature = "encrypted-workspace"))]
            Self::Managed(error) => {
                use rangoon_desktop::workspace_session::Error as ManagedError;
                match error {
                    ManagedError::Locked => (
                        "workspace_locked",
                        "Unlock the workspace before accessing saved content.",
                    ),
                    ManagedError::RecoveryRequired => (
                        "workspace_recovery_required",
                        "Workspace recovery is required. Existing data was not replaced.",
                    ),
                    ManagedError::WriteUncertain => return Self::WriteUncertain.public(),
                    _ => StoreError::Unavailable.public(),
                }
            }
        };
        PublicError { code, message }
    }
    fn managed_failure(self) -> bool {
        #[cfg(all(test, feature = "encrypted-workspace"))]
        if matches!(self, Self::Managed(_)) {
            return true;
        }
        false
    }
    fn workflow(self) -> WorkflowResult {
        match self {
            Self::Store(error) => workflows::failure(error),
            _ => WorkflowResult::Failed {
                error: self.public(),
            },
        }
    }
    fn from_host(error: PublicError) -> Self {
        Self::Host(error.code, error.message)
    }
    fn data(self) -> DataResult {
        match self {
            Self::Store(error) => data_controls::failure(error),
            _ => DataResult::Failed {
                error: self.public(),
            },
        }
    }
    fn bundle(self) -> BundleResult {
        if matches!(
            self,
            Self::Store(StoreError::CapabilityNotFound | StoreError::NotFound)
        ) {
            return instruction_bundles::failure(PublicError {
                code: "instruction_candidate_changed",
                message: "The saved candidate is missing or changed. Refresh and compile again. No file was created.",
            });
        }
        instruction_bundles::failure(self.public())
    }
    fn composition(self) -> CompositionResult {
        match self {
            Self::Store(error) => composition::failure(error),
            _ => CompositionResult::Failed {
                error: self.public(),
            },
        }
    }
    fn compilation(self) -> CompilationResult {
        CompilationResult::Failed {
            error: self.public(),
        }
    }
    fn capability(self) -> CapabilityResult {
        CapabilityResult::Failed {
            error: self.public(),
        }
    }
    fn workspace(self) -> WorkspaceResult {
        WorkspaceResult::Failed {
            error: self.public(),
        }
    }
    fn selection(self) -> SelectionResult {
        SelectionResult::Failed {
            error: self.public(),
        }
    }
}

trait Backend: Send {
    fn control_layout(&self) -> ControlLayout {
        ControlLayout::Unavailable
    }
    fn control_prepare(
        &mut self,
        _kind: reopen::Kind,
    ) -> Result<ControlPreview, ControlOwnerError> {
        Err(ControlOwnerError::BackendUnavailable)
    }
    fn control_confirm(&mut self, _id: &str) -> Result<(), ControlOwnerError> {
        Err(ControlOwnerError::BackendUnavailable)
    }
    fn control_inspect(&mut self) -> Result<RecoveryPreview, ControlOwnerError> {
        Err(ControlOwnerError::BackendUnavailable)
    }
    fn control_reconcile(&mut self, _id: &str) -> Result<(), ControlOwnerError> {
        Err(ControlOwnerError::BackendUnavailable)
    }
    fn control_is_open(&self) -> bool {
        false
    }
    fn ordinary_session_healthy(&self) -> bool {
        self.control_is_open()
    }
    fn ordinary_closed_error(&self) -> Error {
        Error::Store(StoreError::Unavailable)
    }

    /// Memory-only cleanup. Never use this to read, provision or retry storage.
    fn close_session(&mut self) -> bool {
        true
    }
    fn list(&mut self) -> Result<Vec<SnapshotMetadata>, Error>;
    fn open(&mut self, source_id: &str) -> Result<AnalysisReport, Error>;
    fn save(&mut self, report: &AnalysisReport) -> Result<SaveReceipt, Error>;
    fn list_capabilities(&mut self) -> Result<Vec<CapabilitySummary>, Error>;
    fn open_capability(
        &mut self,
        id: &str,
        revision: Option<&str>,
    ) -> Result<CapabilityDetail, Error>;
    fn create_capability(
        &mut self,
        source: &str,
        fragment: &str,
        title: &str,
    ) -> Result<CapabilityReceiptV1, Error>;
    fn revise_capability(
        &mut self,
        id: &str,
        expected: &str,
        title: &str,
        content: &str,
    ) -> Result<CapabilityReceiptV1, Error>;
    fn review_capability(&mut self, id: &str, expected: &str)
    -> Result<CapabilityReceiptV1, Error>;
    fn workflow_data(&mut self) -> Result<WorkflowWorkspaceData, Error>;
    fn list_workflows(&mut self) -> Result<Vec<WorkflowSummary>, Error>;
    fn open_workflow(&mut self, id: &str, revision: Option<&str>) -> Result<WorkflowDetail, Error>;
    fn inspect_workflow_save(
        &mut self,
        candidate: &WorkflowRevision,
    ) -> Result<WorkflowSavePlan, Error>;
    fn save_workflow(
        &mut self,
        candidate: &WorkflowRevision,
        expected: &str,
    ) -> Result<WorkflowSaveReceipt, Error>;
    fn prepare_composition(
        &mut self,
        request: &rangoon_compose::application::Request,
    ) -> Result<RoutedCompositionPreview, Error>;
    fn confirm_composition(
        &mut self,
        handle: CompositionHandle,
        acknowledged: bool,
    ) -> Result<CompositionReceipt, Error>;
    fn compile_capability(
        &mut self,
        id: &str,
        revision: &str,
        profile: rangoon_compile::Profile,
    ) -> Result<WorkspaceCompilation, Error>;
    fn plaintext_backup_export_allowed(&self) -> bool;
    fn export_workflow_backup(&mut self) -> Result<Vec<u8>, Error>;
    fn prepare_workflow_restore(&mut self, backup: &Backup) -> Result<RestorePlan, Error>;
    fn restore_workflow_backup(
        &mut self,
        backup: &Backup,
        plan: &RestorePlan,
    ) -> Result<WorkflowWorkspaceData, Error>;
    fn inspect_workflow_deletion(
        &mut self,
        kind: RecordKind,
        id: &str,
    ) -> Result<DeletionPlan, Error>;
    fn delete_workflow_record(
        &mut self,
        plan: &DeletionPlan,
    ) -> Result<WorkflowWorkspaceData, Error>;
}
struct Legacy(Result<PathBuf, StoreError>);
impl Legacy {
    fn store(&self) -> Result<Workspace, Error> {
        self.0
            .as_ref()
            .map(|root| Workspace::new(root.clone()))
            .map_err(|error| Error::Store(*error))
    }
}
impl Backend for Legacy {
    fn ordinary_session_healthy(&self) -> bool {
        // Legacy has no authenticated Owner session. This also preserves the
        // memory-only close fixtures that use its real ordinary store methods.
        true
    }

    fn list(&mut self) -> Result<Vec<SnapshotMetadata>, Error> {
        self.store()?.list().map_err(Error::Store)
    }
    fn open(&mut self, id: &str) -> Result<AnalysisReport, Error> {
        self.store()?.open(id).map_err(Error::Store)
    }
    fn save(&mut self, report: &AnalysisReport) -> Result<SaveReceipt, Error> {
        self.store()?.save_v1(report).map_err(Error::Store)
    }
    fn list_capabilities(&mut self) -> Result<Vec<CapabilitySummary>, Error> {
        self.store()?.list_capabilities_v1().map_err(Error::Store)
    }
    fn open_capability(
        &mut self,
        id: &str,
        revision: Option<&str>,
    ) -> Result<CapabilityDetail, Error> {
        self.store()?
            .open_capability_v1(id, revision)
            .map_err(Error::Store)
    }
    fn create_capability(
        &mut self,
        source: &str,
        fragment: &str,
        title: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.store()?
            .create_capability_v1(source, fragment, title)
            .map_err(Error::Store)
    }
    fn revise_capability(
        &mut self,
        id: &str,
        expected: &str,
        title: &str,
        content: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.store()?
            .revise_capability_v1(id, expected, title, content)
            .map_err(Error::Store)
    }
    fn review_capability(
        &mut self,
        id: &str,
        expected: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.store()?
            .review_capability_v1(id, expected)
            .map_err(Error::Store)
    }
    fn workflow_data(&mut self) -> Result<WorkflowWorkspaceData, Error> {
        self.store()?.workflow_data().map_err(Error::Store)
    }
    fn list_workflows(&mut self) -> Result<Vec<WorkflowSummary>, Error> {
        self.store()?.list_workflows().map_err(Error::Store)
    }
    fn open_workflow(&mut self, id: &str, revision: Option<&str>) -> Result<WorkflowDetail, Error> {
        self.store()?
            .open_workflow(id, revision)
            .map_err(Error::Store)
    }
    fn inspect_workflow_save(
        &mut self,
        candidate: &WorkflowRevision,
    ) -> Result<WorkflowSavePlan, Error> {
        self.store()?
            .inspect_workflow_save(candidate)
            .map_err(Error::Store)
    }
    fn save_workflow(
        &mut self,
        candidate: &WorkflowRevision,
        expected: &str,
    ) -> Result<WorkflowSaveReceipt, Error> {
        self.store()?
            .save_workflow(candidate, expected)
            .map_err(Error::Store)
    }
    fn prepare_composition(
        &mut self,
        request: &rangoon_compose::application::Request,
    ) -> Result<RoutedCompositionPreview, Error> {
        self.store()?
            .preview_composition(request)
            .map(RoutedCompositionPreview::legacy)
            .map_err(Error::Store)
    }
    fn confirm_composition(
        &mut self,
        handle: CompositionHandle,
        acknowledged: bool,
    ) -> Result<CompositionReceipt, Error> {
        match handle {
            CompositionHandle::Legacy(prepared) => self
                .store()?
                .apply_composition(&prepared, acknowledged)
                .map_err(Error::Store),
            #[cfg(all(test, feature = "encrypted-workspace"))]
            CompositionHandle::Managed(_) => Err(Error::Store(StoreError::CompositionInvalid)),
        }
    }
    fn compile_capability(
        &mut self,
        id: &str,
        revision: &str,
        profile: rangoon_compile::Profile,
    ) -> Result<WorkspaceCompilation, Error> {
        self.store()?
            .compile_capability(id, revision, profile)
            .map_err(Error::Store)
    }
    fn plaintext_backup_export_allowed(&self) -> bool {
        true
    }
    fn export_workflow_backup(&mut self) -> Result<Vec<u8>, Error> {
        self.store()?.export_workflow_backup().map_err(Error::Store)
    }
    fn prepare_workflow_restore(&mut self, backup: &Backup) -> Result<RestorePlan, Error> {
        self.store()?
            .prepare_workflow_restore(backup)
            .map_err(Error::Store)
    }
    fn restore_workflow_backup(
        &mut self,
        backup: &Backup,
        plan: &RestorePlan,
    ) -> Result<WorkflowWorkspaceData, Error> {
        self.store()?
            .restore_workflow_backup(backup, plan)
            .map_err(Error::Store)
    }
    fn inspect_workflow_deletion(
        &mut self,
        kind: RecordKind,
        id: &str,
    ) -> Result<DeletionPlan, Error> {
        self.store()?
            .inspect_workflow_deletion(kind, id)
            .map_err(Error::Store)
    }
    fn delete_workflow_record(
        &mut self,
        plan: &DeletionPlan,
    ) -> Result<WorkflowWorkspaceData, Error> {
        self.store()?
            .delete_workflow_record(plan)
            .map_err(Error::Store)
    }
}

#[cfg(all(test, feature = "encrypted-workspace"))]
struct Managed<V> {
    owner: rangoon_desktop::workspace_session::Owner,
    vault: V,
}
#[cfg(all(test, feature = "encrypted-workspace"))]
impl<V: rangoon_desktop::workspace_session::Vault + Send> Backend for Managed<V> {
    fn control_layout(&self) -> ControlLayout {
        self.owner.inspect_layout()
    }
    fn control_prepare(&mut self, kind: reopen::Kind) -> Result<ControlPreview, ControlOwnerError> {
        match kind {
            reopen::Kind::CreateEmpty => self.owner.prepare_create_empty(),
            reopen::Kind::Unlock => self.owner.prepare_unlock(),
            reopen::Kind::Recovery => Err(ControlOwnerError::ConsentRequired),
        }
    }
    fn control_confirm(&mut self, id: &str) -> Result<(), ControlOwnerError> {
        self.owner.confirm(id, &self.vault)
    }
    fn control_inspect(&mut self) -> Result<RecoveryPreview, ControlOwnerError> {
        self.owner.inspect_recovery(&self.vault)
    }
    fn control_reconcile(&mut self, id: &str) -> Result<(), ControlOwnerError> {
        self.owner.confirm_recovery(id, &self.vault)
    }
    fn control_is_open(&self) -> bool {
        self.owner.is_unlocked() && !self.owner.requires_recovery()
    }

    fn ordinary_closed_error(&self) -> Error {
        Error::Managed(if self.owner.requires_recovery() {
            ControlOwnerError::RecoveryRequired
        } else {
            ControlOwnerError::Locked
        })
    }

    fn close_session(&mut self) -> bool {
        let healthy_for_reopen = !self.owner.requires_recovery();
        self.owner.lock();
        healthy_for_reopen
    }
    fn list(&mut self) -> Result<Vec<SnapshotMetadata>, Error> {
        self.owner.list(&self.vault).map_err(Error::Managed)
    }
    fn open(&mut self, id: &str) -> Result<AnalysisReport, Error> {
        self.owner.open(&self.vault, id).map_err(Error::Managed)
    }
    fn save(&mut self, report: &AnalysisReport) -> Result<SaveReceipt, Error> {
        self.owner.save(&self.vault, report).map_err(Error::Managed)
    }
    fn list_capabilities(&mut self) -> Result<Vec<CapabilitySummary>, Error> {
        self.owner
            .list_capabilities(&self.vault)
            .map_err(Error::Managed)
    }
    fn open_capability(
        &mut self,
        id: &str,
        revision: Option<&str>,
    ) -> Result<CapabilityDetail, Error> {
        self.owner
            .open_capability(&self.vault, id, revision)
            .map_err(Error::Managed)
    }
    fn create_capability(
        &mut self,
        source: &str,
        fragment: &str,
        title: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.owner
            .create_capability(&self.vault, source, fragment, title)
            .map_err(Error::Managed)
    }
    fn revise_capability(
        &mut self,
        id: &str,
        expected: &str,
        title: &str,
        content: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.owner
            .revise_capability(&self.vault, id, expected, title, content)
            .map_err(Error::Managed)
    }
    fn review_capability(
        &mut self,
        id: &str,
        expected: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.owner
            .review_capability(&self.vault, id, expected)
            .map_err(Error::Managed)
    }
    fn workflow_data(&mut self) -> Result<WorkflowWorkspaceData, Error> {
        self.owner
            .workflow_data(&self.vault)
            .map_err(Error::Managed)
    }
    fn list_workflows(&mut self) -> Result<Vec<WorkflowSummary>, Error> {
        self.owner
            .list_workflows(&self.vault)
            .map_err(Error::Managed)
    }
    fn open_workflow(&mut self, id: &str, revision: Option<&str>) -> Result<WorkflowDetail, Error> {
        self.owner
            .open_workflow(&self.vault, id, revision)
            .map_err(Error::Managed)
    }
    fn inspect_workflow_save(
        &mut self,
        candidate: &WorkflowRevision,
    ) -> Result<WorkflowSavePlan, Error> {
        self.owner
            .inspect_workflow_save(&self.vault, candidate)
            .map_err(Error::Managed)
    }
    fn save_workflow(
        &mut self,
        candidate: &WorkflowRevision,
        expected: &str,
    ) -> Result<WorkflowSaveReceipt, Error> {
        self.owner
            .save_workflow(&self.vault, candidate, expected)
            .map_err(Error::Managed)
    }
    fn prepare_composition(
        &mut self,
        request: &rangoon_compose::application::Request,
    ) -> Result<RoutedCompositionPreview, Error> {
        self.owner
            .prepare_composition_confirmation(&self.vault, request)
            .map(RoutedCompositionPreview::managed)
            .map_err(Error::Managed)
    }
    fn confirm_composition(
        &mut self,
        handle: CompositionHandle,
        acknowledged: bool,
    ) -> Result<CompositionReceipt, Error> {
        match handle {
            CompositionHandle::Managed(token) => self
                .owner
                .confirm_composition(&self.vault, &token, acknowledged)
                .map_err(Error::Managed),
            CompositionHandle::Legacy(_) => Err(Error::Store(StoreError::CompositionInvalid)),
        }
    }
    fn compile_capability(
        &mut self,
        id: &str,
        revision: &str,
        profile: rangoon_compile::Profile,
    ) -> Result<WorkspaceCompilation, Error> {
        self.owner
            .compile_capability(&self.vault, id, revision, profile)
            .map_err(Error::Managed)
    }
    fn plaintext_backup_export_allowed(&self) -> bool {
        false
    }
    fn export_workflow_backup(&mut self) -> Result<Vec<u8>, Error> {
        Err(Error::EncryptedRecoveryUnavailable)
    }
    fn prepare_workflow_restore(&mut self, backup: &Backup) -> Result<RestorePlan, Error> {
        self.owner
            .prepare_workflow_restore(&self.vault, backup)
            .map_err(Error::Managed)
    }
    fn restore_workflow_backup(
        &mut self,
        backup: &Backup,
        plan: &RestorePlan,
    ) -> Result<WorkflowWorkspaceData, Error> {
        self.owner
            .restore_workflow_backup(&self.vault, backup, plan)
            .map_err(Error::Managed)
    }
    fn inspect_workflow_deletion(
        &mut self,
        kind: RecordKind,
        id: &str,
    ) -> Result<DeletionPlan, Error> {
        self.owner
            .inspect_workflow_deletion(&self.vault, kind, id)
            .map_err(Error::Managed)
    }
    fn delete_workflow_record(
        &mut self,
        plan: &DeletionPlan,
    ) -> Result<WorkflowWorkspaceData, Error> {
        self.owner
            .delete_workflow_record(&self.vault, plan)
            .map_err(Error::Managed)
    }
}

struct Inner {
    gate: Gate,
    encrypted: bool,
    clients: crate::workspace_requests::Registry,
    closing: Mutex<Option<Arc<CloseCycle>>>,
    session: Session,
    workflow: WorkflowSession,
    composition: CompositionSession,
    backup: BackupSession,
    deletion: DeletionSession,
    backend: Mutex<Box<dyn Backend>>,
    failed: std::sync::atomic::AtomicBool,
    closed_error: Mutex<Error>,
    failure_notifier: Mutex<Option<FailureNotifier>>,
    #[cfg(test)]
    drain_unavailable: std::sync::atomic::AtomicBool,
}
struct CloseCycle {
    drain: Drain,
    healthy: bool,
    released: Mutex<Option<bool>>,
    cleanup_failed: std::sync::atomic::AtomicBool,
}
/// An opaque handle for one SourceAccess close cycle. It cannot resume admission.
pub(super) struct CloseTicket {
    access: SourceAccess,
    cycle: Arc<CloseCycle>,
}
type ClosingGuard<'a> = std::sync::MutexGuard<'a, Option<Arc<CloseCycle>>>;
impl CloseTicket {
    fn current_slot(&self) -> Result<(ClosingGuard<'_>, bool), ()> {
        let (slot, healthy) = match self.access.0.closing.lock() {
            Ok(slot) => (slot, true),
            Err(error) => (error.into_inner(), false),
        };
        if !slot.as_ref().is_some_and(|c| Arc::ptr_eq(c, &self.cycle)) {
            return Err(());
        }
        let healthy = healthy
            && !self.access.failure_latched()
            && self.cycle.healthy
            && !self
                .cycle
                .cleanup_failed
                .load(std::sync::atomic::Ordering::SeqCst);
        Ok((slot, healthy))
    }
    /// Advisory metadata only. Keep this exact closed cycle and its completed
    /// key-release proof through the read; never renew admission or open a key.
    pub(super) fn inspect_layout(&self) -> ControlLayout {
        let Ok((_slot, healthy)) = self.current_slot() else {
            return ControlLayout::Unavailable;
        };
        if !healthy || self.cycle.drain.is_quiescent() != Ok(true) {
            return ControlLayout::Unavailable;
        }
        if !matches!(self.cycle.released.lock(), Ok(released) if *released == Some(true)) {
            return ControlLayout::Unavailable;
        }
        let Ok(backend) = self.access.0.backend.try_lock() else {
            return ControlLayout::Unavailable;
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| backend.control_layout()))
            .unwrap_or(ControlLayout::Unavailable)
    }
    pub(super) fn purge(&self) -> bool {
        let Ok((_slot, healthy)) = self.current_slot() else {
            return false;
        };
        let purged = self.access.purge_retained();
        healthy && purged
    }
    pub(super) fn is_quiescent(&self) -> Result<bool, ()> {
        #[cfg(test)]
        if self
            .access
            .0
            .drain_unavailable
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(());
        }
        let (_slot, _) = self.current_slot()?;
        self.cycle.drain.is_quiescent().map_err(|_| ())
    }
    pub(super) fn release_key(&self) -> Result<bool, ()> {
        let (_slot, healthy) = self.current_slot()?;
        if !self.cycle.drain.is_quiescent().map_err(|_| ())? {
            return Err(());
        }
        let (mut released, release_healthy) = match self.cycle.released.lock() {
            Ok(released) => (released, true),
            Err(error) => (error.into_inner(), false),
        };
        if let Some(result) = *released {
            return Ok(healthy && release_healthy && result);
        }
        // Keep cycle identity protected through key release; an old memo or
        // cleanup cannot race a renewed control or a newly opened session.
        let (mut backend, backend_healthy) = match self.access.0.backend.try_lock() {
            Ok(backend) => (backend, true),
            Err(TryLockError::Poisoned(error)) => (error.into_inner(), false),
            Err(TryLockError::WouldBlock) => return Err(()),
        };
        let closed = backend.close_session();
        let result = healthy && release_healthy && backend_healthy && closed;
        *released = Some(result);
        Ok(result)
    }
}
// Private trusted-host prerequisite. No command or renderer can construct a
// control. Remove this dead-code allowance when the qualified supervisor mounts.
#[allow(dead_code)]
#[path = "workspace_reopen.rs"]
mod reopen;
#[allow(unused_imports)]
pub(super) use reopen::{Control as SourceControl, ControlError, Kind as ControlKind};

type FailureNotifier = Arc<dyn Fn() + Send + Sync>;
struct BackendWatch<'a> {
    access: &'a SourceAccess,
    armed: bool,
    closed_error: Error,
}
impl Drop for BackendWatch<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.access.fail_closed_with(self.closed_error);
        }
    }
}

#[derive(Clone)]
pub(super) struct SourceAccess(Arc<Inner>, Caller);
#[derive(Clone)]
enum Caller {
    Request(crate::workspace_requests::WorkspaceRequest),
    #[cfg(test)]
    HostFixture,
}
impl SourceAccess {
    fn new(gate: Gate, backend: Box<dyn Backend>) -> Self {
        Self::with_mode(gate, backend, false)
    }
    fn with_mode(gate: Gate, backend: Box<dyn Backend>, encrypted: bool) -> Self {
        Self(
            Arc::new(Inner {
                clients: crate::workspace_requests::Registry::new(encrypted),
                gate,
                encrypted,
                closing: Mutex::new(None),
                session: Session::default(),
                workflow: WorkflowSession::default(),
                composition: CompositionSession::default(),
                backup: BackupSession::default(),
                deletion: DeletionSession::default(),
                backend: Mutex::new(backend),
                failed: std::sync::atomic::AtomicBool::new(false),
                closed_error: Mutex::new(Error::Store(StoreError::Unavailable)),
                failure_notifier: Mutex::new(None),
                #[cfg(test)]
                drain_unavailable: std::sync::atomic::AtomicBool::new(false),
            }),
            Caller::Request(if encrypted {
                crate::workspace_requests::WorkspaceRequest::Refused
            } else {
                crate::workspace_requests::WorkspaceRequest::Legacy
            }),
        )
    }
    pub(super) fn for_request(
        &self,
        request: &crate::workspace_requests::WorkspaceRequest,
    ) -> Self {
        let request = if self.0.encrypted
            && matches!(request, crate::workspace_requests::WorkspaceRequest::Legacy)
        {
            crate::workspace_requests::WorkspaceRequest::Refused
        } else {
            request.clone()
        };
        Self(Arc::clone(&self.0), Caller::Request(request))
    }
    pub(super) fn for_model_callback(&self, lease: &crate::model_flight::ModelLease) -> Self {
        self.for_request(&crate::workspace_requests::WorkspaceRequest::Managed(
            Arc::new(lease.witness()),
        ))
    }
    pub(super) fn request_context(
        &self,
        label: &str,
        header: Option<&str>,
    ) -> crate::workspace_requests::WorkspaceRequest {
        if self.failure_latched() || self.0.closing.lock().map_or(true, |slot| slot.is_some()) {
            return crate::workspace_requests::WorkspaceRequest::Refused;
        }
        self.0.clients.request(label, header)
    }
    pub(super) fn request_view(&self) -> crate::workspace_requests::View {
        self.0.clients.view()
    }
    pub(super) fn legacy(gate: Gate, root: Result<PathBuf, StoreError>) -> Self {
        Self::new(gate, Box::new(Legacy(root)))
    }
    #[cfg(test)]
    pub(super) fn close_protocol_fixture(gate: Gate) -> Self {
        // Memory-only protocol fixtures; real keyed Owner closure is tested separately.
        Self::with_mode(gate, Box::new(Legacy(Err(StoreError::Unavailable))), true).host_fixture()
    }
    #[cfg(all(test, feature = "encrypted-workspace"))]
    fn managed<V: rangoon_desktop::workspace_session::Vault + Send + 'static>(
        gate: Gate,
        owner: rangoon_desktop::workspace_session::Owner,
        vault: V,
    ) -> Self {
        Self::with_mode(gate, Box::new(Managed { owner, vault }), true).host_fixture()
    }
    #[cfg(test)]
    fn host_fixture(mut self) -> Self {
        self.1 = Caller::HostFixture;
        self
    }
    pub(super) fn encrypted_close_available(&self) -> bool {
        self.0.encrypted
    }
    pub(super) fn failure_latched(&self) -> bool {
        self.0.failed.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub(super) fn set_failure_notifier(&self, notifier: FailureNotifier) -> Result<(), ()> {
        if !self.encrypted_close_available() {
            return Err(());
        }
        let mut slot = self.0.failure_notifier.lock().map_err(|_| ())?;
        if slot.is_some() {
            return Err(());
        }
        *slot = Some(notifier.clone());
        drop(slot);
        if self.failure_latched() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| notifier()));
        }
        Ok(())
    }
    fn refused_error(&self, error: AdmissionError) -> Option<Error> {
        if error != AdmissionError::Unavailable || !self.failure_latched() {
            return None;
        }
        Some(
            self.0
                .closed_error
                .lock()
                .map(|e| *e)
                .unwrap_or(Error::Store(StoreError::Unavailable)),
        )
    }
    pub(super) fn fail_closed(&self) {
        self.fail_closed_with(Error::Store(StoreError::Unavailable));
    }
    fn fail_closed_with(&self, error: Error) {
        if !self.encrypted_close_available() {
            return;
        }
        // Publish the finite first cause before any reader can observe failure.
        // Later callbacks cannot replace it with a different closed-state hint.
        if let Ok(mut reason) = self.0.closed_error.lock() {
            if !self.failure_latched() {
                *reason = error;
                self.0
                    .failed
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
        } else {
            self.0
                .failed
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        // Keep the real Drain in the existing closing slot before deleting
        // retained data. No callback or key owner is replaced or force-released.
        let _ = self.request_close();
        let _ = self.purge_retained();
        let notifier = match self.0.failure_notifier.lock() {
            Ok(slot) => slot.clone(),
            Err(error) => error.into_inner().clone(),
        };
        if let Some(notifier) = notifier {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| notifier()));
        }
    }
    fn call_backend<T>(
        &self,
        current: impl Fn() -> bool,
        action: impl FnOnce(&mut dyn Backend) -> Result<T, Error>,
        lock_error: Error,
    ) -> Result<T, Error> {
        if !current() {
            return Err(Error::Store(StoreError::Unavailable));
        }
        // Declared before the mutex guard: a panic drops/poisons Backend before
        // this watch closes admission and notifies native cleanup.
        let mut watch = BackendWatch {
            access: self,
            armed: self.0.encrypted,
            closed_error: Error::Store(StoreError::Unavailable),
        };
        let mut backend = self.0.backend.lock().map_err(|_| lock_error)?;
        if !current() {
            watch.armed = false;
            return Err(Error::Store(StoreError::Unavailable));
        }
        let result = action(backend.as_mut());
        if watch.armed {
            watch.armed = !backend.ordinary_session_healthy();
            if watch.armed {
                watch.closed_error = backend.ordinary_closed_error();
            }
        }
        drop(backend);
        result
    }
    pub(super) fn request_close(&self) -> Result<CloseTicket, ()> {
        if !self.encrypted_close_available() {
            return Err(());
        }
        let (mut slot, healthy) = match self.0.closing.lock() {
            Ok(slot) => (slot, true),
            Err(error) => (error.into_inner(), false),
        };
        let registry_healthy = self.0.clients.revoke().is_ok();
        let healthy = healthy && registry_healthy;
        let cycle = match slot.as_ref() {
            Some(cycle) => Arc::clone(cycle),
            None => {
                let drain = self.0.gate.request_drain().map_err(|_| ())?;
                let cycle = Arc::new(CloseCycle {
                    drain,
                    healthy,
                    released: Mutex::new(None),
                    cleanup_failed: std::sync::atomic::AtomicBool::new(false),
                });
                *slot = Some(Arc::clone(&cycle));
                cycle
            }
        };
        Ok(CloseTicket {
            access: self.clone(),
            cycle,
        })
    }
    pub(super) fn purge_retained(&self) -> bool {
        // Do not short-circuit: a poisoned family cannot retain another family's data.
        let source = self.0.session.clear();
        let workflow = self.0.workflow.purge();
        let composition = self.0.composition.purge();
        let backup = self.0.backup.purge();
        let deletion = self.0.deletion.purge();
        source && workflow && composition && backup && deletion
    }
    fn admit_open(&self, lane: Lane) -> Result<Lease, AdmissionError> {
        if self.failure_latched() {
            return Err(AdmissionError::Unavailable);
        }
        let slot = self
            .0
            .closing
            .lock()
            .map_err(|_| AdmissionError::Unavailable)?;
        if slot.is_some() {
            return Err(AdmissionError::Unavailable);
        }
        match &self.1 {
            Caller::Request(request) => request.admit(&self.0.gate, lane),
            #[cfg(test)]
            Caller::HostFixture => admit(&self.0.gate, lane),
        }
    }
    fn begin(&self, lane: Lane) -> Result<Ticket, AdmissionError> {
        let lease = self.admit_open(lane)?;
        let generation = self
            .0
            .session
            .generation()
            .ok_or(AdmissionError::Unavailable)?;
        Ok(Ticket {
            access: self.clone(),
            lease,
            generation,
        })
    }
    fn begin_capability(&self, lane: Lane) -> Result<CapabilityTicket, AdmissionError> {
        Ok(CapabilityTicket {
            access: self.clone(),
            lease: self.admit_open(lane)?,
        })
    }
    pub(super) async fn list_capabilities(&self) -> CapabilityResult {
        let ticket = match self.begin_capability(Lane::Read) {
            Ok(ticket) => ticket,
            Err(error) => {
                return self
                    .refused_error(error)
                    .map_or_else(|| admission_capability(error, false), Error::capability);
            }
        };
        match tauri::async_runtime::spawn_blocking(move || {
            ticket.read(|backend| backend.list_capabilities())
        })
        .await
        {
            Ok(completed) => match completed.deliver() {
                Ok(capabilities) => CapabilityResult::Listed { capabilities },
                Err(error) => error.capability(),
            },
            Err(_) => Error::Store(StoreError::Unavailable).capability(),
        }
    }
    pub(super) async fn open_capability(
        &self,
        id: String,
        revision: Option<String>,
    ) -> CapabilityResult {
        let ticket = match self.begin_capability(Lane::Read) {
            Ok(ticket) => ticket,
            Err(error) => {
                return self
                    .refused_error(error)
                    .map_or_else(|| admission_capability(error, false), Error::capability);
            }
        };
        match tauri::async_runtime::spawn_blocking(move || {
            ticket.read(|backend| backend.open_capability(&id, revision.as_deref()))
        })
        .await
        {
            Ok(completed) => match completed.deliver() {
                Ok(capability) => CapabilityResult::Opened {
                    capability: Box::new(capability),
                    already_applied: false,
                },
                Err(error) => error.capability(),
            },
            Err(_) => Error::Store(StoreError::Unavailable).capability(),
        }
    }
    pub(super) async fn create_capability(
        &self,
        source: String,
        fragment: String,
        title: String,
    ) -> CapabilityResult {
        self.mutate_capability(CapabilityMutation::Create {
            source,
            fragment,
            title,
        })
        .await
    }
    pub(super) async fn revise_capability(
        &self,
        id: String,
        expected: String,
        title: String,
        content: String,
    ) -> CapabilityResult {
        self.mutate_capability(CapabilityMutation::Revise {
            id,
            expected,
            title,
            content,
        })
        .await
    }
    pub(super) async fn review_capability(&self, id: String, expected: String) -> CapabilityResult {
        self.mutate_capability(CapabilityMutation::Review { id, expected })
            .await
    }
    async fn mutate_capability(&self, operation: CapabilityMutation) -> CapabilityResult {
        let ticket = match self.begin_capability(Lane::Exclusive) {
            Ok(ticket) => ticket,
            Err(error) => {
                return self
                    .refused_error(error)
                    .map_or_else(|| admission_capability(error, true), Error::capability);
            }
        };
        match tauri::async_runtime::spawn_blocking(move || {
            ticket.mutate(|backend| operation.apply(backend))
        })
        .await
        {
            Ok(completed) => completed.deliver(),
            Err(_) => Error::WriteUncertain.capability(),
        }
    }
    pub(super) fn begin_model_inputs(&self) -> Result<ModelInputOperation, AdmissionError> {
        self.begin_capability(Lane::Exclusive)
            .map(ModelInputOperation)
    }
    pub(super) fn begin_file(&self) -> Result<FileOperation, AdmissionError> {
        self.begin_capability(Lane::Exclusive).map(FileOperation)
    }
    pub(super) fn purge_data_controls(&self) -> bool {
        let backup = self.0.backup.purge();
        let deletion = self.0.deletion.purge();
        backup && deletion
    }
    pub(super) async fn restore_workspace_backup(
        &self,
        backup_id: String,
        expected: String,
    ) -> DataResult {
        self.data_operation(DataOperation::Restore {
            backup_id,
            expected,
        })
        .await
    }
    pub(super) async fn inspect_workspace_deletion(
        &self,
        kind: NativeRecordKind,
        id: String,
    ) -> DataResult {
        self.data_operation(DataOperation::InspectDeletion { kind, id })
            .await
    }
    pub(super) async fn delete_workspace_record(
        &self,
        kind: NativeRecordKind,
        id: String,
        expected: String,
    ) -> DataResult {
        self.data_operation(DataOperation::Delete { kind, id, expected })
            .await
    }
    async fn data_operation(&self, operation: DataOperation) -> DataResult {
        let write = !matches!(operation, DataOperation::InspectDeletion { .. });
        let ticket = match self.begin_capability(Lane::Exclusive) {
            Ok(ticket) => ticket,
            Err(error) => {
                return self
                    .refused_error(error)
                    .map_or_else(|| data_controls::admission(error), Error::data);
            }
        };
        match tauri::async_runtime::spawn_blocking(move || ticket.data(operation)).await {
            Ok(completed) => completed.deliver(),
            Err(_) => {
                self.purge_data_controls();
                if write {
                    Error::WriteUncertain.data()
                } else {
                    Error::Store(StoreError::Unavailable).data()
                }
            }
        }
    }
    pub(super) fn purge_composition_authoring(&self) -> bool {
        self.0.composition.purge()
    }
    pub(super) async fn preview_composition(
        &self,
        request: rangoon_compose::application::Request,
    ) -> CompositionResult {
        self.composition_operation(CompositionOperation::Preview(request))
            .await
    }
    pub(super) async fn commit_composition(
        &self,
        request: composition::Confirmation,
    ) -> CompositionResult {
        self.composition_operation(CompositionOperation::Commit(request))
            .await
    }
    async fn composition_operation(&self, operation: CompositionOperation) -> CompositionResult {
        let write = matches!(operation, CompositionOperation::Commit(_));
        let ticket = match self.begin_capability(Lane::Exclusive) {
            Ok(ticket) => ticket,
            Err(AdmissionError::Busy) => return composition::busy(),
            Err(error @ AdmissionError::Unavailable) => {
                return self
                    .refused_error(error)
                    .unwrap_or(Error::Store(StoreError::Unavailable))
                    .composition();
            }
        };
        match tauri::async_runtime::spawn_blocking(move || ticket.composition(operation)).await {
            Ok(completed) => completed.deliver(),
            Err(_) => {
                self.purge_composition_authoring();
                if write {
                    Error::WriteUncertain.composition()
                } else {
                    Error::Store(StoreError::Unavailable).composition()
                }
            }
        }
    }
    pub(super) async fn compile_capability(
        &self,
        request: compilation::Request,
    ) -> CompilationResult {
        let ticket = match self.begin_capability(Lane::Exclusive) {
            Ok(ticket) => ticket,
            Err(AdmissionError::Busy) => {
                return CompilationResult::Failed {
                    error: PublicError {
                        code: "workspace_busy",
                        message: "Finish the current local workspace operation before compiling.",
                    },
                };
            }
            Err(error @ AdmissionError::Unavailable) => {
                return self
                    .refused_error(error)
                    .unwrap_or(Error::Store(StoreError::Unavailable))
                    .compilation();
            }
        };
        match tauri::async_runtime::spawn_blocking(move || {
            let result = ticket
                .call(|backend| {
                    backend.compile_capability(
                        &request.capability_id,
                        &request.revision_id,
                        request.profile,
                    )
                })
                .and_then(|result| compilation::compiled(result).map_err(Error::Store));
            CompilationCompletion {
                ticket,
                result: Some(result),
            }
        })
        .await
        {
            Ok(completed) => completed.deliver(),
            Err(_) => Error::Store(StoreError::Unavailable).compilation(),
        }
    }
    pub(super) fn purge_workflow_authoring(&self) -> bool {
        self.0.workflow.purge()
    }
    pub(super) async fn list_workflows(&self) -> WorkflowResult {
        self.workflow_operation(WorkflowOperation::List).await
    }
    pub(super) async fn open_workflow(&self, request: OpenRequest) -> WorkflowResult {
        self.workflow_operation(WorkflowOperation::Open(request))
            .await
    }
    pub(super) async fn begin_workflow_draft(&self, request: BeginRequest) -> WorkflowResult {
        self.workflow_operation(WorkflowOperation::Begin(request))
            .await
    }
    pub(super) async fn inspect_workflow_save(&self, request: InspectRequest) -> WorkflowResult {
        self.workflow_operation(WorkflowOperation::Inspect(request))
            .await
    }
    pub(super) async fn commit_workflow_save(&self, request: Confirmation) -> WorkflowResult {
        self.workflow_operation(WorkflowOperation::Commit(request))
            .await
    }
    pub(super) async fn clear_workflow_draft(&self, request: ClearRequest) -> WorkflowResult {
        self.workflow_operation(WorkflowOperation::Clear(request))
            .await
    }
    async fn workflow_operation(&self, operation: WorkflowOperation) -> WorkflowResult {
        let write = matches!(operation, WorkflowOperation::Commit(_));
        let read = matches!(
            operation,
            WorkflowOperation::List | WorkflowOperation::Open(_)
        );
        let ticket = match self.begin_capability(if read { Lane::Read } else { Lane::Exclusive }) {
            Ok(ticket) => ticket,
            Err(error) => {
                if write && self.failure_latched() {
                    return workflows::failure(StoreError::WorkspaceChanged);
                }
                return self
                    .refused_error(error)
                    .map_or_else(|| admission_workflow(error, !read), Error::workflow);
            }
        };
        match tauri::async_runtime::spawn_blocking(move || ticket.workflow(operation)).await {
            Ok(completed) => completed.deliver(),
            Err(_) => {
                self.purge_workflow_authoring();
                if write {
                    Error::WriteUncertain.workflow()
                } else {
                    Error::Store(StoreError::Unavailable).workflow()
                }
            }
        }
    }
    pub(super) async fn workflow_data(&self) -> DataResult {
        let ticket = match self.begin_capability(Lane::Read) {
            Ok(ticket) => ticket,
            Err(error) => {
                return match error {
                    AdmissionError::Busy => DataResult::Failed {
                        error: PublicError {
                            code: "workspace_busy",
                            message: "Finish the current local workspace operation first.",
                        },
                    },
                    AdmissionError::Unavailable => self
                        .refused_error(error)
                        .unwrap_or(Error::Store(StoreError::Unavailable))
                        .data(),
                };
            }
        };
        match tauri::async_runtime::spawn_blocking(move || {
            let result = ticket.call(|backend| backend.workflow_data());
            WorkflowInventoryCompletion {
                ticket,
                result: Some(result),
            }
        })
        .await
        {
            Ok(completed) => completed.deliver(),
            Err(_) => {
                self.purge_workflow_authoring();
                DataResult::Failed {
                    error: Error::Store(StoreError::Unavailable).public(),
                }
            }
        }
    }
    pub(super) fn begin_selection(&self) -> Result<SelectionOperation, AdmissionError> {
        self.begin(Lane::Exclusive).map(SelectionOperation)
    }
    pub(super) fn clear_analysis(&self) -> WorkspaceResult {
        let _lease = match self.admit_open(Lane::Read) {
            Ok(lease) => lease,
            Err(error) => {
                return admission_workspace(
                    error,
                    "workspace_busy",
                    "Finish the current local workspace operation first.",
                );
            }
        };
        if self.0.session.clear() {
            WorkspaceResult::Cleared
        } else {
            Error::Store(StoreError::Unavailable).workspace()
        }
    }
    pub(super) async fn list_snapshots(&self) -> WorkspaceResult {
        let ticket = match self.begin(Lane::Read) {
            Ok(ticket) => ticket,
            Err(error) => {
                if let Some(reason) = self.refused_error(error) {
                    return reason.workspace();
                }
                return admission_workspace(
                    error,
                    "workspace_busy",
                    "Finish the current local workspace operation first.",
                );
            }
        };
        match tauri::async_runtime::spawn_blocking(move || ticket.read(|backend| backend.list()))
            .await
        {
            Ok(completed) => match completed.deliver() {
                Ok(snapshots) => WorkspaceResult::Listed { snapshots },
                Err(error) => error.workspace(),
            },
            Err(_) => Error::Store(StoreError::Unavailable).workspace(),
        }
    }
    pub(super) async fn open_snapshot(&self, id: String) -> SelectionResult {
        self.open(id, true).await
    }
    pub(super) async fn read_composition_source(&self, id: String) -> SelectionResult {
        self.open(id, false).await
    }
    async fn open(&self, id: String, select: bool) -> SelectionResult {
        if rangoon_store::validate_id(&id).is_err() {
            return Error::Store(StoreError::InvalidId).selection();
        }
        let ticket = match self.begin(Lane::Exclusive) {
            Ok(ticket) => ticket,
            Err(error) => {
                return self
                    .refused_error(error)
                    .map_or_else(|| admission_selection(error), Error::selection);
            }
        };
        match tauri::async_runtime::spawn_blocking(move || {
            let completed = ticket.read(|backend| backend.open(&id));
            completed.into_selection(select)
        })
        .await
        {
            Ok(completed) => completed.deliver(),
            Err(_) => Error::Store(StoreError::Unavailable).selection(),
        }
    }
    pub(super) async fn save_analysis(&self, id: String) -> WorkspaceResult {
        if rangoon_store::validate_id(&id).is_err() {
            return Error::Store(StoreError::InvalidId).workspace();
        }
        let ticket = match self.begin(Lane::Exclusive) {
            Ok(ticket) => ticket,
            Err(error) => {
                if let Some(reason) = self.refused_error(error) {
                    return reason.workspace();
                }
                return admission_workspace(
                    error,
                    "analysis_busy",
                    "Finish the current source operation first.",
                );
            }
        };
        let Some(report) = self.0.session.snapshot(&id) else {
            return WorkspaceResult::Failed {
                error: PublicError {
                    code: "analysis_changed",
                    message: "The current analysis changed. Choose or open the source before saving.",
                },
            };
        };
        match tauri::async_runtime::spawn_blocking(move || ticket.save(&report)).await {
            Ok(completed) => completed.deliver(),
            // A panic or lost completion cannot establish whether a write happened.
            Err(_) => Error::WriteUncertain.workspace(),
        }
    }
}

// Capability work belongs to the workspace, not the selected Import analysis.
struct CapabilityTicket {
    access: SourceAccess,
    lease: Lease,
}
impl CapabilityTicket {
    fn current(&self) -> bool {
        self.lease.is_current().unwrap_or(false)
    }
    fn call<T>(
        &self,
        action: impl FnOnce(&mut dyn Backend) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.access.call_backend(
            || self.current(),
            action,
            Error::Store(StoreError::Unavailable),
        )
    }
    fn read<T>(
        self,
        action: impl FnOnce(&mut dyn Backend) -> Result<T, Error>,
    ) -> CapabilityReadCompletion<T> {
        let result = self.call(action);
        CapabilityReadCompletion {
            ticket: self,
            result,
        }
    }
    fn mutate(
        self,
        action: impl FnOnce(&mut dyn Backend) -> Result<CapabilityReceiptV1, Error>,
    ) -> CapabilityWriteCompletion {
        let result = self.call(action);
        CapabilityWriteCompletion {
            ticket: self,
            result,
        }
    }
}
pub(super) struct ModelInputOperation(CapabilityTicket);
struct BackendInputs<'a> {
    ticket: &'a CapabilityTicket,
    backend: &'a mut dyn Backend,
}
impl rangoon_model_session::InputReader for BackendInputs<'_> {
    fn read_source(
        &mut self,
        id: &str,
    ) -> Result<AnalysisReport, rangoon_model_session::Diagnostic> {
        if !self.ticket.current() {
            return Err(rangoon_model_session::Diagnostic::InputUnavailable);
        }
        self.backend
            .open(id)
            .map_err(|_| rangoon_model_session::Diagnostic::InputUnavailable)
    }
    fn read_capability(
        &mut self,
        id: &str,
        revision: &str,
    ) -> Result<CapabilityDetail, rangoon_model_session::Diagnostic> {
        if !self.ticket.current() {
            return Err(rangoon_model_session::Diagnostic::InputUnavailable);
        }
        self.backend
            .open_capability(id, Some(revision))
            .map_err(|_| rangoon_model_session::Diagnostic::InputUnavailable)
    }
}
impl ModelInputOperation {
    pub(super) fn with_inputs<T>(
        self,
        action: impl FnOnce(
            &mut dyn rangoon_model_session::InputReader,
        ) -> Result<T, rangoon_model_session::Diagnostic>,
    ) -> ModelInputCompletion<T> {
        // Construct completion before callback entry, so unwinding drops any
        // retained preparation before releasing the workspace lease.
        let mut completed = ModelInputCompletion {
            result: Err(rangoon_model_session::Diagnostic::InputUnavailable),
            ticket: self.0,
        };
        completed.result = completed
            .ticket
            .call(|backend| {
                Ok(action(&mut BackendInputs {
                    ticket: &completed.ticket,
                    backend,
                }))
            })
            .map_err(|_| rangoon_model_session::Diagnostic::InputUnavailable)
            .and_then(|result| result);
        completed
    }
}
pub(super) struct ModelInputCompletion<T> {
    // Field order matters: unobserved retained custody drops before the lease.
    result: Result<T, rangoon_model_session::Diagnostic>,
    ticket: CapabilityTicket,
}
impl<T> ModelInputCompletion<T> {
    pub(super) fn deliver(self) -> Result<T, rangoon_model_session::Diagnostic> {
        if !self.ticket.current() && !(self.ticket.access.failure_latched() && self.result.is_err())
        {
            drop(self.result);
            return Err(rangoon_model_session::Diagnostic::Cancelled);
        }
        self.result
    }
    pub(super) fn deliver_using<U>(
        self,
        apply: impl FnOnce(&mut T) -> Result<U, rangoon_model_session::Diagnostic>,
        publish: impl FnOnce(&mut T) -> Result<(), rangoon_model_session::Diagnostic>,
    ) -> Result<U, rangoon_model_session::Diagnostic> {
        let mut value = self.result?;
        if !self.ticket.current() {
            return Err(rangoon_model_session::Diagnostic::Cancelled);
        }
        let result = apply(&mut value)?;
        if !self.ticket.current() {
            return Err(rangoon_model_session::Diagnostic::Cancelled);
        }
        publish(&mut value)?;
        Ok(result)
    }
}

/// Picker/file callbacks carry the same real lease as typed workspace access.
pub(super) struct FileOperation(CapabilityTicket);
impl FileOperation {
    fn current(&self) -> Result<(), Error> {
        if self.0.current() {
            Ok(())
        } else {
            Err(Error::Store(StoreError::Unavailable))
        }
    }
    fn pick(
        &self,
        pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>,
    ) -> Result<Option<PathBuf>, Error> {
        self.current()?;
        let path = pick().map_err(Error::from_host)?;
        self.current()?;
        Ok(path)
    }
    pub(super) fn export_instruction(
        self,
        request: instruction_bundles::ExportRequest,
        pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>,
        write: impl FnOnce(&Path, &[u8]) -> Result<(), PublicError>,
    ) -> BundleCompletion {
        let result = (|| {
            let Some(path) = self.pick(pick)? else {
                return Ok(BundleResult::Cancelled);
            };
            let compiled = self.0.call(|backend| {
                backend.compile_capability(
                    &request.capability_id,
                    &request.revision_id,
                    request.profile,
                )
            })?;
            self.current()?;
            Ok(instruction_bundles::export_compiled(
                &request,
                &path,
                compiled.compilation,
                |path, bytes| {
                    self.current().map_err(Error::public)?;
                    write(path, bytes)
                },
            ))
        })();
        BundleCompletion {
            ticket: self.0,
            result: Some(result),
        }
    }
    pub(super) fn inspect_instruction(
        self,
        pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>,
        read: impl FnOnce(&Path) -> Result<Vec<u8>, PublicError>,
    ) -> BundleCompletion {
        let result = (|| {
            let Some(path) = self.pick(pick)? else {
                return Ok(BundleResult::Cancelled);
            };
            self.current()?;
            let bytes = read(&path).map_err(Error::from_host)?;
            self.current()?;
            Ok(instruction_bundles::inspect_bytes(&bytes))
        })();
        BundleCompletion {
            ticket: self.0,
            result: Some(result),
        }
    }
    pub(super) fn export_backup(
        self,
        pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>,
        write: impl FnOnce(&Path, &[u8]) -> Result<(), PublicError>,
    ) -> DataCompletion {
        let result = (|| {
            if !self
                .0
                .call(|backend| Ok(backend.plaintext_backup_export_allowed()))?
            {
                return Err(Error::EncryptedRecoveryUnavailable);
            }
            let Some(path) = self.pick(pick)? else {
                return Ok(DataResult::Cancelled);
            };
            let bytes = self.0.call(|backend| backend.export_workflow_backup())?;
            self.current()?;
            write(&path, &bytes).map_err(Error::from_host)?;
            Ok(DataResult::Exported {
                backup_id: format!("backup:{}", rangoon_domain::byte_digest(&bytes)),
                byte_length: bytes.len(),
            })
        })();
        DataCompletion {
            ticket: self.0,
            result: Some(result),
            authoring: false,
            write: false,
        }
    }
    pub(super) fn prepare_restore(
        self,
        pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>,
        read: impl FnOnce(&Path) -> Result<Vec<u8>, PublicError>,
    ) -> DataCompletion {
        let result = (|| {
            let Some(path) = self.pick(pick)? else {
                return Ok(DataResult::Cancelled);
            };
            self.current()?;
            let bytes = read(&path).map_err(Error::from_host)?;
            self.current()?;
            let backup = Backup::decode(&bytes)?;
            drop(bytes);
            let plan = self
                .0
                .call(|backend| backend.prepare_workflow_restore(&backup))?;
            self.current()?;
            self.0
                .access
                .0
                .backup
                .stage_routed(backup, plan.clone(), self.0.lease.witness())?;
            Ok(DataResult::RestoreReady { plan })
        })();
        DataCompletion {
            ticket: self.0,
            result: Some(result),
            authoring: true,
            write: false,
        }
    }
}
pub(super) struct BundleCompletion {
    ticket: CapabilityTicket,
    result: Option<Result<BundleResult, Error>>,
}
impl BundleCompletion {
    pub(super) fn deliver(mut self) -> BundleResult {
        let result = self
            .result
            .take()
            .unwrap_or(Err(Error::Store(StoreError::Unavailable)));
        if self.ticket.access.failure_latched()
            && let Err(error) = &result
        {
            self.ticket.access.purge_data_controls();
            return error.bundle();
        }
        if !self.ticket.current() {
            self.ticket.access.purge_data_controls();
            return match result {
                Ok(BundleResult::Exported { .. }) => BundleResult::Failed {
                    error: PublicError {
                        code: "instruction_bundle_export_completed",
                        message: "Export completed before the workspace closed. Closing did not undo the file; inspect your selected output.",
                    },
                    output_state: OutputState::Uncertain,
                },
                Ok(
                    failed @ BundleResult::Failed {
                        output_state: OutputState::Uncertain,
                        ..
                    },
                ) => failed,
                _ => Error::Store(StoreError::Unavailable).bundle(),
            };
        }
        if result.as_ref().is_err_and(|error| error.managed_failure()) {
            self.ticket.access.purge_data_controls();
        }
        match result {
            Ok(result) => result,
            Err(error) => error.bundle(),
        }
    }
}
impl Drop for BundleCompletion {
    fn drop(&mut self) {
        let failed = self
            .result
            .as_ref()
            .is_some_and(|result| result.as_ref().is_err_and(|error| error.managed_failure()));
        if !self.ticket.current() || failed {
            self.ticket.access.purge_data_controls();
        }
    }
}
enum DataOperation {
    Restore {
        backup_id: String,
        expected: String,
    },
    InspectDeletion {
        kind: NativeRecordKind,
        id: String,
    },
    Delete {
        kind: NativeRecordKind,
        id: String,
        expected: String,
    },
}
impl CapabilityTicket {
    fn data(self, operation: DataOperation) -> DataCompletion {
        let write = !matches!(operation, DataOperation::InspectDeletion { .. });
        let result = (|| {
            if !self.current() {
                return Err(Error::Store(StoreError::Unavailable));
            }
            match operation {
                DataOperation::Restore {
                    backup_id,
                    expected,
                } => {
                    let (backup, plan) =
                        self.access.0.backup.claim_routed(&backup_id, &expected)?;
                    self.call(|backend| backend.restore_workflow_backup(&backup, &plan))?;
                    Ok(DataResult::Restored { plan })
                }
                DataOperation::InspectDeletion { kind, id } => {
                    let plan =
                        self.call(|backend| backend.inspect_workflow_deletion(kind.store(), &id))?;
                    self.access
                        .0
                        .deletion
                        .stage_routed(plan.clone(), self.lease.witness())?;
                    Ok(DataResult::DeletionReady { plan })
                }
                DataOperation::Delete { kind, id, expected } => {
                    let plan = self
                        .access
                        .0
                        .deletion
                        .claim_routed(kind.store(), &id, &expected)?;
                    self.call(|backend| backend.delete_workflow_record(&plan))?;
                    Ok(DataResult::Deleted { kind, id })
                }
            }
        })();
        DataCompletion {
            ticket: self,
            result: Some(result),
            authoring: true,
            write,
        }
    }
}
pub(super) struct DataCompletion {
    ticket: CapabilityTicket,
    result: Option<Result<DataResult, Error>>,
    authoring: bool,
    write: bool,
}
impl DataCompletion {
    pub(super) fn deliver(mut self) -> DataResult {
        let current = self.ticket.current();
        let result = self
            .result
            .take()
            .unwrap_or(Err(Error::Store(StoreError::Unavailable)));
        if !current || result.as_ref().is_err_and(|error| error.managed_failure()) {
            self.ticket.access.purge_data_controls();
        }
        match result {
            Ok(DataResult::Restored { .. } | DataResult::Deleted { .. })
                if self.write && !current =>
            {
                Error::WriteCompleted.data()
            }
            Ok(DataResult::Exported { .. }) if !current => DataResult::Failed {
                error: PublicError {
                    code: "workspace_export_completed",
                    message: "Export completed before the workspace closed. Closing did not undo the file; inspect your selected output.",
                },
            },
            Ok(_) if !current => Error::Store(StoreError::Unavailable).data(),
            Ok(result) => result,
            Err(error) => error.data(),
        }
    }
}
impl Drop for DataCompletion {
    fn drop(&mut self) {
        let failed = self
            .result
            .as_ref()
            .is_some_and(|result| result.as_ref().is_err_and(|error| error.managed_failure()));
        if !self.ticket.current() || failed || (self.authoring && self.result.is_some()) {
            self.ticket.access.purge_data_controls();
        }
    }
}

enum CompositionOperation {
    Preview(rangoon_compose::application::Request),
    Commit(composition::Confirmation),
}
impl CapabilityTicket {
    fn composition(self, operation: CompositionOperation) -> CompositionCompletion {
        let write = matches!(operation, CompositionOperation::Commit(_));
        let result = (|| {
            if !self.current() {
                return Err(Error::Store(StoreError::Unavailable));
            }
            match operation {
                CompositionOperation::Preview(request) => self.call(|backend| {
                    let prepared = backend.prepare_composition(&request)?;
                    self.access
                        .0
                        .composition
                        .stage_routed(prepared, self.lease.witness())
                        .map_err(Error::Store)
                }),
                CompositionOperation::Commit(confirmation) => {
                    let handle = self.access.0.composition.claim(&confirmation)?;
                    self.call(|backend| {
                        backend.confirm_composition(handle, confirmation.acknowledged)
                    })
                    .map(|receipt| CompositionResult::Committed { receipt })
                }
            }
        })();
        CompositionCompletion {
            ticket: self,
            result: Some(result),
            write,
        }
    }
}
struct CompositionCompletion {
    ticket: CapabilityTicket,
    result: Option<Result<CompositionResult, Error>>,
    write: bool,
}
impl CompositionCompletion {
    fn deliver(mut self) -> CompositionResult {
        let current = self.ticket.current();
        let result = self
            .result
            .take()
            .unwrap_or(Err(Error::Store(StoreError::Unavailable)));
        if !current || result.as_ref().is_err_and(|error| error.managed_failure()) {
            self.ticket.access.purge_composition_authoring();
        }
        match result {
            Ok(CompositionResult::Committed { .. }) if self.write && !current => {
                Error::WriteCompleted.composition()
            }
            Ok(_) if !current => Error::Store(StoreError::Unavailable).composition(),
            Ok(result) => result,
            Err(error) => error.composition(),
        }
    }
}
impl Drop for CompositionCompletion {
    fn drop(&mut self) {
        let failed_managed = self
            .result
            .as_ref()
            .is_some_and(|result| result.as_ref().is_err_and(|error| error.managed_failure()));
        if !self.ticket.current() || failed_managed || self.result.is_some() {
            self.ticket.access.purge_composition_authoring();
        }
    }
}
struct CompilationCompletion {
    ticket: CapabilityTicket,
    result: Option<Result<CompilationResult, Error>>,
}
impl CompilationCompletion {
    fn deliver(mut self) -> CompilationResult {
        let result = self
            .result
            .take()
            .unwrap_or(Err(Error::Store(StoreError::Unavailable)));
        if self.ticket.access.failure_latched()
            && let Err(error) = &result
        {
            self.ticket.access.purge_composition_authoring();
            return error.compilation();
        }
        if !self.ticket.current() {
            self.ticket.access.purge_composition_authoring();
            return Error::Store(StoreError::Unavailable).compilation();
        }
        if result.as_ref().is_err_and(|error| error.managed_failure()) {
            self.ticket.access.purge_composition_authoring();
        }
        match result {
            Ok(result) => result,
            Err(error) => error.compilation(),
        }
    }
}
impl Drop for CompilationCompletion {
    fn drop(&mut self) {
        let failed_managed = self
            .result
            .as_ref()
            .is_some_and(|result| result.as_ref().is_err_and(|error| error.managed_failure()));
        if !self.ticket.current() || failed_managed {
            self.ticket.access.purge_composition_authoring();
        }
    }
}
enum WorkflowOperation {
    List,
    Open(OpenRequest),
    Begin(BeginRequest),
    Inspect(InspectRequest),
    Commit(Confirmation),
    Clear(ClearRequest),
}
impl CapabilityTicket {
    fn workflow(self, operation: WorkflowOperation) -> WorkflowCompletion {
        let write = matches!(operation, WorkflowOperation::Commit(_));
        let authoring = !matches!(
            operation,
            WorkflowOperation::List | WorkflowOperation::Open(_)
        );
        let session = &self.access.0.workflow;
        let result = (|| {
            if !self.current() {
                return Err(Error::Store(StoreError::Unavailable));
            }
            match operation {
                WorkflowOperation::List => self
                    .call(|backend| backend.list_workflows())
                    .map(|workflows| WorkflowResult::Listed { workflows }),
                WorkflowOperation::Open(request) => {
                    let revision = match &request.selection {
                        Selection::Head {} => None,
                        Selection::Historical { revision_id } => Some(revision_id.as_str()),
                    };
                    self.call(|backend| backend.open_workflow(&request.workflow_id, revision))
                        .map(|workflow| WorkflowResult::Opened {
                            workflow: Box::new(workflow),
                        })
                }
                WorkflowOperation::Begin(request) => self.call(|backend| {
                    session.begin_routed(request.target, self.lease.witness(), |id, new| {
                        if new {
                            if backend
                                .list_workflows()?
                                .iter()
                                .any(|workflow| workflow.id == id)
                            {
                                return Err(Error::Store(StoreError::WorkflowConflict));
                            }
                            Ok(None)
                        } else {
                            backend.open_workflow(id, None).map(Some)
                        }
                    })
                }),
                WorkflowOperation::Inspect(request) => self.call(|backend| {
                    session.inspect_using(request, |candidate| {
                        backend.inspect_workflow_save(candidate)
                    })
                }),
                WorkflowOperation::Commit(confirmation) => {
                    let prepared = session.claim(&confirmation)?;
                    self.call(|backend| {
                        let current = backend.inspect_workflow_save(&prepared.candidate)?;
                        prepared.check_current(&current)?;
                        // A checked preview cannot authorize a write after closure.
                        if !self.current() {
                            return Err(Error::Store(StoreError::Unavailable));
                        }
                        backend.save_workflow(&prepared.candidate, &prepared.plan.expected_state_id)
                    })
                    .map(|receipt| WorkflowResult::Saved {
                        receipt: Box::new(receipt),
                    })
                }
                WorkflowOperation::Clear(request) => {
                    session.clear(&request.draft_id).map_err(Error::Store)
                }
            }
        })();
        WorkflowCompletion {
            ticket: self,
            result: Some(result),
            write,
            authoring,
        }
    }
}
struct WorkflowCompletion {
    ticket: CapabilityTicket,
    result: Option<Result<WorkflowResult, Error>>,
    write: bool,
    authoring: bool,
}
impl WorkflowCompletion {
    fn deliver(mut self) -> WorkflowResult {
        let current = self.ticket.current();
        let result = self
            .result
            .take()
            .unwrap_or(Err(Error::Store(StoreError::Unavailable)));
        if !current || result.as_ref().is_err_and(|error| error.managed_failure()) {
            self.ticket.access.purge_workflow_authoring();
        }
        match result {
            Ok(WorkflowResult::Saved { .. }) if self.write && !current => {
                Error::WriteCompleted.workflow()
            }
            Ok(_) if !current => Error::Store(StoreError::Unavailable).workflow(),
            Ok(result) => result,
            Err(error) => error.workflow(),
        }
    }
}
impl Drop for WorkflowCompletion {
    fn drop(&mut self) {
        let failed_managed = self
            .result
            .as_ref()
            .is_some_and(|result| result.as_ref().is_err_and(|error| error.managed_failure()));
        // Unobserved authoring outputs cannot leave reusable native handles.
        // Read-only success never clears a newer draft while the Gate is open.
        if !self.ticket.current() || failed_managed || (self.authoring && self.result.is_some()) {
            self.ticket.access.purge_workflow_authoring();
        }
    }
}
struct WorkflowInventoryCompletion {
    ticket: CapabilityTicket,
    result: Option<Result<WorkflowWorkspaceData, Error>>,
}
impl WorkflowInventoryCompletion {
    fn deliver(mut self) -> DataResult {
        let result = self
            .result
            .take()
            .unwrap_or(Err(Error::Store(StoreError::Unavailable)));
        if self.ticket.access.failure_latched()
            && let Err(error) = &result
        {
            self.ticket.access.purge_workflow_authoring();
            return error.data();
        }
        if !self.ticket.current() {
            self.ticket.access.purge_workflow_authoring();
            return DataResult::Failed {
                error: Error::Store(StoreError::Unavailable).public(),
            };
        }
        if result.as_ref().is_err_and(|error| error.managed_failure()) {
            self.ticket.access.purge_workflow_authoring();
        }
        match result {
            Ok(workspace) => DataResult::Loaded { workspace },
            Err(error) => DataResult::Failed {
                error: error.public(),
            },
        }
    }
}
impl Drop for WorkflowInventoryCompletion {
    fn drop(&mut self) {
        let failed_managed = self
            .result
            .as_ref()
            .is_some_and(|result| result.as_ref().is_err_and(|error| error.managed_failure()));
        if !self.ticket.current() || failed_managed {
            self.ticket.access.purge_workflow_authoring();
        }
    }
}
fn admission_workflow(error: AdmissionError, exclusive: bool) -> WorkflowResult {
    match error {
        AdmissionError::Busy => WorkflowResult::Failed {
            error: PublicError {
                code: "workspace_busy",
                message: if exclusive {
                    "Finish the current local workspace operation first. Your workflow draft remains available."
                } else {
                    "Finish the current local workspace operation first."
                },
            },
        },
        AdmissionError::Unavailable => Error::Store(StoreError::Unavailable).workflow(),
    }
}

struct CapabilityReadCompletion<T> {
    ticket: CapabilityTicket,
    result: Result<T, Error>,
}
impl<T> CapabilityReadCompletion<T> {
    fn deliver(self) -> Result<T, Error> {
        if !self.ticket.current() && !(self.ticket.access.failure_latched() && self.result.is_err())
        {
            return Err(Error::Store(StoreError::Unavailable));
        }
        self.result
    }
}
struct CapabilityWriteCompletion {
    ticket: CapabilityTicket,
    result: Result<CapabilityReceiptV1, Error>,
}
impl CapabilityWriteCompletion {
    fn deliver(self) -> CapabilityResult {
        match self.result {
            Ok(receipt) if self.ticket.current() => CapabilityResult::Opened {
                capability: Box::new(receipt.capability),
                already_applied: receipt.already_applied,
            },
            Ok(_) => Error::WriteCompleted.capability(),
            Err(error) => error.capability(),
        }
    }
}
enum CapabilityMutation {
    Create {
        source: String,
        fragment: String,
        title: String,
    },
    Revise {
        id: String,
        expected: String,
        title: String,
        content: String,
    },
    Review {
        id: String,
        expected: String,
    },
}
impl CapabilityMutation {
    fn apply(self, backend: &mut dyn Backend) -> Result<CapabilityReceiptV1, Error> {
        match self {
            Self::Create {
                source,
                fragment,
                title,
            } => backend.create_capability(&source, &fragment, &title),
            Self::Revise {
                id,
                expected,
                title,
                content,
            } => backend.revise_capability(&id, &expected, &title, &content),
            Self::Review { id, expected } => backend.review_capability(&id, &expected),
        }
    }
}
fn admission_capability(error: AdmissionError, mutation: bool) -> CapabilityResult {
    match error {
        AdmissionError::Busy => CapabilityResult::Failed {
            error: PublicError {
                code: "workspace_busy",
                message: if mutation {
                    "Finish the current local workspace operation first. Your draft remains available."
                } else {
                    "Finish the current local workspace operation first."
                },
            },
        },
        AdmissionError::Unavailable => Error::Store(StoreError::Unavailable).capability(),
    }
}

fn admission_workspace(
    error: AdmissionError,
    code: &'static str,
    message: &'static str,
) -> WorkspaceResult {
    match error {
        AdmissionError::Busy => WorkspaceResult::Failed {
            error: PublicError { code, message },
        },
        AdmissionError::Unavailable => Error::Store(StoreError::Unavailable).workspace(),
    }
}
fn admission_selection(error: AdmissionError) -> SelectionResult {
    match error {
        AdmissionError::Busy => SelectionResult::failed(
            "analysis_busy",
            "Finish the current source operation first.",
        ),
        AdmissionError::Unavailable => SelectionResult::failed(
            "session_unavailable",
            "Reopen Rangoon to restore the source workbench.",
        ),
    }
}
struct Ticket {
    access: SourceAccess,
    lease: Lease,
    generation: u64,
}
impl Ticket {
    fn current(&self) -> bool {
        self.lease.is_current().unwrap_or(false)
            && self.access.0.session.generation() == Some(self.generation)
    }
    fn read<T>(
        self,
        action: impl FnOnce(&mut dyn Backend) -> Result<T, Error>,
    ) -> ReadCompletion<T> {
        let result = (|| {
            if !self.current() {
                return Err(Error::Cleared);
            }
            self.access.call_backend(
                || self.current(),
                action,
                Error::Store(StoreError::Unavailable),
            )
        })();
        ReadCompletion {
            ticket: self,
            result,
        }
    }
    fn save(self, report: &AnalysisReport) -> WriteCompletion {
        // Captured saves survive source Clear; only workspace admission fences them.
        let result = self.access.call_backend(
            || self.lease.is_current().unwrap_or(false),
            |backend| backend.save(report),
            Error::WriteUncertain,
        );
        WriteCompletion {
            ticket: self,
            result,
        }
    }
}
struct ReadCompletion<T> {
    ticket: Ticket,
    result: Result<T, Error>,
}
impl<T> ReadCompletion<T> {
    fn deliver(self) -> Result<T, Error> {
        if !self.ticket.current() && !(self.ticket.access.failure_latched() && self.result.is_err())
        {
            return Err(Error::Cleared);
        }
        self.result
    }
}
impl ReadCompletion<AnalysisReport> {
    fn into_selection(self, select: bool) -> SelectionCompletion {
        let result = match self.result {
            Ok(report) => SelectionResult::Analyzed {
                report: Box::new(report),
            },
            Err(error) => error.selection(),
        };
        SelectionCompletion {
            ticket: self.ticket,
            result,
            select,
        }
    }
}
pub(super) struct SelectionOperation(Ticket);
impl SelectionOperation {
    pub(super) fn is_current(&self) -> bool {
        self.0.current()
    }
    pub(super) fn complete(self, result: SelectionResult) -> SelectionCompletion {
        SelectionCompletion {
            ticket: self.0,
            result,
            select: true,
        }
    }
}
pub(super) struct SelectionCompletion {
    ticket: Ticket,
    result: SelectionResult,
    select: bool,
}
impl SelectionCompletion {
    pub(super) fn deliver(self) -> SelectionResult {
        if !self.ticket.current()
            && !(self.ticket.access.failure_latched()
                && matches!(self.result, SelectionResult::Failed { .. }))
        {
            return Error::Cleared.selection();
        }
        if self.ticket.access.failure_latched()
            && matches!(self.result, SelectionResult::Failed { .. })
        {
            return self.result;
        }
        if self.select
            && let SelectionResult::Analyzed { ref report } = self.result
            && !self
                .ticket
                .access
                .0
                .session
                .accept(self.ticket.generation, report)
        {
            return Error::Cleared.selection();
        }
        if !self.ticket.current() {
            if self.select {
                self.ticket.access.0.session.clear();
            }
            return Error::Cleared.selection();
        }
        self.result
    }
}
struct WriteCompletion {
    ticket: Ticket,
    result: Result<SaveReceipt, Error>,
}
impl WriteCompletion {
    fn deliver(self) -> WorkspaceResult {
        match self.result {
            Ok(receipt) if self.ticket.lease.is_current().unwrap_or(false) => {
                WorkspaceResult::Saved {
                    snapshot: receipt.snapshot,
                    already_saved: receipt.already_saved,
                }
            }
            Ok(_) => Error::SaveCompleted.workspace(),
            Err(error) => error.workspace(),
        }
    }
}

#[cfg(test)]
#[path = "workspace_source_access_tests.rs"]
mod tests;
