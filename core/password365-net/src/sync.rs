//! Running a sync conversation with a paired device over a secure stream.

use std::io::{Read, Write};

use password365_sync::DeviceId;
use password365_sync::pairing::noise::MAX_PLAINTEXT;
use password365_vault::peer_sync::PeerSync;
use password365_vault::wire::Message;
use password365_vault::{ApplyReport, Session};

use crate::Result;
use crate::secure::SecureStream;

/// Who was synced with and what changed here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    /// The paired device on the other end.
    pub peer: DeviceId,
    /// What merging its changes did locally.
    pub report: ApplyReport,
}

/// Syncs `session` with whoever is on the other end of `stream` (which must
/// be a paired device, identified by its pinned key). Both sides call this.
pub fn sync<S: Read + Write>(
    stream: &mut SecureStream<S>,
    session: &mut Session,
) -> Result<SyncOutcome> {
    let (mut peer, hello) = PeerSync::start(session, &stream.remote_static(), MAX_PLAINTEXT)?;
    stream.send(&hello.encode()?)?;
    while !peer.is_finished() {
        let message = Message::decode(&stream.receive()?)?;
        for reply in peer.handle(session, message)? {
            stream.send(&reply.encode()?)?;
        }
    }
    Ok(SyncOutcome {
        peer: peer.peer(),
        report: peer.report().clone(),
    })
}
