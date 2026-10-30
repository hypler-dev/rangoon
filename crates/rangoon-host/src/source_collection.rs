//! Bounded read-only inspection of files explicitly selected by the native user.
//! This is neither directory discovery nor trusted filesystem custody for writes.
use super::{PublicError, SelectionResult, analyze_selected_path};
use serde::Serialize;
use std::path::PathBuf;

pub const MAX_COLLECTION_FILES: usize = 32;
pub const MAX_COLLECTION_BYTES: u64 = 4 * 1024 * 1024;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCollectionItem {
    pub selection_index: u32,
    pub display_name: String,
    pub result: SelectionResult,
}
#[derive(Debug, Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SourceCollectionResult {
    Cancelled,
    Inspected {
        schema_version: &'static str,
        selected_count: u32,
        retained_bytes: u64,
        items: Vec<SourceCollectionItem>,
    },
    Failed {
        error: PublicError,
    },
}
impl SourceCollectionResult {
    pub fn failed(code: &'static str, message: &'static str) -> Self {
        Self::Failed {
            error: PublicError { code, message },
        }
    }
    pub fn stale() -> Self {
        Self::failed(
            "source_collection_cleared",
            "File inspection was cleared. Choose files again.",
        )
    }
}
/// Call only with paths returned by a fresh native multi-file picker. No path is
/// serialized. Cancellation is checked before every file and before delivery.
pub fn inspect_selected_sources(
    paths: &[PathBuf],
    current: impl Fn() -> bool,
) -> SourceCollectionResult {
    if paths.is_empty() || paths.len() > MAX_COLLECTION_FILES {
        return SourceCollectionResult::failed(
            "source_collection_limit",
            "Choose between 1 and 32 Markdown files. No files were read.",
        );
    }
    let mut items = Vec::with_capacity(paths.len());
    let mut retained_bytes = 0u64;
    let mut budget_exhausted = false;
    for (index, path) in paths.iter().enumerate() {
        if !current() {
            return SourceCollectionResult::stale();
        }
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.len() <= 255 && !name.chars().any(char::is_control))
            .unwrap_or("Unsupported file")
            .to_owned();
        let result = if budget_exhausted {
            SelectionResult::failed(
                "source_collection_budget",
                "Not read: this selection reached its 4 MiB retained-content limit. Choose a smaller batch.",
            )
        } else {
            match analyze_selected_path(path) {
                SelectionResult::Analyzed { report } => {
                    // Native single-file analysis already bounds each input at
                    // 256 KiB. At most one such report is discarded on overflow;
                    // after overflow no further selected paths are read.
                    let next = retained_bytes.checked_add(report.source.byte_length);
                    if next.is_none_or(|bytes| bytes > MAX_COLLECTION_BYTES) {
                        budget_exhausted = true;
                        SelectionResult::failed(
                            "source_collection_budget",
                            "Not retained: this file exceeds the batch's remaining 4 MiB content budget. Choose a smaller batch.",
                        )
                    } else {
                        retained_bytes = next.unwrap();
                        SelectionResult::Analyzed { report }
                    }
                }
                other => other,
            }
        };
        items.push(SourceCollectionItem {
            selection_index: index as u32,
            display_name,
            result,
        });
    }
    if !current() {
        return SourceCollectionResult::stale();
    }
    SourceCollectionResult::Inspected {
        schema_version: "rangoon.source-collection.v1",
        selected_count: paths.len() as u32,
        retained_bytes,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "rangoon-source-collection-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn explicit_files_remain_unchanged_with_partial_refusals_and_no_paths_in_wire() {
        let root = Root::new();
        let a = root.file("AGENTS.md", b"# Rules\r\nKeep originals.\r\n");
        let bad = root.file("binary.md", b"bad\0bytes");
        let paths = vec![a.clone(), bad];
        let result = inspect_selected_sources(&paths, || true);
        let wire = serde_json::to_value(&result).unwrap();
        assert_eq!(wire["outcome"], "inspected");
        assert_eq!(wire["selectedCount"], 2);
        assert_eq!(wire["items"][0]["result"]["outcome"], "analyzed");
        assert_eq!(wire["items"][1]["result"]["outcome"], "rejected");
        assert_eq!(fs::read(a).unwrap(), b"# Rules\r\nKeep originals.\r\n");
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains(root.0.to_str().unwrap())
        );
    }
    #[test]
    fn count_limits_and_cancellation_refuse_before_any_file_read() {
        use std::cell::Cell;
        let calls = Cell::new(0);
        let paths = vec![PathBuf::from("must-not-be-read.md"); 33];
        assert!(matches!(
            inspect_selected_sources(&paths, || {
                calls.set(calls.get() + 1);
                true
            }),
            SourceCollectionResult::Failed { .. }
        ));
        assert_eq!(calls.get(), 0);
        assert!(matches!(
            inspect_selected_sources(&[], || true),
            SourceCollectionResult::Failed { .. }
        ));
        assert!(matches!(
            inspect_selected_sources(&paths[..1], || false),
            SourceCollectionResult::Failed {
                error: PublicError {
                    code: "source_collection_cleared",
                    ..
                }
            }
        ));
    }
    #[test]
    fn budget_has_explicit_outcomes_and_stops_further_reads() {
        let root = Root::new();
        // A compact source near the 256 KiB bound: 16 long lines.
        let data = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n".repeat(7900);
        assert!(data.len() < 262144);
        let file = root.file("large.md", &data);
        let mut paths = vec![file; 17];
        paths.push(root.0.join("missing-after-budget.md"));
        let SourceCollectionResult::Inspected {
            items,
            retained_bytes,
            ..
        } = inspect_selected_sources(&paths, || true)
        else {
            panic!("expected inspected");
        };
        assert!(retained_bytes <= MAX_COLLECTION_BYTES);
        assert_eq!(items.len(), 18);
        assert!(matches!(
            items[17].result,
            SelectionResult::Failed {
                error: PublicError {
                    code: "source_collection_budget",
                    ..
                }
            }
        ));
    }
    #[cfg(unix)]
    #[test]
    fn selected_link_is_refused_and_duplicate_names_retain_selection_indices() {
        use std::os::unix::fs::symlink;
        let root = Root::new();
        let a = root.file("AGENTS.md", b"# Scope\nBounded.\n");
        let link = root.0.join("link.md");
        symlink(&a, &link).unwrap();
        let SourceCollectionResult::Inspected { items, .. } =
            inspect_selected_sources(&[a.clone(), a, link], || true)
        else {
            panic!("expected collection");
        };
        assert_eq!(items[0].selection_index, 0);
        assert_eq!(items[1].selection_index, 1);
        assert!(matches!(items[2].result, SelectionResult::Rejected { .. }));
    }
    #[test]
    fn cancellation_between_files_drops_all_retained_content() {
        use std::cell::Cell;
        let root = Root::new();
        let file = root.file("AGENTS.md", b"# Synthetic private content\n");
        let checks = Cell::new(0);
        let result = inspect_selected_sources(&[file, root.0.join("must-not-read.md")], || {
            let n = checks.get() + 1;
            checks.set(n);
            n == 1
        });
        assert!(matches!(
            &result,
            SourceCollectionResult::Failed {
                error: PublicError {
                    code: "source_collection_cleared",
                    ..
                }
            }
        ));
        assert_eq!(checks.get(), 2);
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("Synthetic private content")
        );
    }
}
