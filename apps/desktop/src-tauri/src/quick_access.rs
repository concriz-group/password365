//! Quick Access, like 1Password's: a small window over any program to find
//! an item and copy its username, password or one-time password.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

/// Label of the Quick Access window (the frontend shows the search for it).
pub const LABEL: &str = "quick-access";
/// Sent to the Quick Access window each time it is shown (clear the search).
pub const SHOWN_EVENT: &str = "quick-access-shown";
/// Sent to the main window: open this item.
pub const OPEN_ITEM_EVENT: &str = "open-item";

/// Shows Quick Access (creating it the first time), or hides it when it is in front.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(LABEL) {
        if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.emit(SHOWN_EVENT, ());
        }
        return;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Quick Access")
        .inner_size(640.0, 420.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .focused(true)
        .build();
    if let Err(e) = built {
        eprintln!("Quick Access window: {e}");
    }
}

/// Hides Quick Access (Escape, a copy, or the window losing focus).
#[tauri::command]
pub fn hide_quick_access(window: tauri::WebviewWindow) {
    if window.label() == LABEL {
        let _ = window.hide();
    }
}

#[derive(Clone, Serialize)]
struct OpenItem {
    id: Option<String>,
}

/// Brings the main window forward, showing item `id` (or just the app, to unlock).
#[tauri::command]
pub fn open_in_app(app: AppHandle, id: Option<String>) {
    if let Some(quick) = app.get_webview_window(LABEL) {
        let _ = quick.hide();
    }
    crate::auto_typer::bring_forward(&app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit(OPEN_ITEM_EVENT, OpenItem { id });
    }
}
