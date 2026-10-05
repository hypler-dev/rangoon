fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "select_and_analyze",
            "list_snapshots",
            "save_analysis",
            "open_snapshot",
            "clear_analysis",
            "get_engine_status",
            "list_capabilities",
            "open_capability",
            "create_capability",
            "revise_capability",
            "review_capability",
            "get_workspace_data",
            "export_workspace_backup",
            "prepare_workspace_restore",
            "restore_workspace_backup",
            "inspect_workspace_deletion",
            "delete_workspace_record",
        ]),
    ))
    .expect("desktop configuration must be valid");
}
