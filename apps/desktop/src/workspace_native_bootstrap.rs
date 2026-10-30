//! Shared startup ordering. A window is not evidence of an unlocked workspace.
use crate::{Gate, SourceAccess, WorkspaceClose};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// One deferred native startup factory. The ordinary and isolated entry points
/// must share it when startup adoption is mounted; no existing user path or key
/// is opened here, and no mutable Tauri state replacement is necessary.
#[cfg(feature = "encrypted-workspace")]
pub(super) fn startup_state<V: rangoon_desktop::workspace_session::Vault + Send + 'static>(
    gate: &Gate,
    anchor: std::path::PathBuf,
    vault: V,
) -> (
    SourceAccess,
    WorkspaceClose,
    crate::workspace_startup_host::Host,
) {
    let roots = crate::workspace_startup::StartupRoots::new(anchor.clone());
    let source = SourceAccess::managed_deferred(gate.clone(), roots.managed(), vault);
    let close = WorkspaceClose::new(source.clone());
    let host = crate::workspace_startup_host::Host::new(anchor, close.clone());
    (source, close, host)
}

/// Shared installation for ordinary and disposable native entry points. State
/// registration is one-shot; a failure cannot replace an existing service.
#[cfg(feature = "encrypted-workspace")]
pub(super) fn install_startup_state<
    V: rangoon_desktop::workspace_session::Vault + Send + 'static,
>(
    app: &AppHandle,
    anchor: std::path::PathBuf,
    vault: V,
) -> Result<(), StartupFailure> {
    let (source, close, host) = startup_state(app.state::<Gate>().inner(), anchor, vault);
    if !app.manage(source) || !app.manage(close) || !app.manage(host) {
        return Err(StartupFailure::StateInstallation);
    }
    Ok(())
}

pub(super) fn install_workbench_state(
    app: &AppHandle,
    anchor: Result<std::path::PathBuf, rangoon_store::StoreError>,
) -> Result<(), StartupFailure> {
    #[cfg(feature = "encrypted-workspace")]
    if let Ok(anchor) = anchor {
        return install_startup_state(app, anchor, native_startup_vault(app));
    }
    #[cfg(not(feature = "encrypted-workspace"))]
    let _ = anchor;

    let source = SourceAccess::unavailable(app.state::<Gate>().inner().clone());
    let close = WorkspaceClose::new(source.clone());
    if !app.manage(source) || !app.manage(close) {
        return Err(StartupFailure::StateInstallation);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StartupFailure {
    StateInstallation,
    MissingAnchor,
    InitialClose,
    WindowCreation,
}

/// The caller retains the real lifetime anchor outside managed state. Only its
/// installation result enters this private ordering helper; no wire DTO can
/// supply it. Managed startup never opens a client or acknowledges renderer purge.
fn prepare_workbench(
    gate: &Gate,
    source: &SourceAccess,
    close: &WorkspaceClose,
    anchor_installed: bool,
    cleanup: impl FnOnce(bool) -> bool,
    present: impl FnOnce() -> Result<(), ()>,
) -> Result<(), StartupFailure> {
    let result = if source.encrypted_close_available() && !anchor_installed {
        Err(StartupFailure::MissingAnchor)
    } else if source.encrypted_close_available() && !close.begin_initial(cleanup) {
        Err(StartupFailure::InitialClose)
    } else {
        present().map_err(|()| StartupFailure::WindowCreation)
    };
    if result.is_err() {
        close.fail_startup();
        // Managed SourceAccess retains its real drain. Legacy has no encrypted
        // close protocol, but a failed window must still fence its shared Gate.
        // Dropping a Drain never reopens admission.
        if !source.encrypted_close_available() {
            let _ = gate.request_drain();
        }
    }
    result
}

pub(super) fn present_workbench(
    app: &AppHandle,
    anchor_installed: bool,
) -> Result<(), StartupFailure> {
    prepare_workbench(
        app.state::<Gate>().inner(),
        app.state::<SourceAccess>().inner(),
        app.state::<WorkspaceClose>().inner(),
        anchor_installed,
        |drained| {
            #[cfg(all(test, feature = "native-encrypted-qualification"))]
            if crate::native_encrypted_qualification::case_is("cleanup-failure") {
                return false;
            }
            crate::workspace_close::native_cleanup(app, drained)
        },
        || {
            #[cfg(all(test, feature = "native-encrypted-qualification"))]
            if crate::native_encrypted_qualification::case_is("window-failure") {
                return Err(());
            }
            create_main_window(app)
        },
    )
}

fn create_main_window(app: &AppHandle) -> Result<(), ()> {
    let work_area = app.primary_monitor().ok().flatten().map(|monitor| {
        (
            monitor.work_area().size.width,
            monitor.work_area().size.height,
            monitor.scale_factor(),
        )
    });
    let (width, height) = crate::initial_window_size(work_area);
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("analyze.html".into()))
        .title("Rangoon")
        .inner_size(width, height)
        .min_inner_size(320.0, 480.0)
        .center()
        .on_navigation(crate::bundled_navigation)
        .build()
        .map(|_| ())
        .map_err(|_| ())
}

#[cfg(test)]
#[path = "workspace_native_bootstrap_tests.rs"]
mod tests;

/// Native-owned slot selector. Construction performs no credential-store access.
/// Credential access occurs only through a separately confirmed native action.
#[cfg(feature = "encrypted-workspace")]
pub(super) struct OsVault {
    application_identifier: String,
}
#[cfg(feature = "encrypted-workspace")]
impl OsVault {
    pub(super) fn new(application_identifier: String) -> Self {
        Self {
            application_identifier,
        }
    }
}
#[cfg(feature = "encrypted-workspace")]
impl rangoon_desktop::workspace_session::Vault for OsVault {
    type Slot = rangoon_desktop::workspace_keys::OsSlot;
    fn open(
        &self,
        identity: &rangoon_desktop::workspace_keys::WorkspaceId,
    ) -> Result<Self::Slot, rangoon_desktop::workspace_keys::Error> {
        Self::Slot::open(&self.application_identifier, identity.clone())
    }
}

/// Use one native selector policy for ordinary control and startup adoption.
/// The isolated target decorates that same adapter with its exact registered
/// test service; startup copy/recovery must not bypass that custody wrapper.
#[cfg(feature = "encrypted-workspace")]
pub(super) fn native_startup_vault(
    app: &AppHandle,
) -> impl rangoon_desktop::workspace_session::Vault + Send + 'static + use<> {
    #[cfg(all(test, feature = "native-encrypted-qualification", target_os = "macos"))]
    {
        let _ = app;
        crate::native_encrypted_qualification::startup_vault()
    }
    #[cfg(not(all(test, feature = "native-encrypted-qualification", target_os = "macos")))]
    {
        OsVault::new(app.config().identifier.clone())
    }
}
