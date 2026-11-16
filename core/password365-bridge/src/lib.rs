//! Bridge between the browser extension and the desktop app.
//!
//! ```text
//! extension ──native messaging (stdio)──▶ password365-host ──local socket──▶ desktop app
//! ```
//!
//! - [`protocol`]: JSON requests/responses.
//! - [`framing`]: 4-byte length-prefixed messages (Chrome native-messaging format,
//!   reused on the local socket).
//! - [`dispatch`]: maps requests onto `AppService` (runs inside the app).
//! - [`endpoint`]: where the per-user socket lives.
//! - [`server`] / [`client`]: the two ends of the local socket.
//!
//! The host is a dumb relay: it holds no keys and makes no decisions.

pub mod client;
pub mod dispatch;
pub mod endpoint;
pub mod framing;
pub mod protocol;
pub mod server;
