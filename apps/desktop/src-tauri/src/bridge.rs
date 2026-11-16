//! Starts the browser-extension bridge (local socket) inside the app.

use std::sync::Arc;

use password365_bridge::dispatch::handle_bytes;
use password365_bridge::endpoint::Endpoint;
use password365_bridge::protocol::Response;

use crate::state::AppState;

/// Answers one raw request using the shared app state. Calls `on_status_change`
/// when the request locked or unlocked the vault (e.g. unlock from the browser),
/// so the window can follow.
pub fn handle(state: &AppState, request: &[u8], on_status_change: &dyn Fn()) -> Vec<u8> {
    let mut changed = false;
    let response = state
        .with(|service| {
            let before = service.status().unlocked;
            let out = handle_bytes(service, request);
            changed = service.status().unlocked != before;
            Ok(out)
        })
        .unwrap_or_else(|e| {
            serde_json::to_vec(&Response::failure(0, e.code(), &e.to_string())).unwrap_or_default()
        });
    if changed {
        on_status_change();
    }
    response
}

/// Listens for the native-messaging host. Failure only disables the browser
/// integration; the app keeps working.
pub fn start(state: Arc<AppState>, on_status_change: impl Fn() + Send + Sync + 'static) {
    let started = Endpoint::for_current_user().and_then(|endpoint| {
        password365_bridge::server::serve(
            &endpoint,
            Arc::new(move |req: &[u8]| handle(&state, req, &on_status_change)),
        )
    });
    if let Err(e) = started {
        eprintln!("browser integration disabled: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_support::state;

    #[test]
    fn handles_requests_through_the_state() {
        let (_d, state) = state();
        let out: serde_json::Value =
            serde_json::from_slice(&handle(&state, br#"{"id":5,"type":"status"}"#, &|| {
                panic!("status does not change the lock state")
            }))
            .unwrap();
        assert_eq!(out["id"], 5);
        assert_eq!(out["result"]["initialized"], false);
    }

    #[test]
    fn notifies_when_the_browser_unlocks() {
        let (_d, state) = state();
        let pw = "long enough password";
        state
            .with(|s| {
                s.create_account(pw.into(), &pw.into())?;
                s.lock();
                Ok(())
            })
            .unwrap();
        let notified = std::cell::Cell::new(0);
        let request = format!(r#"{{"id":1,"type":"unlock","password":"{pw}"}}"#);
        handle(&state, request.as_bytes(), &|| {
            notified.set(notified.get() + 1);
        });
        assert_eq!(notified.get(), 1);
        handle(&state, br#"{"id":2,"type":"status"}"#, &|| notified.set(99));
        assert_eq!(notified.get(), 1);
    }
}
