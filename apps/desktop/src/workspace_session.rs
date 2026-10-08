//! Exclusive, explicitly confirmed encrypted source-session ownership.
//! No production command or renderer is mounted here. Raw trusted-host key
//! primitives remain outside this owner's guarantees until native adoption.
use crate::workspace_keys::{self, RetainedKey, Store, WorkspaceId};
use rangoon_domain::AnalysisReport;
use rangoon_store::{SaveReceipt, SnapshotMetadata, Workspace};
use serde::Serialize;
use std::{
    fs::{self, File, Metadata, OpenOptions, TryLockError},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const MARKER: &[u8] = b"rangoon.workspace-binding.v1\0";
const BINDING_BYTES: usize = 94;
const CONTROL: &str = "encrypted-workspace.lock";
const BINDING: &str = "encrypted-workspace.binding";
const PARENT: &str = "encrypted-workspaces";
const MAX_DATABASE_BYTES: u64 = 64 * 1024 * 1024;

pub trait Vault {
    type Slot: Store;
    fn open(&self, identity: &WorkspaceId) -> Result<Self::Slot, workspace_keys::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    BackendUnavailable,
    Unavailable,
    Busy,
    InvalidBinding,
    RecoveryRequired,
    Stale,
    Locked,
    AlreadyExists,
    WriteUncertain,
    InvalidSource,
    StoreInvalid,
    Key(workspace_keys::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Create,
    Unlock,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub id: String,
    pub kind: Action,
    pub workspace_id: String,
    pub source_id: Option<String>,
}

struct Binding {
    ready: bool,
    identity: WorkspaceId,
}
impl Binding {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(BINDING_BYTES);
        bytes.extend_from_slice(MARKER);
        bytes.push(u8::from(self.ready));
        bytes.extend_from_slice(self.identity.as_hex().as_bytes());
        bytes
    }
    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != BINDING_BYTES || !bytes.starts_with(MARKER) {
            return Err(Error::InvalidBinding);
        }
        let ready = match bytes[MARKER.len()] {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidBinding),
        };
        let hex =
            std::str::from_utf8(&bytes[MARKER.len() + 1..]).map_err(|_| Error::InvalidBinding)?;
        let identity = WorkspaceId::parse(hex).map_err(|_| Error::InvalidBinding)?;
        Ok(Self { ready, identity })
    }
}
struct DirectoryProof {
    metadata: Metadata,
    #[cfg(unix)]
    _held: File,
}
struct Directories {
    parent: DirectoryProof,
    identity: DirectoryProof,
}
struct Pending {
    token: String,
    binding: Binding,
    source: Option<AnalysisReport>,
    observed: Option<Vec<u8>>,
    directories: Option<Directories>,
}
struct Opened {
    binding: Binding,
    retained: RetainedKey,
    workspace: Workspace,
    directories: Directories,
}

/// Owns cooperating-file custody even while the managed key session is locked.
/// No raw handle, key, closure or clonable owner is returned by this API.
///
/// ```compile_fail
/// use rangoon_desktop::workspace_session::Owner;
/// fn requires_debug<T: std::fmt::Debug>() {}
/// requires_debug::<Owner>();
/// ```
/// ```compile_fail
/// use rangoon_desktop::workspace_session::Owner;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<Owner>();
/// ```
/// ```compile_fail
/// use rangoon_desktop::workspace_session::Owner;
/// fn requires_serialization<T: serde::Serialize>() {}
/// requires_serialization::<Owner>();
/// ```
pub struct Owner {
    root: PathBuf,
    root_metadata: Metadata,
    control: File,
    pending: Option<Pending>,
    opened: Option<Opened>,
    recovery: bool,
    #[cfg(all(test, feature = "encrypted-workspace"))]
    fault: Option<Fault>,
}

#[cfg(all(test, feature = "encrypted-workspace"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    BeforeProvisionWrite,
    BeforeReadyWrite,
    BeforeReadySync,
    BeforeReadyReadback,
    AfterSave,
}

impl Owner {
    /// Explicit native acquisition; default builds do not touch the filesystem.
    pub fn acquire(host_root: PathBuf) -> Result<Self, Error> {
        workspace_keys::ensure_backend().map_err(|_| Error::BackendUnavailable)?;
        let root_metadata = plain_directory(&host_root)?;
        let path = host_root.join(CONTROL);
        let mut options = options();
        options.read(true).write(true).create_new(true);
        let control = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                regular(&fs::symlink_metadata(&path).map_err(|_| Error::Unavailable)?)?;
                options.create_new(false);
                options.open(&path).map_err(|_| Error::Unavailable)?
            }
            Err(_) => return Err(Error::Unavailable),
        };
        verify_file(&control, &path)?;
        if control.metadata().map_err(|_| Error::Unavailable)?.len() != 0 {
            return Err(Error::InvalidBinding);
        }
        match control.try_lock() {
            Ok(()) => (),
            Err(TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(TryLockError::Error(_)) => return Err(Error::Unavailable),
        }
        let owner = Self {
            root: host_root,
            root_metadata,
            control,
            pending: None,
            opened: None,
            recovery: false,
            #[cfg(all(test, feature = "encrypted-workspace"))]
            fault: None,
        };
        owner.check_control()?;
        Ok(owner)
    }

    fn check_control(&self) -> Result<(), Error> {
        let current = plain_directory(&self.root)?;
        if !same_file(&current, &self.root_metadata) {
            return Err(Error::Unavailable);
        }
        verify_file(&self.control, &self.root.join(CONTROL))?;
        if self
            .control
            .metadata()
            .map_err(|_| Error::Unavailable)?
            .len()
            != 0
        {
            return Err(Error::InvalidBinding);
        }
        Ok(())
    }
    fn available(&self) -> Result<(), Error> {
        if self.recovery {
            return Err(Error::RecoveryRequired);
        }
        if self.opened.is_some() || self.pending.is_some() {
            return Err(Error::Busy);
        }
        self.check_control()
    }
    fn absent_binding(&self) -> Result<(), Error> {
        match fs::symlink_metadata(self.root.join(BINDING)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Ok(_) => Err(Error::AlreadyExists),
            Err(_) => Err(Error::Unavailable),
        }
    }
    fn read_binding(&self) -> Result<Vec<u8>, Error> {
        let path = self.root.join(BINDING);
        let before = fs::symlink_metadata(&path).map_err(|_| Error::InvalidBinding)?;
        regular(&before).map_err(|_| Error::InvalidBinding)?;
        if before.len() != BINDING_BYTES as u64 {
            return Err(Error::InvalidBinding);
        }
        let file = options()
            .read(true)
            .open(&path)
            .map_err(|_| Error::InvalidBinding)?;
        verify_file(&file, &path).map_err(|_| Error::InvalidBinding)?;
        if !same_file(
            &before,
            &file.metadata().map_err(|_| Error::InvalidBinding)?,
        ) {
            return Err(Error::InvalidBinding);
        }
        let mut bytes = Vec::with_capacity(BINDING_BYTES + 1);
        (&file)
            .take((BINDING_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::InvalidBinding)?;
        verify_file(&file, &path).map_err(|_| Error::InvalidBinding)?;
        Binding::decode(&bytes)?;
        Ok(bytes)
    }
    fn directory(&self, binding: &Binding) -> PathBuf {
        self.root.join(PARENT).join(binding.identity.as_hex())
    }
    fn directories(&self, binding: &Binding) -> Result<Directories, Error> {
        Ok(Directories {
            parent: directory_proof(&self.root.join(PARENT)).map_err(|_| Error::StoreInvalid)?,
            identity: directory_proof(&self.directory(binding)).map_err(|_| Error::StoreInvalid)?,
        })
    }
    fn unchanged_directories(&self, binding: &Binding, held: &Directories) -> Result<(), Error> {
        let parent = plain_directory(&self.root.join(PARENT)).map_err(|_| Error::StoreInvalid)?;
        let identity =
            plain_directory(&self.directory(binding)).map_err(|_| Error::StoreInvalid)?;
        if !same_file(&parent, &held.parent.metadata)
            || !same_file(&identity, &held.identity.metadata)
        {
            return Err(Error::StoreInvalid);
        }
        Ok(())
    }
    fn database(&self, binding: &Binding) -> Result<Metadata, Error> {
        plain_directory(&self.root.join(PARENT)).map_err(|_| Error::StoreInvalid)?;
        let directory = self.directory(binding);
        plain_directory(&directory).map_err(|_| Error::StoreInvalid)?;
        let metadata = fs::symlink_metadata(directory.join("workspace.sqlite3"))
            .map_err(|_| Error::StoreInvalid)?;
        regular(&metadata).map_err(|_| Error::StoreInvalid)?;
        if metadata.len() == 0 || metadata.len() > MAX_DATABASE_BYTES {
            return Err(Error::StoreInvalid);
        }
        Ok(metadata)
    }
    fn unchanged_database(&self, binding: &Binding, previous: &Metadata) -> Result<(), Error> {
        if !same_file(&self.database(binding)?, previous) {
            return Err(Error::StoreInvalid);
        }
        Ok(())
    }

    pub fn prepare_create(&mut self, source: &AnalysisReport) -> Result<Preview, Error> {
        self.available()?;
        self.absent_binding()?;
        let checked = rangoon_import::analyze(
            &source.source.display_name,
            source.source.content.as_bytes(),
        )
        .map_err(|_| Error::InvalidSource)?;
        if &checked != source {
            return Err(Error::InvalidSource);
        }
        let identity = WorkspaceId::generate().map_err(|_| Error::Unavailable)?;
        let token = WorkspaceId::generate()
            .map_err(|_| Error::Unavailable)?
            .as_hex();
        if token == identity.as_hex() {
            return Err(Error::Unavailable);
        }
        let preview = Preview {
            id: token.clone(),
            kind: Action::Create,
            workspace_id: identity.as_hex(),
            source_id: Some(checked.source.id.clone()),
        };
        self.pending = Some(Pending {
            token,
            binding: Binding {
                ready: false,
                identity,
            },
            source: Some(checked),
            observed: None,
            directories: None,
        });
        Ok(preview)
    }
    pub fn prepare_unlock(&mut self) -> Result<Preview, Error> {
        self.available()?;
        let observed = self.read_binding()?;
        let binding = Binding::decode(&observed)?;
        if !binding.ready {
            return Err(Error::RecoveryRequired);
        }
        let directories = self.directories(&binding)?;
        self.database(&binding)?;
        self.unchanged_directories(&binding, &directories)?;
        let token = WorkspaceId::generate()
            .map_err(|_| Error::Unavailable)?
            .as_hex();
        let preview = Preview {
            id: token.clone(),
            kind: Action::Unlock,
            workspace_id: binding.identity.as_hex(),
            source_id: None,
        };
        self.pending = Some(Pending {
            token,
            binding,
            source: None,
            observed: Some(observed),
            directories: Some(directories),
        });
        Ok(preview)
    }

    /// Matching confirmation consumes its exact action before any vault access.
    /// The future native caller must obtain real user consent before invoking it.
    pub fn confirm(&mut self, preview_id: &str, vault: &impl Vault) -> Result<(), Error> {
        if self.recovery {
            return Err(Error::RecoveryRequired);
        }
        if !self.pending.as_ref().is_some_and(|p| p.token == preview_id) {
            return Err(Error::Stale);
        }
        let pending = self.pending.take().ok_or(Error::Stale)?;
        self.check_control().map_err(|_| Error::Stale)?;
        if let Some(source) = pending.source {
            self.absent_binding().map_err(|_| Error::Stale)?;
            if fs::symlink_metadata(self.directory(&pending.binding)).is_ok() {
                return Err(Error::Stale);
            }
            match self.create(pending.binding, source, vault) {
                Ok(opened) => self.opened = Some(opened),
                Err(_) => {
                    self.lock();
                    self.recovery = true;
                    return Err(Error::WriteUncertain);
                }
            }
        } else {
            if self.read_binding().map_err(|_| Error::Stale)?
                != pending.observed.ok_or(Error::Stale)?
            {
                return Err(Error::Stale);
            }
            self.opened = Some(self.unlock(
                pending.binding,
                pending.directories.ok_or(Error::StoreInvalid)?,
                vault,
            )?);
        }
        Ok(())
    }

    fn slot<V: Vault>(&self, vault: &V, identity: &WorkspaceId) -> Result<V::Slot, Error> {
        let slot = vault.open(identity).map_err(Error::Key)?;
        if slot.identity() != identity {
            return Err(Error::Key(workspace_keys::Error::Changed));
        }
        Ok(slot)
    }
    fn create(
        &self,
        mut binding: Binding,
        source: AnalysisReport,
        vault: &impl Vault,
    ) -> Result<Opened, Error> {
        let parent = self.root.join(PARENT);
        match fs::symlink_metadata(&parent) {
            Ok(_) => {
                plain_directory(&parent)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                create_directory(&parent)?;
                sync_directory(&self.root)?;
            }
            Err(_) => return Err(Error::Unavailable),
        }
        create_directory(&self.directory(&binding))?;
        sync_directory(&parent)?;
        let directories = self.directories(&binding)?;
        let path = self.root.join(BINDING);
        let mut file = options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| Error::WriteUncertain)?;
        #[cfg(all(test, feature = "encrypted-workspace"))]
        self.fail_at(Fault::BeforeProvisionWrite)?;
        file.write_all(&binding.bytes())
            .map_err(|_| Error::WriteUncertain)?;
        file.sync_all().map_err(|_| Error::WriteUncertain)?;
        sync_directory(&self.root)?;
        verify_file(&file, &path)?;
        if self.read_binding()? != binding.bytes() {
            return Err(Error::WriteUncertain);
        }
        self.unchanged_directories(&binding, &directories)?;
        let slot = self.slot(vault, &binding.identity)?;
        self.check_control()?;
        self.unchanged_directories(&binding, &directories)?;
        if self.read_binding()? != binding.bytes() {
            return Err(Error::Stale);
        }
        let retained = workspace_keys::create_key(&slot).map_err(Error::Key)?;
        let workspace =
            workspace_keys::verified_workspace(&slot, &retained, self.directory(&binding))
                .map_err(Error::Key)?;
        workspace
            .save_v1(&source)
            .map_err(|_| Error::StoreInvalid)?;
        let previous = self.database(&binding)?;
        workspace.workflow_data().map_err(|_| Error::StoreInvalid)?;
        self.unchanged_database(&binding, &previous)?;
        self.unchanged_directories(&binding, &directories)?;
        sync_directory(&self.directory(&binding))?;
        let provisioning = binding.bytes();
        binding.ready = true;
        #[cfg(all(test, feature = "encrypted-workspace"))]
        self.fail_at(Fault::BeforeReadyWrite)?;
        self.check_control()?;
        self.unchanged_directories(&binding, &directories)?;
        verify_file(&file, &path)?;
        if self.read_binding()? != provisioning {
            return Err(Error::Stale);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| Error::WriteUncertain)?;
        file.write_all(&binding.bytes())
            .map_err(|_| Error::WriteUncertain)?;
        #[cfg(all(test, feature = "encrypted-workspace"))]
        self.fail_at(Fault::BeforeReadySync)?;
        file.sync_all().map_err(|_| Error::WriteUncertain)?;
        sync_directory(&self.root)?;
        #[cfg(all(test, feature = "encrypted-workspace"))]
        self.fail_at(Fault::BeforeReadyReadback)?;
        if self.read_binding()? != binding.bytes() {
            return Err(Error::WriteUncertain);
        }
        self.unchanged_directories(&binding, &directories)?;
        Ok(Opened {
            binding,
            retained,
            workspace,
            directories,
        })
    }
    fn unlock(
        &self,
        binding: Binding,
        directories: Directories,
        vault: &impl Vault,
    ) -> Result<Opened, Error> {
        self.unchanged_directories(&binding, &directories)?;
        let previous = self.database(&binding)?;
        let slot = self.slot(vault, &binding.identity)?;
        self.check_control()?;
        self.unchanged_directories(&binding, &directories)?;
        self.unchanged_database(&binding, &previous)?;
        if self.read_binding()? != binding.bytes() {
            return Err(Error::Stale);
        }
        let retained = workspace_keys::read_key(&slot).map_err(Error::Key)?;
        let workspace =
            workspace_keys::verified_workspace(&slot, &retained, self.directory(&binding))
                .map_err(Error::Key)?;
        workspace.workflow_data().map_err(|_| Error::StoreInvalid)?;
        self.unchanged_database(&binding, &previous)?;
        if self.read_binding()? != binding.bytes() {
            return Err(Error::Stale);
        }
        self.unchanged_directories(&binding, &directories)?;
        Ok(Opened {
            binding,
            retained,
            workspace,
            directories,
        })
    }

    fn checked(&mut self, vault: &impl Vault) -> Result<Metadata, Error> {
        let checked = (|| {
            if self.recovery {
                return Err(Error::RecoveryRequired);
            }
            let opened = self.opened.as_ref().ok_or(Error::Locked)?;
            self.check_control()?;
            if self.read_binding()? != opened.binding.bytes() {
                return Err(Error::InvalidBinding);
            }
            self.unchanged_directories(&opened.binding, &opened.directories)?;
            let previous = self.database(&opened.binding)?;
            let slot = self.slot(vault, &opened.binding.identity)?;
            self.check_control()?;
            self.unchanged_directories(&opened.binding, &opened.directories)?;
            self.unchanged_database(&opened.binding, &previous)?;
            if self.read_binding()? != opened.binding.bytes() {
                return Err(Error::InvalidBinding);
            }
            // The ephemeral revalidated handle stays inside this module.
            let _verified = workspace_keys::verified_workspace(
                &slot,
                &opened.retained,
                self.directory(&opened.binding),
            )
            .map_err(Error::Key)?;
            Ok(previous)
        })();
        if checked.is_err() {
            self.lock();
        }
        checked
    }
    fn finish_read<T>(
        &mut self,
        result: Result<T, rangoon_store::StoreError>,
        previous: Metadata,
    ) -> Result<T, Error> {
        let checked = (|| {
            let value = result.map_err(|_| Error::StoreInvalid)?;
            let opened = self.opened.as_ref().ok_or(Error::Locked)?;
            self.check_control()?;
            if self.read_binding()? != opened.binding.bytes() {
                return Err(Error::InvalidBinding);
            }
            self.unchanged_directories(&opened.binding, &opened.directories)?;
            self.unchanged_database(&opened.binding, &previous)?;
            Ok(value)
        })();
        if checked.is_err() {
            self.lock();
        }
        checked
    }
    pub fn list(&mut self, vault: &impl Vault) -> Result<Vec<SnapshotMetadata>, Error> {
        let previous = self.checked(vault)?;
        let result = self.opened.as_ref().ok_or(Error::Locked)?.workspace.list();
        self.finish_read(result, previous)
    }
    pub fn open(&mut self, vault: &impl Vault, source_id: &str) -> Result<AnalysisReport, Error> {
        let previous = self.checked(vault)?;
        let result = self
            .opened
            .as_ref()
            .ok_or(Error::Locked)?
            .workspace
            .open(source_id);
        self.finish_read(result, previous)
    }
    pub fn save(
        &mut self,
        vault: &impl Vault,
        source: &AnalysisReport,
    ) -> Result<SaveReceipt, Error> {
        let previous = self.checked(vault)?;
        let result = self
            .opened
            .as_ref()
            .ok_or(Error::Locked)?
            .workspace
            .save_v1(source);
        #[cfg(all(test, feature = "encrypted-workspace"))]
        if self.fail_at(Fault::AfterSave).is_err() {
            self.lock();
            self.recovery = true;
            return Err(Error::WriteUncertain);
        }
        match self.finish_read(result, previous) {
            Ok(receipt) => Ok(receipt),
            Err(_) => {
                self.lock();
                self.recovery = true;
                Err(Error::WriteUncertain)
            }
        }
    }
    pub fn lock(&mut self) {
        self.pending = None;
        self.opened = None;
    }
    pub fn is_unlocked(&self) -> bool {
        self.opened.is_some()
    }
    #[cfg(all(test, feature = "encrypted-workspace"))]
    fn inject_fault(&mut self, fault: Fault) {
        self.fault = Some(fault);
    }
    #[cfg(all(test, feature = "encrypted-workspace"))]
    fn fail_at(&self, fault: Fault) -> Result<(), Error> {
        if self.fault == Some(fault) {
            Err(Error::WriteUncertain)
        } else {
            Ok(())
        }
    }
}

fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Microsoft's CreateFileW FILE_FLAG_OPEN_REPARSE_POINT: open the
        // reparse point itself, then reject its metadata before any content use.
        options.custom_flags(0x00200000);
    }
    options
}
fn linked(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn regular(metadata: &Metadata) -> Result<(), Error> {
    if linked(metadata) || !metadata.is_file() {
        return Err(Error::InvalidBinding);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(Error::InvalidBinding);
        }
    }
    Ok(())
}
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        true
    }
}
fn verify_file(file: &File, path: &Path) -> Result<(), Error> {
    let held = file.metadata().map_err(|_| Error::Unavailable)?;
    let current = fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    regular(&held)?;
    regular(&current)?;
    if !same_file(&held, &current) {
        return Err(Error::InvalidBinding);
    }
    Ok(())
}
fn plain_directory(path: &Path) -> Result<Metadata, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    if !path.is_absolute()
        || linked(&metadata)
        || !metadata.is_dir()
        || fs::canonicalize(path).map_err(|_| Error::Unavailable)? != path
    {
        return Err(Error::Unavailable);
    }
    Ok(metadata)
}
fn directory_proof(path: &Path) -> Result<DirectoryProof, Error> {
    let metadata = plain_directory(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let held = options()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_DIRECTORY)
            .open(path)
            .map_err(|_| Error::Unavailable)?;
        let opened = held.metadata().map_err(|_| Error::Unavailable)?;
        let current = plain_directory(path)?;
        if !opened.is_dir() || !same_file(&metadata, &opened) || !same_file(&current, &opened) {
            return Err(Error::Unavailable);
        }
        Ok(DirectoryProof {
            metadata: opened,
            _held: held,
        })
    }
    #[cfg(not(unix))]
    {
        Ok(DirectoryProof { metadata })
    }
}
fn create_directory(path: &Path) -> Result<(), Error> {
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut configured = builder;
        configured.mode(0o700);
        configured
    };
    builder.create(path).map_err(|_| Error::Unavailable)?;
    plain_directory(path)?;
    Ok(())
}
fn sync_directory(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Unavailable)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
#[path = "workspace_session_tests.rs"]
mod tests;
