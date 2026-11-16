//! `password365-host` — started by the browser via native messaging. It only
//! relays messages to the running Password365 app; it holds no keys and makes no
//! decisions.

mod relay;

use std::io;
use std::process::ExitCode;

use password365_bridge::client::BridgeClient;
use password365_bridge::endpoint::Endpoint;

fn main() -> ExitCode {
    let Ok(endpoint) = Endpoint::for_current_user() else {
        return ExitCode::FAILURE;
    };
    let result = relay::relay(&mut io::stdin().lock(), &mut io::stdout().lock(), || {
        BridgeClient::connect(&endpoint)
    });
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
