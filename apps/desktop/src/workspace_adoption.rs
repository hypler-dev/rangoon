//! Native Legacy-to-Managed conversion. No renderer trust or ordinary client issuance.
#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{StartupBinding, StartupPermit, setup::Consent};
#[cfg(feature = "encrypted-workspace")]
use crate::workspace_startup::{EmptyProvisioningProof, Inspection};
use crate::workspace_startup::{StartupRoots, State as StartupState};
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::workspace_keys::{Error as KeyError, Store};
use rangoon_desktop::{
    workspace_keys::WorkspaceId,
    workspace_session::{Owner, Vault},
};
use rangoon_domain::byte_digest;
use rangoon_store::{ExistingWorkspaceCapture, Workspace, WorkspaceBackup};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};
use zeroize::Zeroizing;

#[path = "workspace_adoption_record.rs"]
mod records;
use records::{Inventory, Phase, Record};
const DIRECTORY: &str = "workspace-adoption";
const CONTROL: &str = "adoption.lock";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    UnsupportedPlatform,
    BackendUnavailable,
    Unavailable,
    RecoveryRequired,
    Changed,
    ConsentRequired,
    SameProcess,
    WriteUncertain,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    NoAttempt,
    Interrupted,
    SelectionRecorded,
    LegacyDiverged,
    RecoveryRequired,
    Unavailable,
    UnsupportedPlatform,
}

/// Routing observations only. Destination identity still requires the live
/// existing Owner before any key lookup or ordinary unlock can be offered.
pub(super) enum Route {
    NoAttempt,
    RestartRequired,
    VerifyRequired,
    Selected(Selection),
    Recovery(Recovery),
    RecoveryRequired,
    Unavailable,
}

/// Retains the exact selected journal across native destination binding.
pub(super) struct Selection {
    anchor: PathBuf,
    journal: Journal,
    destination: String,
}
impl Selection {
    pub(super) fn destination(&self) -> &str {
        &self.destination
    }
    pub(super) fn check(&self) -> Result<(), Error> {
        self.journal.check(None)
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn original_matches(&self, permit: &StartupPermit<'_>) -> Result<bool, Error> {
        matches_original(&self.anchor, &self.journal, permit)
    }
}

pub(super) fn route(anchor: &Path) -> Route {
    match fs::symlink_metadata(anchor.join(DIRECTORY)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Route::NoAttempt,
        Err(_) => return Route::Unavailable,
        _ => (),
    }
    let Ok(journal) = Journal::open(anchor) else {
        return Route::RecoveryRequired;
    };
    let Some(last) = journal.records.last().map(|stored| &stored.record) else {
        return Route::RecoveryRequired;
    };
    match last.phase {
        Phase::Copied => match process_identity() {
            Ok(current) if last.creator_process == current => Route::RestartRequired,
            Ok(_) => Route::VerifyRequired,
            Err(_) => Route::Unavailable,
        },
        Phase::Selected => match last.destination.clone() {
            Some(destination) => Route::Selected(Selection {
                anchor: anchor.to_owned(),
                journal,
                destination,
            }),
            None => Route::RecoveryRequired,
        },
        Phase::Target | Phase::Reopened => {
            let phase = last.phase;
            Route::Recovery(Recovery {
                anchor: anchor.to_owned(),
                journal,
                phase,
                #[cfg(feature = "encrypted-workspace")]
                inspection: None,
                #[cfg(feature = "encrypted-workspace")]
                native_binding: None,
            })
        }
        _ => Route::RecoveryRequired,
    }
}

#[cfg(feature = "encrypted-workspace")]
pub(super) fn original_matches(anchor: &Path, permit: &StartupPermit<'_>) -> Result<bool, Error> {
    permit.checkpoint().map_err(|_| Error::Unavailable)?;
    let journal = Journal::open(anchor)?;
    matches_original(anchor, &journal, permit)
}

#[cfg(feature = "encrypted-workspace")]
fn matches_original(
    anchor: &Path,
    journal: &Journal,
    permit: &StartupPermit<'_>,
) -> Result<bool, Error> {
    permit.checkpoint().map_err(|_| Error::Unavailable)?;
    let record = &journal
        .records
        .last()
        .ok_or(Error::RecoveryRequired)?
        .record;
    let mut capture = Workspace::new(StartupRoots::new(anchor.to_owned()).legacy())
        .capture_existing_snapshot()
        .map_err(|_| Error::Changed)?;
    matches(record, &capture)?;
    revalidate(anchor, &mut capture)?;
    journal.check(None)?;
    permit.checkpoint().map_err(|_| Error::Unavailable)?;
    Ok(true)
}

fn process_identity() -> Result<&'static str, Error> {
    static ID: OnceLock<Result<String, Error>> = OnceLock::new();
    ID.get_or_init(|| {
        WorkspaceId::generate()
            .map(|id| id.as_hex())
            .map_err(|_| Error::Unavailable)
    })
    .as_ref()
    .map(String::as_str)
    .map_err(|error| *error)
}
fn identity(left: &Metadata, right: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (left, right);
        false
    }
}
fn regular(metadata: &Metadata, limit: u64) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.nlink() == 1
            && metadata.len() <= limit
    }
    #[cfg(not(unix))]
    {
        let _ = (metadata, limit);
        false
    }
}
fn options(write: bool) -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(write);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    options
}
fn verify(file: &File, path: &Path, limit: u64) -> Result<Metadata, Error> {
    let held = file.metadata().map_err(|_| Error::Unavailable)?;
    let current = fs::symlink_metadata(path).map_err(|_| Error::Changed)?;
    if !regular(&held, limit) || !regular(&current, limit) || !identity(&held, &current) {
        return Err(Error::Changed);
    }
    Ok(held)
}
fn absent(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(Error::RecoveryRequired),
    }
}
fn read_error(error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        return Error::Changed;
    }
    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::ELOOP) {
        return Error::Changed;
    }
    Error::Unavailable
}
fn directory(path: &Path) -> Result<Metadata, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::Changed);
    }
    Ok(metadata)
}
struct Ancestry(Vec<(PathBuf, Metadata)>);
impl Ancestry {
    fn observe(path: &Path) -> Result<Self, Error> {
        if !cfg!(unix) {
            return Err(Error::UnsupportedPlatform);
        }
        if !path.is_absolute()
            || path.components().count() > 64
            || path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(Error::Unavailable);
        }
        let mut proof = Vec::new();
        for path in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
            proof.push((path.to_owned(), directory(path)?));
        }
        let result = Self(proof);
        result.check()?;
        Ok(result)
    }
    fn check(&self) -> Result<(), Error> {
        for (path, held) in &self.0 {
            if !identity(held, &directory(path)?) {
                return Err(Error::Changed);
            }
        }
        Ok(())
    }
}
fn sync(path: &Path) -> Result<(), Error> {
    if !cfg!(unix) {
        return Err(Error::UnsupportedPlatform);
    }
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::WriteUncertain)
}
fn create_directory(path: &Path) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| Error::WriteUncertain)
}

#[cfg(test)]
type Probe = Box<dyn FnMut(&str, &str) -> Result<(), Error>>;
#[cfg(test)]
std::thread_local! {
    static PROBE: std::cell::RefCell<Option<Probe>> = std::cell::RefCell::new(None);
}
fn point(stage: &str, boundary: &str) -> Result<(), Error> {
    #[cfg(all(test, feature = "native-encrypted-qualification", target_os = "macos"))]
    crate::native_encrypted_qualification::adoption_checkpoint(stage, boundary)
        .map_err(|()| Error::WriteUncertain)?;
    #[cfg(test)]
    {
        PROBE.with(|probe| match probe.borrow_mut().as_mut() {
            Some(probe) => probe(stage, boundary),
            None => Ok(()),
        })
    }
    #[cfg(not(test))]
    {
        let _ = (stage, boundary);
        Ok(())
    }
}

struct Stored {
    record: Record,
    bytes: Vec<u8>,
    metadata: Metadata,
}
struct Journal {
    root: PathBuf,
    ancestry: Ancestry,
    control: File,
    records: Vec<Stored>,
}
impl Journal {
    fn create(anchor: &Path) -> Result<Self, Error> {
        let ancestry = Ancestry::observe(anchor)?;
        let root = anchor.join(DIRECTORY);
        absent(&root)?;
        point("journal", "before_root_create")?;
        ancestry.check()?;
        create_directory(&root)?;
        point("journal", "after_root_create")?;
        ancestry.check()?;
        point("journal", "before_parent_sync")?;
        sync(anchor)?;
        point("journal", "after_parent_sync")?;
        let root_proof = Ancestry::observe(&root)?;
        point("journal", "before_lock_create")?;
        root_proof.check()?;
        let control = options(true)
            .create_new(true)
            .open(root.join(CONTROL))
            .map_err(|_| Error::WriteUncertain)?;
        point("journal", "after_lock_create")?;
        verify(&control, &root.join(CONTROL), 0)?;
        control.try_lock().map_err(|_| Error::Unavailable)?;
        point("journal", "before_lock_sync")?;
        control.sync_all().map_err(|_| Error::WriteUncertain)?;
        point("journal", "after_lock_sync")?;
        point("journal", "before_root_sync")?;
        sync(&root)?;
        point("journal", "after_root_sync")?;
        let result = Self {
            root,
            ancestry: root_proof,
            control,
            records: Vec::new(),
        };
        result.check(None)?;
        Ok(result)
    }
    fn open(anchor: &Path) -> Result<Self, Error> {
        let root = anchor.join(DIRECTORY);
        let ancestry = Ancestry::observe(&root)?;
        let control_path = root.join(CONTROL);
        let metadata = fs::symlink_metadata(&control_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Error::RecoveryRequired
            } else {
                Error::Unavailable
            }
        })?;
        if !regular(&metadata, 0) {
            return Err(Error::Changed);
        }
        let control = options(false).open(&control_path).map_err(read_error)?;
        verify(&control, &control_path, 0)?;
        control.try_lock().map_err(|_| Error::Unavailable)?;
        let mut result = Self {
            root,
            ancestry,
            control,
            records: Vec::new(),
        };
        let names = result.names()?;
        for phase in Phase::ALL {
            if !names.iter().any(|name| name == phase.name()) {
                break;
            }
            let (bytes, metadata) = result.read(phase)?;
            let record = Record::decode(&bytes)?;
            result.validate_next(&record)?;
            result.records.push(Stored {
                record,
                bytes,
                metadata,
            });
        }
        result.check(None)?;
        Ok(result)
    }
    fn names(&self) -> Result<Vec<std::ffi::OsString>, Error> {
        let mut names = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_| Error::Unavailable)? {
            if names.len() == 6 {
                return Err(Error::RecoveryRequired);
            }
            names.push(entry.map_err(|_| Error::Unavailable)?.file_name());
        }
        Ok(names)
    }
    fn read(&self, phase: Phase) -> Result<(Vec<u8>, Metadata), Error> {
        let path = self.root.join(phase.name());
        if !regular(
            &fs::symlink_metadata(&path).map_err(read_error)?,
            records::LIMIT as u64,
        ) {
            return Err(Error::Changed);
        }
        let mut file = options(false).open(&path).map_err(read_error)?;
        let before = verify(&file, &path, records::LIMIT as u64)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(records::LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Unavailable)?;
        let after = verify(&file, &path, records::LIMIT as u64)?;
        if !identity(&before, &after)
            || before.len() != after.len()
            || bytes.len() as u64 != after.len()
        {
            return Err(Error::Changed);
        }
        Ok((bytes, after))
    }
    fn check(&self, staged: Option<Phase>) -> Result<(), Error> {
        self.ancestry.check()?;
        verify(&self.control, &self.root.join(CONTROL), 0)?;
        let names = self.names()?;
        if names.len() != 1 + self.records.len() + usize::from(staged.is_some())
            || !names.iter().any(|n| n == CONTROL)
        {
            return Err(Error::Changed);
        }
        for saved in &self.records {
            let (bytes, metadata) = self.read(saved.record.phase)?;
            if bytes != saved.bytes || !identity(&metadata, &saved.metadata) {
                return Err(Error::Changed);
            }
        }
        if staged.is_some_and(|phase| !names.iter().any(|name| name == phase.name())) {
            return Err(Error::Changed);
        }
        Ok(())
    }
    fn validate_next(&self, next: &Record) -> Result<(), Error> {
        if next.phase.index() != self.records.len() {
            return Err(Error::RecoveryRequired);
        }
        if self.records.last().is_some_and(|previous| {
            !next.same_binding(&previous.record)
                || next.previous_digest.as_deref() != Some(byte_digest(&previous.bytes).as_str())
        }) {
            return Err(Error::RecoveryRequired);
        }
        Ok(())
    }
    fn append(&mut self, record: Record) -> Result<(), Error> {
        self.validate_next(&record)?;
        let bytes = record.encode()?;
        let phase = record.phase;
        let path = self.root.join(phase.name());
        point(phase.name(), "before_create")?;
        self.check(None)?;
        let mut file = options(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| Error::WriteUncertain)?;
        let created = verify(&file, &path, records::LIMIT as u64)?;
        point(phase.name(), "after_create")?;
        point(phase.name(), "before_write")?;
        self.check(Some(phase))?;
        verify(&file, &path, records::LIMIT as u64)?;
        file.write_all(&bytes).map_err(|_| Error::WriteUncertain)?;
        point(phase.name(), "after_write")?;
        point(phase.name(), "before_file_sync")?;
        self.check(Some(phase))?;
        verify(&file, &path, records::LIMIT as u64)?;
        file.sync_all().map_err(|_| Error::WriteUncertain)?;
        point(phase.name(), "after_file_sync")?;
        point(phase.name(), "before_directory_sync")?;
        self.check(Some(phase))?;
        sync(&self.root)?;
        point(phase.name(), "after_directory_sync")?;
        point(phase.name(), "before_readback")?;
        self.check(Some(phase))?;
        let (readback, metadata) = self.read(phase)?;
        if readback != bytes || !identity(&metadata, &created) {
            return Err(Error::WriteUncertain);
        }
        point(phase.name(), "after_readback")?;
        self.check(Some(phase))?;
        let (final_bytes, metadata) = self.read(phase)?;
        if final_bytes != bytes || !identity(&metadata, &created) {
            return Err(Error::WriteUncertain);
        }
        self.records.push(Stored {
            record,
            bytes,
            metadata,
        });
        self.check(None)
    }
    fn next(
        &self,
        phase: Phase,
        destination: Option<String>,
        verifier: Option<String>,
    ) -> Result<Record, Error> {
        let previous = self.records.last().ok_or(Error::RecoveryRequired)?;
        let mut record = previous.record.clone();
        record.phase = phase;
        record.previous_digest = Some(byte_digest(&previous.bytes));
        record.destination = destination;
        record.verifier_process = verifier;
        Ok(record)
    }
}

#[cfg(test)]
pub(super) fn inspect(anchor: &Path) -> Status {
    if !cfg!(unix) {
        return Status::UnsupportedPlatform;
    }
    if Ancestry::observe(anchor).is_err() {
        return Status::Unavailable;
    }
    match fs::symlink_metadata(anchor.join(DIRECTORY)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Status::NoAttempt,
        Err(_) => return Status::Unavailable,
        _ => (),
    }
    match Journal::open(anchor) {
        Ok(journal) => match journal.records.last() {
            Some(last) if last.record.phase == Phase::Selected => {
                let captured = Workspace::new(StartupRoots::new(anchor.to_owned()).legacy())
                    .capture_existing_snapshot();
                if captured
                    .as_ref()
                    .is_ok_and(|capture| matches(&last.record, capture).is_ok())
                {
                    Status::SelectionRecorded
                } else {
                    Status::LegacyDiverged
                }
            }
            _ => Status::Interrupted,
        },
        Err(Error::Unavailable) => Status::Unavailable,
        Err(_) => Status::RecoveryRequired,
    }
}

pub(super) struct CopyConsent {
    token: String,
}
pub(super) struct VerifyConsent {
    token: String,
}
pub(super) struct Conversion {
    anchor: PathBuf,
    capture: ExistingWorkspaceCapture,
    intent: Record,
    token: String,
    #[cfg(feature = "encrypted-workspace")]
    native_binding: Option<StartupBinding>,
}
pub(super) struct Verification {
    anchor: PathBuf,
    journal: Journal,
    capture: ExistingWorkspaceCapture,
    owner: Owner,
    preview: String,
    token: String,
    #[cfg(feature = "encrypted-workspace")]
    native_binding: Option<StartupBinding>,
}

/// Metadata custody only until an explicit native recovery action. Source
/// capture, Owner inspection and every effect remain inside that action's Flight.
pub(super) struct Recovery {
    anchor: PathBuf,
    journal: Journal,
    phase: Phase,
    #[cfg(feature = "encrypted-workspace")]
    inspection: Option<RecoveryInspection>,
    #[cfg(feature = "encrypted-workspace")]
    native_binding: Option<StartupBinding>,
}

#[cfg(feature = "encrypted-workspace")]
enum RecoveryInspection {
    Complete(Inspection),
    Empty(EmptyProvisioningProof),
}

#[cfg(feature = "encrypted-workspace")]
impl RecoveryInspection {
    fn check(&self, roots: &StartupRoots) -> Result<(), Error> {
        match self {
            Self::Complete(inspection) => roots.revalidate(inspection).map(|_| ()),
            Self::Empty(proof) => proof.check(),
        }
        .map_err(|_| Error::Changed)
    }
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveryStep {
    Inspect,
    InitializeEmpty,
    Reconcile,
    Unlock,
    Restore,
    RecordCopied,
    CompleteSelection,
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveryOutcome {
    Cancelled,
    Copied,
    Selected,
}

/// Existing-key custody only. Reject the wrong identity before even opening a
/// slot, and never delegate a key write while recovering an existing target.
#[cfg(feature = "encrypted-workspace")]
struct RecoveryVault<'a, V> {
    expected: &'a str,
    vault: &'a V,
}
#[cfg(feature = "encrypted-workspace")]
struct ExistingKeySlot<S>(S);
#[cfg(feature = "encrypted-workspace")]
impl<V: Vault> Vault for RecoveryVault<'_, V> {
    type Slot = ExistingKeySlot<V::Slot>;
    fn open(&self, identity: &WorkspaceId) -> Result<Self::Slot, KeyError> {
        if identity.as_hex() != self.expected {
            return Err(KeyError::Changed);
        }
        self.vault.open(identity).map(ExistingKeySlot)
    }
}
#[cfg(feature = "encrypted-workspace")]
impl<S: Store> Store for ExistingKeySlot<S> {
    fn identity(&self) -> &WorkspaceId {
        self.0.identity()
    }
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, KeyError> {
        self.0.read()
    }
    fn write(&self, _bytes: &[u8]) -> Result<(), KeyError> {
        Err(KeyError::Unavailable)
    }
}

#[cfg(feature = "encrypted-workspace")]
impl Recovery {
    pub(super) fn bind_native(mut self, permit: &StartupPermit<'_>) -> Result<Self, Error> {
        permit.checkpoint().map_err(|_| Error::Unavailable)?;
        self.journal.check(None)?;
        let roots = StartupRoots::new(self.anchor.clone());
        let inspection = roots.inspect();
        let inspection = if inspection.state() == StartupState::Mixed {
            roots.revalidate(&inspection).map_err(|_| Error::Changed)?;
            RecoveryInspection::Complete(inspection)
        } else if self.phase == Phase::Target {
            let destination = self
                .journal
                .records
                .last()
                .and_then(|record| record.record.destination.as_deref())
                .ok_or(Error::RecoveryRequired)?;
            RecoveryInspection::Empty(
                roots
                    .empty_provisioning_candidate(destination)
                    .map_err(|_| Error::RecoveryRequired)?,
            )
        } else {
            return Err(Error::RecoveryRequired);
        };
        self.native_binding = Some(permit.binding().map_err(|_| Error::Unavailable)?);
        self.inspection = Some(inspection);
        Ok(self)
    }
    pub(super) fn revalidate_native(&mut self, permit: &StartupPermit<'_>) -> Result<(), Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        self.journal.check(None)?;
        self.inspection
            .as_ref()
            .ok_or(Error::Unavailable)?
            .check(&StartupRoots::new(self.anchor.clone()))?;
        Ok(())
    }
    fn checkpoint(
        &self,
        permit: &StartupPermit<'_>,
        capture: &mut ExistingWorkspaceCapture,
    ) -> Result<(), Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        self.journal.check(None)?;
        revalidate(&self.anchor, capture)?;
        matches(
            &self
                .journal
                .records
                .last()
                .ok_or(Error::RecoveryRequired)?
                .record,
            capture,
        )?;
        permit.checkpoint().map_err(|_| Error::Unavailable)
    }
    fn ask(
        &self,
        permit: &StartupPermit<'_>,
        capture: &mut ExistingWorkspaceCapture,
        step: RecoveryStep,
        destination: &str,
        confirmation: &mut impl FnMut(RecoveryStep, &str) -> Consent,
    ) -> Result<bool, Error> {
        self.checkpoint(permit, capture)?;
        let consent = confirmation(step, destination);
        self.checkpoint(permit, capture)?;
        match consent {
            Consent::Approved => Ok(true),
            Consent::Cancelled => Ok(false),
            Consent::Unavailable => Err(Error::Unavailable),
        }
    }
    pub(super) fn recover_native(
        mut self,
        permit: &StartupPermit<'_>,
        vault: &impl Vault,
        mut confirmation: impl FnMut(RecoveryStep, &str) -> Consent,
    ) -> Result<RecoveryOutcome, Error> {
        use rangoon_desktop::workspace_session::{Error as OwnerError, RecoveryOperation};

        self.revalidate_native(permit)?;
        let last = self
            .journal
            .records
            .last()
            .ok_or(Error::RecoveryRequired)?
            .record
            .clone();
        if last.phase != self.phase || !matches!(last.phase, Phase::Target | Phase::Reopened) {
            return Err(Error::RecoveryRequired);
        }
        if last.phase == Phase::Reopened && last.creator_process == process_identity()? {
            return Err(Error::SameProcess);
        }
        let destination = last.destination.clone().ok_or(Error::RecoveryRequired)?;
        WorkspaceId::parse(&destination).map_err(|_| Error::RecoveryRequired)?;
        let scoped = RecoveryVault {
            expected: &destination,
            vault,
        };
        let roots = StartupRoots::new(self.anchor.clone());
        let mut capture = Workspace::new(roots.legacy())
            .capture_existing_snapshot()
            .map_err(|_| Error::Changed)?;
        self.checkpoint(permit, &mut capture)?;
        let inspection = self.inspection.take().ok_or(Error::Unavailable)?;
        inspection.check(&roots)?;
        let mut owner =
            Owner::acquire_existing(roots.managed()).map_err(|_| Error::RecoveryRequired)?;
        inspection.check(&roots)?;
        let empty_provisioning = matches!(inspection, RecoveryInspection::Empty(_));
        // Intentional Owner writes may replace operation metadata. From here
        // the Owner, full source capture and retained journal own revalidation.
        self.checkpoint(permit, &mut capture)?;

        if last.phase == Phase::Reopened {
            let preview = owner
                .prepare_unlock()
                .map_err(|_| Error::RecoveryRequired)?;
            if preview.workspace_id != destination {
                return Err(Error::Changed);
            }
            if !self.ask(
                permit,
                &mut capture,
                RecoveryStep::CompleteSelection,
                &destination,
                &mut confirmation,
            )? {
                return Ok(RecoveryOutcome::Cancelled);
            }
            owner
                .confirm(&preview.id, &scoped)
                .map_err(|_| Error::RecoveryRequired)?;
            self.checkpoint(permit, &mut capture)?;
            parity(&mut owner, &scoped, &capture)?;
            self.checkpoint(permit, &mut capture)?;
            let verifier = last.verifier_process.ok_or(Error::RecoveryRequired)?;
            let selected = self
                .journal
                .next(Phase::Selected, Some(destination), Some(verifier))?;
            self.journal.append(selected)?;
            self.checkpoint(permit, &mut capture)?;
            owner.lock();
            capture.cancel();
            return Ok(RecoveryOutcome::Selected);
        }

        let preview = match owner.prepare_unlock() {
            Ok(preview) => Some(preview),
            Err(OwnerError::RecoveryRequired) => None,
            Err(_) => return Err(Error::RecoveryRequired),
        };
        if preview
            .as_ref()
            .is_some_and(|preview| preview.workspace_id != destination)
        {
            return Err(Error::Changed);
        }
        if !self.ask(
            permit,
            &mut capture,
            RecoveryStep::Inspect,
            &destination,
            &mut confirmation,
        )? {
            return Ok(RecoveryOutcome::Cancelled);
        }
        if let Some(preview) = preview {
            owner
                .confirm(&preview.id, &scoped)
                .map_err(|_| Error::RecoveryRequired)?;
        } else {
            if empty_provisioning {
                let recovery = owner
                    .inspect_empty_provisioning(&scoped)
                    .map_err(|_| Error::RecoveryRequired)?;
                if recovery.workspace_id != destination {
                    return Err(Error::Changed);
                }
                if !self.ask(
                    permit,
                    &mut capture,
                    RecoveryStep::InitializeEmpty,
                    &destination,
                    &mut confirmation,
                )? {
                    return Ok(RecoveryOutcome::Cancelled);
                }
                owner
                    .finish_empty_provisioning(&recovery.id, &scoped)
                    .map_err(|_| Error::RecoveryRequired)?;
            } else {
                let recovery = owner
                    .inspect_recovery(&scoped)
                    .map_err(|_| Error::RecoveryRequired)?;
                if recovery.workspace_id != destination
                    || !matches!(
                        recovery.operation,
                        RecoveryOperation::CreateEmpty | RecoveryOperation::Restore
                    )
                {
                    return Err(Error::RecoveryRequired);
                }
                if !self.ask(
                    permit,
                    &mut capture,
                    RecoveryStep::Reconcile,
                    &destination,
                    &mut confirmation,
                )? {
                    return Ok(RecoveryOutcome::Cancelled);
                }
                owner
                    .confirm_recovery(&recovery.id, &scoped)
                    .map_err(|_| Error::RecoveryRequired)?;
            }
            self.checkpoint(permit, &mut capture)?;
            let preview = owner
                .prepare_unlock()
                .map_err(|_| Error::RecoveryRequired)?;
            if preview.workspace_id != destination {
                return Err(Error::Changed);
            }
            if !self.ask(
                permit,
                &mut capture,
                RecoveryStep::Unlock,
                &destination,
                &mut confirmation,
            )? {
                return Ok(RecoveryOutcome::Cancelled);
            }
            owner
                .confirm(&preview.id, &scoped)
                .map_err(|_| Error::RecoveryRequired)?;
        }
        self.checkpoint(permit, &mut capture)?;
        if archive_matches(&mut owner, &scoped, &capture)? {
            if !self.ask(
                permit,
                &mut capture,
                RecoveryStep::RecordCopied,
                &destination,
                &mut confirmation,
            )? {
                return Ok(RecoveryOutcome::Cancelled);
            }
        } else {
            let backup = WorkspaceBackup::decode(capture.archive().map_err(|_| Error::Changed)?)
                .map_err(|_| Error::Changed)?;
            let plan = owner
                .prepare_adoption_restore(
                    &scoped,
                    &backup,
                    capture.schema_version().map_err(|_| Error::Changed)?,
                )
                .map_err(|_| Error::RecoveryRequired)?;
            if !self.ask(
                permit,
                &mut capture,
                RecoveryStep::Restore,
                &destination,
                &mut confirmation,
            )? {
                return Ok(RecoveryOutcome::Cancelled);
            }
            owner
                .restore_adoption(&scoped, &backup, &plan)
                .map_err(|_| Error::RecoveryRequired)?;
        }
        self.checkpoint(permit, &mut capture)?;
        parity(&mut owner, &scoped, &capture)?;
        self.checkpoint(permit, &mut capture)?;
        let copied = self.journal.next(Phase::Copied, Some(destination), None)?;
        self.journal.append(copied)?;
        self.checkpoint(permit, &mut capture)?;
        owner.lock();
        capture.cancel();
        Ok(RecoveryOutcome::Copied)
    }
}
macro_rules! redacted {
    ($kind:ty, $name:literal) => {
        impl std::fmt::Debug for $kind {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str($name)
            }
        }
    };
}
redacted!(Conversion, "Conversion(<redacted>)");
redacted!(Verification, "Verification(<redacted>)");
redacted!(CopyConsent, "CopyConsent(<redacted>)");
redacted!(VerifyConsent, "VerifyConsent(<redacted>)");

fn revalidate(anchor: &Path, capture: &mut ExistingWorkspaceCapture) -> Result<(), Error> {
    Workspace::new(StartupRoots::new(anchor.to_owned()).legacy())
        .revalidate_existing_capture(capture)
        .map_err(|_| Error::Changed)
}
fn matches(record: &Record, capture: &ExistingWorkspaceCapture) -> Result<(), Error> {
    if capture.schema_version().map_err(|_| Error::Changed)? != record.source_schema
        || capture.digest().map_err(|_| Error::Changed)? != record.source_archive
        || capture.identity_fingerprint().map_err(|_| Error::Changed)? != record.source_identity
        || Inventory::from(capture.inventory().map_err(|_| Error::Changed)?) != record.inventory
    {
        return Err(Error::Changed);
    }
    Ok(())
}
fn parity(
    owner: &mut Owner,
    vault: &impl Vault,
    capture: &ExistingWorkspaceCapture,
) -> Result<(), Error> {
    if archive_matches(owner, vault, capture)? {
        Ok(())
    } else {
        Err(Error::Changed)
    }
}
fn archive_matches(
    owner: &mut Owner,
    vault: &impl Vault,
    capture: &ExistingWorkspaceCapture,
) -> Result<bool, Error> {
    let exported = Zeroizing::new(
        owner
            .export_workflow_backup(vault)
            .map_err(|_| Error::RecoveryRequired)?,
    );
    Ok(exported.as_slice() == capture.archive().map_err(|_| Error::Changed)?)
}

impl Conversion {
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn revalidate_native(&mut self, permit: &StartupPermit<'_>) -> Result<(), Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        revalidate(&self.anchor, &mut self.capture)?;
        matches(&self.intent, &self.capture)
    }
    #[cfg(test)]
    pub(super) fn prepare(anchor: PathBuf) -> Result<Self, Error> {
        Self::prepare_checked(anchor, &mut || Ok(()))
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn prepare_native(
        anchor: PathBuf,
        permit: &StartupPermit<'_>,
    ) -> Result<Self, Error> {
        let mut prepared = Self::prepare_checked(anchor, &mut || {
            permit.checkpoint().map_err(|_| Error::Unavailable)
        })?;
        prepared.native_binding = Some(permit.binding().map_err(|_| Error::Unavailable)?);
        Ok(prepared)
    }
    fn prepare_checked(
        anchor: PathBuf,
        checkpoint: &mut dyn FnMut() -> Result<(), Error>,
    ) -> Result<Self, Error> {
        checkpoint()?;
        if !cfg!(unix) {
            return Err(Error::UnsupportedPlatform);
        }
        if !cfg!(feature = "encrypted-workspace") {
            return Err(Error::BackendUnavailable);
        }
        absent(&anchor.join(DIRECTORY))?;
        let roots = StartupRoots::new(anchor.clone());
        if roots.inspect().state() != StartupState::LegacyCandidate {
            return Err(Error::RecoveryRequired);
        }
        let capture = roots
            .capture_existing()
            .map_err(|_| Error::Unavailable)?
            .into_adoption_capture()
            .map_err(|_| Error::Changed)?;
        checkpoint()?;
        let intent = Record {
            schema_version: records::SCHEMA.into(),
            phase: Phase::Intent,
            attempt_id: WorkspaceId::generate()
                .map_err(|_| Error::Unavailable)?
                .as_hex(),
            creator_process: process_identity()?.into(),
            previous_digest: None,
            source_identity: capture.identity_fingerprint().map_err(|_| Error::Changed)?,
            source_archive: capture.digest().map_err(|_| Error::Changed)?.into(),
            source_schema: capture.schema_version().map_err(|_| Error::Changed)?,
            target_schema: capture.schema_version().map_err(|_| Error::Changed)?.max(1),
            inventory: Inventory::from(capture.inventory().map_err(|_| Error::Changed)?),
            destination: None,
            verifier_process: None,
        };
        let token = WorkspaceId::generate()
            .map_err(|_| Error::Unavailable)?
            .as_hex();
        Ok(Self {
            anchor,
            capture,
            intent,
            token,
            #[cfg(feature = "encrypted-workspace")]
            native_binding: None,
        })
    }
    #[cfg(test)]
    fn synthetic_consent(&self) -> CopyConsent {
        CopyConsent {
            token: self.token.clone(),
        }
    }
    /// The trusted host supplies the native-sheet callback. IPC cannot supply
    /// this result, manufacture the permit, or retain a cancelled capture.
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn confirm_native(
        mut self,
        permit: &StartupPermit<'_>,
        confirmation: impl FnOnce() -> Consent,
    ) -> Result<Option<(Self, CopyConsent)>, Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        revalidate(&self.anchor, &mut self.capture)?;
        matches(&self.intent, &self.capture)?;
        permit.checkpoint().map_err(|_| Error::Unavailable)?;
        let outcome = confirmation();
        permit.checkpoint().map_err(|_| Error::Unavailable)?;
        match outcome {
            Consent::Approved => {
                revalidate(&self.anchor, &mut self.capture)?;
                matches(&self.intent, &self.capture)?;
                permit.checkpoint().map_err(|_| Error::Unavailable)?;
                let consent = CopyConsent {
                    token: self.token.clone(),
                };
                Ok(Some((self, consent)))
            }
            Consent::Cancelled => {
                self.capture.cancel();
                Ok(None)
            }
            Consent::Unavailable => {
                self.capture.cancel();
                Err(Error::Unavailable)
            }
        }
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn copy_native(
        self,
        consent: CopyConsent,
        vault: &impl Vault,
        permit: &StartupPermit<'_>,
    ) -> Result<(), Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        self.copy_checked(consent, vault, &mut || {
            permit.checkpoint().map_err(|_| Error::Unavailable)
        })
    }
    #[cfg(test)]
    pub(super) fn copy(self, consent: CopyConsent, vault: &impl Vault) -> Result<(), Error> {
        self.copy_checked(consent, vault, &mut || Ok(()))
    }
    fn copy_checked(
        mut self,
        consent: CopyConsent,
        vault: &impl Vault,
        checkpoint: &mut dyn FnMut() -> Result<(), Error>,
    ) -> Result<(), Error> {
        checkpoint()?;
        if consent.token != self.token {
            return Err(Error::ConsentRequired);
        }
        revalidate(&self.anchor, &mut self.capture)?;
        matches(&self.intent, &self.capture)?;
        let roots = StartupRoots::new(self.anchor.clone());
        if roots.inspect().state() != StartupState::LegacyCandidate {
            return Err(Error::Changed);
        }
        checkpoint()?;
        let mut journal = Journal::create(&self.anchor)?;
        checkpoint()?;
        journal.append(self.intent)?;
        checkpoint()?;
        revalidate(&self.anchor, &mut self.capture)?;
        absent(&roots.managed())?;
        journal.check(None)?;
        point("destination", "before_create")?;
        revalidate(&self.anchor, &mut self.capture)?;
        journal.check(None)?;
        absent(&roots.managed())?;
        checkpoint()?;
        create_directory(&roots.managed())?;
        sync(&self.anchor)?;
        checkpoint()?;
        point("destination", "after_create")?;
        let mut owner = Owner::acquire(roots.managed()).map_err(|_| Error::RecoveryRequired)?;
        checkpoint()?;
        let prepared = owner
            .prepare_create_empty()
            .map_err(|_| Error::RecoveryRequired)?;
        let target = journal.next(Phase::Target, Some(prepared.workspace_id.clone()), None)?;
        checkpoint()?;
        journal.append(target)?;
        checkpoint()?;
        revalidate(&self.anchor, &mut self.capture)?;
        journal.check(None)?;
        point("destination", "before_confirm")?;
        revalidate(&self.anchor, &mut self.capture)?;
        journal.check(None)?;
        checkpoint()?;
        owner
            .confirm(&prepared.id, vault)
            .map_err(|_| Error::RecoveryRequired)?;
        checkpoint()?;
        point("destination", "after_confirm")?;
        revalidate(&self.anchor, &mut self.capture)?;
        journal.check(None)?;
        let backup = WorkspaceBackup::decode(self.capture.archive().map_err(|_| Error::Changed)?)
            .map_err(|_| Error::Changed)?;
        checkpoint()?;
        let plan = owner
            .prepare_adoption_restore(
                vault,
                &backup,
                self.capture.schema_version().map_err(|_| Error::Changed)?,
            )
            .map_err(|_| Error::RecoveryRequired)?;
        checkpoint()?;
        point("destination", "before_restore")?;
        revalidate(&self.anchor, &mut self.capture)?;
        journal.check(None)?;
        checkpoint()?;
        owner
            .restore_adoption(vault, &backup, &plan)
            .map_err(|_| Error::RecoveryRequired)?;
        checkpoint()?;
        point("destination", "after_restore")?;
        parity(&mut owner, vault, &self.capture)?;
        checkpoint()?;
        revalidate(&self.anchor, &mut self.capture)?;
        let copied = journal.next(Phase::Copied, Some(prepared.workspace_id), None)?;
        checkpoint()?;
        journal.append(copied)?;
        checkpoint()?;
        owner.lock();
        self.capture.cancel();
        point("destination", "after_close")?;
        Ok(())
    }
}

impl Verification {
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn revalidate_native(&mut self, permit: &StartupPermit<'_>) -> Result<(), Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        revalidate(&self.anchor, &mut self.capture)?;
        self.journal.check(None)
    }
    #[cfg(test)]
    pub(super) fn prepare(anchor: PathBuf) -> Result<Self, Error> {
        Self::prepare_checked(anchor, &mut || Ok(()))
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn prepare_native(
        anchor: PathBuf,
        permit: &StartupPermit<'_>,
    ) -> Result<Self, Error> {
        let mut prepared = Self::prepare_checked(anchor, &mut || {
            permit.checkpoint().map_err(|_| Error::Unavailable)
        })?;
        prepared.native_binding = Some(permit.binding().map_err(|_| Error::Unavailable)?);
        Ok(prepared)
    }
    fn prepare_checked(
        anchor: PathBuf,
        checkpoint: &mut dyn FnMut() -> Result<(), Error>,
    ) -> Result<Self, Error> {
        checkpoint()?;
        if !cfg!(unix) {
            return Err(Error::UnsupportedPlatform);
        }
        if !cfg!(feature = "encrypted-workspace") {
            return Err(Error::BackendUnavailable);
        }
        let journal = Journal::open(&anchor)?;
        let last = &journal
            .records
            .last()
            .ok_or(Error::RecoveryRequired)?
            .record;
        if last.phase != Phase::Copied {
            return Err(Error::RecoveryRequired);
        }
        if last.creator_process == process_identity()? {
            return Err(Error::SameProcess);
        }
        let roots = StartupRoots::new(anchor.clone());
        if roots.inspect().state() != StartupState::Mixed {
            return Err(Error::RecoveryRequired);
        }
        let capture = Workspace::new(roots.legacy())
            .capture_existing_snapshot()
            .map_err(|_| Error::Changed)?;
        matches(last, &capture)?;
        checkpoint()?;
        let mut owner =
            Owner::acquire_existing(roots.managed()).map_err(|_| Error::RecoveryRequired)?;
        let preview = owner
            .prepare_unlock()
            .map_err(|_| Error::RecoveryRequired)?;
        if last.destination.as_deref() != Some(preview.workspace_id.as_str()) {
            return Err(Error::Changed);
        }
        checkpoint()?;
        let token = WorkspaceId::generate()
            .map_err(|_| Error::Unavailable)?
            .as_hex();
        Ok(Self {
            anchor,
            journal,
            capture,
            owner,
            preview: preview.id,
            token,
            #[cfg(feature = "encrypted-workspace")]
            native_binding: None,
        })
    }
    #[cfg(test)]
    fn synthetic_consent(&self) -> VerifyConsent {
        VerifyConsent {
            token: self.token.clone(),
        }
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn confirm_native(
        mut self,
        permit: &StartupPermit<'_>,
        confirmation: impl FnOnce() -> Consent,
    ) -> Result<Option<(Self, VerifyConsent)>, Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        revalidate(&self.anchor, &mut self.capture)?;
        self.journal.check(None)?;
        permit.checkpoint().map_err(|_| Error::Unavailable)?;
        let outcome = confirmation();
        permit.checkpoint().map_err(|_| Error::Unavailable)?;
        match outcome {
            Consent::Approved => {
                revalidate(&self.anchor, &mut self.capture)?;
                self.journal.check(None)?;
                permit.checkpoint().map_err(|_| Error::Unavailable)?;
                let consent = VerifyConsent {
                    token: self.token.clone(),
                };
                Ok(Some((self, consent)))
            }
            Consent::Cancelled => Ok(None),
            Consent::Unavailable => Err(Error::Unavailable),
        }
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn select_native(
        self,
        consent: VerifyConsent,
        vault: &impl Vault,
        permit: &StartupPermit<'_>,
    ) -> Result<(), Error> {
        permit
            .check_binding(self.native_binding.as_ref().ok_or(Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        self.select_checked(consent, vault, &mut || {
            permit.checkpoint().map_err(|_| Error::Unavailable)
        })
    }
    #[cfg(test)]
    pub(super) fn select(self, consent: VerifyConsent, vault: &impl Vault) -> Result<(), Error> {
        self.select_checked(consent, vault, &mut || Ok(()))
    }
    fn select_checked(
        mut self,
        consent: VerifyConsent,
        vault: &impl Vault,
        checkpoint: &mut dyn FnMut() -> Result<(), Error>,
    ) -> Result<(), Error> {
        checkpoint()?;
        if consent.token != self.token {
            return Err(Error::ConsentRequired);
        }
        revalidate(&self.anchor, &mut self.capture)?;
        self.journal.check(None)?;
        checkpoint()?;
        self.owner
            .confirm(&self.preview, vault)
            .map_err(|_| Error::RecoveryRequired)?;
        checkpoint()?;
        point("verification", "after_unlock")?;
        parity(&mut self.owner, vault, &self.capture)?;
        checkpoint()?;
        revalidate(&self.anchor, &mut self.capture)?;
        let destination = self
            .journal
            .records
            .last()
            .ok_or(Error::RecoveryRequired)?
            .record
            .destination
            .clone();
        let verifier = Some(process_identity()?.into());
        let reopened = self
            .journal
            .next(Phase::Reopened, destination.clone(), verifier.clone())?;
        checkpoint()?;
        self.journal.append(reopened)?;
        checkpoint()?;
        point("verification", "before_selection")?;
        revalidate(&self.anchor, &mut self.capture)?;
        checkpoint()?;
        parity(&mut self.owner, vault, &self.capture)?;
        let selected = self.journal.next(Phase::Selected, destination, verifier)?;
        checkpoint()?;
        self.journal.append(selected)?;
        checkpoint()?;
        self.owner.lock();
        self.capture.cancel();
        Ok(())
    }
}
impl Drop for Verification {
    fn drop(&mut self) {
        self.owner.lock();
        self.capture.cancel();
    }
}

#[cfg(test)]
#[path = "workspace_adoption_tests.rs"]
mod tests;
