#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use rangoon_host::{SelectionResult, analyze_selected_path};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct PickerState(Arc<AtomicBool>);

struct PendingPicker(Arc<AtomicBool>);
impl Drop for PendingPicker {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// The renderer supplies no path, content or options. Every read starts with a
/// fresh native picker. Its return value remains in the one local workbench.
#[tauri::command]
async fn select_and_analyze(app: AppHandle) -> SelectionResult {
    let busy = Arc::clone(&app.state::<PickerState>().0);
    if busy.swap(true, Ordering::AcqRel) {
        return SelectionResult::failed(
            "selection_busy",
            "Finish or cancel the current file selection first.",
        );
    }
    let pending = PendingPicker(busy);
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        let Some(window) = app.get_webview_window("main") else {
            return SelectionResult::failed(
                "selection_failed",
                "The source workbench is unavailable.",
            );
        };
        let selected = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("Choose a Markdown file to analyze")
            .add_filter("Markdown", &["md"])
            .blocking_pick_file();
        match selected {
            None => SelectionResult::Cancelled,
            Some(selected) => match selected.into_path() {
                Ok(path) => analyze_selected_path(&path),
                Err(_) => SelectionResult::failed(
                    "selection_failed",
                    "Choose a regular file available on this computer.",
                ),
            },
        }
    })
    .await
    .unwrap_or_else(|_| {
        SelectionResult::failed(
            "selection_failed",
            "File selection did not complete. Try again.",
        )
    })
}

fn bundled_navigation(url: &tauri::Url) -> bool {
    matches!(
        (url.scheme(), url.host_str()),
        ("tauri", Some("localhost")) | ("http", Some("tauri.localhost"))
    )
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(PickerState::default())
        .invoke_handler(tauri::generate_handler![select_and_analyze])
        .setup(|app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("analyze.html".into()))
                .title("Rangoon · Import & Analyze")
                .inner_size(1440.0, 960.0)
                .min_inner_size(320.0, 480.0)
                .on_navigation(bundled_navigation)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("could not start Rangoon desktop");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_stays_on_bundled_origins() {
        for allowed in [
            "tauri://localhost/analyze.html",
            "http://tauri.localhost/index.html#command",
        ] {
            assert!(bundled_navigation(&allowed.parse().unwrap()));
        }
        for blocked in [
            "https://example.com",
            "file:///etc/passwd",
            "http://localhost:4377",
            "http://tauri.localhost.evil/analyze.html",
        ] {
            assert!(!bundled_navigation(&blocked.parse().unwrap()));
        }
    }
    #[test]
    fn picker_guard_releases_busy_state() {
        let busy = Arc::new(AtomicBool::new(true));
        {
            let _guard = PendingPicker(Arc::clone(&busy));
        }
        assert!(!busy.load(Ordering::Acquire));
    }
}
