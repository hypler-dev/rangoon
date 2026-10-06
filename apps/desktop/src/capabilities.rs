//! Local content operations only. Imported or edited instructions are inert data.
use super::{AppHandle, PublicError, StoreError, Workspace, begin_operation, workspace};
use rangoon_domain::capability_v1::{CapabilityDetail, CapabilitySummary};
use rangoon_store::CapabilityReceiptV1 as CapabilityReceipt;
use serde::Serialize;

#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CapabilityResult {
    Listed {
        capabilities: Vec<CapabilitySummary>,
    },
    Opened {
        capability: Box<CapabilityDetail>,
        already_applied: bool,
    },
    Failed {
        error: PublicError,
    },
}
fn failure(error: StoreError) -> CapabilityResult {
    let (code, message) = error.public();
    CapabilityResult::Failed {
        error: PublicError { code, message },
    }
}
fn receipt(result: Result<CapabilityReceipt, StoreError>) -> CapabilityResult {
    match result {
        Ok(receipt) => CapabilityResult::Opened {
            capability: Box::new(receipt.capability),
            already_applied: receipt.already_applied,
        },
        Err(error) => failure(error),
    }
}
async fn mutate<F>(app: AppHandle, action: F) -> CapabilityResult
where
    F: FnOnce(Workspace) -> Result<CapabilityReceipt, StoreError> + Send + 'static,
{
    let Some(pending) = begin_operation(&app) else {
        return CapabilityResult::Failed {
            error: PublicError {
                code: "workspace_busy",
                message: "Finish the current local workspace operation first. Your draft remains available.",
            },
        };
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        receipt(workspace(&app).and_then(action))
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable))
}

#[tauri::command]
pub async fn list_capabilities(app: AppHandle) -> CapabilityResult {
    tauri::async_runtime::spawn_blocking(move || {
        match workspace(&app).and_then(|w| w.list_capabilities_v1()) {
            Ok(capabilities) => CapabilityResult::Listed { capabilities },
            Err(error) => failure(error),
        }
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable))
}
#[tauri::command]
pub async fn open_capability(
    app: AppHandle,
    capability_id: String,
    revision_id: Option<String>,
) -> CapabilityResult {
    tauri::async_runtime::spawn_blocking(move || {
        match workspace(&app)
            .and_then(|w| w.open_capability_v1(&capability_id, revision_id.as_deref()))
        {
            Ok(capability) => CapabilityResult::Opened {
                capability: Box::new(capability),
                already_applied: false,
            },
            Err(error) => failure(error),
        }
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable))
}
#[tauri::command]
pub async fn create_capability(
    app: AppHandle,
    source_id: String,
    fragment_id: String,
    title: String,
) -> CapabilityResult {
    mutate(app, move |w| {
        w.create_capability_v1(&source_id, &fragment_id, &title)
    })
    .await
}
#[tauri::command]
pub async fn revise_capability(
    app: AppHandle,
    capability_id: String,
    expected_revision_id: String,
    title: String,
    content: String,
) -> CapabilityResult {
    mutate(app, move |w| {
        w.revise_capability_v1(&capability_id, &expected_revision_id, &title, &content)
    })
    .await
}
#[tauri::command]
pub async fn review_capability(
    app: AppHandle,
    capability_id: String,
    expected_revision_id: String,
) -> CapabilityResult {
    mutate(app, move |w| {
        w.review_capability_v1(&capability_id, &expected_revision_id)
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn local_permissions_and_command_generation_cover_exact_capability_commands() {
        let permissions = include_str!("../capabilities/source-analysis.json");
        let manifest = include_str!("../build.rs");
        for command in [
            "list_capabilities",
            "open_capability",
            "create_capability",
            "revise_capability",
            "review_capability",
        ] {
            assert!(manifest.contains(&format!("\"{command}\"")));
            assert!(permissions.contains(&format!("\"allow-{}\"", command.replace('_', "-"))));
        }
        assert!(permissions.contains("\"local\": true"));
        assert!(!permissions.contains("\"remote\""));
    }
}
