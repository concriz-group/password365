//! The SSH agent protocol (draft-miller-ssh-agent): lists the vault's SSH keys
//! and signs with them when the user allows it. The transport (a Unix socket
//! or a Windows named pipe) and the approval prompt belong to the app; this
//! crate reads requests, answers them and never keeps keys longer than a request.

mod agent;
mod keys;
mod wire;

pub use agent::{KeySource, serve};
pub use keys::{AgentKey, KeyError, load_key};
pub use wire::{Request, Response, read_request, write_response};
