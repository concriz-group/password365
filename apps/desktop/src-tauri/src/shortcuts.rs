//! Global shortcuts (Settings › General › Keyboard shortcuts): Quick Access,
//! auto-type and lock. Another program may own a shortcut already; the
//! feature then works from the app only.

use std::collections::HashMap;
use std::sync::Mutex;

use password365_app::AppError;
use password365_app::settings::Settings;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::state::{STATUS_EVENT, SharedState};
use crate::{auto_typer, quick_access};

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    QuickAccess,
    AutoType,
    Lock,
}

/// The shortcuts registered now, by shortcut id.
#[derive(Default)]
pub struct Registered(Mutex<HashMap<u32, Action>>);

/// The plugin, sending each shortcut to its action.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let action = app
                .try_state::<Registered>()
                .and_then(|r| r.0.lock().ok().and_then(|m| m.get(&shortcut.id()).copied()));
            match action {
                Some(Action::QuickAccess) => quick_access::toggle(app),
                Some(Action::AutoType) => auto_typer::on_shortcut(app),
                Some(Action::Lock) => lock(app),
                None => {}
            }
        })
        .build()
}

fn lock<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<SharedState>();
    let _ = state.with(|s| {
        s.lock();
        Ok(())
    });
    let _ = app.emit(STATUS_EVENT, ());
}

/// The shortcuts `settings` asks for, parsed (empty ones are off).
fn wanted(settings: &Settings) -> Result<Vec<(Shortcut, Action)>, AppError> {
    [
        (&settings.shortcut_quick_access, Action::QuickAccess),
        (&settings.shortcut_auto_type, Action::AutoType),
        (&settings.shortcut_lock, Action::Lock),
    ]
    .into_iter()
    .filter(|(text, _)| !text.is_empty())
    .map(|(text, action)| {
        text.parse::<Shortcut>()
            .map(|s| (s, action))
            .map_err(|_| AppError::Validation(format!("“{text}” is not a keyboard shortcut")))
    })
    .collect()
}

/// Checks the shortcuts of `settings` before they are saved.
pub fn validate(settings: &Settings) -> Result<(), AppError> {
    wanted(settings).map(|_| ())
}

/// Registers the shortcuts `settings` asks for, replacing the previous ones.
pub fn apply<R: Runtime>(app: &AppHandle<R>, settings: &Settings) {
    let Ok(shortcuts) = wanted(settings) else {
        return;
    };
    let global = app.global_shortcut();
    let _ = global.unregister_all();
    let mut map = HashMap::new();
    for (shortcut, action) in shortcuts {
        match global.register(shortcut) {
            Ok(()) => {
                map.insert(shortcut.id(), action);
            }
            Err(e) => eprintln!("{action:?} shortcut unavailable: {e}"),
        }
    }
    if let Some(registered) = app.try_state::<Registered>()
        && let Ok(mut current) = registered.0.lock()
    {
        *current = map;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_settings_shortcuts() {
        let defaults = wanted(&Settings::default()).unwrap();
        assert_eq!(
            defaults.iter().map(|(_, a)| *a).collect::<Vec<_>>(),
            [Action::QuickAccess, Action::AutoType]
        );
        let custom = Settings {
            shortcut_lock: "Shift+Super+KeyL".into(),
            ..Settings::default()
        };
        assert_eq!(wanted(&custom).unwrap().len(), 3);
        let bad = Settings {
            shortcut_lock: "Hyper+Banana".into(),
            ..Settings::default()
        };
        assert_eq!(validate(&bad).unwrap_err().code(), "validation");
    }
}
