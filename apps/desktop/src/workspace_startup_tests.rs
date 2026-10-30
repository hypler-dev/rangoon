use super::*;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
const DATABASE: &str = "workspace.sqlite3";
#[cfg(unix)]
const CONTROL: &str = "encrypted-workspace.lock";
#[cfg(unix)]
const BINDING: &str = "encrypted-workspace.binding";
#[cfg(unix)]
const PARENT: &str = "encrypted-workspaces";

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

struct TestDir {
    root: PathBuf,
}

impl TestDir {
    fn new(label: &str) -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let root = parent.join(format!(
            "rangoon-startup-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
        }
    }

    #[cfg(unix)]
    fn join(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn write(path: impl AsRef<Path>, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
}

#[cfg(unix)]
fn legacy(root: &Path, bytes: &[u8]) {
    let directory = root.join("source-workspace");
    fs::create_dir(&directory).unwrap();
    write(directory.join(DATABASE), bytes);
}

#[cfg(unix)]
fn managed(root: &Path) {
    let directory = root.join("managed-workspace");
    fs::create_dir(&directory).unwrap();
    write(directory.join(CONTROL), b"");
    write(directory.join(BINDING), b"fake-binding");
    let parent = directory.join(PARENT);
    fs::create_dir(&parent).unwrap();
    let workspace = parent.join("a".repeat(64));
    fs::create_dir(&workspace).unwrap();
    write(workspace.join(DATABASE), b"fake-database-metadata-only");
}

fn assert_state(root: &Path, expected: State) {
    assert_eq!(
        StartupRoots::new(root.to_owned()).inspect().state(),
        expected
    );
}

#[cfg(unix)]
#[test]
fn missing_and_empty_anchors_are_fresh_without_filesystem_mutation() {
    let directory = TestDir::new("fresh");
    let missing = directory.join("missing-anchor");
    assert_state(&missing, State::FreshCandidate);
    assert!(!missing.exists());
    assert!(fs::read_dir(&directory.root).unwrap().next().is_none());

    assert_state(&directory.root, State::FreshCandidate);
    assert!(fs::read_dir(&directory.root).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn legacy_candidate_uses_nonempty_database_metadata_only() {
    let directory = TestDir::new("legacy");
    legacy(&directory.root, b"not-a-sqlite-database");

    assert_state(&directory.root, State::LegacyCandidate);
    assert_eq!(
        fs::read(directory.join("source-workspace").join(DATABASE)).unwrap(),
        b"not-a-sqlite-database"
    );
}

#[cfg(unix)]
#[test]
fn managed_candidate_accepts_only_bounded_shape_presence() {
    let directory = TestDir::new("managed");
    managed(&directory.root);

    assert_state(&directory.root, State::ManagedCandidate);
    assert_eq!(
        fs::read(directory.join("managed-workspace").join(BINDING)).unwrap(),
        b"fake-binding"
    );
}

#[cfg(unix)]
#[test]
fn both_known_layouts_are_mixed() {
    let directory = TestDir::new("mixed");
    legacy(&directory.root, b"legacy-only-metadata");
    managed(&directory.root);

    assert_state(&directory.root, State::Mixed);
}

#[cfg(unix)]
#[test]
fn partial_empty_sidecar_or_unknown_layouts_require_recovery() {
    let empty_legacy = TestDir::new("empty-legacy");
    fs::create_dir(empty_legacy.join("source-workspace")).unwrap();
    assert_state(&empty_legacy.root, State::RecoveryRequired);

    let zero_database = TestDir::new("zero-database");
    legacy(&zero_database.root, b"");
    assert_state(&zero_database.root, State::RecoveryRequired);

    let sidecar = TestDir::new("legacy-sidecar");
    legacy(&sidecar.root, b"metadata");
    write(
        sidecar
            .join("source-workspace")
            .join("workspace.sqlite3-wal"),
        b"sidecar",
    );
    assert_state(&sidecar.root, State::RecoveryRequired);

    let orphan = TestDir::new("managed-orphan");
    let managed_root = orphan.join("managed-workspace");
    fs::create_dir(&managed_root).unwrap();
    write(managed_root.join(CONTROL), b"");
    write(managed_root.join(BINDING), b"binding");
    fs::create_dir(managed_root.join(PARENT)).unwrap();
    assert_state(&orphan.root, State::RecoveryRequired);

    let unknown = TestDir::new("unknown-managed-entry");
    managed(&unknown.root);
    write(
        unknown.join("managed-workspace").join("unexpected"),
        b"unknown",
    );
    assert_state(&unknown.root, State::RecoveryRequired);
}

#[cfg(unix)]
#[test]
fn candidate_inspection_creates_no_cipher_key_or_owner_artifacts() {
    let directory = TestDir::new("inert-candidates");
    let missing = directory.join("missing");
    assert_state(&missing, State::FreshCandidate);
    assert!(!missing.exists());

    legacy(&directory.root, b"metadata");
    assert_state(&directory.root, State::LegacyCandidate);
    assert!(!directory.join("cipher").exists());
    assert!(!directory.join("key").exists());
    assert!(!directory.join("owner").exists());

    fs::remove_dir_all(directory.join("source-workspace")).unwrap();
    managed(&directory.root);
    assert_state(&directory.root, State::ManagedCandidate);
    assert!(!directory.join("cipher").exists());
    assert!(!directory.join("key").exists());
    assert!(!directory.join("owner").exists());
}

#[cfg(all(unix, target_family = "unix"))]
#[test]
fn symlink_and_hardlink_paths_require_recovery_without_touching_targets() {
    use std::os::unix::fs::symlink;

    let root_link = TestDir::new("root-link");
    let root_target = root_link.join("target");
    fs::create_dir(&root_target).unwrap();
    write(root_target.join("sentinel"), b"root-target");
    let alias = root_link.join("alias");
    symlink(&root_target, &alias).unwrap();
    assert_state(&alias, State::RecoveryRequired);
    assert_eq!(
        fs::read(root_target.join("sentinel")).unwrap(),
        b"root-target"
    );

    let ancestor_link = TestDir::new("ancestor-link");
    let physical = ancestor_link.join("physical");
    fs::create_dir(&physical).unwrap();
    let anchor = physical.join("anchor");
    fs::create_dir(&anchor).unwrap();
    write(physical.join("sentinel"), b"ancestor-target");
    let linked_parent = ancestor_link.join("linked-parent");
    symlink(&physical, &linked_parent).unwrap();
    assert_state(&linked_parent.join("anchor"), State::RecoveryRequired);
    assert_eq!(
        fs::read(physical.join("sentinel")).unwrap(),
        b"ancestor-target"
    );

    let file_link = TestDir::new("database-link");
    let target = file_link.join("database-target");
    write(&target, b"database-target");
    fs::create_dir(file_link.join("source-workspace")).unwrap();
    symlink(&target, file_link.join("source-workspace").join(DATABASE)).unwrap();
    assert_state(&file_link.root, State::RecoveryRequired);
    assert_eq!(fs::read(&target).unwrap(), b"database-target");

    let hard_link = TestDir::new("database-hardlink");
    legacy(&hard_link.root, b"database-target");
    let database = hard_link.join("source-workspace").join(DATABASE);
    let alias = hard_link.join("database-alias");
    fs::hard_link(&database, &alias).unwrap();
    assert_state(&hard_link.root, State::RecoveryRequired);
    assert_eq!(fs::read(&alias).unwrap(), b"database-target");
}

#[cfg(unix)]
#[test]
fn stale_preview_rejects_root_replacement_and_new_layout() {
    let replaced = TestDir::new("replaced-root");
    let anchor = replaced.join("anchor");
    fs::create_dir(&anchor).unwrap();
    legacy(&anchor, b"metadata");
    let roots = StartupRoots::new(anchor.clone());
    let inspection = roots.inspect();
    assert_eq!(inspection.state(), State::LegacyCandidate);
    fs::remove_dir_all(&anchor).unwrap();
    fs::create_dir(&anchor).unwrap();
    legacy(&anchor, b"replacement-metadata");
    assert_eq!(roots.revalidate(&inspection), Err(Error::Changed));

    let new_layout = TestDir::new("new-layout");
    let roots = StartupRoots::new(new_layout.root.clone());
    let inspection = roots.inspect();
    assert_eq!(inspection.state(), State::FreshCandidate);
    managed(&new_layout.root);
    assert_eq!(roots.revalidate(&inspection), Err(Error::Changed));
}

#[cfg(all(unix, target_family = "unix"))]
#[test]
fn held_control_lock_still_reports_shape_only_without_mutation() {
    use std::fs::OpenOptions;

    let directory = TestDir::new("held-control-lock");
    managed(&directory.root);
    let control = directory.join("managed-workspace").join(CONTROL);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&control)
        .unwrap();
    lock.lock().unwrap();
    assert_state(&directory.root, State::ManagedCandidate);
    assert_eq!(fs::metadata(&control).unwrap().len(), 0);
}

#[cfg(unix)]
#[test]
fn debug_output_is_fixed_and_redacts_paths() {
    let directory = TestDir::new("debug");
    let roots = StartupRoots::new(directory.root.clone());
    let inspection = roots.inspect();

    assert_eq!(format!("{roots:?}"), "StartupRoots(<redacted>)");
    assert_eq!(format!("{inspection:?}"), "Inspection(<redacted>)");
    assert!(!format!("{roots:?}").contains(directory.root.to_string_lossy().as_ref()));
}

#[cfg(unix)]
fn saved_legacy(directory: &TestDir) -> Workspace {
    let workspace = Workspace::new(directory.join("source-workspace"));
    let report = rangoon_import::analyze(
        "AGENTS.md",
        b"# Private startup marker\nRetain the exact original source.\n",
    )
    .unwrap();
    workspace.save(&report).unwrap();
    workspace
}

#[cfg(unix)]
#[test]
fn retained_real_capture_revalidates_and_cancel_clears_all_accessors() {
    let directory = TestDir::new("real-capture");
    saved_legacy(&directory);
    let database = directory.join("source-workspace").join(DATABASE);
    let before = fs::read(&database).unwrap();
    let roots = StartupRoots::new(directory.root.clone());
    assert_eq!(roots.inspect().state(), State::LegacyCandidate);
    let mut retained = roots.capture_existing().unwrap();
    assert_eq!(retained.schema_version().unwrap(), 1);
    assert_eq!(retained.inventory().unwrap().sources, 1);
    assert!(!retained.digest().unwrap().is_empty());
    assert!(
        retained
            .archive()
            .unwrap()
            .starts_with(b"RANGOON-BACKUP-V2\n")
    );
    assert_eq!(format!("{retained:?}"), "RetainedSource(<redacted>)");
    retained.revalidate().unwrap();
    assert_eq!(fs::read(&database).unwrap(), before);
    assert_eq!(
        fs::read_dir(directory.join("source-workspace"))
            .unwrap()
            .count(),
        1
    );

    retained.cancel();
    assert_eq!(retained.schema_version(), Err(Error::Changed));
    assert_eq!(retained.inventory(), Err(Error::Changed));
    assert_eq!(retained.digest(), Err(Error::Changed));
    assert_eq!(retained.archive(), Err(Error::Changed));
    assert_eq!(retained.revalidate(), Err(Error::Changed));
}

#[cfg(unix)]
#[test]
fn retained_capture_refuses_domain_drift_and_new_managed_layout() {
    let directory = TestDir::new("real-drift");
    let workspace = saved_legacy(&directory);
    let roots = StartupRoots::new(directory.root.clone());
    let mut retained = roots.capture_existing().unwrap();
    workspace
        .save(&rangoon_import::analyze("SKILL.md", b"# Another source\nNew content.\n").unwrap())
        .unwrap();
    assert_eq!(retained.revalidate(), Err(Error::Changed));
    assert_eq!(retained.archive(), Err(Error::Changed));

    let mut retained = roots.capture_existing().unwrap();
    managed(&directory.root);
    assert_eq!(retained.revalidate(), Err(Error::Changed));
    assert_eq!(retained.inventory(), Err(Error::Changed));
    assert_eq!(roots.inspect().state(), State::Mixed);
}

#[cfg(unix)]
#[test]
fn adoption_handoff_checks_legacy_only_once_then_retains_existing_source_proof() {
    let directory = TestDir::new("adoption-handoff");
    saved_legacy(&directory);
    let roots = StartupRoots::new(directory.root.clone());
    let retained = roots.capture_existing().unwrap();
    let expected = retained.archive().unwrap().to_vec();
    let mut capture = retained.into_adoption_capture().unwrap();
    let fingerprint = capture.identity_fingerprint().unwrap();
    managed(&directory.root);
    assert_eq!(roots.inspect().state(), State::Mixed);
    let workspace = rangoon_store::Workspace::new(roots.legacy());
    workspace.revalidate_existing_capture(&mut capture).unwrap();
    assert_eq!(capture.archive().unwrap(), expected);
    assert_eq!(capture.identity_fingerprint().unwrap(), fingerprint);
    capture.cancel();
    assert!(capture.identity_fingerprint().is_err());
    assert!(roots.capture_existing().is_err());
}

#[cfg(unix)]
#[test]
fn capture_does_not_treat_metadata_candidate_as_valid_sqlite() {
    let directory = TestDir::new("invalid-candidate");
    legacy(&directory.root, b"not-a-sqlite-database");
    let roots = StartupRoots::new(directory.root.clone());
    assert_eq!(roots.inspect().state(), State::LegacyCandidate);
    assert!(matches!(roots.capture_existing(), Err(Error::Unavailable)));
    assert_eq!(
        fs::read(directory.join("source-workspace").join(DATABASE)).unwrap(),
        b"not-a-sqlite-database"
    );
    assert_eq!(
        fs::read_dir(directory.join("source-workspace"))
            .unwrap()
            .count(),
        1
    );
}

#[cfg(unix)]
fn root_names(path: &Path) -> Vec<String> {
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[cfg(unix)]
fn control_only(path: &Path) {
    assert_eq!(root_names(path), [CONTROL]);
    assert_eq!(fs::read(path.join(CONTROL)).unwrap(), b"");
    for absent in [BINDING, PARENT, DATABASE, "workspace-adoption"] {
        assert!(!path.join(absent).exists(), "{absent} must not exist");
    }
}

#[cfg(unix)]
#[test]
fn fresh_binding_creates_only_fixed_metadata_for_missing_anchor_ancestry() {
    let directory = TestDir::new("fresh-binding");
    let anchor = directory.join("missing-parent").join("anchor");
    write(directory.join("other-app-state"), b"unchanged");
    let roots = StartupRoots::new(anchor.clone());
    let inspection = roots.inspect();
    assert_eq!(inspection.state(), State::FreshCandidate);
    assert!(!anchor.exists());

    let proof = roots.fresh_binding(inspection).unwrap();
    assert_eq!(proof.root(), anchor.join("managed-workspace"));
    assert!(!proof.existing());
    proof.check_before().unwrap();
    assert_eq!(
        root_names(&directory.root),
        ["missing-parent", "other-app-state"]
    );
    assert_eq!(
        fs::read(directory.join("other-app-state")).unwrap(),
        b"unchanged"
    );
    assert_eq!(root_names(&anchor), ["managed-workspace"]);
    assert!(root_names(&proof.root()).is_empty());
    assert!(!anchor.join("source-workspace").exists());
    assert!(!anchor.join("workspace-adoption").exists());
    assert_eq!(proof.check_after(), Err(Error::Changed));

    write(proof.root().join("unexpected"), b"unexpected");
    assert_eq!(proof.check_before(), Err(Error::Changed));
    assert_eq!(proof.check_after(), Err(Error::Changed));
    fs::remove_file(proof.root().join("unexpected")).unwrap();
    write(proof.root().join(CONTROL), b"");
    proof.check_after().unwrap();
    control_only(&proof.root());
}

#[cfg(unix)]
#[test]
fn fresh_binding_refuses_stale_or_recovery_observations_before_creating_managed_root() {
    let stale = TestDir::new("fresh-stale");
    let roots = StartupRoots::new(stale.root.clone());
    let inspection = roots.inspect();
    legacy(&stale.root, b"legacy metadata");
    assert!(matches!(
        roots.fresh_binding(inspection),
        Err(Error::Changed)
    ));
    assert!(!roots.managed().exists());
    assert_eq!(root_names(&stale.root), ["source-workspace"]);

    let unknown = TestDir::new("fresh-managed-unknown");
    fs::create_dir(unknown.join("managed-workspace")).unwrap();
    write(
        unknown.join("managed-workspace").join("unrecognized"),
        b"do not replace this",
    );
    let roots = StartupRoots::new(unknown.root.clone());
    let inspection = roots.inspect();
    assert_eq!(inspection.state(), State::RecoveryRequired);
    assert!(matches!(
        roots.fresh_binding(inspection),
        Err(Error::Changed)
    ));
    assert_eq!(root_names(&unknown.root), ["managed-workspace"]);
    assert_eq!(root_names(&roots.managed()), ["unrecognized"]);
    assert_eq!(
        fs::read(roots.managed().join("unrecognized")).unwrap(),
        b"do not replace this"
    );
}

#[cfg(unix)]
#[test]
fn setup_residue_binding_accepts_only_one_zero_byte_regular_control() {
    let valid = TestDir::new("residue-valid");
    fs::create_dir(valid.join("managed-workspace")).unwrap();
    write(valid.join("managed-workspace").join(CONTROL), b"");
    let roots = StartupRoots::new(valid.root.clone());
    let proof = roots.setup_residue_binding().unwrap();
    assert_eq!(proof.root(), roots.managed());
    assert!(proof.existing());
    proof.check_before().unwrap();
    proof.check_after().unwrap();
    control_only(&roots.managed());

    for label in ["binding", "unknown", "nonzero"] {
        let directory = TestDir::new(label);
        let managed = directory.join("managed-workspace");
        fs::create_dir(&managed).unwrap();
        write(managed.join(CONTROL), b"");
        match label {
            "binding" => write(managed.join(BINDING), b"binding"),
            "unknown" => write(managed.join("unknown"), b"unknown"),
            "nonzero" => write(managed.join(CONTROL), b"nonzero"),
            _ => unreachable!(),
        }
        assert!(matches!(
            StartupRoots::new(directory.root.clone()).setup_residue_binding(),
            Err(Error::RecoveryRequired)
        ));
        assert_eq!(
            root_names(&managed).len(),
            if label == "unknown" || label == "binding" {
                2
            } else {
                1
            }
        );
    }

    for label in ["legacy", "journal"] {
        let directory = TestDir::new(label);
        let managed = directory.join("managed-workspace");
        fs::create_dir(&managed).unwrap();
        write(managed.join(CONTROL), b"");
        match label {
            "legacy" => legacy(&directory.root, b"legacy metadata"),
            "journal" => fs::create_dir(directory.join("workspace-adoption")).unwrap(),
            _ => unreachable!(),
        }
        assert!(matches!(
            StartupRoots::new(directory.root.clone()).setup_residue_binding(),
            Err(Error::RecoveryRequired)
        ));
        control_only(&managed);
    }
}

#[cfg(unix)]
#[test]
fn existing_binding_retains_managed_identity_while_ignoring_original_divergence() {
    let directory = TestDir::new("existing-proof");
    managed(&directory.root);
    let roots = StartupRoots::new(directory.root.clone());
    let proof = roots.existing_binding().unwrap();
    assert_eq!(proof.root(), roots.managed());
    assert!(proof.existing());
    proof.check_before().unwrap();

    legacy(&directory.root, b"later original divergence");
    proof.check_before().unwrap();
    proof.check_after().unwrap();
    assert_eq!(
        fs::read(roots.legacy().join(DATABASE)).unwrap(),
        b"later original divergence"
    );
    assert!(!directory.join("workspace-adoption").exists());
}

#[cfg(unix)]
#[test]
fn existing_binding_rejects_replaced_managed_root_or_control_without_creation() {
    let root = TestDir::new("existing-root-replaced");
    managed(&root.root);
    let roots = StartupRoots::new(root.root.clone());
    let proof = roots.existing_binding().unwrap();
    let retained = root.join("retained-managed");
    fs::rename(roots.managed(), &retained).unwrap();
    managed(&root.root);
    assert_eq!(proof.check_before(), Err(Error::Changed));
    assert!(!root.join("workspace-adoption").exists());

    let control = TestDir::new("existing-control-replaced");
    managed(&control.root);
    let roots = StartupRoots::new(control.root.clone());
    let proof = roots.existing_binding().unwrap();
    let retained = roots.managed().join("retained-control");
    fs::rename(roots.managed().join(CONTROL), &retained).unwrap();
    write(roots.managed().join(CONTROL), b"");
    assert_eq!(proof.check_after(), Err(Error::Changed));
    assert_eq!(fs::read(&retained).unwrap(), b"");
    assert_eq!(fs::read(roots.managed().join(CONTROL)).unwrap(), b"");
    assert!(!control.join("workspace-adoption").exists());
}

#[cfg(all(unix, target_family = "unix"))]
#[test]
fn setup_residue_and_fresh_after_checks_reject_linked_controls() {
    use std::os::unix::fs::symlink;

    let linked = TestDir::new("residue-symlink");
    let target = linked.join("target");
    write(&target, b"");
    fs::create_dir(linked.join("managed-workspace")).unwrap();
    symlink(&target, linked.join("managed-workspace").join(CONTROL)).unwrap();
    assert!(matches!(
        StartupRoots::new(linked.root.clone()).setup_residue_binding(),
        Err(Error::RecoveryRequired)
    ));
    assert_eq!(fs::read(target).unwrap(), b"");

    let hard_link = TestDir::new("fresh-hardlink");
    let roots = StartupRoots::new(hard_link.root.clone());
    let proof = roots.fresh_binding(roots.inspect()).unwrap();
    let target = hard_link.join("control-target");
    write(&target, b"");
    fs::hard_link(&target, proof.root().join(CONTROL)).unwrap();
    assert_eq!(proof.check_after(), Err(Error::Changed));
    assert_eq!(fs::read(&target).unwrap(), b"");
}

#[cfg(not(unix))]
#[test]
fn inspection_reports_unsupported_platform() {
    let directory = TestDir::new("unsupported");
    assert_state(&directory.root, State::UnsupportedPlatform);
}

/// Presence fixtures are intentionally not valid Owner records. Only the
/// native Owner can interpret the operation and key after explicit consent.
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn empty_provisioning_shape(root: &Path) -> String {
    let identity = "a".repeat(64);
    legacy(root, b"presence-only-original");
    let managed = root.join("managed-workspace");
    fs::create_dir(&managed).unwrap();
    write(managed.join(CONTROL), b"");
    write(managed.join(BINDING), b"presence-only-binding");
    write(managed.join(OPERATIONS[0]), b"presence-only-pending");
    fs::create_dir(managed.join(PARENT)).unwrap();
    fs::create_dir(managed.join(PARENT).join(&identity)).unwrap();
    identity
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
#[test]
fn empty_provisioning_presence_never_substitutes_for_complete_managed_binding() {
    let directory = TestDir::new("empty-presence");
    let identity = empty_provisioning_shape(&directory.root);
    let roots = StartupRoots::new(directory.root.clone());
    let proof = roots.empty_provisioning_candidate(&identity).unwrap();
    proof.check().unwrap();
    assert_state(&directory.root, State::RecoveryRequired);
    assert!(roots.existing_binding().is_err());
    assert!(
        fs::read_dir(roots.managed().join(PARENT).join(identity))
            .unwrap()
            .next()
            .is_none()
    );
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
#[test]
fn empty_provisioning_presence_rejects_foreign_or_noncanonical_destination() {
    let directory = TestDir::new("empty-presence-identity");
    empty_provisioning_shape(&directory.root);
    let roots = StartupRoots::new(directory.root.clone());
    for destination in [
        "0".repeat(64),
        "A".repeat(64),
        "b".repeat(64),
        "../escape".into(),
    ] {
        assert!(roots.empty_provisioning_candidate(&destination).is_err());
    }
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
#[test]
fn empty_provisioning_presence_rejects_partial_sidecar_terminal_and_extra_files() {
    for case in [
        "zero",
        "partial",
        "wal",
        "terminal",
        "extra-root",
        "extra-parent",
    ] {
        let directory = TestDir::new(case);
        let identity = empty_provisioning_shape(&directory.root);
        let roots = StartupRoots::new(directory.root.clone());
        let managed = roots.managed();
        let destination = managed.join(PARENT).join(&identity);
        match case {
            "zero" => write(destination.join(DATABASE), b""),
            "partial" => write(destination.join(DATABASE), b"partial"),
            "wal" => write(destination.join("workspace.sqlite3-wal"), b""),
            "terminal" => write(managed.join(OPERATIONS[1]), b"existing-terminal"),
            "extra-root" => write(managed.join("unexpected"), b"retained"),
            "extra-parent" => fs::create_dir(managed.join(PARENT).join("b".repeat(64))).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            roots.empty_provisioning_candidate(&identity).is_err(),
            "{case}"
        );
    }
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
#[test]
fn empty_provisioning_presence_revalidates_absence_and_directory_identities() {
    for case in ["database", "directory", "parent", "binding"] {
        let directory = TestDir::new(case);
        let identity = empty_provisioning_shape(&directory.root);
        let roots = StartupRoots::new(directory.root.clone());
        let proof = roots.empty_provisioning_candidate(&identity).unwrap();
        let managed = roots.managed();
        let destination = managed.join(PARENT).join(&identity);
        match case {
            "database" => write(destination.join(DATABASE), b""),
            "directory" => {
                fs::rename(&destination, directory.join("retained-directory")).unwrap();
                fs::create_dir(&destination).unwrap();
            }
            "parent" => {
                fs::rename(managed.join(PARENT), directory.join("retained-parent")).unwrap();
                fs::create_dir(managed.join(PARENT)).unwrap();
                fs::create_dir(&destination).unwrap();
            }
            "binding" => {
                let path = managed.join(BINDING);
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, directory.join("retained-binding")).unwrap();
                write(path, &bytes);
            }
            _ => unreachable!(),
        }
        assert!(proof.check().is_err(), "{case}");
    }
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
#[test]
fn empty_provisioning_presence_rejects_linked_records_and_destination() {
    use std::os::unix::fs::symlink;
    for case in ["hardlink", "symlink", "directory"] {
        let directory = TestDir::new(case);
        let identity = empty_provisioning_shape(&directory.root);
        let roots = StartupRoots::new(directory.root.clone());
        let managed = roots.managed();
        match case {
            "hardlink" => {
                fs::hard_link(managed.join(BINDING), directory.join("linked-binding")).unwrap()
            }
            "symlink" => {
                let path = managed.join(BINDING);
                let retained = directory.join("retained-binding");
                fs::rename(&path, &retained).unwrap();
                symlink(retained, path).unwrap();
            }
            "directory" => {
                let path = managed.join(PARENT).join(&identity);
                let retained = directory.join("retained-directory");
                fs::rename(&path, &retained).unwrap();
                symlink(retained, path).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            roots.empty_provisioning_candidate(&identity).is_err(),
            "{case}"
        );
    }
}
