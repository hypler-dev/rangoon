//! Native-only failure progress. A wake is not cleanup or authorization.
use super::{WorkspaceClose, native_cleanup};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const INTERVAL: Duration = Duration::from_millis(25);

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
}

pub(crate) struct Monitor {
    host: Weak<dyn Host>,
    running: AtomicBool,
}
impl Monitor {
    pub(crate) fn new(host: Weak<dyn Host>) -> Arc<Self> {
        Arc::new(Self {
            host,
            running: AtomicBool::new(false),
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
                    let Some(host) = monitor.host.upgrade() else {
                        break;
                    };
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.progress()));
                    drop(host);
                    if !matches!(result, Ok(Progress::Pending)) {
                        break;
                    }
                    std::thread::sleep(INTERVAL);
                }
                monitor.running.store(false, Ordering::SeqCst);
            }) {
            Ok(handle) => Some(handle),
            Err(_) => {
                // Failed dispatch proves no cleanup. The source's terminal failure
                // still fences admission, and explicit inspection remains closed.
                self.running.store(false, Ordering::SeqCst);
                None
            }
        }
    }
}

struct NativeHost {
    app: Weak<AppHandle>,
    alive: Arc<AtomicBool>,
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
                .progress_failure(|drained| native_cleanup(&app, drained));
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
}

pub(crate) fn wait_for_progress(
    receiver: mpsc::Receiver<Progress>,
    alive: &AtomicBool,
) -> Progress {
    loop {
        if !alive.load(Ordering::SeqCst) {
            return Progress::Stopped;
        }
        match receiver.recv_timeout(INTERVAL) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Progress::Stopped,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Held by App::run, never by managed application state or SourceAccess.
pub(crate) struct Anchor {
    _app: Arc<AppHandle>,
    _host: Arc<dyn Host>,
    alive: Arc<AtomicBool>,
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
    let host: Arc<dyn Host> = Arc::new(NativeHost {
        app: Arc::downgrade(&app),
        alive: Arc::clone(&alive),
    });
    let monitor = Monitor::new(Arc::downgrade(&host));
    if source
        .set_failure_notifier(Arc::new(move || monitor.wake()))
        .is_err()
    {
        source.fail_closed();
        return None;
    }
    Some(Anchor {
        _app: app,
        _host: host,
        alive,
    })
}
