//! Native-only close progress. A wake is not cleanup or authorization.
use super::{WorkspaceClose, native_cleanup};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const INTERVAL: Duration = Duration::from_millis(25);
pub(crate) const CLEAN_EXIT_CODE: i32 = 0;
pub(crate) const RECOVERY_EXIT_CODE: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExitAction {
    Pass,
    Prevent,
    PreventAndStart,
}

#[derive(Clone, Copy)]
enum ExitState {
    Idle,
    Holding,
    Sent(i32),
    Consumed,
}

/// Tauri provides an exit code, not an event identity. This one-shot guard
/// permits a matching coordinator code only after native terminal inspection.
pub(crate) struct ExitCoordinator(Mutex<ExitState>);
impl ExitCoordinator {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(ExitState::Idle))
    }
    pub(crate) fn request(&self, code: Option<i32>) -> ExitAction {
        if code == Some(tauri::RESTART_EXIT_CODE) {
            return ExitAction::Pass;
        }
        let Ok(mut state) = self.0.lock() else {
            return ExitAction::Prevent;
        };
        match *state {
            ExitState::Idle => {
                *state = ExitState::Holding;
                ExitAction::PreventAndStart
            }
            ExitState::Holding | ExitState::Consumed => ExitAction::Prevent,
            ExitState::Sent(expected) if code == Some(expected) => {
                *state = ExitState::Consumed;
                ExitAction::Pass
            }
            ExitState::Sent(_) => ExitAction::Prevent,
        }
    }
    pub(crate) fn is_holding(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|state| matches!(*state, ExitState::Holding))
    }
    pub(crate) fn send(&self, code: i32) -> bool {
        if code != CLEAN_EXIT_CODE && code != RECOVERY_EXIT_CODE {
            return false;
        }
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if !matches!(*state, ExitState::Holding) {
            return false;
        }
        *state = ExitState::Sent(code);
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Progress {
    Pending,
    /// Native inspection ended, including unhealthy results. Never permission.
    Finished,
    /// Host lifetime or dispatch ended without proving cleanup.
    Stopped,
}

pub(crate) trait Host: Send + Sync {
    fn progress(&self) -> Progress;
    fn terminal(&self, _result: Progress) {}
}

pub(crate) struct Monitor {
    host: Weak<dyn Host>,
    running: AtomicBool,
    pending: AtomicBool,
}
impl Monitor {
    pub(crate) fn new(host: Weak<dyn Host>) -> Arc<Self> {
        Arc::new(Self {
            host,
            running: AtomicBool::new(false),
            pending: AtomicBool::new(false),
        })
    }
    pub(crate) fn wake(self: &Arc<Self>) {
        let _ = self.start();
    }
    #[cfg(all(test, feature = "encrypted-workspace"))]
    pub(crate) fn wake_joined_for_test(self: &Arc<Self>) -> Option<std::thread::JoinHandle<()>> {
        self.start()
    }
    fn start(self: &Arc<Self>) -> Option<std::thread::JoinHandle<()>> {
        self.pending.store(true, Ordering::SeqCst);
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
        let monitor = Arc::clone(self);
        match std::thread::Builder::new()
            .name("rangoon-workspace-close".into())
            .spawn(move || {
                loop {
                    let terminal = loop {
                        monitor.pending.store(false, Ordering::SeqCst);
                        let Some(host) = monitor.host.upgrade() else {
                            break Progress::Stopped;
                        };
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            host.progress()
                        }))
                        .unwrap_or(Progress::Stopped);
                        drop(host);
                        if result != Progress::Pending {
                            break result;
                        }
                        std::thread::sleep(INTERVAL);
                    };
                    monitor.running.store(false, Ordering::SeqCst);
                    if monitor.pending.swap(false, Ordering::SeqCst)
                        && monitor
                            .running
                            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                    {
                        continue;
                    }
                    if let Some(host) = monitor.host.upgrade() {
                        host.terminal(terminal);
                    }
                    break;
                }
            }) {
            Ok(handle) => Some(handle),
            Err(_) => {
                self.dispatch_failed();
                None
            }
        }
    }
    fn dispatch_failed(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(host) = self.host.upgrade() {
            host.terminal(Progress::Stopped);
        }
    }
    #[cfg(all(test, feature = "encrypted-workspace"))]
    pub(crate) fn dispatch_failed_for_test(&self) {
        self.dispatch_failed();
    }
}

struct NativeHost {
    app: Weak<AppHandle>,
    alive: Arc<AtomicBool>,
    main_thread: std::thread::ThreadId,
    exit: Arc<ExitCoordinator>,
}

/// Application state keeps no strong host or app lifetime.
pub(crate) struct Wake(Weak<Monitor>);
pub(crate) fn wake(app: &AppHandle) {
    if let Some(signal) = app.try_state::<Wake>()
        && let Some(monitor) = signal.0.upgrade()
    {
        monitor.wake();
    }
}

/// Managed state contains only a weak dispatcher. Flight ownership and waiting
/// remain on a blocking worker; the event loop must never wait on itself.
#[derive(Clone)]
pub(crate) struct WorkerCleanup(Weak<NativeHost>);
impl WorkerCleanup {
    pub(crate) fn alive(&self) -> bool {
        self.0
            .upgrade()
            .is_some_and(|host| host.alive.load(Ordering::SeqCst))
    }
    pub(crate) fn cleanup(&self, drained: bool) -> bool {
        let Some(host) = self.0.upgrade() else {
            return false;
        };
        if std::thread::current().id() == host.main_thread || !host.alive.load(Ordering::SeqCst) {
            return false;
        }
        let Some(app) = host.app.upgrade() else {
            return false;
        };
        let weak_app = host.app.clone();
        let alive = Arc::clone(&host.alive);
        let (sender, receiver) = mpsc::sync_channel(1);
        let queued = app.run_on_main_thread(move || {
            let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                alive.load(Ordering::SeqCst)
                    && weak_app
                        .upgrade()
                        .is_some_and(|app| native_cleanup(&app, drained))
            }))
            .unwrap_or(false);
            let _ = sender.send(completed);
        });
        drop(app);
        queued.is_ok() && wait_for_result(receiver, &host.alive) == Some(true)
    }
    pub(crate) fn consume(&self, receiver: mpsc::Receiver<()>) -> bool {
        let Some(host) = self.0.upgrade() else {
            return false;
        };
        std::thread::current().id() != host.main_thread
            && wait_for_result(receiver, &host.alive).is_some()
    }
}
impl Host for NativeHost {
    fn progress(&self) -> Progress {
        if !self.alive.load(Ordering::SeqCst) {
            return Progress::Stopped;
        }
        let Some(app) = self.app.upgrade() else {
            return Progress::Stopped;
        };
        let weak_app = self.app.clone();
        let alive = Arc::clone(&self.alive);
        let (sender, receiver) = mpsc::sync_channel(1);
        let queued = app.run_on_main_thread(move || {
            if !alive.load(Ordering::SeqCst) {
                let _ = sender.send(Progress::Stopped);
                return;
            }
            let Some(app) = weak_app.upgrade() else {
                let _ = sender.send(Progress::Stopped);
                return;
            };
            let completed = app
                .state::<WorkspaceClose>()
                .progress_native(|drained| native_cleanup(&app, drained));
            let _ = sender.send(match completed {
                Ok(true) => Progress::Finished,
                Ok(false) => Progress::Pending,
                Err(()) => Progress::Stopped,
            });
        });
        // Neither SourceAccess nor a waiting worker retains a strong AppHandle.
        // Exit clears alive even while a main-loop callback has not run.
        drop(app);
        if queued.is_err() {
            return Progress::Stopped;
        }
        wait_for_progress(receiver, &self.alive)
    }
    fn terminal(&self, result: Progress) {
        if !self.alive.load(Ordering::SeqCst) || !self.exit.is_holding() {
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let weak_app = self.app.clone();
        let alive = Arc::clone(&self.alive);
        let exit = Arc::clone(&self.exit);
        let queued = app.run_on_main_thread(move || {
            if !alive.load(Ordering::SeqCst) || !exit.is_holding() {
                return;
            }
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            let clean = result == Progress::Finished
                && app.state::<WorkspaceClose>().clean_native_completion();
            let code = if clean {
                CLEAN_EXIT_CODE
            } else {
                RECOVERY_EXIT_CODE
            };
            if exit.send(code) {
                app.exit(code);
            }
        });
        if queued.is_err() && self.exit.send(RECOVERY_EXIT_CODE) {
            // No callback or cleanup proof exists when main-loop dispatch fails.
            // Pinned Tauri may force this nonzero exit if request dispatch fails.
            app.exit(RECOVERY_EXIT_CODE);
        }
    }
}

pub(crate) fn wait_for_progress(
    receiver: mpsc::Receiver<Progress>,
    alive: &AtomicBool,
) -> Progress {
    wait_for_result(receiver, alive).unwrap_or(Progress::Stopped)
}

pub(crate) fn wait_for_result<T>(receiver: mpsc::Receiver<T>, alive: &AtomicBool) -> Option<T> {
    loop {
        if !alive.load(Ordering::SeqCst) {
            return None;
        }
        match receiver.recv_timeout(INTERVAL) {
            Ok(result) => return alive.load(Ordering::SeqCst).then_some(result),
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Held by App::run, never by managed application state or SourceAccess.
pub(crate) struct Anchor {
    app: Arc<AppHandle>,
    _host: Arc<dyn Host>,
    alive: Arc<AtomicBool>,
    exit: Arc<ExitCoordinator>,
}
impl Anchor {
    pub(crate) fn exit_request(&self, code: Option<i32>) -> ExitAction {
        self.exit.request(code)
    }
    pub(crate) fn begin_exit_close(&self) {
        self.app
            .state::<WorkspaceClose>()
            .begin(|drained| native_cleanup(&self.app, drained));
        wake(&self.app);
    }
}
impl Drop for Anchor {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
    }
}
pub(crate) fn install(app: &AppHandle) -> Option<Anchor> {
    let source = app.state::<crate::SourceAccess>();
    if !source.encrypted_close_available() {
        return None;
    }
    let app = Arc::new(app.clone());
    let alive = Arc::new(AtomicBool::new(true));
    let exit = Arc::new(ExitCoordinator::new());
    let native = Arc::new(NativeHost {
        app: Arc::downgrade(&app),
        alive: Arc::clone(&alive),
        main_thread: std::thread::current().id(),
        exit: Arc::clone(&exit),
    });
    let worker = WorkerCleanup(Arc::downgrade(&native));
    let host: Arc<dyn Host> = native;
    let monitor = Monitor::new(Arc::downgrade(&host));
    if source
        .set_failure_notifier(Arc::new({
            let monitor = Arc::clone(&monitor);
            move || monitor.wake()
        }))
        .is_err()
    {
        source.fail_closed();
        return None;
    }
    if !app.manage(worker) {
        source.fail_closed();
        alive.store(false, Ordering::SeqCst);
        return None;
    }
    if !app.manage(Wake(Arc::downgrade(&monitor))) {
        source.fail_closed();
        alive.store(false, Ordering::SeqCst);
        return None;
    }
    Some(Anchor {
        app,
        _host: host,
        alive,
        exit,
    })
}
