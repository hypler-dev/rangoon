//! Local content operations only. Imported or edited instructions are inert data.
use super::{AppHandle, PublicError, workspace_source_access::SourceAccess};
use rangoon_domain::capability_v1::{CapabilityDetail, CapabilitySummary};
use serde::Serialize;
use tauri::Manager;

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
#[tauri::command]
pub async fn list_capabilities(app: AppHandle) -> CapabilityResult {
    let access = app.state::<SourceAccess>().inner().clone();
    access.list_capabilities().await
}
#[tauri::command]
pub async fn open_capability(
    app: AppHandle,
    capability_id: String,
    revision_id: Option<String>,
) -> CapabilityResult {
    let access = app.state::<SourceAccess>().inner().clone();
    access.open_capability(capability_id, revision_id).await
}
#[tauri::command]
pub async fn create_capability(
    app: AppHandle,
    source_id: String,
    fragment_id: String,
    title: String,
) -> CapabilityResult {
    let access = app.state::<SourceAccess>().inner().clone();
    access
        .create_capability(source_id, fragment_id, title)
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
    let access = app.state::<SourceAccess>().inner().clone();
    access
        .revise_capability(capability_id, expected_revision_id, title, content)
        .await
}
#[tauri::command]
pub async fn review_capability(
    app: AppHandle,
    capability_id: String,
    expected_revision_id: String,
) -> CapabilityResult {
    let access = app.state::<SourceAccess>().inner().clone();
    access
        .review_capability(capability_id, expected_revision_id)
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
