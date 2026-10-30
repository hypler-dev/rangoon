//! Incremental metadata observations of a native-selected directory object.
//! No content read, hash, writes, renderer paths, transport or snapshot claim.
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryError {
    UnsupportedPlatform,
    InvalidRoot,
    Unavailable,
    Cancelled,
    Closed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialReason {
    ReadError,
    ChangedDirectory,
    OtherDevice,
    RepeatedDirectory,
    RepeatedEntry,
    DepthLimit,
    EntryLimit,
    RecordLimit,
    UnsupportedName,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileObservation {
    /// Session-local ordinal; not a persistent file or revision identity.
    pub observation_id: u64,
    pub relative_path: String,
    pub byte_length: u64,
    /// Observed filesystem time, at whole-second precision. Not freshness proof.
    pub modified_at_ms: Option<u64>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryPage {
    pub schema_version: &'static str,
    pub entries: Vec<FileObservation>,
    pub visited_entries: u64,
    pub emitted_records: u64,
    pub excluded_entries: u64,
    /// Traversal stopped/exhausted; inspect partial_reasons independently.
    pub exhausted: bool,
    pub partial_reasons: Vec<PartialReason>,
}

pub const MAX_FOLDER_COLLECTION_BYTES: usize = 2 * 1024 * 1024;
fn folder_window_bound(entries: &[FileObservation]) -> usize {
    // Conservative JSON upper bound: fixed envelope + per-row keys/numbers,
    // then exact string escaping. No original bytes are serialized.
    2048 + entries
        .iter()
        .map(|row| {
            256 + row
                .relative_path
                .chars()
                .map(|c| match c {
                    '"' | '\\' => 2,
                    c if c.is_control() => 6,
                    c => c.len_utf8(),
                })
                .sum::<usize>()
        })
        .sum::<usize>()
}

/// One bounded, metadata-only window. No directory capability survives delivery.
#[derive(Debug, Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum FolderCollectionResult {
    Cancelled,
    Listed {
        schema_version: &'static str,
        root_label: String,
        entries: Vec<FileObservation>,
        visited_entries: u64,
        emitted_records: u64,
        excluded_entries: u64,
        exhausted: bool,
        partial_reasons: Vec<PartialReason>,
    },
    Failed {
        error: crate::PublicError,
    },
}
impl FolderCollectionResult {
    pub fn failed(code: &'static str, message: &'static str) -> Self {
        Self::Failed {
            error: crate::PublicError { code, message },
        }
    }
    pub fn stale() -> Self {
        Self::failed(
            "folder_inventory_cleared",
            "Folder listing was cleared. Choose the folder again.",
        )
    }
}
/// Native picker paths only. Metadata is bounded and never retained as source.
pub fn list_selected_root(path: &Path, current: impl Fn() -> bool) -> FolderCollectionResult {
    let label = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|name| name.len() <= 255 && !name.chars().any(char::is_control))
        .unwrap_or("Selected folder")
        .to_owned();
    #[cfg(unix)]
    let label = if unix::safe_component(&label) {
        label
    } else {
        "Selected folder".to_owned()
    };
    let mut inventory = match SelectedRootInventory::open_selected_root(path, &current) {
        Ok(inventory) => inventory,
        Err(InventoryError::Cancelled) => return FolderCollectionResult::stale(),
        Err(InventoryError::UnsupportedPlatform) => {
            return FolderCollectionResult::failed(
                "folder_inventory_unsupported",
                "Folder discovery is unavailable on this platform. Choose individual files instead.",
            );
        }
        Err(_) => {
            return FolderCollectionResult::failed(
                "folder_inventory_unavailable",
                "The selected folder could not be listed. Choose an available folder without linked ancestors.",
            );
        }
    };
    let mut entries = Vec::new();
    loop {
        match inventory.next_page(&current) {
            Ok(mut page) => {
                entries.append(&mut page.entries);
                if folder_window_bound(&entries) > MAX_FOLDER_COLLECTION_BYTES {
                    inventory.close();
                    return FolderCollectionResult::failed(
                        "folder_inventory_budget",
                        "Folder metadata exceeds this view's 2 MiB limit. Choose a smaller folder.",
                    );
                }
                if page.exhausted {
                    inventory.close();
                    if !current() {
                        return FolderCollectionResult::stale();
                    }
                    return FolderCollectionResult::Listed {
                        schema_version: "rangoon.folder-collection.v1",
                        root_label: label,
                        entries,
                        visited_entries: page.visited_entries,
                        emitted_records: page.emitted_records,
                        excluded_entries: page.excluded_entries,
                        exhausted: true,
                        partial_reasons: page.partial_reasons,
                    };
                }
            }
            Err(_) => {
                inventory.close();
                return FolderCollectionResult::stale();
            }
        }
    }
}

/// Construct only from a fresh native folder selection. Never expose this
/// constructor as a renderer path API. Lock/cancel must close this owner.
pub struct SelectedRootInventory {
    #[cfg(unix)]
    inner: unix::Inventory,
}
impl SelectedRootInventory {
    pub fn open_selected_root(
        path: &Path,
        current: impl Fn() -> bool,
    ) -> Result<Self, InventoryError> {
        #[cfg(unix)]
        {
            unix::Inventory::open(path, current).map(|inner| Self { inner })
        }
        #[cfg(not(unix))]
        {
            let _ = (path, current);
            Err(InventoryError::UnsupportedPlatform)
        }
    }
    pub fn next_page(
        &mut self,
        current: impl Fn() -> bool,
    ) -> Result<InventoryPage, InventoryError> {
        #[cfg(unix)]
        {
            self.inner.next_page(current, |_| {})
        }
        #[cfg(not(unix))]
        {
            let _ = current;
            Err(InventoryError::UnsupportedPlatform)
        }
    }
    pub fn close(&mut self) {
        #[cfg(unix)]
        self.inner.close();
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, Stat, fstat, open, openat, statat};
    use std::collections::HashSet;
    use std::path::Component;

    const FLAGS: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);
    #[derive(Clone, Copy)]
    struct Limits {
        depth: usize,
        visited: u64,
        records: u64,
        page_records: usize,
        page_work: usize,
    }
    impl Default for Limits {
        fn default() -> Self {
            Self {
                depth: 16,
                visited: 10_000,
                records: 1000,
                page_records: 50,
                page_work: 512,
            }
        }
    }
    struct Frame {
        dir: Dir,
        relative: String,
        depth: usize,
    }
    pub(super) struct Inventory {
        frames: Vec<Frame>,
        directories: HashSet<(u64, u64)>,
        files: HashSet<String>,
        device: u64,
        visited: u64,
        emitted: u64,
        excluded: u64,
        reasons: Vec<PartialReason>,
        closed: bool,
        limits: Limits,
    }
    // Unix dev_t/ino_t widths differ across supported targets.
    #[allow(clippy::unnecessary_cast)]
    fn identity(stat: &Stat) -> (u64, u64) {
        (stat.st_dev as u64, stat.st_ino as u64)
    }
    fn directory(stat: &Stat) -> bool {
        FileType::from_raw_mode(stat.st_mode) == FileType::Directory
    }
    pub(super) fn safe_component(name: &str) -> bool {
        !name.is_empty() && name.encode_utf16().count() <= 255 && name != "." && name != ".." &&
        !name.chars().any(|c| (!c.is_ascii() && !c.is_alphanumeric()) || c.is_control() || matches!(c, '\\' | ':' | '%' | '?' | '#' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')) &&
        !name.ends_with(['.', ' ']) &&
        !matches!(name.split('.').next().unwrap_or("").to_ascii_lowercase().as_str(), "con" | "prn" | "aux" | "nul" | "com1" | "com2" | "com3" | "com4" | "com5" | "com6" | "com7" | "com8" | "com9" | "lpt1" | "lpt2" | "lpt3" | "lpt4" | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9")
    }
    fn excluded(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        matches!(
            lower.as_str(),
            "node_modules" | "target" | "vendor" | "dist" | "build"
        ) || name.starts_with('.')
            && !matches!(
                lower.as_str(),
                ".claude" | ".cursor" | ".windsurf" | ".github" | ".codex" | ".agents"
            )
    }
    impl Inventory {
        pub(super) fn open(
            path: &Path,
            current: impl Fn() -> bool,
        ) -> Result<Self, InventoryError> {
            if !current() {
                return Err(InventoryError::Cancelled);
            }
            if !path.is_absolute() || path.components().count() > 65 {
                return Err(InventoryError::InvalidRoot);
            }
            let mut fd =
                open("/", FLAGS, Mode::empty()).map_err(|_| InventoryError::Unavailable)?;
            let mut count = 0;
            for component in path.components() {
                if !current() {
                    return Err(InventoryError::Cancelled);
                }
                match component {
                    Component::RootDir => continue,
                    Component::Normal(name) => {
                        let before = statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW)
                            .map_err(|_| InventoryError::Unavailable)?;
                        if !directory(&before) {
                            return Err(InventoryError::InvalidRoot);
                        }
                        let next = openat(&fd, name, FLAGS, Mode::empty())
                            .map_err(|_| InventoryError::Unavailable)?;
                        let after = fstat(&next).map_err(|_| InventoryError::Unavailable)?;
                        if !directory(&after) || identity(&before) != identity(&after) {
                            return Err(InventoryError::Unavailable);
                        }
                        fd = next;
                        count += 1;
                    }
                    _ => return Err(InventoryError::InvalidRoot),
                }
            }
            // Never allow an accidental whole-filesystem root selection.
            if count == 0 {
                return Err(InventoryError::InvalidRoot);
            }
            let stat = fstat(&fd).map_err(|_| InventoryError::Unavailable)?;
            let dir = Dir::new(fd).map_err(|_| InventoryError::Unavailable)?;
            if !current() {
                return Err(InventoryError::Cancelled);
            }
            Ok(Self {
                frames: vec![Frame {
                    dir,
                    relative: String::new(),
                    depth: 0,
                }],
                directories: HashSet::from([identity(&stat)]),
                files: HashSet::new(),
                device: identity(&stat).0,
                visited: 0,
                emitted: 0,
                excluded: 0,
                reasons: Vec::new(),
                closed: false,
                limits: Limits::default(),
            })
        }
        fn reason(&mut self, reason: PartialReason) {
            if !self.reasons.contains(&reason) {
                self.reasons.push(reason);
            }
        }
        pub(super) fn close(&mut self) {
            self.frames.clear();
            self.directories.clear();
            self.files.clear();
            self.closed = true;
        }
        pub(super) fn next_page(
            &mut self,
            current: impl Fn() -> bool,
            mut before_directory_open: impl FnMut(&str),
        ) -> Result<InventoryPage, InventoryError> {
            if self.closed {
                return Err(InventoryError::Closed);
            }
            let mut entries = Vec::new();
            let mut work = 0;
            while !self.frames.is_empty()
                && entries.len() < self.limits.page_records
                && work < self.limits.page_work
            {
                if !current() {
                    self.close();
                    return Err(InventoryError::Cancelled);
                }
                if self.visited >= self.limits.visited {
                    self.reason(PartialReason::EntryLimit);
                    self.frames.clear();
                    break;
                }
                if self.emitted >= self.limits.records {
                    self.reason(PartialReason::RecordLimit);
                    self.frames.clear();
                    break;
                }
                let frame = self.frames.last_mut().unwrap();
                let item = match frame.dir.next() {
                    None => {
                        self.frames.pop();
                        continue;
                    }
                    Some(Err(_)) => {
                        self.frames.pop();
                        self.reason(PartialReason::ReadError);
                        continue;
                    }
                    Some(Ok(item)) => item,
                };
                work += 1;
                self.visited += 1;
                let name = match item.file_name().to_str() {
                    Ok("." | "..") => continue,
                    Ok(name) if safe_component(name) => name.to_owned(),
                    _ => {
                        self.reason(PartialReason::UnsupportedName);
                        continue;
                    }
                };
                if excluded(&name) {
                    self.excluded += 1;
                    continue;
                }
                let relative = if frame.relative.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{name}", frame.relative)
                };
                if relative.encode_utf16().count() > 1024 {
                    self.reason(PartialReason::UnsupportedName);
                    continue;
                }
                let fd = match frame.dir.fd() {
                    Ok(fd) => fd,
                    Err(_) => {
                        self.reason(PartialReason::ReadError);
                        continue;
                    }
                };
                let stat = match statat(fd, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(stat) => stat,
                    Err(_) => {
                        self.reason(PartialReason::ReadError);
                        continue;
                    }
                };
                if identity(&stat).0 != self.device {
                    self.reason(PartialReason::OtherDevice);
                    continue;
                }
                match FileType::from_raw_mode(stat.st_mode) {
                    FileType::Directory => {
                        if frame.depth >= self.limits.depth {
                            self.reason(PartialReason::DepthLimit);
                            continue;
                        }
                        before_directory_open(&relative);
                        let child = match openat(fd, name.as_str(), FLAGS, Mode::empty()) {
                            Ok(child) => child,
                            Err(error) => {
                                self.reason(
                                    if matches!(
                                        error,
                                        rustix::io::Errno::LOOP
                                            | rustix::io::Errno::NOTDIR
                                            | rustix::io::Errno::NOENT
                                    ) {
                                        PartialReason::ChangedDirectory
                                    } else {
                                        PartialReason::ReadError
                                    },
                                );
                                continue;
                            }
                        };
                        let opened = match fstat(&child) {
                            Ok(opened) => opened,
                            Err(_) => {
                                self.reason(PartialReason::ReadError);
                                continue;
                            }
                        };
                        if !directory(&opened) || identity(&stat) != identity(&opened) {
                            self.reason(PartialReason::ChangedDirectory);
                            continue;
                        }
                        if !self.directories.insert(identity(&opened)) {
                            self.reason(PartialReason::RepeatedDirectory);
                            continue;
                        }
                        let depth = frame.depth + 1;
                        match Dir::new(child) {
                            Ok(dir) => self.frames.push(Frame {
                                dir,
                                relative,
                                depth,
                            }),
                            Err(_) => self.reason(PartialReason::ReadError),
                        }
                    }
                    FileType::RegularFile
                        if name.to_ascii_lowercase().ends_with(".md") && stat.st_size >= 0 =>
                    {
                        if !self.files.insert(relative.clone()) {
                            self.reason(PartialReason::RepeatedEntry);
                            continue;
                        }
                        self.emitted += 1;
                        entries.push(FileObservation {
                            observation_id: self.emitted,
                            relative_path: relative,
                            byte_length: stat.st_size as u64,
                            modified_at_ms: u64::try_from(stat.st_mtime)
                                .ok()
                                .and_then(|seconds| seconds.checked_mul(1000)),
                        });
                    }
                    _ => self.excluded += 1,
                }
            }
            if !current() {
                self.close();
                return Err(InventoryError::Cancelled);
            }
            Ok(InventoryPage {
                schema_version: "rangoon.local-inventory.observation.v1",
                entries,
                visited_entries: self.visited,
                emitted_records: self.emitted,
                excluded_entries: self.excluded,
                exhausted: self.frames.is_empty(),
                partial_reasons: self.reasons.clone(),
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{
            fs,
            os::unix::fs::{FileTypeExt, symlink},
            sync::atomic::{AtomicU64, Ordering},
        };
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        struct Root(std::path::PathBuf);
        impl Root {
            fn new() -> Self {
                let path = std::env::temp_dir().join(format!(
                    "rangoon-inventory-{}-{}",
                    std::process::id(),
                    SERIAL.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&path).unwrap();
                Self(fs::canonicalize(path).unwrap())
            }
            fn file(&self, name: &str) {
                let path = self.0.join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, b"private content must not enter inventory").unwrap();
            }
            fn inventory(&self) -> Inventory {
                Inventory::open(&self.0, || true).unwrap()
            }
        }
        impl Drop for Root {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fn all(inventory: &mut Inventory) -> (Vec<FileObservation>, InventoryPage) {
            let mut rows = Vec::new();
            loop {
                let mut page = inventory.next_page(|| true, |_| {}).unwrap();
                rows.append(&mut page.entries);
                if page.exhausted {
                    return (rows, page);
                }
            }
        }
        #[test]
        fn aggregate_metadata_json_budget_bounds_wide_unicode_paths() {
            let rows = (0..1000)
                .map(|index| FileObservation {
                    observation_id: index,
                    relative_path: "界".repeat(1000),
                    byte_length: u64::MAX,
                    modified_at_ms: Some(u64::MAX),
                })
                .collect::<Vec<_>>();
            assert!(folder_window_bound(&rows) > MAX_FOLDER_COLLECTION_BYTES);
            let encoded = serde_json::to_vec(&rows).unwrap();
            assert!(encoded.len() < folder_window_bound(&rows));
        }
        #[test]
        fn collected_window_has_only_metadata_and_clears_on_late_cancellation() {
            let root = Root::new();
            root.file("AGENTS.md");
            let successful_calls = std::cell::Cell::new(0);
            let result = list_selected_root(&root.0, || {
                successful_calls.set(successful_calls.get() + 1);
                true
            });
            let encoded = serde_json::to_value(result).unwrap();
            assert_eq!(encoded["outcome"], "listed");
            assert_eq!(encoded["entries"].as_array().unwrap().len(), 1);
            assert!(!encoded.to_string().contains("private content"));
            assert!(!encoded.to_string().contains(root.0.to_str().unwrap()));
            let calls = std::cell::Cell::new(0);
            let result = list_selected_root(&root.0, || {
                calls.set(calls.get() + 1);
                calls.get() < successful_calls.get()
            });
            assert_eq!(serde_json::to_value(result).unwrap()["outcome"], "failed");
        }
        #[test]
        fn metadata_only_relative_markdown_and_explicit_policy() {
            let root = Root::new();
            for name in [
                "AGENTS.md",
                "project/CLAUDE.MD",
                ".claude/SKILL.md",
                ".cursor/rules/example.md",
                ".ssh/secret.md",
                ".git/history.md",
                "node_modules/no.md",
                "target/no.md",
                "notes.txt",
                ".env",
                "vendor/no.md",
            ] {
                root.file(name);
            }
            let (rows, page) = all(&mut root.inventory());
            assert_eq!(rows.len(), 4);
            assert!(page.partial_reasons.is_empty());
            assert!(page.excluded_entries >= 7);
            for row in &rows {
                assert!(!row.relative_path.starts_with('/'));
                assert!(row.modified_at_ms.is_some());
            }
            let encoded = serde_json::to_string(&rows).unwrap();
            assert!(!encoded.contains("private content"));
            assert!(!encoded.contains(root.0.to_str().unwrap()));
            assert!(!encoded.contains("sha256"));
        }
        #[test]
        fn symlink_root_and_ancestor_are_refused() {
            let root = Root::new();
            root.file("real/AGENTS.md");
            symlink(root.0.join("real"), root.0.join("alias")).unwrap();
            assert!(Inventory::open(&root.0.join("alias"), || true).is_err());
            fs::create_dir(root.0.join("real/sub")).unwrap();
            assert!(Inventory::open(&root.0.join("alias/sub"), || true).is_err());
            assert!(matches!(
                Inventory::open(Path::new("/"), || true),
                Err(InventoryError::InvalidRoot)
            ));
            assert!(Inventory::open(Path::new("relative"), || true).is_err());
        }
        #[test]
        fn child_symlinks_and_special_files_are_not_followed() {
            let root = Root::new();
            let outside = Root::new();
            outside.file("SECRET.md");
            root.file("AGENTS.md");
            symlink(&outside.0, root.0.join("escape")).unwrap();
            symlink(outside.0.join("SECRET.md"), root.0.join("link.md")).unwrap();
            symlink(&root.0, root.0.join("loop")).unwrap();
            assert!(
                std::process::Command::new("/usr/bin/mkfifo")
                    .arg(root.0.join("fifo.md"))
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(
                fs::symlink_metadata(root.0.join("fifo.md"))
                    .unwrap()
                    .file_type()
                    .is_fifo()
            );
            let socket = std::os::unix::net::UnixListener::bind(root.0.join("socket.md")).unwrap();
            assert!(
                fs::symlink_metadata(root.0.join("socket.md"))
                    .unwrap()
                    .file_type()
                    .is_socket()
            );
            let (rows, page) = all(&mut root.inventory());
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].relative_path, "AGENTS.md");
            assert!(page.excluded_entries >= 5);
            drop(socket);
        }
        #[test]
        fn directory_substitution_between_stat_and_open_cannot_escape() {
            let root = Root::new();
            let outside = Root::new();
            root.file("child/AGENTS.md");
            outside.file("SECRET.md");
            let mut inventory = root.inventory();
            let page = inventory
                .next_page(
                    || true,
                    |relative| {
                        if relative == "child" {
                            fs::rename(root.0.join("child"), root.0.join("retired")).unwrap();
                            symlink(&outside.0, root.0.join("child")).unwrap();
                        }
                    },
                )
                .unwrap();
            assert!(
                page.partial_reasons
                    .contains(&PartialReason::ChangedDirectory)
            );
            assert!(
                !page
                    .entries
                    .iter()
                    .any(|row| row.relative_path.contains("SECRET"))
            );
            let (rest, _) = all(&mut inventory);
            assert!(!rest.iter().any(|row| row.relative_path.contains("SECRET")));
        }
        #[test]
        fn directory_replaced_by_fifo_is_refused_without_opening_content() {
            let root = Root::new();
            root.file("child/AGENTS.md");
            let mut inventory = root.inventory();
            let page = inventory
                .next_page(
                    || true,
                    |relative| {
                        if relative == "child" {
                            fs::rename(root.0.join("child"), root.0.join("retired")).unwrap();
                            assert!(
                                std::process::Command::new("/usr/bin/mkfifo")
                                    .arg(root.0.join("child"))
                                    .status()
                                    .unwrap()
                                    .success()
                            );
                        }
                    },
                )
                .unwrap();
            assert!(
                page.partial_reasons
                    .contains(&PartialReason::ChangedDirectory)
            );
            assert!(
                !page
                    .entries
                    .iter()
                    .any(|row| row.relative_path == "child/AGENTS.md")
            );
        }
        #[test]
        fn root_path_replacement_does_not_redirect_held_directory_object() {
            let root = Root::new();
            let outside = Root::new();
            root.file("AGENTS.md");
            outside.file("SECRET.md");
            let mut inventory = root.inventory();
            let moved = root.0.with_extension("moved");
            fs::rename(&root.0, &moved).unwrap();
            symlink(&outside.0, &root.0).unwrap();
            let (rows, _) = all(&mut inventory);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].relative_path, "AGENTS.md");
            inventory.close();
            fs::remove_file(&root.0).unwrap();
            fs::rename(moved, &root.0).unwrap();
        }
        #[test]
        fn bounded_incremental_pages_and_session_ordinals() {
            let root = Root::new();
            for n in 0..121 {
                root.file(&format!("file-{n}.md"));
            }
            let mut inventory = root.inventory();
            let first = inventory.next_page(|| true, |_| {}).unwrap();
            assert_eq!(first.entries.len(), 50);
            assert!(!first.exhausted);
            let second = inventory.next_page(|| true, |_| {}).unwrap();
            assert_eq!(second.entries.len(), 50);
            assert_eq!(second.entries[0].observation_id, 51);
            let (rest, last) = all(&mut inventory);
            assert_eq!(rest.len(), 21);
            assert_eq!(last.emitted_records, 121);
            assert!(last.partial_reasons.is_empty());
        }
        #[test]
        fn production_record_budget_stops_without_claiming_complete_coverage() {
            let root = Root::new();
            for index in 0..1001 {
                root.file(&format!("file-{index}.md"));
            }
            let (rows, page) = all(&mut root.inventory());
            assert_eq!(rows.len(), 1000);
            assert_eq!(page.emitted_records, 1000);
            assert!(page.exhausted);
            assert!(page.partial_reasons.contains(&PartialReason::RecordLimit));
        }
        #[test]
        fn work_depth_and_total_limits_are_explicit() {
            let root = Root::new();
            for n in 0..20 {
                root.file(&format!("file-{n}.md"));
            }
            root.file("child/deeper/a.md");
            let mut inventory = root.inventory();
            inventory.limits.page_work = 2;
            let page = inventory.next_page(|| true, |_| {}).unwrap();
            assert!(page.visited_entries <= 2);
            assert!(!page.exhausted);
            let mut inventory = root.inventory();
            inventory.limits.records = 3;
            let (rows, page) = all(&mut inventory);
            assert_eq!(rows.len(), 3);
            assert!(page.partial_reasons.contains(&PartialReason::RecordLimit));
            let mut inventory = root.inventory();
            inventory.limits.visited = 3;
            let (_, page) = all(&mut inventory);
            assert!(page.partial_reasons.contains(&PartialReason::EntryLimit));
            let mut inventory = root.inventory();
            inventory.limits.depth = 0;
            let (_, page) = all(&mut inventory);
            assert!(page.partial_reasons.contains(&PartialReason::DepthLimit));
        }
        #[test]
        fn cancel_before_open_and_during_page_closes_owner_and_discards_page() {
            let root = Root::new();
            root.file("a.md");
            assert!(matches!(
                Inventory::open(&root.0, || false),
                Err(InventoryError::Cancelled)
            ));
            let mut inventory = root.inventory();
            let calls = std::cell::Cell::new(0);
            let result = inventory.next_page(
                || {
                    calls.set(calls.get() + 1);
                    calls.get() < 3
                },
                |_| {},
            );
            assert!(matches!(result, Err(InventoryError::Cancelled)));
            assert!(inventory.frames.is_empty());
            assert!(matches!(
                inventory.next_page(|| true, |_| {}),
                Err(InventoryError::Closed)
            ));
        }
        #[test]
        fn invalid_names_are_reported_not_normalized_or_lossily_decoded() {
            let root = Root::new();
            for name in ["a%2f.md", "NUL.md", "bad\u{202e}.md", "line\n.md"] {
                root.file(name);
            }
            // APFS refuses non-UTF-8 names at creation. Linux can exercise the
            // actual directory-entry branch with a disposable byte-name file.
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::ffi::OsStringExt;
                fs::write(
                    root.0
                        .join(std::ffi::OsString::from_vec(vec![0xff, b'.', b'm', b'd'])),
                    b"hidden",
                )
                .unwrap();
            }
            root.file("café.md");
            let (rows, page) = all(&mut root.inventory());
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].relative_path, "café.md");
            assert!(
                page.partial_reasons
                    .contains(&PartialReason::UnsupportedName)
            );
        }
        #[test]
        fn device_and_repeated_directory_refusals_are_explicit() {
            let root = Root::new();
            root.file("child/AGENTS.md");
            let mut inventory = root.inventory();
            inventory.device = inventory.device.wrapping_add(1);
            let (rows, page) = all(&mut inventory);
            assert!(rows.is_empty());
            assert!(page.partial_reasons.contains(&PartialReason::OtherDevice));
            let mut inventory = root.inventory();
            let child = open(root.0.join("child"), FLAGS, Mode::empty()).unwrap();
            inventory
                .directories
                .insert(identity(&fstat(&child).unwrap()));
            let (rows, page) = all(&mut inventory);
            assert!(rows.is_empty());
            assert!(
                page.partial_reasons
                    .contains(&PartialReason::RepeatedDirectory)
            );
        }
        #[test]
        fn close_is_terminal_and_exact_root_errors_do_not_echo_paths() {
            let root = Root::new();
            let mut inventory =
                SelectedRootInventory::open_selected_root(&root.0, || true).unwrap();
            inventory.close();
            assert!(matches!(
                inventory.next_page(|| true),
                Err(InventoryError::Closed)
            ));
            assert_eq!(
                serde_json::to_string(&InventoryError::Unavailable).unwrap(),
                "\"unavailable\""
            );
        }
    }
}
