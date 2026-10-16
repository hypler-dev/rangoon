//! OS credential commands; this module cannot call any model/provider transport.
use crate::{
    cloud_custody::{Custody, Decision, EDIT_WINDOW, Lease, Phase, parse_submission},
    cloud_store::{self, Error, OsStore},
};
use serde::Serialize;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
    ipc::{InvokeBody, Request},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogResult};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    schema_version: &'static str,
    provider: &'static str,
    authority: &'static str,
    backend: &'static str,
    #[serde(flatten)]
    outcome: Outcome,
}
#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum Outcome {
    Stored { revision: String },
    Saved { revision: String },
    Missing,
    Removed,
    Cancelled,
    Failed { code: Error },
}
fn receipt(outcome: Outcome) -> Receipt {
    Receipt {
        schema_version: "rangoon.cloud-custody.v1",
        provider: "openai",
        authority: "none",
        backend: backend(),
        outcome,
    }
}
fn failed(code: Error) -> Receipt {
    receipt(Outcome::Failed { code })
}
/// Result custody outlives the blocking worker and remains held through the
/// final generation check or unobserved discard. Owned results drop first.
struct CredentialCompletion {
    result: Result<Outcome, Error>,
    lease: Lease,
    mutation: bool,
}
impl CredentialCompletion {
    fn read(lease: Lease, result: Result<Outcome, Error>) -> Self {
        Self {
            result,
            lease,
            mutation: false,
        }
    }
    fn mutation(lease: Lease, result: Result<Outcome, Error>) -> Self {
        Self {
            result,
            lease,
            mutation: true,
        }
    }
    fn deliver(self) -> Receipt {
        if !self.lease.workspace_is_current() {
            return failed(if matches!(self.result, Err(Error::WriteUncertain)) {
                Error::WriteUncertain
            } else if self.mutation {
                Error::Changed
            } else {
                Error::Unavailable
            });
        }
        if !self.lease.is_current()
            && !matches!(
                self.result,
                Ok(Outcome::Cancelled) | Err(Error::WriteUncertain)
            )
        {
            return failed(Error::Unavailable);
        }
        match self.result {
            Ok(outcome) => receipt(outcome),
            Err(error) => failed(error),
        }
    }
}

fn inspect_with(
    lease: Lease,
    lookup: impl FnOnce() -> Result<Option<cloud_store::Secret>, Error>,
) -> CredentialCompletion {
    let result = if lease.is_current() {
        lookup().map(|value| match value {
            Some(secret) => Outcome::Stored {
                revision: secret.revision(),
            },
            None => Outcome::Missing,
        })
    } else {
        Err(Error::Unavailable)
    };
    CredentialCompletion::read(lease, result)
}

/// Distinguish workspace invalidation from explicit local cancellation before
/// beginning credential lookup. None means no lookup was started.
fn mutation_read<T>(
    lease: &Lease,
    lookup: impl FnOnce() -> Result<T, Error>,
) -> Result<Option<T>, Error> {
    if !lease.workspace_is_current() {
        return Err(Error::Changed);
    }
    if !lease.is_current() {
        return Ok(None);
    }
    lookup().map(Some)
}

fn backend() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macos_keychain"
    }
    #[cfg(target_os = "windows")]
    {
        "windows_credential_manager"
    }
    #[cfg(target_os = "linux")]
    {
        "linux_secret_service"
    }
}
fn allowed(actual: &str, expected: &str) -> Result<(), ()> {
    if actual == expected { Ok(()) } else { Err(()) }
}
fn empty(body: &InvokeBody) -> bool {
    match body {
        InvokeBody::Json(value) => value.as_object().is_some_and(|o| o.is_empty()),
        InvokeBody::Raw(bytes) => bytes.is_empty(),
    }
}
fn raw(body: &InvokeBody) -> Result<&[u8], Error> {
    match body {
        InvokeBody::Raw(bytes) => Ok(bytes),
        _ => Err(Error::InvalidRequest),
    }
}
fn editor_navigation(url: &tauri::Url) -> bool {
    matches!(
        (url.scheme(), url.host_str()),
        ("tauri", Some("localhost")) | ("http", Some("tauri.localhost"))
    ) && url.path() == "/cloud-credential-entry.html"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}
struct EditorCleanup(AppHandle);
impl Drop for EditorCleanup {
    fn drop(&mut self) {
        if let Some(window) = self.0.get_webview_window(EDIT_WINDOW) {
            let _ = window.destroy();
        }
    }
}
fn open_editor(app: &AppHandle, id: &str) -> Result<WebviewWindow, Error> {
    if app.get_webview_window(EDIT_WINDOW).is_some() {
        return Err(Error::Busy);
    }
    let parent = app
        .get_webview_window("main")
        .ok_or(Error::ConfirmationUnavailable)?;
    let window = WebviewWindowBuilder::new(
        app,
        EDIT_WINDOW,
        WebviewUrl::App("cloud-credential-entry.html".into()),
    )
    .title("Rangoon — OpenAI credential")
    .inner_size(640.0, 600.0)
    .min_inner_size(360.0, 480.0)
    .parent(&parent)
    .map_err(|_| Error::ConfirmationUnavailable)?
    .on_navigation(editor_navigation)
    .build()
    .map_err(|_| Error::ConfirmationUnavailable)?;
    let owner = app.state::<Custody>().inner().clone();
    let id = id.to_owned();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            if owner.cancel(Some(&id)) {
                api.prevent_close();
            }
        }
        WindowEvent::Destroyed => {
            owner.cancel(Some(&id));
        }
        _ => {}
    });
    Ok(window)
}
fn confirm(app: AppHandle, parent: WebviewWindow, save: bool) -> Result<bool, Error> {
    let (title, question, action) = if save {
        (
            "Save OpenAI credential?",
            "Create or replace Rangoon's saved OpenAI credential in this user's operating-system credential store?\n\nThis does not test the key, contact OpenAI, or authorize source transfer. It does not encrypt the Rangoon workspace.",
            "Save credential",
        )
    } else {
        (
            "Remove OpenAI credential?",
            "Remove Rangoon's saved OpenAI credential from this user's operating-system credential store?\n\nThis does not revoke the key at OpenAI or erase provider data. Another application may retain its own copy.",
            "Remove credential",
        )
    };
    let answer = {
        app.dialog()
            .message(question)
            .title(title)
            .parent(&parent)
            .buttons(MessageDialogButtons::OkCancelCustom(
                action.into(),
                "Cancel".into(),
            ))
            .blocking_show_with_result()
    };
    Ok(matches!(answer, MessageDialogResult::Custom(label) if label == action))
}

#[tauri::command]
pub async fn inspect_cloud_credential(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Receipt, ()> {
    allowed(window.label(), "main")?;
    if !empty(request.body()) {
        return Ok(failed(Error::InvalidRequest));
    }
    let (lease, _) = match app.state::<Custody>().begin_for(&workspace, Phase::Reading) {
        Ok(value) => value,
        Err(e) => return Ok(failed(e)),
    };
    let identifier = app.config().identifier.clone();
    // The result retains custody after the worker exits, including waiter loss.
    let result = tauri::async_runtime::spawn_blocking(move || {
        inspect_with(lease, || {
            let store = OsStore::open(&identifier)?;
            cloud_store::inspect(&store)
        })
    })
    .await;
    Ok(match result {
        Ok(completed) => completed.deliver(),
        Err(_) => failed(Error::Unavailable),
    })
}

#[tauri::command]
pub async fn edit_cloud_credential(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Receipt, ()> {
    allowed(window.label(), "main")?;
    if !empty(request.body()) {
        return Ok(failed(Error::InvalidRequest));
    }
    let custody = app.state::<Custody>().inner().clone();
    let (lease, receiver) = match custody.begin_for(&workspace, Phase::Reading) {
        Ok(value) => value,
        Err(e) => return Ok(failed(e)),
    };
    let identifier = app.config().identifier.clone();
    let initial = tauri::async_runtime::spawn_blocking(move || {
        let result = mutation_read(&lease, || {
            OsStore::open(&identifier).and_then(|s| cloud_store::inspect(&s))
        });
        (lease, result)
    })
    .await;
    let (lease, expected) = match initial {
        Ok((lease, result)) => {
            if !lease.workspace_is_current() {
                return Ok(failed(Error::Changed));
            }
            if !lease.is_current() {
                return Ok(receipt(Outcome::Cancelled));
            }
            match result {
                Ok(Some(expected)) => (lease, expected),
                Ok(None) => return Ok(receipt(Outcome::Cancelled)),
                Err(e) => return Ok(failed(e)),
            }
        }
        Err(_) => return Ok(failed(Error::Unavailable)),
    };
    if !custody.advance(&lease.id, Phase::Editing) {
        return Ok(CredentialCompletion::mutation(lease, Ok(Outcome::Cancelled)).deliver());
    }
    let editor = match open_editor(&app, &lease.id) {
        Ok(value) => value,
        Err(e) => return Ok(failed(e)),
    };
    let cleanup = EditorCleanup(app.clone());
    let secret = match receiver.await {
        Ok(Some(value)) => value,
        _ => return Ok(CredentialCompletion::mutation(lease, Ok(Outcome::Cancelled)).deliver()),
    };
    if !custody.advance(&lease.id, Phase::Prompting) {
        return Ok(CredentialCompletion::mutation(lease, Ok(Outcome::Cancelled)).deliver());
    }
    // One blocking owner retains the lease and parent through the OS question,
    // pre-write cancellation check, mutation and verification, even if the IPC
    // future disappears. No callback can resurrect a cancelled operation.
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _cleanup = cleanup;
        let result = (|| {
            if !lease.is_current() || !confirm(app.clone(), editor, true)? {
                return Ok(Outcome::Cancelled);
            }
            if !custody.advance(&lease.id, Phase::Storage) || !lease.is_current() {
                return Ok(Outcome::Cancelled);
            }
            let store = OsStore::open(&app.config().identifier)?;
            cloud_store::save(&store, &secret, expected.as_ref())
                .map(|revision| Outcome::Saved { revision })
        })();
        CredentialCompletion::mutation(lease, result)
    })
    .await;
    Ok(match result {
        Ok(completed) => completed.deliver(),
        Err(_) => failed(Error::WriteUncertain),
    })
}

#[tauri::command]
pub async fn remove_cloud_credential(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Receipt, ()> {
    allowed(window.label(), "main")?;
    if !empty(request.body()) {
        return Ok(failed(Error::InvalidRequest));
    }
    let custody = app.state::<Custody>().inner().clone();
    let (lease, _) = match custody.begin_for(&workspace, Phase::Reading) {
        Ok(value) => value,
        Err(e) => return Ok(failed(e)),
    };
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let Some((store, expected)) = mutation_read(&lease, || {
                let store = OsStore::open(&app.config().identifier)?;
                let expected = cloud_store::inspect(&store)?;
                Ok((store, expected))
            })?
            else {
                return Ok(Outcome::Cancelled);
            };
            let Some(expected) = expected else {
                return Ok(Outcome::Missing);
            };
            if !custody.advance(&lease.id, Phase::Prompting) || !lease.is_current() {
                return Ok(Outcome::Cancelled);
            }
            if !confirm(app, window, false)? {
                return Ok(Outcome::Cancelled);
            }
            if !custody.advance(&lease.id, Phase::Storage) || !lease.is_current() {
                return Ok(Outcome::Cancelled);
            }
            cloud_store::remove(&store, &expected)?;
            Ok(Outcome::Removed)
        })();
        CredentialCompletion::mutation(lease, result)
    })
    .await;
    Ok(match result {
        Ok(completed) => completed.deliver(),
        Err(_) => failed(Error::WriteUncertain),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditView {
    schema_version: &'static str,
    provider: &'static str,
    authority: &'static str,
    edit_id: String,
}
#[derive(Serialize)]
pub struct Accepted {
    accepted: bool,
}
#[tauri::command]
pub fn get_cloud_credential_edit(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Option<EditView>, ()> {
    allowed(window.label(), EDIT_WINDOW)?;
    if !empty(request.body()) {
        return Ok(None);
    }
    Ok(app.state::<Custody>().edit_id().map(|edit_id| EditView {
        schema_version: "rangoon.cloud-credential-edit.v1",
        provider: "openai",
        authority: "none",
        edit_id,
    }))
}
#[tauri::command]
pub fn submit_cloud_credential(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Accepted, ()> {
    allowed(window.label(), EDIT_WINDOW)?;
    let result = raw(request.body())
        .and_then(parse_submission)
        .and_then(|(id, secret)| app.state::<Custody>().submit(&id, secret));
    Ok(Accepted {
        accepted: result.is_ok(),
    })
}
#[tauri::command]
pub fn cancel_cloud_credential_edit(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Accepted, ()> {
    allowed(window.label(), EDIT_WINDOW)?;
    let accepted = cancel_input(request.body(), &app.state::<Custody>());
    Ok(Accepted { accepted })
}
fn cancel_input(body: &InvokeBody, custody: &Custody) -> bool {
    raw(body)
        .and_then(Decision::parse)
        .is_ok_and(|decision| custody.cancel_edit(&decision.edit_id))
}
pub fn window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() == "main"
        && let WindowEvent::CloseRequested { api, .. } = event
        && window.state::<Custody>().cancel(None)
    {
        api.prevent_close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_navigation_and_native_labels() {
        assert!(allowed("main", EDIT_WINDOW).is_err());
        assert!(allowed(EDIT_WINDOW, "main").is_err());
        for origin in ["tauri://localhost", "http://tauri.localhost"] {
            assert!(editor_navigation(
                &format!("{origin}/cloud-credential-entry.html")
                    .parse()
                    .unwrap()
            ));
            for tail in [
                "/analyze.html",
                "/cloud-credential-entry.html?secret=x",
                "/cloud-credential-entry.html#x",
            ] {
                assert!(!editor_navigation(
                    &format!("{origin}{tail}").parse().unwrap()
                ));
            }
        }
        assert!(!editor_navigation(
            &"https://example.com/cloud-credential-entry.html"
                .parse()
                .unwrap()
        ));
    }
    #[test]
    fn receipts_are_closed_and_secret_free() {
        let json = serde_json::to_value(receipt(Outcome::Saved {
            revision: "a".repeat(64),
        }))
        .unwrap();
        assert_eq!(json.as_object().unwrap().len(), 6);
        assert_eq!(json["authority"], "none");
        assert_eq!(json["provider"], "openai");
        assert_eq!(json["schemaVersion"], "rangoon.cloud-custody.v1");
        assert!(json.get("secret").is_none());
        assert!(json.get("credential").is_none());
        assert!(json.get("key").is_none());
    }
}

#[cfg(test)]
mod permissions_tests {
    #[test]
    fn credential_window_has_only_three_private_commands_and_no_remote_channels() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/cloud-credential-entry.json"))
                .unwrap();
        assert_eq!(
            capability["windows"],
            serde_json::json!(["cloud-credential-entry"])
        );
        assert_eq!(capability["local"], true);
        assert!(capability.get("remote").is_none());
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "allow-get-cloud-credential-edit",
                "allow-submit-cloud-credential",
                "allow-cancel-cloud-credential-edit"
            ])
        );
        let main: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
        for private in capability["permissions"].as_array().unwrap() {
            assert!(!main["permissions"].as_array().unwrap().contains(private));
        }
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = config["app"]["security"]["csp"].as_str().unwrap();
        for directive in [
            "default-src 'none'",
            "connect-src ipc: http://ipc.localhost",
            "form-action 'none'",
            "frame-src 'none'",
            "object-src 'none'",
            "script-src 'self'",
            "img-src 'self' asset: http://asset.localhost",
        ] {
            assert!(csp.split(';').any(|value| value.trim() == directive));
        }
        assert!(!csp.contains("https:"));
        assert!(!csp.contains("ws:"));
        assert!(!csp.contains('*'));
        assert!(config["app"]["security"].get("assetProtocol").is_none());
        assert!(super::empty(&tauri::ipc::InvokeBody::Json(
            serde_json::json!({})
        )));
        assert!(!super::empty(&tauri::ipc::InvokeBody::Json(
            serde_json::json!({"provider":"evil"})
        )));
    }
}

#[cfg(test)]
mod cancellation_command_tests {
    use super::*;
    #[test]
    fn cancel_command_works_after_submission_and_before_storage() {
        for phase in [Phase::Submitted, Phase::Prompting] {
            let custody = Custody::default();
            let (lease, _) = custody.begin(Phase::Editing).unwrap();
            assert!(custody.advance(&lease.id, phase));
            let bytes = serde_json::to_vec(&serde_json::json!({"schemaVersion":"rangoon.cloud-credential-decision.v1","editId":lease.id})).unwrap();
            assert!(cancel_input(&InvokeBody::Raw(bytes.clone()), &custody));
            assert!(!cancel_input(&InvokeBody::Raw(bytes.clone()), &custody));
            assert!(!custody.advance(&lease.id, Phase::Storage));
            drop(lease);
            let (next, _) = custody.begin(Phase::Storage).unwrap();
            assert!(!cancel_input(&InvokeBody::Raw(bytes), &custody));
            let bytes = serde_json::to_vec(&serde_json::json!({"schemaVersion":"rangoon.cloud-credential-decision.v1","editId":next.id})).unwrap();
            assert!(!cancel_input(&InvokeBody::Raw(bytes), &custody));
        }
    }
}

#[cfg(test)]
#[path = "workspace_credential_access_tests.rs"]
mod credential_access;
