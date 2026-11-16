//! Copying secrets: marked so clipboard history, cloud clipboard and
//! clipboard managers skip them (Windows: excluded from history, cloud and
//! monitoring; macOS: `org.nspasteboard.ConcealedType`), then cleared after
//! the delay chosen in Settings unless something else was copied meanwhile.

use std::thread;
use std::time::Duration;

use password365_app::AppError;

fn concealed_set(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    let set = clipboard.set();
    #[cfg(target_os = "windows")]
    let set = {
        use arboard::SetExtWindows;
        set.exclude_from_history()
            .exclude_from_cloud()
            .exclude_from_monitoring()
    };
    #[cfg(target_os = "macos")]
    let set = {
        use arboard::SetExtApple;
        set.exclude_from_history()
    };
    set.text(text)
}

fn clear_if_unchanged(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new()
        && clipboard.get_text().is_ok_and(|current| current == text)
    {
        let _ = clipboard.clear();
    }
}

/// Copies `text` as a secret; clears it after `clear_after_ms` (0: keep it).
#[tauri::command]
pub fn copy_secret(text: String, clear_after_ms: u64) -> Result<(), AppError> {
    concealed_set(&text).map_err(|e| AppError::Internal(format!("clipboard: {e}")))?;
    if clear_after_ms > 0 {
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(clear_after_ms));
            clear_if_unchanged(&text);
        });
    }
    Ok(())
}
