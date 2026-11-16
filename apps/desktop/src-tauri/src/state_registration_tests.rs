//! Commands read state registered with `app.manage`. Tauri looks state up by
//! exact type, so registering `SharedState` while a command asks for something
//! else fails only at runtime ("state not managed"). These tests catch that.

use std::sync::Arc;

use tauri::Manager;
use tauri::test::{mock_builder, mock_context, noop_assets};

use crate::commands;
use crate::state::{SharedState, test_support};

#[test]
fn commands_read_the_state_type_the_app_registers() {
    let (_dir, state) = test_support::state();
    let shared: SharedState = Arc::new(state);
    let app = mock_builder()
        .manage(shared)
        .build(mock_context(noop_assets()))
        .unwrap();

    let status = commands::status(app.state()).unwrap();
    assert!(!status.initialized);
    assert!(commands::settings(app.state()).is_ok());
}
