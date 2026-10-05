use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use rangoon_host::{SelectionResult, analyze_selected_path};
use rangoon_import::MAX_SOURCE_BYTES;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rangoon-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn error_code(result: SelectionResult) -> &'static str {
    match result {
        SelectionResult::Rejected { error } | SelectionResult::Failed { error } => error.code,
        _ => panic!("expected an error"),
    }
}

#[test]
fn selected_fixture_matches_the_cli_contract_without_disclosing_path() {
    let fixture = Fixture::new();
    let path = fixture.write(
        "AGENTS.md",
        include_bytes!("../../../fixtures/contracts/AGENTS.md"),
    );
    let result = analyze_selected_path(&path);
    let SelectionResult::Analyzed { report } = &result else {
        panic!("expected report")
    };
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/contracts/source-analysis.golden.json"
    ))
    .unwrap();
    assert_eq!(serde_json::to_value(report).unwrap(), expected);
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(fixture.0.to_str().unwrap())
    );
}

#[test]
fn reports_fixed_errors_without_source_or_path() {
    let fixture = Fixture::new();
    let missing = fixture.0.join("private-name.md");
    let result = analyze_selected_path(&missing);
    let encoded = serde_json::to_string(&result).unwrap();
    assert!(!encoded.contains("private-name"));
    assert!(!encoded.contains(fixture.0.to_str().unwrap()));
    assert_eq!(error_code(result), "read_failed");
    assert_eq!(
        error_code(analyze_selected_path(
            &fixture.write("secret.txt", b"private")
        )),
        "unsupported_format"
    );
    assert_eq!(
        error_code(analyze_selected_path(
            &fixture.write("secret.md", b"private\0")
        )),
        "binary_input"
    );
    assert_eq!(
        error_code(analyze_selected_path(&fixture.write("invalid.md", &[0xff]))),
        "invalid_utf8"
    );
}

#[test]
fn rejects_large_files_and_folders() {
    let fixture = Fixture::new();
    let path = fixture.write("large.md", &vec![b'a'; MAX_SOURCE_BYTES + 1]);
    assert_eq!(error_code(analyze_selected_path(&path)), "input_too_large");
    let dir = fixture.0.join("folder.md");
    fs::create_dir(&dir).unwrap();
    assert_eq!(error_code(analyze_selected_path(&dir)), "unsupported_file");
}

#[test]
fn empty_selected_file_is_analyzed_with_explicit_diagnostic() {
    let fixture = Fixture::new();
    let SelectionResult::Analyzed { report } =
        analyze_selected_path(&fixture.write("empty.md", b""))
    else {
        panic!("expected empty report")
    };
    assert_eq!(report.source.byte_length, 0);
    assert!(report.fragments.is_empty());
    assert_eq!(
        serde_json::to_value(SelectionResult::Cancelled).unwrap(),
        serde_json::json!({"outcome":"cancelled"})
    );
}

#[cfg(unix)]
#[test]
fn rejects_final_symlink_and_socket_without_reading_target() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let fixture = Fixture::new();
    let target = fixture.write("target.md", b"# should not read\n");
    let link = fixture.0.join("link.md");
    symlink(target, &link).unwrap();
    assert_eq!(error_code(analyze_selected_path(&link)), "unsupported_file");
    let socket = fixture.0.join("socket.md");
    let _listener = UnixListener::bind(&socket).unwrap();
    assert_eq!(
        error_code(analyze_selected_path(&socket)),
        "unsupported_file"
    );
}

#[cfg(windows)]
#[test]
fn rejects_windows_file_and_directory_reparse_points() {
    use std::os::windows::fs::{MetadataExt, symlink_dir, symlink_file};
    let fixture = Fixture::new();
    let target = fixture.write("target.md", b"# should not read\n");
    let file_link = fixture.0.join("link.md");
    // Do not silently skip: the Windows CI runner must permit link fixtures.
    symlink_file(target, &file_link).expect("Windows link fixture requires symlink privilege");
    assert_ne!(
        fs::symlink_metadata(&file_link).unwrap().file_attributes() & 0x400,
        0
    );
    assert_eq!(
        error_code(analyze_selected_path(&file_link)),
        "unsupported_file"
    );
    let directory = fixture.0.join("target-directory");
    fs::create_dir(&directory).unwrap();
    let directory_link = fixture.0.join("directory.md");
    symlink_dir(directory, &directory_link)
        .expect("Windows directory link fixture requires symlink privilege");
    assert_eq!(
        error_code(analyze_selected_path(&directory_link)),
        "unsupported_file"
    );
}
