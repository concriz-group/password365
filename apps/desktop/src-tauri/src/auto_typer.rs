//! Auto-type: performs the keystrokes `password365_app::auto_type` plans,
//! from the item's "Auto-type" button or the global shortcut.
//!
//! The shortcut looks at the window in front: one matching item types at
//! once; none or several bring Password365 forward to choose (the app then
//! steps aside so the keystrokes reach that window again). A web browser's
//! window title is whatever the page says, so a browser always gets the
//! choice (the user sees the title and picks). Right before typing, the
//! window in front must still be the one the shortcut saw — same program and
//! title — or nothing is typed.

use std::sync::Mutex;
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

/// The window the keystrokes are meant for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Its program's process.
    pub process_id: u64,
    /// Its title (or program name, see [`front_target`]).
    pub title: String,
    /// Its program's name.
    pub app_name: String,
}

/// The target the shortcut saw, while the user chooses an item.
static CHOSEN_FOR: Mutex<Option<Target>> = Mutex::new(None);

/// Programs whose window titles web pages set.
const BROWSERS: &[&str] = &[
    "arc",
    "brave",
    "chrome",
    "chromium",
    "duckduckgo",
    "edge",
    "firefox",
    "librewolf",
    "msedge",
    "opera",
    "orion",
    "safari",
    "vivaldi",
    "waterfox",
    "zen",
];

/// Whether `app_name` is a web browser (its title comes from the page).
#[must_use]
pub fn is_browser(app_name: &str) -> bool {
    let name = app_name.to_lowercase();
    BROWSERS
        .iter()
        .any(|b| name.split(|c: char| !c.is_alphanumeric()).any(|w| w == *b))
}

/// Types `steps` on a background thread once things have settled, if the
/// window in front is then still `expected` (any other window but ours when
/// `None`).
fn type_later(steps: Vec<Step>, expected: Option<Target>) {
    thread::spawn(move || {
        thread::sleep(SETTLE);
        let now = front_target();
        let same = match (&expected, &now) {
            (Some(want), Some(now)) => want == now,
            (None, Some(_)) => true,
            (_, None) => false,
        };
        if !same {
            eprintln!("auto-type: the window in front changed; nothing typed");
            return;
        }
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
    // Chosen after the shortcut: the window it saw; from an item: whatever is next.
    let expected = CHOSEN_FOR.lock().ok().and_then(|mut t| t.take());
    step_aside(app);
    type_later(steps, expected);
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

/// The window in front (`None` when it is Password365's own). Its title is
/// what the shortcut matches against: the window title and program, or the
/// program's name when the title is hidden (macOS without screen-recording
/// permission).
fn front_target() -> Option<Target> {
    let window = active_win_pos_rs::get_active_window().ok()?;
    if window.process_id == u64::from(std::process::id()) {
        return None; // Password365 itself is in front: nothing to type into.
    }
    let title = window.title.trim();
    let title = if title.is_empty() {
        window.app_name.clone()
    } else {
        format!("{title} - {}", window.app_name)
    };
    Some(Target {
        process_id: window.process_id,
        title,
        app_name: window.app_name,
    })
}

/// The global shortcut was pressed.
pub fn on_shortcut<R: Runtime>(app: &AppHandle<R>) {
    let Some(target) = front_target() else {
        return;
    };
    let window = target.title.clone();
    let state = app.state::<SharedState>();
    let Ok(candidates) = state.with(|s| s.auto_type_candidates(&window)) else {
        bring_forward(app); // locked: unlock first
        return;
    };
    if let [only] = candidates.as_slice()
        && !is_browser(&target.app_name)
        && let Ok(steps) = state.with(|s| s.auto_type_steps(&only.id))
    {
        type_later(steps, Some(target));
        return;
    }
    if let Ok(mut chosen) = CHOSEN_FOR.lock() {
        *chosen = Some(target);
    }
    bring_forward(app);
    let _ = app.emit(CHOOSE_EVENT, Choice { window, candidates });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsers_are_recognised_by_name() {
        for name in [
            "Google Chrome",
            "Safari",
            "Firefox",
            "Microsoft Edge",
            "Brave Browser",
            "Arc",
            "msedge.exe",
            "firefox-esr",
        ] {
            assert!(is_browser(name), "{name}");
        }
        for name in [
            "Terminal",
            "Microsoft Remote Desktop",
            "KeePass",
            "Steam",
            "Chromecast helper",
        ] {
            assert!(!is_browser(name), "{name}");
        }
    }

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
