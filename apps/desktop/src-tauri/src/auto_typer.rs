//! Auto-type: performs the keystrokes `password365_app::auto_type` plans,
//! from the item's "Auto-type" button or the global shortcut.
//!
//! The shortcut looks at the window in front: one matching item types at
//! once; none or several bring Password365 forward to choose (the app then
//! steps aside so the keystrokes reach that window again).

use std::thread;
use std::time::Duration;

use enigo::{Direction, Enigo, Keyboard as _, Settings};
use password365_app::auto_type::{AutoTypeCandidate, Key, Step};
use password365_app::{AppError, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::state::SharedState;

/// Event asking the window to let the user choose an item.
pub const CHOOSE_EVENT: &str = "auto-type-choose";
/// Time for the shortcut's keys to be released, or for focus to return to
/// the target window after Password365 steps aside.
const SETTLE: Duration = Duration::from_millis(450);

/// Where keystrokes go.
pub trait Keyboard {
    /// Types text.
    fn text(&mut self, text: &str) -> Result<()>;
    /// Presses and releases a key.
    fn key(&mut self, key: Key) -> Result<()>;
}

/// Performs `steps`, one character at a time so the key delay applies.
pub fn perform(
    steps: &[Step],
    keyboard: &mut impl Keyboard,
    sleep: &mut impl FnMut(u32),
) -> Result<()> {
    let mut delay = 0;
    for step in steps {
        match step {
            Step::KeyDelay(ms) => delay = *ms,
            Step::Pause(ms) => sleep(*ms),
            Step::Key(key) => {
                keyboard.key(*key)?;
                sleep(delay);
            }
            Step::Text(text) => {
                for ch in text.expose().chars() {
                    keyboard.text(ch.encode_utf8(&mut [0; 4]))?;
                    sleep(delay);
                }
            }
        }
    }
    Ok(())
}

/// The real keyboard.
struct SystemKeyboard(Enigo);

impl SystemKeyboard {
    fn new() -> Result<Self> {
        Enigo::new(&Settings::default())
            .map(Self)
            .map_err(|e| AppError::Io(format!("cannot type into other programs: {e}")))
    }
}

impl Keyboard for SystemKeyboard {
    fn text(&mut self, text: &str) -> Result<()> {
        self.0.text(text).map_err(typing_failed)
    }

    fn key(&mut self, key: Key) -> Result<()> {
        let key = match key {
            Key::Tab => enigo::Key::Tab,
            Key::Enter => enigo::Key::Return,
            Key::Space => enigo::Key::Space,
            Key::Backspace => enigo::Key::Backspace,
            Key::Escape => enigo::Key::Escape,
            Key::Up => enigo::Key::UpArrow,
            Key::Down => enigo::Key::DownArrow,
            Key::Left => enigo::Key::LeftArrow,
            Key::Right => enigo::Key::RightArrow,
        };
        self.0.key(key, Direction::Click).map_err(typing_failed)
    }
}

fn typing_failed(e: enigo::InputError) -> AppError {
    AppError::Io(format!("typing failed: {e}"))
}

/// Types `steps` on a background thread once things have settled.
fn type_later(steps: Vec<Step>) {
    thread::spawn(move || {
        thread::sleep(SETTLE);
        let typed = SystemKeyboard::new().and_then(|mut keyboard| {
            perform(&steps, &mut keyboard, &mut |ms| {
                thread::sleep(Duration::from_millis(ms.into()));
            })
        });
        if let Err(e) = typed {
            eprintln!("auto-type: {e}");
        }
    });
}

/// "Auto-type" on an item: step aside so the previous window gets focus
/// back, then type into it.
pub fn type_into_previous_window<R: Runtime>(
    app: &AppHandle<R>,
    state: &SharedState,
    id: &str,
) -> Result<()> {
    let steps = state.with(|s| s.auto_type_steps(id))?;
    step_aside(app);
    type_later(steps);
    Ok(())
}

fn step_aside<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "macos")]
    let _ = app.hide();
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
}

pub fn bring_forward<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// What the window shows when the shortcut needs a choice.
#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    /// Title of the window that was in front.
    pub window: String,
    /// Matching items (may be empty).
    pub candidates: Vec<AutoTypeCandidate>,
}

/// The window title the shortcut matches against: the title, or the
/// program's name when the title is hidden (macOS without screen-recording
/// permission).
fn front_window() -> Option<String> {
    let window = active_win_pos_rs::get_active_window().ok()?;
    if window.process_id == u64::from(std::process::id()) {
        return None; // Password365 itself is in front: nothing to type into.
    }
    let title = window.title.trim();
    Some(if title.is_empty() {
        window.app_name
    } else {
        format!("{title} - {}", window.app_name)
    })
}

/// The global shortcut was pressed.
pub fn on_shortcut<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = front_window() else {
        return;
    };
    let state = app.state::<SharedState>();
    let Ok(candidates) = state.with(|s| s.auto_type_candidates(&window)) else {
        bring_forward(app); // locked: unlock first
        return;
    };
    if let [only] = candidates.as_slice()
        && let Ok(steps) = state.with(|s| s.auto_type_steps(&only.id))
    {
        type_later(steps);
        return;
    }
    bring_forward(app);
    let _ = app.emit(CHOOSE_EVENT, Choice { window, candidates });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Vec<String>);

    impl Keyboard for Recorder {
        fn text(&mut self, text: &str) -> Result<()> {
            self.0.push(text.to_owned());
            Ok(())
        }

        fn key(&mut self, key: Key) -> Result<()> {
            self.0.push(format!("{key:?}"));
            Ok(())
        }
    }

    #[test]
    fn steps_are_typed_one_character_at_a_time_with_the_delays() {
        let steps = [
            Step::KeyDelay(5),
            Step::Text("ab".into()),
            Step::Key(Key::Tab),
            Step::Pause(300),
            Step::KeyDelay(0),
            Step::Text("é".into()),
        ];
        let mut keyboard = Recorder::default();
        let mut pauses = Vec::new();
        perform(&steps, &mut keyboard, &mut |ms| pauses.push(ms)).unwrap();
        assert_eq!(keyboard.0, ["a", "b", "Tab", "é"]);
        assert_eq!(pauses, [5, 5, 5, 300, 0]);
    }

    struct Broken;

    impl Keyboard for Broken {
        fn text(&mut self, _: &str) -> Result<()> {
            Err(AppError::Io("no".into()))
        }

        fn key(&mut self, _: Key) -> Result<()> {
            Err(AppError::Io("no".into()))
        }
    }

    #[test]
    fn a_keyboard_failure_stops_typing() {
        let mut sleeps = 0;
        let r = perform(
            &[Step::Key(Key::Enter), Step::Text("x".into())],
            &mut Broken,
            &mut |_| sleeps += 1,
        );
        assert!(r.is_err());
        assert_eq!(sleeps, 0);
    }
}
