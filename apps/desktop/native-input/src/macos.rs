//! UI references never leave the native main thread. The thread-local slot
//! retains widgets only; the caller's existing Host owns all authorization,
//! selection, deadlines, cancellation and workspace admission.
use super::{InputError, InputGuard, MAX_SECRET_BYTES, SecretInput};
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSAlert, NSModalResponse, NSSecureTextField, NSWindow};
use objc2_foundation::{NSSize, NSString};
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use tauri::{Manager, Runtime, Window};
use zeroize::Zeroizing;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static UI: RefCell<Option<UiState>> = const { RefCell::new(None) };
}

struct UiState {
    id: u64,
    parent: Retained<NSWindow>,
    alert: Retained<NSAlert>,
    field: Option<Retained<NSSecureTextField>>,
    guard: Arc<dyn InputGuard>,
    completed: mpsc::Sender<Result<Reply, InputError>>,
}

enum Request {
    Review(String),
    Credential,
}

enum Reply {
    Confirmed,
    Credential(SecretInput),
}

pub(super) fn confirm<R: Runtime>(
    parent: Window<R>,
    text: String,
    guard: Arc<dyn InputGuard>,
) -> Result<(), InputError> {
    match request(parent, Request::Review(text), guard)? {
        Reply::Confirmed => Ok(()),
        Reply::Credential(_) => Err(InputError::Unavailable),
    }
}

pub(super) fn acquire<R: Runtime>(
    parent: Window<R>,
    guard: Arc<dyn InputGuard>,
) -> Result<SecretInput, InputError> {
    match request(parent, Request::Credential, guard)? {
        Reply::Credential(value) => Ok(value),
        Reply::Confirmed => Err(InputError::Unavailable),
    }
}

/// All callers are trusted native worker callbacks. Reject a main-thread call
/// rather than blocking the very loop needed for native completion.
fn request<R: Runtime>(
    parent: Window<R>,
    request: Request,
    guard: Arc<dyn InputGuard>,
) -> Result<Reply, InputError> {
    if MainThreadMarker::new().is_some() {
        return Err(InputError::Unavailable);
    }
    guard.check()?;
    let id = NEXT_ID
        .try_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
        .map_err(|_| InputError::Unavailable)?;
    let (tx, rx) = mpsc::channel();
    let is_review = matches!(&request, Request::Review(_));
    let ui_parent = parent.clone();
    let ui_guard = Arc::clone(&guard);
    parent
        .run_on_main_thread(move || present(ui_parent, request, id, ui_guard, tx))
        .map_err(|_| InputError::Unavailable)?;
    if is_review {
        guard.review_queued();
    }
    let mut dismissal_requested = false;
    let result = loop {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(value) => break value,
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err(InputError::Unavailable),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !dismissal_requested && guard.check().is_err() {
                    // A queued presentation checks the same guard. An open one
                    // is dismissed only by its own retained parent/alert pair.
                    if parent.run_on_main_thread(move || dismiss(id)).is_ok() {
                        dismissal_requested = true;
                    }
                }
            }
        }
    };
    // Sending a completion from inside the native block is not proof that the
    // block has returned. Queue a second turn FROM THIS WORKER. It cannot run
    // inline on the main thread and acknowledges the prior callback's return.
    let (ack_tx, ack_rx) = mpsc::channel();
    let queued = parent.app_handle().run_on_main_thread(move || {
        let _ = ack_tx.send(());
    });
    if queued.is_err() || ack_rx.recv().is_err() {
        drop(result);
        // Runtime loss is not proven cleanup. Keep caller admission retained;
        // the owned qualification process has an independent bounded timeout.
        loop {
            std::thread::park();
        }
    }
    guard.check()?;
    result
}

fn present<R: Runtime>(
    window: Window<R>,
    request: Request,
    id: u64,
    guard: Arc<dyn InputGuard>,
    completed: mpsc::Sender<Result<Reply, InputError>>,
) {
    let Some(mtm) = MainThreadMarker::new() else {
        let _ = completed.send(Err(InputError::Unavailable));
        return;
    };
    if let Err(error) = guard.check() {
        let _ = completed.send(Err(error));
        return;
    }
    if UI.with(|slot| slot.borrow().is_some()) {
        let _ = completed.send(Err(InputError::Busy));
        return;
    }
    let parent = match retain_parent(&window, mtm) {
        Ok(parent) => parent,
        Err(error) => {
            let _ = completed.send(Err(error));
            return;
        }
    };
    if !parent.isVisible() {
        let _ = completed.send(Err(InputError::ParentClosed));
        return;
    }
    if parent.attachedSheet().is_some() {
        let _ = completed.send(Err(InputError::Busy));
        return;
    }
    let alert = NSAlert::new(mtm);
    let field = match request {
        Request::Review(text) => {
            alert.setMessageText(&NSString::from_str("Review system access"));
            alert.setInformativeText(&NSString::from_str(&text));
            alert.addButtonWithTitle(&NSString::from_str("Continue"));
            None
        }
        Request::Credential => {
            alert.setMessageText(&NSString::from_str("Read once with OpenShell"));
            alert.setInformativeText(&NSString::from_str("Enter the credential for the system you just reviewed. It is used for this one read and is not saved. Cancel leaves local work available."));
            alert.addButtonWithTitle(&NSString::from_str("Read once"));
            let field = NSSecureTextField::new(mtm);
            field.setFrameSize(NSSize::new(360.0, 28.0));
            field.setUsesSingleLineMode(true);
            field.setPlaceholderString(Some(&NSString::from_str(
                "Credential — one line, up to 16 KiB",
            )));
            field.setStringValue(&NSString::from_str(""));
            alert.setAccessoryView(Some(&field));
            Some(field)
        }
    };
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    let callback = RcBlock::new(move |response: NSModalResponse| complete(id, response));
    UI.with(|slot| {
        *slot.borrow_mut() = Some(UiState {
            id,
            parent: parent.clone(),
            alert: alert.clone(),
            field,
            guard,
            completed,
        })
    });
    alert.beginSheetModalForWindow_completionHandler(&parent, Some(&callback));
    let focus = UI.with(|slot| {
        slot.borrow().as_ref().filter(|s| s.id == id).and_then(|s| {
            s.field
                .as_ref()
                .map(|field| (s.alert.clone(), field.clone(), Arc::clone(&s.guard)))
        })
    });
    if let Some((alert, field, guard)) = focus {
        alert.window().makeFirstResponder(Some(&field));
        // Observe only after beginSheet and field setup. Release the TLS
        // borrow before invoking caller code; a queued request is not open UI.
        guard.input_presented();
    }
}

/// The only direct native pointer operation in this crate. The pointer is
/// obtained from a trusted Tauri Window on the native main thread, never IPC.
#[allow(unsafe_code)]
fn retain_parent<R: Runtime>(
    window: &Window<R>,
    _mtm: MainThreadMarker,
) -> Result<Retained<NSWindow>, InputError> {
    let pointer = window.ns_window().map_err(|_| InputError::ParentClosed)?;
    // SAFETY: Tauri's pinned ns_window implementation returns the actual
    // autoreleased NSWindow associated with this Window's native view. This
    // call and retain occur on the main thread before its autorelease pool can
    // drain. Retained owns the extra reference; it remains in main-thread-only
    // UiState and is dropped there. No pointer or UI object crosses threads.
    unsafe { Retained::retain(pointer.cast::<NSWindow>()) }.ok_or(InputError::ParentClosed)
}

fn dismiss(id: u64) {
    let owned = UI.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|s| s.id == id)
            .map(|s| (s.parent.clone(), s.alert.clone()))
    });
    if let Some((parent, alert)) = owned {
        // Release the RefCell borrow before AppKit can call completion.
        parent.endSheet_returnCode(&alert.window(), 1001);
    }
}

fn complete(id: u64, response: NSModalResponse) {
    let state = UI.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|s| s.id == id) {
            slot.take()
        } else {
            None
        }
    });
    let Some(state) = state else {
        return;
    }; // duplicate/late completion
    let result = match state.guard.check() {
        Err(error) => Err(error),
        Ok(()) if response != 1000 => Err(InputError::Cancelled),
        Ok(()) => match &state.field {
            None => Ok(Reply::Confirmed),
            Some(field) => {
                state.alert.window().makeFirstResponder(None);
                let native = field.stringValue();
                if native.length() > MAX_SECRET_BYTES {
                    Err(InputError::InvalidInput)
                } else {
                    SecretInput::new(Zeroizing::new(native.to_string())).map(Reply::Credential)
                }
            }
        },
    };
    if let Some(field) = &state.field {
        state.alert.window().makeFirstResponder(None);
        field.setStringValue(&NSString::from_str(""));
        field.setEditable(false);
        state.alert.setAccessoryView(None);
    }
    let completed = state.completed.clone();
    let final_guard = Arc::clone(&state.guard);
    drop(state); // owned widget/parent references released on this thread
    let result = final_guard.check().and(result);
    let _ = completed.send(result); // send failure drops/clears owned secret
}
