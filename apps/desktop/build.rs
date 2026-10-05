fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["select_and_analyze"])),
    )
    .expect("desktop configuration must be valid");
}
