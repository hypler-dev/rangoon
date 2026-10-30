//! Explicit E1c-4 target. No entry, fault selector, or key deletion in product builds.
#[cfg(not(target_os = "macos"))]
pub(crate) fn run() {
    eprintln!("e1c4: macOS qualification only");
    std::process::exit(78);
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn case_is(_: &str) -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn before_ready(_: &tauri::AppHandle) {}
#[cfg(not(target_os = "macos"))]
pub(crate) fn after_ready(_: &tauri::AppHandle, _: bool, _: bool) {}
#[cfg(not(target_os = "macos"))]
pub(crate) fn exited(_: &tauri::AppHandle) {}
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    adoption_checkpoint, after_ready, before_ready, case_is, consent_observed, exited, run,
    startup_observed, startup_vault,
};

#[cfg(target_os = "macos")]
mod macos {
    use crate::{SourceAccess, WorkspaceClose};
    use crate::{native_encrypted_case_plan as cases, native_encrypted_seeds as seeds};
    use cases::{CasePlan, Expectation, Prerequisite, Seed};
    use keyring_core::api::CredentialStoreApi;
    use rangoon_desktop::{
        workspace_keys::{Error, OsSlot, Store, WorkspaceId},
        workspace_session::Vault,
    };
    use serde::Deserialize;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeMap,
        fs::{self, File, OpenOptions},
        io::{BufRead, Read, Write},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
        path::{Path, PathBuf},
        sync::{
            Arc, Condvar, Mutex, OnceLock,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };
    use tauri::{AppHandle, Manager};
    use zeroize::Zeroizing;

    const IDENTIFIER: &str = "si.rangoon.encrypted-qualification";
    const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: http://asset.localhost; font-src 'self'; connect-src ipc: http://ipc.localhost; base-uri 'none'; form-action 'none'; frame-src 'none'; object-src 'none'";
    const PERMISSIONS: &[&str] = &[
        "allow-workspace-startup-status",
        "allow-workspace-startup-action",
        "allow-workspace-startup-quit",
        "allow-get-workspace-close-state",
        "allow-get-workspace-control-state",
        "allow-get-workspace-protection-state",
        "allow-prepare-workspace-control",
        "allow-confirm-workspace-control",
        "allow-cancel-workspace-control",
        "allow-get-workspace-request-state",
        "allow-begin-workspace-close",
        "allow-acknowledge-workspace-purge",
        "allow-select-and-analyze",
        "allow-list-snapshots",
        "allow-save-analysis",
        "allow-open-snapshot",
        "allow-read-composition-source",
        "allow-clear-analysis",
        "allow-get-engine-status",
        "allow-list-capabilities",
        "allow-open-capability",
        "allow-create-capability",
        "allow-revise-capability",
        "allow-review-capability",
        "allow-compile-capability",
        "allow-export-instruction-bundle",
        "allow-inspect-instruction-bundle",
        "allow-get-workspace-data",
        "allow-export-workspace-backup",
        "allow-create-workspace-recovery-key",
        "allow-export-encrypted-workspace-backup",
        "allow-prepare-encrypted-workspace-restore",
        "allow-prepare-workspace-restore",
        "allow-restore-workspace-backup",
        "allow-preview-composition",
        "allow-commit-composition",
        "allow-list-workflows",
        "allow-open-workflow",
        "allow-begin-workflow-draft",
        "allow-inspect-workflow-save",
        "allow-commit-workflow-save",
        "allow-clear-workflow-draft",
    ];
    static RUN: OnceLock<Run> = OnceLock::new();
    static ORIGINAL: OnceLock<seeds::Original> = OnceLock::new();
    static EVENTS: Mutex<u64> = Mutex::new(0);
    static EXITED: AtomicBool = AtomicBool::new(false);
    static OPENS: AtomicU64 = AtomicU64::new(0);
    static READS: AtomicU64 = AtomicU64::new(0);
    static WRITES: AtomicU64 = AtomicU64::new(0);
    static ADOPTION_HELD: AtomicBool = AtomicBool::new(false);

    // The barrier delays one genuine read before OS custody. No keys or results
    // are retained here, and ordinary builds do not include this module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ReadPhase {
        Idle,
        Armed,
        Holding,
        Released,
        Finished,
        Failed,
    }
    struct ReadBarrier {
        phase: Mutex<ReadPhase>,
        wake: Condvar,
    }
    impl ReadBarrier {
        const fn new() -> Self {
            Self {
                phase: Mutex::new(ReadPhase::Idle),
                wake: Condvar::new(),
            }
        }
        fn arm(&self, observed: impl FnOnce()) -> Result<(), ()> {
            let mut phase = self.phase.lock().map_err(|_| ())?;
            if *phase != ReadPhase::Idle {
                return Err(());
            }
            *phase = ReadPhase::Armed;
            observed();
            Ok(())
        }
        fn release(&self, observed: impl FnOnce()) -> Result<(), ()> {
            let mut phase = self.phase.lock().map_err(|_| ())?;
            if *phase != ReadPhase::Holding {
                return Err(());
            }
            observed();
            *phase = ReadPhase::Released;
            self.wake.notify_all();
            Ok(())
        }
        fn wait(&self, timeout: Duration, held: impl FnOnce()) -> Result<bool, ()> {
            let mut phase = self.phase.lock().map_err(|_| ())?;
            match *phase {
                ReadPhase::Idle | ReadPhase::Finished => return Ok(false),
                ReadPhase::Armed => *phase = ReadPhase::Holding,
                _ => return Err(()),
            }
            let deadline = Instant::now() + timeout;
            held();
            while *phase == ReadPhase::Holding {
                let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                    *phase = ReadPhase::Failed;
                    return Err(());
                };
                let (next, _) = self.wake.wait_timeout(phase, remaining).map_err(|_| ())?;
                phase = next;
            }
            if *phase != ReadPhase::Released || Instant::now() >= deadline {
                *phase = ReadPhase::Failed;
                return Err(());
            }
            *phase = ReadPhase::Finished;
            Ok(true)
        }
    }
    static READ_BARRIER: ReadBarrier = ReadBarrier::new();

    #[test]
    fn q3_barrier_requires_hold_release_and_is_one_shot() {
        let barrier = Arc::new(ReadBarrier::new());
        assert_eq!(
            barrier.wait(Duration::ZERO, || panic!("not armed")),
            Ok(false)
        );
        assert!(barrier.release(|| {}).is_err());
        barrier.arm(|| {}).unwrap();
        assert!(barrier.arm(|| {}).is_err());
        assert!(barrier.release(|| {}).is_err());
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = barrier.clone();
        let held = std::thread::spawn(move || {
            worker.wait(Duration::from_secs(5), || tx.send(()).unwrap())
        });
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(barrier.wait(Duration::ZERO, || {}).is_err());
        barrier.release(|| {}).unwrap();
        assert_eq!(held.join().unwrap(), Ok(true));
        assert!(barrier.release(|| {}).is_err());
        assert!(barrier.arm(|| {}).is_err());
        assert_eq!(
            barrier.wait(Duration::ZERO, || panic!("already used")),
            Ok(false)
        );
    }
    #[test]
    fn q3_barrier_timeout_and_poison_never_resume_a_read() {
        let barrier = ReadBarrier::new();
        barrier.arm(|| {}).unwrap();
        assert!(barrier.wait(Duration::ZERO, || {}).is_err());
        assert!(barrier.release(|| {}).is_err());
        assert!(barrier.wait(Duration::from_secs(1), || {}).is_err());
        let poisoned = ReadBarrier::new();
        let _ = std::panic::catch_unwind(|| {
            let _guard = poisoned.phase.lock().unwrap();
            panic!("synthetic poison");
        });
        assert!(poisoned.arm(|| {}).is_err());
        assert!(poisoned.release(|| {}).is_err());
        assert!(poisoned.wait(Duration::ZERO, || {}).is_err());
    }

    #[derive(Clone, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Identity {
        dev: u64,
        ino: u64,
        uid: u32,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        schema: String,
        nonce: String,
        #[serde(rename = "casePlanSha256")]
        case_plan_sha256: String,
        directory: Identity,
        roots: BTreeMap<String, Identity>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Permit {
        schema: String,
        nonce: String,
        case: String,
        prior: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SlotRecord {
        schema: String,
        nonce: String,
        root: String,
        workspace_id: String,
    }
    struct Run {
        nonce: String,
        case: String,
        directory: PathBuf,
        manifest: Manifest,
        plan: CasePlan,
    }

    fn refuse(message: &'static str, code: i32) -> ! {
        eprintln!("e1c4: {message}");
        std::process::exit(code);
    }
    fn emit(event: &'static str, fields: Value) {
        let mut seq = EVENTS
            .lock()
            .unwrap_or_else(|_| refuse("event failure", 70));
        *seq += 1;
        let record = json!({"schema":"rangoon.e1c4.event.v1", "seq":*seq, "pid":std::process::id(), "case":RUN.get().map(|r|r.case.as_str()), "event":event, "fields":fields});
        let line = serde_json::to_vec(&record).unwrap_or_else(|_| refuse("event failure", 70));
        if line.len() > 8192 || *seq > 512 {
            refuse("event bound", 70);
        }
        let mut out = std::io::stdout().lock();
        if out
            .write_all(&line)
            .and_then(|_| out.write_all(b"\n"))
            .and_then(|_| out.flush())
            .is_err()
        {
            refuse("event output unavailable", 70);
        }
    }
    fn hex_nonce(value: &str) -> bool {
        value.len() == 32
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    fn same_directory(path: &Path, id: &Identity) -> bool {
        fs::symlink_metadata(path).is_ok_and(|m| {
            m.is_dir()
                && !m.file_type().is_symlink()
                && m.dev() == id.dev
                && m.ino() == id.ino
                && m.uid() == id.uid
                && m.mode() & 0o777 == 0o700
        })
    }
    fn read_record<T: serde::de::DeserializeOwned>(path: &Path, uid: u32) -> Result<T, ()> {
        let before = fs::symlink_metadata(path).map_err(|_| ())?;
        if !before.is_file()
            || before.file_type().is_symlink()
            || before.uid() != uid
            || before.mode() & 0o777 != 0o600
            || before.nlink() != 1
            || before.len() > 16384
        {
            return Err(());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| ())?;
        let after = file.metadata().map_err(|_| ())?;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err(());
        }
        let mut bytes = Vec::new();
        file.take(16385).read_to_end(&mut bytes).map_err(|_| ())?;
        if bytes.len() > 16384 {
            return Err(());
        }
        serde_json::from_slice(&bytes).map_err(|_| ())
    }
    fn write_new(path: &Path, value: &Value) -> Result<(), ()> {
        let bytes = serde_json::to_vec(value).map_err(|_| ())?;
        if bytes.len() > 16384 {
            return Err(());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| ())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| ())?;
        File::open(path.parent().ok_or(())?)
            .and_then(|d| d.sync_all())
            .map_err(|_| ())
    }
    fn plan_digest() -> String {
        Sha256::digest(cases::BYTES)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    impl Run {
        fn validate(&self) -> Result<(), ()> {
            if !same_directory(&self.directory, &self.manifest.directory) {
                return Err(());
            }
            for (name, id) in &self.manifest.roots {
                if id.uid != self.manifest.directory.uid
                    || !same_directory(&self.directory.join(name), id)
                {
                    return Err(());
                }
            }
            Ok(())
        }
        fn root(&self) -> PathBuf {
            self.directory.join(self.root_name())
        }
        fn case_spec(&self) -> &cases::Case {
            self.plan
                .case(&self.case)
                .unwrap_or_else(|| refuse("case plan mismatch", 64))
        }
        fn root_name(&self) -> &str {
            &self.case_spec().root
        }
        fn identifier(&self) -> String {
            format!("{IDENTIFIER}.{}", self.nonce)
        }
        fn registry(&self, root: &str) -> PathBuf {
            self.directory.join(format!("slot-{root}.json"))
        }
        fn slot(&self, root: &str) -> Result<SlotRecord, ()> {
            self.validate()?;
            let record: SlotRecord =
                read_record(&self.registry(root), self.manifest.directory.uid)?;
            if record.schema != "rangoon.e1c4.slot.v1"
                || record.nonce != self.nonce
                || record.root != root
                || WorkspaceId::parse(&record.workspace_id).is_err()
            {
                return Err(());
            }
            Ok(record)
        }
    }
    fn prepare_run(args: &[String]) -> Result<Run, ()> {
        let plan = cases::embedded()?;
        if args.len() != 2 || !hex_nonce(&args[0]) || plan.case(&args[1]).is_none() {
            return Err(());
        }
        let base = std::env::temp_dir().canonicalize().map_err(|_| ())?;
        let directory = base.join(format!("rangoon-e1c4-{}", args[0]));
        let meta = fs::symlink_metadata(&directory).map_err(|_| ())?;
        if !meta.is_dir() || meta.file_type().is_symlink() || meta.mode() & 0o777 != 0o700 {
            return Err(());
        }
        let manifest: Manifest = read_record(&directory.join("manifest.json"), meta.uid())?;
        if manifest.schema != "rangoon.e1c4.run.v2"
            || manifest.nonce != args[0]
            || manifest.case_plan_sha256 != plan_digest()
            || manifest.roots.len() != plan.roots.len()
            || !plan
                .roots
                .iter()
                .all(|name| manifest.roots.contains_key(name))
        {
            return Err(());
        }
        let run = Run {
            nonce: args[0].clone(),
            case: args[1].clone(),
            directory,
            manifest,
            plan,
        };
        run.validate()?;
        let permit: Permit = read_record(
            &run.directory.join(format!("permit-{}.json", run.case)),
            meta.uid(),
        )?;
        if permit.schema != "rangoon.e1c4.permit.v1"
            || permit.nonce != run.nonce
            || permit.case != run.case
        {
            return Err(());
        }
        let expected = match &run.case_spec().prerequisite {
            Prerequisite::Initial { .. } => None,
            Prerequisite::Completed { case } => Some(case.clone()),
            Prerequisite::Running { case } => Some(format!("{case}-running")),
            Prerequisite::AllStopped { .. } => Some("children-stopped".to_owned()),
        };
        if permit.prior != expected {
            return Err(());
        }
        if expected.is_none() && fs::read_dir(run.root()).map_err(|_| ())?.next().is_some() {
            return Err(());
        }
        write_new(
            &run.directory.join(format!("claimed-{}.json", run.case)),
            &json!({"pid":std::process::id(),"case":run.case}),
        )?;
        Ok(run)
    }
    pub(crate) fn case_is(value: &str) -> bool {
        RUN.get().is_some_and(|r| r.case == value)
    }

    struct CountingVault(crate::workspace_native_bootstrap::OsVault);
    pub(crate) fn startup_vault() -> impl Vault + Send {
        let run = RUN
            .get()
            .unwrap_or_else(|| refuse("qualification run unavailable", 64));
        if run.validate().is_err() {
            refuse("qualification root changed", 64);
        }
        CountingVault(crate::workspace_native_bootstrap::OsVault::new(
            run.identifier(),
        ))
    }
    struct CountingSlot(OsSlot);
    impl Vault for CountingVault {
        type Slot = CountingSlot;
        fn open(&self, identity: &WorkspaceId) -> Result<Self::Slot, Error> {
            RUN.get()
                .ok_or(Error::Unavailable)?
                .validate()
                .map_err(|_| Error::Unavailable)?;
            OPENS.fetch_add(1, Ordering::SeqCst);
            self.0.open(identity).map(CountingSlot)
        }
    }
    impl Store for CountingSlot {
        fn identity(&self) -> &WorkspaceId {
            self.0.identity()
        }
        fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
            let held = READ_BARRIER
                .wait(Duration::from_secs(120), || {
                    emit("read_barrier_held", json!({"beforeOsRead":true}));
                })
                .map_err(|()| {
                    emit("read_barrier_failed", json!({}));
                    Error::Unavailable
                })?;
            READS.fetch_add(1, Ordering::SeqCst);
            let result = self.0.read();
            if held {
                emit(
                    "read_barrier_read_returned",
                    json!({"present":result.as_ref().is_ok_and(|value|value.is_some())}),
                );
            }
            result
        }
        fn write(&self, envelope: &[u8]) -> Result<(), Error> {
            let run = RUN.get().ok_or(Error::Unavailable)?;
            run.validate().map_err(|_| Error::Unavailable)?;
            let root = run.root_name();
            // create_key has checked absence. Record the exact native-created
            // selector durably before an OS write whose outcome could be uncertain.
            write_new(
                &run.registry(root),
                &json!({"schema":"rangoon.e1c4.slot.v1", "nonce":run.nonce,
                "root":root, "workspace_id":self.identity().as_hex()}),
            )
            .map_err(|_| Error::Unavailable)?;
            WRITES.fetch_add(1, Ordering::SeqCst);
            let result = self.0.write(envelope);
            emit(
                "slot_write",
                json!({"workspaceId":self.identity().as_hex(),"succeeded":result.is_ok()}),
            );
            let interrupted_copy =
                run.case_spec().expectation == Expectation::InterruptedProvisioning;
            if result.is_ok() && (case_is("interrupted-setup") || interrupted_copy) {
                let mut fields = json!({"afterRealKeyWrite":true});
                if interrupted_copy {
                    fields["vaultOpens"] = json!(OPENS.load(Ordering::SeqCst));
                    fields["vaultReads"] = json!(READS.load(Ordering::SeqCst));
                    fields["vaultWrites"] = json!(WRITES.load(Ordering::SeqCst));
                }
                emit("provisioning_barrier", fields);
                // Only the parent-owned child is terminated. Never return to
                // Owner or manufacture a completed setup after this barrier.
                let deadline = Instant::now() + Duration::from_secs(120);
                while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                    std::thread::park_timeout(remaining);
                }
                emit("provisioning_barrier_timeout", json!({}));
                return Err(Error::WriteUncertain);
            }
            result
        }
    }
    /// Pause only after the shared adoption core reports the selected durable
    /// boundary. Do not reopen its journal/Owner while their locks are held.
    pub(crate) fn adoption_checkpoint(stage: &str, boundary: &str) -> Result<(), ()> {
        let Some(run) = RUN.get() else {
            return Ok(());
        };
        if run.case_spec().expectation.adoption_checkpoint() != Some((stage, boundary)) {
            return Ok(());
        }
        run.validate()?;
        if ADOPTION_HELD.swap(true, Ordering::SeqCst) {
            return Err(());
        }
        emit(
            "adoption_barrier",
            json!({"stage":stage,"boundary":boundary,
                "vaultOpens":OPENS.load(Ordering::SeqCst),
                "vaultReads":READS.load(Ordering::SeqCst),
                "vaultWrites":WRITES.load(Ordering::SeqCst)}),
        );
        let deadline = Instant::now() + Duration::from_secs(120);
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            // Spurious wakeups never continue the interrupted operation.
            std::thread::park_timeout(remaining);
        }
        emit("adoption_barrier_timeout", json!({}));
        Err(())
    }

    fn delete_slots(run: &Run) -> Result<(), ()> {
        let roots: Vec<&str> = if run.case == "lose-key" {
            vec!["authoring"]
        } else {
            run.plan.roots.iter().map(String::as_str).collect()
        };
        for root in roots {
            match fs::symlink_metadata(run.registry(root)) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && run.case == "cleanup" => {
                    continue;
                }
                Err(_) => return Err(()),
                Ok(_) => {}
            }
            let slot = run.slot(root).map_err(|()| {
                emit(
                    "slot_registry_refused",
                    json!({"root":root,"registry":format!("slot-{root}.json")}),
                );
            })?;
            let service = format!("{}.encrypted-workspace-key.v1", run.identifier());
            let account = format!("workspace-{}", slot.workspace_id);
            OPENS.fetch_add(1, Ordering::SeqCst);
            emit(
                "slot_delete_attempt",
                json!({"root":root,"service":service,"account":account,"vaultOpens":OPENS.load(Ordering::SeqCst)}),
            );
            let entry = apple_native_keyring_store::keychain::Store::new()
                .and_then(|store|store.build(&service, &account, None))
                .map_err(|_| {
                    emit("slot_cleanup_failed", json!({"root":root,"service":service,"account":account,"operation":"open_exact_entry"}));
                })?;
            match entry.delete_credential() {
                Ok(()) => {}
                Err(keyring_core::Error::NoEntry) if run.case == "cleanup" => {}
                Err(_) => {
                    emit(
                        "slot_cleanup_failed",
                        json!({"root":root,"service":service,"account":account,"operation":"delete"}),
                    );
                    return Err(());
                }
            }
            READS.fetch_add(1, Ordering::SeqCst);
            emit(
                "slot_absence_check",
                json!({"root":root,"service":service,"account":account,"vaultReads":READS.load(Ordering::SeqCst)}),
            );
            match entry.get_secret() {
                Err(keyring_core::Error::NoEntry) => {}
                Ok(secret) => {
                    let _secret = Zeroizing::new(secret);
                    emit(
                        "slot_cleanup_failed",
                        json!({"root":root,"service":service,"account":account,"operation":"confirm_absence"}),
                    );
                    return Err(());
                }
                Err(_) => {
                    emit(
                        "slot_cleanup_failed",
                        json!({"root":root,"service":service,"account":account,"operation":"confirm_absence"}),
                    );
                    return Err(());
                }
            }
            emit(
                "slot_absent",
                json!({"root":root,"service":service,"account":account}),
            );
        }
        Ok(())
    }
    fn valid_context(config: &Value, assets: bool) -> bool {
        let Some(caps) = config
            .pointer("/app/security/capabilities")
            .and_then(Value::as_array)
        else {
            return false;
        };
        config["identifier"] == IDENTIFIER
            && config["bundle"]["active"] == false
            && config["app"]["withGlobalTauri"] == true
            && config["app"]["windows"]
                .as_array()
                .is_some_and(Vec::is_empty)
            && config["app"]["security"]["csp"] == CSP
            && config["build"]["frontendDist"] == "../../../preview"
            && [
                "devUrl",
                "beforeDevCommand",
                "beforeBuildCommand",
                "beforeBundleCommand",
            ]
            .iter()
            .all(|k| config["build"].get(k).is_none_or(Value::is_null))
            && config
                .get("plugins")
                .is_none_or(|p| p.as_object().is_some_and(|o| o.is_empty()))
            && caps.len() == 1
            && caps[0]["identifier"] == "e1c4-local-authoring"
            && caps[0]["local"] == true
            && caps[0]["windows"] == json!(["main"])
            && caps[0]
                .get("webviews")
                .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
            && caps[0].get("remote").is_none_or(Value::is_null)
            && caps[0]["permissions"] == json!(PERMISSIONS)
            && assets
    }
    fn check_context(context: &tauri::Context<tauri::Wry>) {
        let config = serde_json::to_value(context.config())
            .unwrap_or_else(|_| refuse("context unavailable", 70));
        let assets = context.assets().iter().next().is_some();
        if !valid_context(&config, assets) {
            refuse("context isolation failed", 70);
        }
        for (pointer, bad) in [
            ("/identifier", json!("ai.rangoon.studio.dev")),
            ("/app/security/capabilities", json!(["source-analysis"])),
            ("/app/security/csp", json!("default-src *")),
            ("/app/security/capabilities/0/windows", json!(["*"])),
            (
                "/app/security/capabilities/0/permissions",
                json!(["allow-send-cloud-model"]),
            ),
            ("/bundle/active", json!(true)),
        ] {
            let mut altered = config.clone();
            *altered
                .pointer_mut(pointer)
                .unwrap_or_else(|| refuse("context test unavailable", 70)) = bad;
            if valid_context(&altered, assets) {
                refuse("context negative check failed", 70);
            }
        }
        if valid_context(&config, false) {
            refuse("assets negative check failed", 70);
        }
        emit(
            "context_validated",
            json!({"identifier":IDENTIFIER,"permissions":PERMISSIONS.len(),"negativeChecks":7,"embeddedWorkbench":true}),
        );
    }
    pub(crate) fn run() {
        std::panic::set_hook(Box::new(|_| eprintln!("e1c4: qualification panic")));
        let args: Vec<_> = std::env::args().skip(1).take(4).collect();
        let run =
            prepare_run(&args).unwrap_or_else(|_| refuse("invalid run, phase, or directory", 64));
        RUN.set(run).unwrap_or_else(|_| refuse("duplicate run", 70));
        let run = RUN.get().unwrap_or_else(|| refuse("run unavailable", 70));
        emit(
            "started",
            json!({"nonce":run.nonce,"root":run.root_name(),"casePlanSha256":plan_digest(),"credentialService":format!("{}.encrypted-workspace-key.v1",run.identifier())}),
        );
        if case_is("lose-key") || case_is("cleanup") {
            if delete_slots(run).is_err() {
                refuse("exact test slot cleanup failed; inspect registry", 74);
            }
            emit("cleanup_complete", json!({}));
            return;
        }
        prepare_original(run)
            .unwrap_or_else(|_| refuse("disposable original unavailable or changed", 74));
        let context = tauri::generate_context!("native-encrypted-qualification/tauri.conf.json");
        check_context(&context);
        let app = crate::desktop_builder().setup(|app| {
            let Some(run) = RUN.get() else { refuse("qualification run unavailable",64); };
            if run.validate().is_err() { refuse("qualification root changed",64); }
            crate::workspace_native_bootstrap::install_startup_state(app.handle(), run.root(), startup_vault())
                .unwrap_or_else(|_| refuse("qualification services unavailable", 70));
            emit("os_directories",json!({"appData":app.path().app_data_dir().ok(),"appLocalData":app.path().app_local_data_dir().ok(),"appCache":app.path().app_cache_dir().ok()}));
            Ok(())
        }).build(context);
        match app {
            Ok(app) => crate::run_workbench(app),
            Err(_) => refuse("native build or Owner unavailable", 4),
        }
    }
    pub(crate) fn before_ready(app: &AppHandle) {
        if case_is("anchor-failure")
            && app
                .state::<SourceAccess>()
                .set_failure_notifier(Arc::new(|| {}))
                .is_err()
        {
            refuse("fault setup failed", 70);
        }
    }
    fn observation(app: &AppHandle) -> Value {
        let close = app.state::<WorkspaceClose>();
        let source = app.state::<SourceAccess>();
        let request = serde_json::to_value(source.request_view()).unwrap_or(Value::Null);
        json!({"close":close.inspect(|drained|crate::workspace_close::native_cleanup(app,drained)),
            "protection":crate::workspace_close::setup::ProtectionResponse::observe(&close),
            "requestState":request["state"],"storage":request["storage"],"clientPresent":request["clientId"].is_string(),
            "failedClosed":source.failure_latched(),"clean":close.clean_native_completion(),
            "vaultOpens":OPENS.load(Ordering::SeqCst),"vaultReads":READS.load(Ordering::SeqCst),"vaultWrites":WRITES.load(Ordering::SeqCst)})
    }
    pub(crate) fn startup_observed(app: &AppHandle, view: &crate::workspace_startup_host::View) {
        let view = serde_json::to_value(view)
            .unwrap_or_else(|_| refuse("startup observation unavailable", 70));
        let state = view
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_else(|| refuse("startup observation invalid", 70));
        emit(
            "startup",
            json!({"state":state,"copyPhase":view["copyPhase"],"retainedOriginal":view["retainedOriginal"],"observation":observation(app)}),
        );
    }

    fn prepare_original(run: &Run) -> Result<(), ()> {
        let kind = run.plan.seed_for_root(run.root_name()).ok_or(())?;
        if kind == Seed::None {
            return Ok(());
        }
        run.validate()?;
        let path = run
            .directory
            .join(format!("original-{}.json", run.root_name()));
        let original = if matches!(run.case_spec().prerequisite, Prerequisite::Initial { .. }) {
            let original = seeds::seed(&run.root(), kind)?;
            write_new(&path, &serde_json::to_value(&original).map_err(|_| ())?)?;
            original
        } else {
            let baseline: seeds::Original = read_record(&path, run.manifest.directory.uid)?;
            if seeds::observe(&run.root())? != baseline {
                return Err(());
            }
            baseline
        };
        emit(
            "original_before",
            serde_json::to_value(&original).map_err(|_| ())?,
        );
        ORIGINAL.set(original).map_err(|_| ())
    }

    pub(crate) fn consent_observed(
        purpose: &'static str,
        consent: crate::workspace_close::setup::Consent,
    ) {
        use crate::workspace_close::setup::Consent;
        if ![
            "create_empty",
            "unlock",
            "copy_existing",
            "verify_copy",
            "recovery_inspect",
            "recovery_initialize_empty",
            "recovery_reconcile",
            "recovery_unlock",
            "recovery_restore",
            "recovery_record_copied",
            "recovery_complete_selection",
        ]
        .contains(&purpose)
        {
            refuse("invalid consent observation", 70);
        }
        let outcome = match consent {
            Consent::Approved => "approved",
            Consent::Cancelled => "cancelled",
            Consent::Unavailable => "unavailable",
        };
        emit("consent", json!({"purpose":purpose,"outcome":outcome}));
    }
    // Fixed test-only WebView script. The parent cannot supply JavaScript or
    // payloads. Only finite classifications cross the evaluation callback.
    const HEADER_PROBE: &str = r#"(() => {
      globalThis.__rangoonQ3HeaderProbe = 'pending';
      const client = __CLIENT_JSON__;
      const options = client === null ? undefined : {headers: {'x-rangoon-workspace-client': client}};
      Promise.resolve().then(() => window.__TAURI__.core.invoke('list_snapshots', {}, options)).then(result => {
        const keys = value => value && typeof value === 'object' && !Array.isArray(value)
          ? Object.keys(value).sort().join(',') : '';
        const refused = keys(result) === 'error,outcome' && result.outcome === 'failed'
          && keys(result.error) === 'code,message' && result.error.code === 'workspace_unavailable'
          && typeof result.error.message === 'string';
        const listed = keys(result) === 'outcome,snapshots' && result.outcome === 'listed'
          && Array.isArray(result.snapshots) && result.snapshots.length > 0;
        globalThis.__rangoonQ3HeaderProbe = refused ? 'refused' : listed ? 'listed' : 'failed';
      }).catch(() => { globalThis.__rangoonQ3HeaderProbe = 'failed'; });
    })()"#;
    const HEADER_POLL: &str = "globalThis.__rangoonQ3HeaderProbe";

    #[derive(Debug, PartialEq, Eq)]
    enum HeaderReply {
        Pending,
        Refused,
        Listed,
    }
    fn header_reply(raw: &str) -> Result<HeaderReply, ()> {
        if raw.len() > 32 {
            return Err(());
        }
        match serde_json::from_str::<String>(raw)
            .map_err(|_| ())?
            .as_str()
        {
            "pending" => Ok(HeaderReply::Pending),
            "refused" => Ok(HeaderReply::Refused),
            "listed" => Ok(HeaderReply::Listed),
            _ => Err(()),
        }
    }
    fn open_client(app: &AppHandle) -> Result<String, ()> {
        let state = observation(app);
        if state["close"]["state"] != "open" || state["failedClosed"] != false {
            return Err(());
        }
        let view =
            serde_json::to_value(app.state::<SourceAccess>().request_view()).map_err(|_| ())?;
        let id = view["clientId"].as_str().ok_or(())?;
        if view["state"] != "open"
            || view["storage"] != "managed_encrypted"
            || id.len() != 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(());
        }
        Ok(id.to_owned())
    }
    fn vault_counts() -> [u64; 3] {
        [
            OPENS.load(Ordering::SeqCst),
            READS.load(Ordering::SeqCst),
            WRITES.load(Ordering::SeqCst),
        ]
    }
    fn invoke_header_probe(app: &AppHandle, client: Option<&str>) -> Result<HeaderReply, ()> {
        let window = app.get_webview_window("main").ok_or(())?;
        let script = HEADER_PROBE.replace(
            "__CLIENT_JSON__",
            &serde_json::to_string(&client).map_err(|_| ())?,
        );
        window.eval(script).map_err(|_| ())?;
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            window
                .eval_with_callback(HEADER_POLL, move |raw| {
                    let _ = tx.try_send(header_reply(&raw));
                })
                .map_err(|_| ())?;
            let remaining = deadline.checked_duration_since(Instant::now()).ok_or(())?;
            match rx.recv_timeout(remaining).map_err(|_| ())?? {
                HeaderReply::Pending => std::thread::sleep(Duration::from_millis(30)),
                result => {
                    window
                        .eval("delete globalThis.__rangoonQ3HeaderProbe")
                        .map_err(|_| ())?;
                    return Ok(result);
                }
            }
        }
    }
    fn check_headers(app: &AppHandle, old: &str) -> Result<(), ()> {
        let fresh = open_client(app)?;
        if old == fresh {
            return Err(());
        }
        emit("header_probe_started", json!({"differentClient":true}));
        for (name, header, expected) in [
            ("header_probe_missing", None, HeaderReply::Refused),
            ("header_probe_stale", Some(old), HeaderReply::Refused),
            (
                "header_probe_current",
                Some(fresh.as_str()),
                HeaderReply::Listed,
            ),
        ] {
            let before = vault_counts();
            if invoke_header_probe(app, header)? != expected || open_client(app)? != fresh {
                return Err(());
            }
            let after = vault_counts();
            let valid = if expected == HeaderReply::Listed {
                after[1] > before[1] && after[2] == before[2]
            } else {
                after == before
            };
            if !valid {
                return Err(());
            }
            emit(
                name,
                json!({"matched":true,"vaultReadDelta":after[1]-before[1],"vaultWriteDelta":after[2]-before[2]}),
            );
        }
        Ok(())
    }
    pub(crate) fn after_ready(app: &AppHandle, anchor: bool, presented: bool) {
        let run = RUN
            .get()
            .unwrap_or_else(|| refuse("qualification run unavailable", 64));
        if let Some(window) = app.get_webview_window("main") {
            window
                .set_title(&format!("Rangoon Recovery QA {}", &run.nonce[..8]))
                .unwrap_or_else(|_| refuse("qualification title unavailable", 70));
        }
        emit(
            "ready",
            json!({"anchor":anchor,"windowPresented":presented,"state":observation(app)}),
        );
        let watching = app.clone();
        std::thread::spawn(move || {
            let mut previous = Value::Null;
            while !EXITED.load(Ordering::SeqCst) {
                let current = observation(&watching);
                if current != previous {
                    emit("state", current.clone());
                    previous = current;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        let controlling = app.clone();
        std::thread::spawn(move || {
            let mut input = std::io::stdin().lock();
            let mut captured_client: Option<String> = None;
            let mut header_started = false;
            let mut recovery_snapshot_taken = false;
            loop {
                let mut line = String::new();
                let read = (&mut input).take(65).read_line(&mut line);
                match read {
                    Ok(0) => {
                        controlling.exit(4);
                        break;
                    }
                    Ok(_) if line == "observe\n" => emit("observed", observation(&controlling)),
                    Ok(_)
                        if line == "recovery-snapshot\n"
                            && (case_is("authoring") || case_is("relaunch"))
                            && !recovery_snapshot_taken =>
                    {
                        recovery_snapshot_taken = true;
                        match controlling
                            .state::<SourceAccess>()
                            .qualification_recovery_snapshot()
                        {
                            Ok((backup_id, byte_length)) => emit(
                                "recovery_snapshot",
                                json!({"backupId":backup_id,"byteLength":byte_length}),
                            ),
                            Err(_) => {
                                emit("recovery_snapshot_failed", json!({}));
                                controlling.exit(4);
                                break;
                            }
                        }
                    }
                    Ok(_)
                        if line == "capture-client\n"
                            && case_is("authoring")
                            && !header_started =>
                    {
                        header_started = true;
                        match open_client(&controlling) {
                            Ok(client) => {
                                captured_client = Some(client);
                                emit("header_client_captured", json!({"open":true}));
                            }
                            Err(()) => {
                                emit("header_probe_failed", json!({}));
                                controlling.exit(4);
                                break;
                            }
                        }
                    }
                    Ok(_) if line == "check-headers\n" && case_is("authoring") => {
                        let result = captured_client
                            .take()
                            .ok_or(())
                            .and_then(|old| check_headers(&controlling, &old));
                        if result.is_err() {
                            emit("header_probe_failed", json!({}));
                            controlling.exit(4);
                            break;
                        }
                    }
                    Ok(_) if line == "arm-read\n" && case_is("authoring") => {
                        let state = observation(&controlling);
                        if state["close"]["state"] != "open"
                            || state["clientPresent"] != true
                            || state["failedClosed"] != false
                            || READ_BARRIER
                                .arm(|| emit("read_barrier_armed", state))
                                .is_err()
                        {
                            emit("control_refused", json!({}));
                            controlling.exit(4);
                            break;
                        }
                    }
                    Ok(_) if line == "release-read\n" && case_is("authoring") => {
                        let state = observation(&controlling);
                        if state["close"]["state"] != "closing"
                            || state["clientPresent"] != false
                            || state["requestState"] != "unavailable"
                            || state["failedClosed"] != false
                            || READ_BARRIER
                                .release(|| emit("read_barrier_released", state))
                                .is_err()
                        {
                            emit("control_refused", json!({}));
                            controlling.exit(4);
                            break;
                        }
                    }
                    Ok(_) if line == "quit\n" => controlling.exit(0),
                    _ => {
                        emit("control_refused", json!({}));
                        controlling.exit(4);
                        break;
                    }
                }
            }
        });
    }
    pub(crate) fn exited(app: &AppHandle) {
        EXITED.store(true, Ordering::SeqCst);
        if let Some(original) = ORIGINAL.get() {
            let run = RUN
                .get()
                .unwrap_or_else(|| refuse("qualification run unavailable", 64));
            if run.validate().is_err() || seeds::observe(&run.root()).as_ref() != Ok(original) {
                refuse("disposable original changed during phase", 74);
            }
            emit(
                "original_after",
                serde_json::to_value(original)
                    .unwrap_or_else(|_| refuse("original observation unavailable", 70)),
            );
        }
        emit(
            "exit",
            json!({"clean":app.state::<WorkspaceClose>().clean_native_completion(),"failedClosed":app.state::<SourceAccess>().failure_latched(),"state":observation(app)}),
        );
    }
    #[cfg(test)]
    mod header_tests {
        use super::*;
        #[test]
        fn header_callback_accepts_only_finite_classifications() {
            assert_eq!(header_reply("\"pending\""), Ok(HeaderReply::Pending));
            assert_eq!(header_reply("\"refused\""), Ok(HeaderReply::Refused));
            assert_eq!(header_reply("\"listed\""), Ok(HeaderReply::Listed));
            for raw in [
                "null",
                "true",
                "1",
                "{}",
                "[]",
                "\"failed\"",
                "\"Listed\"",
                "\"refused\" {}",
                "\"refused\\n\"",
                "\"snapshot content must never be accepted\"",
            ] {
                assert_eq!(header_reply(raw), Err(()));
            }
        }
    }
}
