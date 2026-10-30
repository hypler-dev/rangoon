//! Startup observations and retained Legacy capture. The native host owns
//! confirmation, lifecycle coordination and conversion; these observations grant no authority.
#[cfg(test)]
use rangoon_store::WorkflowRecoveryCounts;
use rangoon_store::{ExistingWorkspaceCapture, Workspace};
use std::{
    fs,
    path::{Path, PathBuf},
};

const DATABASE: &str = "workspace.sqlite3";
const CONTROL: &str = "encrypted-workspace.lock";
const BINDING: &str = "encrypted-workspace.binding";
const PARENT: &str = "encrypted-workspaces";
const OPERATIONS: [&str; 2] = [
    "encrypted-workspace.operation.pending",
    "encrypted-workspace.operation.terminal",
];
const MAX_DATABASE: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    FreshCandidate,
    LegacyCandidate,
    ManagedCandidate,
    Mixed,
    RecoveryRequired,
    Unavailable,
    UnsupportedPlatform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    RecoveryRequired,
    Unavailable,
    UnsupportedPlatform,
    Changed,
}

pub(super) struct StartupRoots {
    anchor: PathBuf,
}
pub(super) struct Inspection {
    state: State,
    proof: Option<Proof>,
}
pub(super) struct RetainedSource {
    roots: StartupRoots,
    inspection: Inspection,
    capture: Option<ExistingWorkspaceCapture>,
}

/// Native-only proof for one fixed-root Owner binding. Neither the path nor the
/// creating/existing distinction can be supplied by a renderer request.
pub(super) struct BindingProof {
    roots: StartupRoots,
    proof: Proof,
    kind: BindingKind,
}

/// Presence-only evidence for one interrupted adoption target. This is not a
/// complete Managed binding and cannot be used to publish a content client.
#[cfg(feature = "encrypted-workspace")]
pub(super) struct EmptyProvisioningProof {
    roots: StartupRoots,
    proof: Proof,
    destination: String,
}
enum BindingKind {
    Complete,
    Residue,
    Fresh,
}

macro_rules! redacted_debug {
    ($kind:ty, $label:literal) => {
        impl std::fmt::Debug for $kind {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str($label)
            }
        }
    };
}
redacted_debug!(StartupRoots, "StartupRoots(<redacted>)");
redacted_debug!(Inspection, "Inspection(<redacted>)");
redacted_debug!(RetainedSource, "RetainedSource(<redacted>)");
redacted_debug!(BindingProof, "BindingProof(<redacted>)");

struct Node {
    path: PathBuf,
    metadata: Option<fs::Metadata>,
    directory: bool,
}
struct Proof {
    nodes: Vec<Node>,
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
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

fn regular(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.is_file() && metadata.nlink() == 1
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

impl Proof {
    fn observe(&mut self, path: &Path, directory: bool) -> Result<Option<fs::Metadata>, Error> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || if directory {
                        !metadata.is_dir()
                    } else {
                        !regular(&metadata)
                    }
                {
                    return Err(Error::RecoveryRequired);
                }
                Some(metadata)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(Error::Unavailable),
        };
        self.nodes.push(Node {
            path: path.to_owned(),
            metadata: metadata.clone(),
            directory,
        });
        Ok(metadata)
    }
    fn check(&self) -> Result<(), Error> {
        for node in &self.nodes {
            match (&node.metadata, fs::symlink_metadata(&node.path)) {
                (None, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => (),
                (Some(before), Ok(after))
                    if !after.file_type().is_symlink()
                        && (if node.directory {
                            after.is_dir()
                        } else {
                            regular(&after)
                        })
                        && same_file(before, &after)
                        && (node.directory || before.len() == after.len()) => {}
                _ => return Err(Error::Changed),
            }
        }
        Ok(())
    }
}

fn names(path: &Path, limit: usize) -> Result<Vec<std::ffi::OsString>, Error> {
    let mut result = Vec::new();
    for entry in fs::read_dir(path).map_err(|_| Error::Unavailable)? {
        if result.len() == limit {
            return Err(Error::RecoveryRequired);
        }
        result.push(entry.map_err(|_| Error::Unavailable)?.file_name());
    }
    result.sort();
    Ok(result)
}

impl StartupRoots {
    /// Only a native application-data anchor, never an IPC-selected root.
    pub(super) fn new(anchor: PathBuf) -> Self {
        Self { anchor }
    }
    pub(super) fn legacy(&self) -> PathBuf {
        self.anchor.join("source-workspace")
    }
    pub(super) fn managed(&self) -> PathBuf {
        self.anchor.join("managed-workspace")
    }
    pub(super) fn inspect(&self) -> Inspection {
        match self.observe() {
            Ok((state, proof)) => Inspection {
                state,
                proof: Some(proof),
            },
            Err(error) => Inspection {
                state: match error {
                    Error::UnsupportedPlatform => State::UnsupportedPlatform,
                    Error::Unavailable => State::Unavailable,
                    Error::RecoveryRequired | Error::Changed => State::RecoveryRequired,
                },
                proof: None,
            },
        }
    }
    fn observe(&self) -> Result<(State, Proof), Error> {
        if !cfg!(unix) {
            return Err(Error::UnsupportedPlatform);
        }
        if !self.anchor.is_absolute()
            || self.anchor.components().count() > 64
            || self.anchor.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(Error::Unavailable);
        }
        let mut proof = Proof { nodes: Vec::new() };
        let ancestors: Vec<_> = self.anchor.ancestors().collect();
        for path in ancestors.into_iter().rev() {
            proof.observe(path, true)?;
        }
        let legacy = self.legacy();
        let managed = self.managed();
        let has_legacy = proof.observe(&legacy, true)?.is_some();
        let has_managed = proof.observe(&managed, true)?.is_some();
        if has_legacy {
            if names(&legacy, 4)? != [std::ffi::OsString::from(DATABASE)] {
                return Err(Error::RecoveryRequired);
            }
            Self::database(&mut proof, &legacy)?;
        }
        if has_managed {
            Self::managed_shape(&mut proof, &managed)?;
        }
        proof.check()?;
        Ok((
            match (has_legacy, has_managed) {
                (false, false) => State::FreshCandidate,
                (true, false) => State::LegacyCandidate,
                (false, true) => State::ManagedCandidate,
                (true, true) => State::Mixed,
            },
            proof,
        ))
    }
    fn database(proof: &mut Proof, root: &Path) -> Result<(), Error> {
        let metadata = proof
            .observe(&root.join(DATABASE), false)?
            .ok_or(Error::RecoveryRequired)?;
        if metadata.len() == 0 || metadata.len() > MAX_DATABASE {
            return Err(Error::RecoveryRequired);
        }
        Ok(())
    }
    fn managed_shape(proof: &mut Proof, root: &Path) -> Result<(), Error> {
        let entries = names(root, 5)?;
        if ![CONTROL, BINDING, PARENT]
            .iter()
            .all(|name| entries.contains(&(*name).into()))
            || entries.iter().any(|name| {
                ![CONTROL, BINDING, PARENT, OPERATIONS[0], OPERATIONS[1]]
                    .iter()
                    .any(|allowed| name == allowed)
            })
        {
            return Err(Error::RecoveryRequired);
        }
        let control = proof
            .observe(&root.join(CONTROL), false)?
            .ok_or(Error::RecoveryRequired)?;
        let binding = proof
            .observe(&root.join(BINDING), false)?
            .ok_or(Error::RecoveryRequired)?;
        if control.len() != 0 || binding.len() == 0 || binding.len() > 4096 {
            return Err(Error::RecoveryRequired);
        }
        for name in OPERATIONS {
            if let Some(metadata) = proof.observe(&root.join(name), false)?
                && (metadata.len() == 0 || metadata.len() > 4096)
            {
                return Err(Error::RecoveryRequired);
            }
        }
        let parent = root.join(PARENT);
        proof
            .observe(&parent, true)?
            .ok_or(Error::RecoveryRequired)?;
        let identities = names(&parent, 1)?;
        let identity = identities
            .first()
            .and_then(|name| name.to_str())
            .ok_or(Error::RecoveryRequired)?;
        if identity.len() != 64
            || !identity
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::RecoveryRequired);
        }
        let directory = parent.join(identity);
        proof
            .observe(&directory, true)?
            .ok_or(Error::RecoveryRequired)?;
        if names(&directory, 1)? != [std::ffi::OsString::from(DATABASE)] {
            return Err(Error::RecoveryRequired);
        }
        // Presence only: Owner still validates binding/version, recovery,
        // ownership and decrypted state before any client can be issued.
        Self::database(proof, &directory)
    }
    pub(super) fn revalidate(&self, inspection: &Inspection) -> Result<State, Error> {
        let proof = inspection.proof.as_ref().ok_or(Error::Changed)?;
        proof.check()?;
        let (state, _) = self.observe()?;
        if state != inspection.state {
            return Err(Error::Changed);
        }
        // Recheck original identities after the fresh path observation too.
        proof.check()?;
        Ok(state)
    }
    pub(super) fn capture_existing(&self) -> Result<RetainedSource, Error> {
        let inspection = self.inspect();
        match inspection.state {
            State::LegacyCandidate => (),
            State::UnsupportedPlatform => return Err(Error::UnsupportedPlatform),
            State::Unavailable => return Err(Error::Unavailable),
            _ => return Err(Error::RecoveryRequired),
        }
        self.revalidate(&inspection)?;
        let capture = Workspace::new(self.legacy())
            .capture_existing_snapshot()
            .map_err(|_| Error::Unavailable)?;
        self.revalidate(&inspection)?;
        Ok(RetainedSource {
            roots: Self::new(self.anchor.clone()),
            inspection,
            capture: Some(capture),
        })
    }
}

impl Inspection {
    pub(super) fn state(&self) -> State {
        self.state
    }
}

impl RetainedSource {
    /// Consumes the Legacy-only proof before a native-created target makes it Mixed.
    pub(super) fn into_adoption_capture(mut self) -> Result<ExistingWorkspaceCapture, Error> {
        self.revalidate()?;
        self.capture.take().ok_or(Error::Changed)
    }
    #[cfg(test)]
    fn capture(&self) -> Result<&ExistingWorkspaceCapture, Error> {
        self.capture.as_ref().ok_or(Error::Changed)
    }
    #[cfg(test)]
    pub(super) fn schema_version(&self) -> Result<u32, Error> {
        self.capture()?.schema_version().map_err(|_| Error::Changed)
    }
    #[cfg(test)]
    pub(super) fn inventory(&self) -> Result<&WorkflowRecoveryCounts, Error> {
        self.capture()?.inventory().map_err(|_| Error::Changed)
    }
    #[cfg(test)]
    pub(super) fn digest(&self) -> Result<&str, Error> {
        self.capture()?.digest().map_err(|_| Error::Changed)
    }
    #[cfg(test)]
    pub(super) fn archive(&self) -> Result<&[u8], Error> {
        self.capture()?.archive().map_err(|_| Error::Changed)
    }
    pub(super) fn cancel(&mut self) {
        self.capture = None;
    }
    pub(super) fn revalidate(&mut self) -> Result<(), Error> {
        let result = (|| {
            self.roots.revalidate(&self.inspection)?;
            Workspace::new(self.roots.legacy())
                .revalidate_existing_capture(self.capture.as_mut().ok_or(Error::Changed)?)
                .map_err(|_| Error::Changed)?;
            self.roots.revalidate(&self.inspection)?;
            Ok(())
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
}

impl StartupRoots {
    fn binding_ancestry(&self) -> Result<Proof, Error> {
        if !cfg!(unix) {
            return Err(Error::UnsupportedPlatform);
        }
        if !self.anchor.is_absolute()
            || self.anchor.components().count() > 64
            || self.anchor.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(Error::Unavailable);
        }
        let mut proof = Proof { nodes: Vec::new() };
        for path in self
            .anchor
            .ancestors()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            proof.observe(path, true)?.ok_or(Error::Changed)?;
        }
        Ok(proof)
    }

    #[cfg(feature = "encrypted-workspace")]
    pub(super) fn empty_provisioning_candidate(
        &self,
        destination: &str,
    ) -> Result<EmptyProvisioningProof, Error> {
        rangoon_desktop::workspace_keys::WorkspaceId::parse(destination)
            .map_err(|_| Error::RecoveryRequired)?;
        let mut proof = self.binding_ancestry()?;
        let legacy = self.legacy();
        proof.observe(&legacy, true)?.ok_or(Error::Changed)?;
        if names(&legacy, 1)? != [std::ffi::OsString::from(DATABASE)] {
            return Err(Error::RecoveryRequired);
        }
        Self::database(&mut proof, &legacy)?;
        let managed = self.managed();
        proof.observe(&managed, true)?.ok_or(Error::Changed)?;
        let mut expected = [CONTROL, BINDING, PARENT, OPERATIONS[0]]
            .into_iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        expected.sort();
        if names(&managed, 4)? != expected {
            return Err(Error::RecoveryRequired);
        }
        let control = proof
            .observe(&managed.join(CONTROL), false)?
            .ok_or(Error::Changed)?;
        let binding = proof
            .observe(&managed.join(BINDING), false)?
            .ok_or(Error::Changed)?;
        let pending = proof
            .observe(&managed.join(OPERATIONS[0]), false)?
            .ok_or(Error::Changed)?;
        if control.len() != 0
            || binding.len() == 0
            || binding.len() > 4096
            || pending.len() == 0
            || pending.len() > 4096
            || proof
                .observe(&managed.join(OPERATIONS[1]), false)?
                .is_some()
        {
            return Err(Error::RecoveryRequired);
        }
        let parent = managed.join(PARENT);
        proof.observe(&parent, true)?.ok_or(Error::Changed)?;
        if names(&parent, 1)? != [std::ffi::OsString::from(destination)] {
            return Err(Error::RecoveryRequired);
        }
        let directory = parent.join(destination);
        proof.observe(&directory, true)?.ok_or(Error::Changed)?;
        if !names(&directory, 1)?.is_empty() {
            return Err(Error::RecoveryRequired);
        }
        for name in [
            DATABASE,
            "workspace.sqlite3-wal",
            "workspace.sqlite3-shm",
            "workspace.sqlite3-journal",
        ] {
            if proof.observe(&directory.join(name), false)?.is_some() {
                return Err(Error::RecoveryRequired);
            }
        }
        proof.check()?;
        Ok(EmptyProvisioningProof {
            roots: Self::new(self.anchor.clone()),
            proof,
            destination: destination.to_owned(),
        })
    }

    /// Inspect only the fixed destination. A retained original may legitimately
    /// diverge after conversion; it cannot replace destination validation.
    pub(super) fn existing_binding(&self) -> Result<BindingProof, Error> {
        let mut proof = self.binding_ancestry()?;
        proof
            .observe(&self.managed(), true)?
            .ok_or(Error::Changed)?;
        Self::managed_shape(&mut proof, &self.managed())?;
        let result = BindingProof {
            roots: Self::new(self.anchor.clone()),
            proof,
            kind: BindingKind::Complete,
        };
        result.check_before()?;
        Ok(result)
    }

    /// The sole resumable setup residue: one empty existing control, no data,
    /// binding, attempt, original workspace or unknown entry.
    pub(super) fn setup_residue_binding(&self) -> Result<BindingProof, Error> {
        let mut proof = self.binding_ancestry()?;
        if proof.observe(&self.legacy(), true)?.is_some()
            || proof
                .observe(&self.anchor.join("workspace-adoption"), true)?
                .is_some()
        {
            return Err(Error::RecoveryRequired);
        }
        proof
            .observe(&self.managed(), true)?
            .ok_or(Error::Changed)?;
        if names(&self.managed(), 2)? != [std::ffi::OsString::from(CONTROL)] {
            return Err(Error::RecoveryRequired);
        }
        if proof
            .observe(&self.managed().join(CONTROL), false)?
            .ok_or(Error::Changed)?
            .len()
            != 0
        {
            return Err(Error::RecoveryRequired);
        }
        let result = BindingProof {
            roots: Self::new(self.anchor.clone()),
            proof,
            kind: BindingKind::Residue,
        };
        result.check_before()?;
        Ok(result)
    }

    /// Called only for an explicit native setup action. Creates metadata, never
    /// a key, database or a replacement for an observed existing destination.
    pub(super) fn fresh_binding(&self, mut inspection: Inspection) -> Result<BindingProof, Error> {
        if self.revalidate(&inspection)? != State::FreshCandidate {
            return Err(Error::Changed);
        }
        let mut proof = inspection.proof.take().ok_or(Error::Changed)?;
        if proof
            .observe(&self.anchor.join("workspace-adoption"), true)?
            .is_some()
        {
            return Err(Error::RecoveryRequired);
        }
        // Missing native app-data ancestry is created one component at a time.
        // Update only the node just created; any concurrent appearance refuses.
        for index in 0..proof.nodes.len() {
            let node = &proof.nodes[index];
            if node.metadata.is_some() || !self.anchor.starts_with(&node.path) {
                continue;
            }
            let path = node.path.clone();
            proof.check()?;
            create_startup_directory(&path)?;
            proof.nodes[index].metadata =
                Some(fs::symlink_metadata(&path).map_err(|_| Error::Unavailable)?);
            proof.check()?;
        }
        let managed = self.managed();
        proof.check()?;
        create_startup_directory(&managed)?;
        let node = proof
            .nodes
            .iter_mut()
            .find(|node| node.path == managed)
            .ok_or(Error::Changed)?;
        if node.metadata.is_some() {
            return Err(Error::Changed);
        }
        node.metadata = Some(fs::symlink_metadata(&managed).map_err(|_| Error::Unavailable)?);
        let result = BindingProof {
            roots: Self::new(self.anchor.clone()),
            proof,
            kind: BindingKind::Fresh,
        };
        result.check_before()?;
        Ok(result)
    }
}

#[cfg(feature = "encrypted-workspace")]
impl EmptyProvisioningProof {
    pub(super) fn check(&self) -> Result<(), Error> {
        self.proof.check()?;
        let current = self.roots.empty_provisioning_candidate(&self.destination)?;
        current.proof.check()?;
        self.proof.check()
    }
}

fn create_startup_directory(path: &Path) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| Error::Changed)?;
    let parent = path.parent().ok_or(Error::Changed)?;
    fs::File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Unavailable)
}

impl BindingProof {
    pub(super) fn root(&self) -> PathBuf {
        self.roots.managed()
    }
    pub(super) fn existing(&self) -> bool {
        !matches!(self.kind, BindingKind::Fresh)
    }
    pub(super) fn check_before(&self) -> Result<(), Error> {
        self.check(false)
    }
    pub(super) fn check_after(&self) -> Result<(), Error> {
        self.check(true)
    }
    fn check(&self, bound: bool) -> Result<(), Error> {
        self.proof.check()?;
        match self.kind {
            BindingKind::Complete => {
                let mut observed = Proof { nodes: Vec::new() };
                StartupRoots::managed_shape(&mut observed, &self.root())?;
                observed.check()?;
            }
            BindingKind::Fresh if !bound => {
                if !names(&self.root(), 1)?.is_empty() {
                    return Err(Error::Changed);
                }
            }
            BindingKind::Residue | BindingKind::Fresh => {
                if names(&self.root(), 2)? != [std::ffi::OsString::from(CONTROL)] {
                    return Err(Error::Changed);
                }
                let metadata =
                    fs::symlink_metadata(self.root().join(CONTROL)).map_err(|_| Error::Changed)?;
                if !regular(&metadata) || metadata.file_type().is_symlink() || metadata.len() != 0 {
                    return Err(Error::Changed);
                }
            }
        }
        self.proof.check()
    }
}

#[cfg(test)]
#[path = "workspace_startup_tests.rs"]
mod tests;
