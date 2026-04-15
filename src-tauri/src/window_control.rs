use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_log::log::error;

fn create_main_window(app: &tauri::AppHandle) -> Option<WebviewWindow> {
    match WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .visible(false)
        .build()
    {
        Ok(window) => Some(window),
        Err(e) => {
            error!("Failed to create main window: {:?}", e);
            None
        }
    }
}

fn ensure_main_window(app: &tauri::AppHandle) -> (Option<WebviewWindow>, bool) {
    if let Some(window) = app.get_webview_window("main") {
        return (Some(window), false);
    }
    (create_main_window(app), true)
}

pub fn show_main_window(app: &tauri::AppHandle) {
    let (window, created_new) = ensure_main_window(app);
    if let Some(window) = window {
        if created_new {
            // Initial show is controlled by frontend after render completes.
            return;
        }
        window.show().ok();
        window.unminimize().ok();
        window.set_focus().ok();
    }
}

pub fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.destroy().ok();
    }
}

pub fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            hide_main_window(app);
        } else {
            show_main_window(app);
        }
    } else {
        show_main_window(app);
    }
}
