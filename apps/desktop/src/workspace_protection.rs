//! Read-only layout observations. They do not authenticate storage or keys.
use super::*;
use std::ffi::{OsStr, OsString};

/// Metadata-only observations for a future source-free protection interface.
/// Locked does not prove decryptability, key availability or tamper resistance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Absent,
    Locked,
    Busy,
    RecoveryRequired,
    Unavailable,
}

const MAX_ENTRIES: usize = 8;

/// This is a bounded, nonrecursive, path-based observation, not a
/// descriptor-relative filesystem security boundary. No observed name escapes.
fn entries(path: &Path, allowed: &[&OsStr]) -> Result<Vec<OsString>, Error> {
    plain_directory(path)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(path).map_err(|_| Error::Unavailable)? {
        let entry = entry.map_err(|_| Error::Unavailable)?;
        let name = entry.file_name();
        if names.len() >= MAX_ENTRIES || !allowed.contains(&name.as_os_str()) {
            return Err(Error::RecoveryRequired);
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| Error::Unavailable)?;
        if linked(&metadata) {
            return Err(Error::RecoveryRequired);
        }
        names.push(name);
    }
    names.sort();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::RecoveryRequired);
    }
    Ok(names)
}

impl Owner {
    /// Does not prepare an attempt, open the database, generate randomness or
    /// access a vault. Callers must still revalidate actual setup/unlock actions.
    pub fn inspect_layout(&self) -> Layout {
        self.inspect_layout_after(|| {})
    }

    #[cfg(all(test, feature = "encrypted-workspace"))]
    pub(super) fn inspect_layout_after_observation_for_test(
        &self,
        after_database: impl FnOnce(),
    ) -> Layout {
        self.inspect_layout_after(after_database)
    }

    fn inspect_layout_after(&self, after_database: impl FnOnce()) -> Layout {
        if self.recovery {
            return Layout::RecoveryRequired;
        }
        if self.opened.is_some() || self.pending.is_some() || self.reconciliation.is_some() {
            return Layout::Busy;
        }
        match self.layout_inner(after_database) {
            Ok(layout) => layout,
            Err(Error::Unavailable | Error::BackendUnavailable) => Layout::Unavailable,
            Err(_) => Layout::RecoveryRequired,
        }
    }

    fn layout_inner(&self, after_database: impl FnOnce()) -> Result<Layout, Error> {
        self.check_control().map_err(|_| Error::Unavailable)?;
        let root_allowed = [
            OsStr::new(CONTROL),
            OsStr::new(BINDING),
            OsStr::new(recovery::FILES[0]),
            OsStr::new(recovery::FILES[1]),
            OsStr::new(PARENT),
        ];
        let root_names = entries(&self.root, &root_allowed)?;
        match fs::symlink_metadata(self.root.join(BINDING)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.absent_binding()?;
                if root_names != [OsString::from(CONTROL)]
                    || entries(&self.root, &root_allowed)? != root_names
                {
                    return Err(Error::RecoveryRequired);
                }
                self.check_control().map_err(|_| Error::Unavailable)?;
                return Ok(Layout::Absent);
            }
            Err(_) => return Err(Error::Unavailable),
            Ok(_) => {}
        }

        let observed = self.read_binding()?;
        let binding = Binding::decode(&observed)?;
        if !binding.ready {
            return Err(Error::RecoveryRequired);
        }
        let checkpoint = self.terminal_operation(&binding)?;
        let directories = self.directories(&binding)?;
        let identity = binding.identity.as_hex();
        let parent = self.root.join(PARENT);
        let directory = self.directory(&binding);
        let parent_allowed = [OsStr::new(&identity)];
        let database_allowed = [OsStr::new(DATABASE)];
        let parent_names = entries(&parent, &parent_allowed)?;
        let database_names = entries(&directory, &database_allowed)?;
        if parent_names != [OsString::from(&identity)]
            || database_names != [OsString::from(DATABASE)]
        {
            return Err(Error::RecoveryRequired);
        }
        let database = self.database(&binding)?;
        after_database();

        self.check_control().map_err(|_| Error::Unavailable)?;
        if entries(&self.root, &root_allowed)? != root_names
            || self.read_binding()? != observed
            || self.terminal_operation(&binding)? != checkpoint
            || entries(&parent, &parent_allowed)? != parent_names
            || entries(&directory, &database_allowed)? != database_names
        {
            return Err(Error::RecoveryRequired);
        }
        self.unchanged_directories(&binding, &directories)?;
        // Existing unchanged_database checks identity alone. This observation
        // also rejects a same-inode resize, even if both sizes are bounded.
        let current_database = self.database(&binding)?;
        if !same_file(&database, &current_database) || database.len() != current_database.len() {
            return Err(Error::RecoveryRequired);
        }
        self.check_control().map_err(|_| Error::Unavailable)?;
        Ok(Layout::Locked)
    }
}
